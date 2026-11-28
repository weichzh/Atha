// Build reader/app first. Use the same Playwright setup as reader-app.browser.mjs.
import assert from "node:assert/strict";
import { mkdir, readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { preview as startPreview } from "vite";

const { chromium } = createRequire(import.meta.url)("playwright");
const appRoot = fileURLToPath(new URL("../", import.meta.url));
const artifacts = new URL("../../../artifacts/local/page-turn/", import.meta.url);
const manifest = JSON.parse(await readFile(new URL("../public/demo/manifest.json", import.meta.url)));
const version = manifest.contentVersion;
const csp = JSON.parse(await readFile(new URL("../src-tauri/tauri.conf.json", import.meta.url))).app.security.csp;
manifest.resources = [];
manifest.toc = manifest.sections.map(({ href }, i) => ({ href, label: `原创合成章 ${i + 1}` }));
const body = (chapter, paragraphs) => `<html xmlns="http://www.w3.org/1999/xhtml"><head><title>卷页检查</title></head><body>
  <h1>潮汐记录 · 第 ${chapter} 章</h1>${Array.from({ length: paragraphs }, (_, i) => `
  <p>第 ${chapter} 章 · 第 ${i + 1} 段。灰白的云在港口缓缓开阔，灯塔后面有一条窄小的石路。
  On the far side of the harbor, each stone recorded a different tide.</p>`).join("")}</body></html>`;

const saved = (page) => page.evaluate((key) => JSON.parse(localStorage.getItem(key) ?? "null")?.locator,
  `atha.reader.progress.${version.slice(0, 16)}.v1`);
const position = (page) => page.evaluate(() => Number(document.title.match(/page (\d+) \/ (\d+)$/)?.[1]));
const section = (page) => page.locator("html").getAttribute("data-section-position");
const stage = (page, mode) => page.locator(`[data-page-turn-renderer="${mode}"]`);
const settled = (page) => page.locator("[data-page-turn-renderer]").waitFor({ state: "detached" });

async function setToolbar(page, open) {
  const toolbar = page.locator(".reader-controls");
  await page.locator(".reader-shell").focus();
  if (await toolbar.isVisible() !== open) await page.keyboard.press("h");
  await toolbar.waitFor({ state: open ? "visible" : "detached" });
}

async function mode(page, name) {
  await setToolbar(page, true);
  await page.getByRole("button", { name: "排版与主题设置" }).click();
  await page.getByRole("tab", { name: "本书覆盖" }).click();
  const option = page.getByRole("group", { name: "阅读方式" }).getByRole("button", { name, exact: true });
  await option.click();
  await page.waitForFunction((label) => Array.from(document.querySelectorAll(".reader-settings-dialog button"))
    .some((button) => button.textContent.trim() === label && button.getAttribute("aria-pressed") === "true"), name);
  await page.keyboard.press("Escape");
  await page.getByRole("dialog", { name: "阅读排版设置" }).waitFor({ state: "hidden" });
  await setToolbar(page, false);
}

async function advance(page, direction, renderer, button = false) {
  const before = await position(page);
  if (button) {
    await page.getByRole("button", { name: "进度", exact: true }).click();
    await page.getByRole("button", { name: direction > 0 ? "下一页" : "上一页" }).click();
  } else await page.keyboard.press(direction > 0 ? "ArrowRight" : "ArrowLeft");
  if (renderer) {
    try {
      await stage(page, renderer).waitFor({ timeout: 3000 });
    } catch (error) {
      throw new Error(`Expected ${renderer}; ${await page.title()}; observed ${await page.evaluate(() => JSON.stringify(window.turnRenderers))}`, {
        cause: error,
      });
    }
  }
  await page.waitForFunction((wanted) => new RegExp(`page ${wanted} / `).test(document.title), before + direction);
  if (!renderer) await settled(page);
  if (button) {
    await page.getByRole("button", { name: "关闭阅读进度" }).click();
    await page.locator(".reader-progress-panel").waitFor({ state: "hidden" });
    await settled(page);
  }
}

async function prepare(browser, base, { mobile = false, noGl = false, complex = false, paragraphs = 70, touch = mobile, reducedMotion = "no-preference", prefix = "", inspectClones = false, viewport } = {}) {
  const page = await browser.newPage({
    viewport: viewport ?? (mobile ? { width: 390, height: 844 } : { width: 1120, height: 820 }),
    deviceScaleFactor: mobile ? 2 : 1,
    hasTouch: touch,
    reducedMotion,
  });
  page.setDefaultTimeout(7000);
  const errors = [];
  await page.addInitScript(() => {
    window.turnRenderers = [];
    new MutationObserver((records) => {
      for (const { target } of records) {
        const value = target.getAttribute("data-page-turn-renderer");
        if (value) window.turnRenderers.push(value);
      }
    }).observe(document, { subtree: true, attributes: true, attributeFilter: ["data-page-turn-renderer"] });
  });
  page.on("pageerror", error => errors.push(error.stack));
  if (inspectClones) {
    await page.addInitScript(() => {
      const attach = Element.prototype.attachShadow;
      Element.prototype.attachShadow = function (options) {
        return attach.call(this, this.classList.contains("reader-content-host")
          ? { ...options, mode: "open" } : options);
      };
    });
  }
  if (noGl) {
    await page.addInitScript(() => {
      const get = HTMLCanvasElement.prototype.getContext;
      HTMLCanvasElement.prototype.getContext = function (type, ...args) {
        return type.startsWith("webgl") ? null : get.call(this, type, ...args);
      };
    });
  }
  await page.route(`${base}**`, async (route) => {
    const pathname = new URL(route.request().url()).pathname;
    if (route.request().isNavigationRequest() && pathname === "/") {
      const response = await route.fetch();
      await route.fulfill({ response, headers: { ...response.headers(), "content-security-policy": csp } });
    } else if (pathname === "/demo/manifest.json") {
      await route.fulfill({ json: manifest });
    } else if (/\/demo\/ch[123]\.xhtml$/.test(pathname)) {
      const source = body(pathname.match(/ch(\d)/)[1], paragraphs);
      await route.fulfill({
        contentType: "application/xhtml+xml",
        body: source.replace("<h1>", `${complex ? "<blockquote><p>原样保留的引文</p></blockquote>" : ""}${prefix}<h1>`),
      });
    } else {
      await route.continue();
    }
  });
  await page.route((url) => url.origin !== new URL(base).origin, (route) => route.abort());
  return { page, errors };
}

async function open(page, base, fromShelf = false) {
  if (fromShelf) await page.locator('[data-shelf-area="catalog"]').first().click();
  else await page.goto(`${base}?manifest=demo/manifest.json`);
  await page.waitForFunction(() => document.querySelector(".reader-root")?.dataset.screenReady === "true" &&
    !document.querySelector(".opening-cover, .opening-back"));
  await page.locator(".reader-shell").focus();
}

async function touchDrag(page, dx) {
  const cdp = await page.context().newCDPSession(page);
  const { x, y, width, height } = await page.locator(".reader-frame").boundingBox();
  const startX = Math.round(x + width * (dx < 0 ? 0.8 : 0.2));
  const startY = Math.round(y + height * 0.58);
  const move = (type, delta) => cdp.send("Input.dispatchTouchEvent", {
    type,
    touchPoints: delta === undefined ? [] : [{ x: startX + delta, y: startY }],
  });
  await move("touchStart", 0);
  await move("touchMove", Math.round(dx / 2));
  await move("touchMove", dx);
  return async () => {
    await move("touchEnd");
    await cdp.detach();
  };
}

async function mouseGutterDrag(page, direction) {
  const frame = await page.locator(".reader-frame").boundingBox();
  const paper = await page.locator(".page").boundingBox();
  const x = direction > 0
    ? (frame.x + frame.width + paper.x + paper.width) / 2
    : (frame.x + paper.x) / 2;
  const y = frame.y + frame.height * 0.68;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x - direction * 415, y, { steps: 8 });
  return () => page.mouse.up();
}

async function facingText(page) {
  return page.evaluate(() => {
    const aperture = document.querySelector(".page").getBoundingClientRect();
    const spine = (aperture.left + aperture.right) / 2;
    const book = document.querySelector(".reader-content-host").shadowRoot.querySelector(".book");
    const text = [new Set(), new Set()];
    for (const paragraph of book.querySelectorAll("p")) {
      const range = document.createRange();
      range.selectNodeContents(paragraph);
      for (const rect of range.getClientRects()) {
        if (rect.width < 1 || rect.left < aperture.left - 1 || rect.right > aperture.right + 1) continue;
        text[rect.left < spine ? 0 : 1].add(Number(paragraph.textContent.match(/第 (\d+) 段/)?.[1]));
      }
    }
    return text.map(side => [...side]);
  });
}

async function verifyFacingPages(browser, base) {
  const viewport = { width: 1500, height: 900 };
  const { page, errors } = await prepare(browser, base, { viewport, touch: true, inspectClones: true, paragraphs: 93 });
  const spread = page.locator(".reader[data-reader-spread]");
  const capture = name => page.screenshot({ path: fileURLToPath(new URL(name, artifacts)) });
  try {
    await open(page, base);
    await spread.waitFor();
    const opening = await facingText(page);
    assert.equal(opening[0][0], 1);
    assert.ok(opening[1][0] >= opening[0].at(-1), "the right leaf continues the left");
    await capture("facing-pages-desktop.png");

    await mode(page, "卷页");
    await setToolbar(page, true);
    await advance(page, 1, null);
    await advance(page, -1, null, true);
    const fast = await touchDrag(page, -415);
    await fast();
    await page.waitForFunction(() => /page 2 \/ /.test(document.title));
    assert.equal(await page.locator(".reader-controls").isVisible(), true);
    assert.deepEqual(await page.evaluate(() => window.turnRenderers), [], "open toolbar always uses the default turn");
    await setToolbar(page, false);
    await advance(page, -1, "spread");
    await settled(page);

    const beforeDrag = await page.evaluate(() => window.turnRenderers.length);
    const drag = await mouseGutterDrag(page, 1);
    await drag();
    await settled(page);
    assert.equal(await position(page), 1, "mouse gutter drag must not turn the page");
    assert.equal(await page.evaluate(() => window.turnRenderers.length), beforeDrag, "mouse gutter drag must not preview a turn");
    // The browser may extend a text selection from the gutter onto the page.
    await page.keyboard.press("Escape");
    await page.locator(".selection-capsule").waitFor({ state: "hidden" });

    const frame = await page.locator(".reader-frame").boundingBox();
    await page.mouse.click(frame.x + frame.width * 0.9, frame.y + frame.height * 0.68);
    await stage(page, "spread").waitFor();
    await page.waitForFunction(() => /page 2 \/ /.test(document.title));
    await settled(page);
    await page.mouse.wheel(0, -120);
    await stage(page, "spread").waitFor();
    await page.waitForFunction(() => /page 1 \/ /.test(document.title));
    await settled(page);

    const release = await touchDrag(page, -415);
    await stage(page, "spread").waitFor();
    assert.equal(await position(page), 1, "preview does not change navigation");
    const faces = await stage(page, "spread").evaluate(root => {
      const canvas = root.querySelector("canvas.visible");
      const gl = canvas.getContext("webgl");
      const program = gl.getParameter(gl.CURRENT_PROGRAM);
      const read = name => gl.getUniform(program, gl.getUniformLocation(program, name));
      const paper = root.closest(".reader-frame").querySelector(".page").getBoundingClientRect();
      const spine = (paper.left + paper.right) / 2 - root.getBoundingClientRect().left;
      return { pinned: Math.abs(read("u_spine") - spine) <= 1, folded: read("u_radius") > 0, back: read("u_incoming") };
    });
    assert.deepEqual(faces, { pinned: true, folded: true, back: 1 });
    assert.equal(await page.locator(".reader-controls").count(), 0, "curl does not reveal the toolbar");
    await capture("facing-pages-turn.png");
    await release();
    await page.waitForFunction(() => /page 2 \/ /.test(document.title));
    await settled(page);
    const second = await facingText(page);
    assert.ok(second[0][0] >= opening[1].at(-1));
    await advance(page, -1, "spread");
    await settled(page);
    assert.deepEqual(await facingText(page), opening);

    const line = await page.locator(".reader-content-host").evaluate(host => {
      const text = host.shadowRoot.querySelector(".book p").firstChild;
      const range = document.createRange();
      range.setStart(text, 2);
      range.setEnd(text, 14);
      return range.getBoundingClientRect().toJSON();
    });
    await page.mouse.move(line.left + 1, line.top + line.height / 2);
    await page.mouse.down();
    await page.mouse.move(line.right - 1, line.top + line.height / 2, { steps: 8 });
    await page.mouse.up();
    await page.locator(".selection-capsule").waitFor();
    await settled(page);
    assert.equal(await position(page), 1, "dragging on printed text selects instead of turning");
    await page.keyboard.press("Escape");
    await page.locator(".selection-capsule").waitFor({ state: "hidden" });

    const cancel = await touchDrag(page, -36);
    await stage(page, "spread").waitFor();
    await setToolbar(page, true);
    await settled(page);
    await cancel();
    await settled(page);
    assert.equal(await position(page), 1);
    await setToolbar(page, false);

    const before = await saved(page);
    await page.setViewportSize({ width: 1240, height: 1030 });
    await spread.waitFor({ state: "detached" });
    await page.locator('html[data-viewport-stable^="1240x"]').waitFor({ state: "attached" });
    await page.setViewportSize(viewport);
    await spread.waitFor();
    await page.locator('html[data-viewport-stable^="1500x"]').waitFor({ state: "attached" });
    assert.equal(await saved(page), before);
    const lost = await touchDrag(page, -415);
    await stage(page, "spread").waitFor();
    await stage(page, "spread").locator("canvas.visible").evaluate(canvas => {
      canvas.getContext("webgl").getExtension("WEBGL_lose_context").loseContext();
    });
    await stage(page, "cover").waitFor();
    await lost();
    await page.waitForFunction(() => /page 2 \/ /.test(document.title));
    await settled(page);
    await page.emulateMedia({ reducedMotion: "reduce" });
    await advance(page, -1, null);
    await mode(page, "连续滚动");
    await spread.waitFor({ state: "detached" });
    assert.deepEqual(errors, []);
  } finally {
    await page.close();
  }
}

async function verifyOddLongSpread(browser, base) {
  const { page, errors } = await prepare(browser, base, {
    viewport: { width: 1500, height: 900 },
    inspectClones: true,
    paragraphs: 35,
    reducedMotion: "reduce",
  });
  try {
    await open(page, base);
    // Give each original paragraph a leaf to make the final blank leaf independent of fonts.
    await page.locator(".reader-content-host").evaluate(host => {
      const style = document.createElement("style");
      style.textContent = ".book p + p { break-before: column; }";
      host.shadowRoot.append(style);
    });
    await page.setViewportSize({ width: 1502, height: 900 });
    await page.waitForFunction(() => /page 1 \/ 18$/.test(document.title) &&
      document.querySelector(".reader").hasAttribute("data-native-page-scroll"));
    for (let i = 1; i < 18; ++i) await advance(page, 1, null);
    assert.deepEqual(await facingText(page), [[35], []], "the odd final leaf stays on its side of the spine");
    const geometry = await page.locator(".page").evaluate(page => ({
      left: page.scrollLeft,
      step: page.clientWidth + parseFloat(getComputedStyle(page).getPropertyValue("--reader-column-gap")),
    }));
    assert.ok(Math.abs(geometry.left - 17 * geometry.step) <= 1);
    assert.deepEqual(errors, []);
  } finally {
    await page.close();
  }
}

async function verifyTurns(browser, base) {
  await verifyFacingPages(browser, base);
  if (process.argv.includes("--spread-only")) {
    console.log("Facing pages: curl, return, cancellation, resize and reduced motion passed.");
    return;
  }
  await verifyOddLongSpread(browser, base);

  const desktop = await prepare(browser, base, { touch: true });
  const { page } = desktop;
  await page.goto(base);
  await open(page, base, true);
  await mode(page, "卷页");
  const commitWebgl = await touchDrag(page, -440);
  await stage(page, "webgl").waitFor();
  await page.screenshot({ path: fileURLToPath(new URL("webgl-desktop.png", artifacts)) });
  const lost = await page.locator('.page-turn-stage canvas.visible').evaluate((canvas) => {
    const extension = canvas.getContext("webgl").getExtension("WEBGL_lose_context");
    if (!extension) return false;
    extension.loseContext();
    return true;
  });
  assert.equal(lost, true);
  await stage(page, "canvas2d").waitFor();
  await commitWebgl();
  await page.waitForFunction(() => /page 2 \/ /.test(document.title));
  await settled(page);
  await advance(page, -1, "canvas2d");
  await settled(page);
  await mode(page, "覆盖");
  await advance(page, 1, "cover");
  await page.screenshot({ path: fileURLToPath(new URL("cover-desktop.png", artifacts)) });
  await advance(page, -1, "cover");
  await page.locator(".reader-shell").focus();
  await advance(page, 1, "cover");
  await settled(page);
  const beforeScroll = await saved(page);
  await mode(page, "连续滚动");
  assert.equal(await page.locator(".reader").getAttribute("data-reading-mode"), "scroll");
  assert.equal(await page.evaluate(() => document.querySelector(".reader").scrollTop > 0), true);
  await mode(page, "覆盖");
  assert.equal(await saved(page), beforeScroll, "mode round trip reopens the same paged Locator");
  await setToolbar(page, true);
  await page.getByRole("button", { name: "返回书架" }).click();
  await page.locator(".reader-root").waitFor({ state: "detached" });
  await open(page, base, true);
  assert.equal(await saved(page), beforeScroll, "shelf round trip restores the stored Locator");
  assert.equal(await position(page), 2);
  await page.keyboard.press("ArrowLeft");
  await settled(page);
  const first = await section(page);
  const count = Number((await page.title()).match(/page \d+ \/ (\d+)$/)[1]);
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  await page.waitForFunction(() => /page 3 \/ /.test(document.title));
  for (let i = 3; i < count; i++) await advance(page, 1, "cover");
  const finishChapter = await touchDrag(page, -440);
  await finishChapter();
  await page.waitForFunction(() => document.documentElement.dataset.sectionPosition === "2 / 3" &&
    /page 1 \/ /.test(document.title));
  await settled(page);
  assert.equal(await position(page), 1);
  await page.keyboard.press("ArrowLeft");
  await page.waitForFunction(({ first, count }) => document.documentElement.dataset.sectionPosition === first &&
    new RegExp(`page ${count} / `).test(document.title), { first, count });
  assert.equal(await position(page), count);
  await settled(page);
  assert.deepEqual(desktop.errors, []);

  const fallback = await prepare(browser, base, { mobile: true, noGl: true });
  await open(fallback.page, base);
  await mode(fallback.page, "卷页");
  const firstPage = await saved(fallback.page);
  const cancel = await touchDrag(fallback.page, -32);
  await stage(fallback.page, "canvas2d").waitFor();
  await cancel();
  await settled(fallback.page);
  assert.equal(await saved(fallback.page), firstPage);
  const forward = await touchDrag(fallback.page, -185);
  await stage(fallback.page, "canvas2d").waitFor();
  await fallback.page.screenshot({ path: fileURLToPath(new URL("canvas2d-touch-390.png", artifacts)) });
  assert.equal(await position(fallback.page), 1);
  await forward();
  await fallback.page.waitForFunction(() => /page 2 \/ /.test(document.title));
  await settled(fallback.page);
  const backward = await touchDrag(fallback.page, 185);
  await stage(fallback.page, "canvas2d").waitFor();
  await backward();
  await fallback.page.waitForFunction(() => /page 1 \/ /.test(document.title));
  await settled(fallback.page);
  await mode(fallback.page, "滑动");
  await advance(fallback.page, 1, null);
  await mode(fallback.page, "连续滚动");
  const beforeWheel = await fallback.page.locator(".reader").evaluate(el => el.scrollTop);
  await fallback.page.mouse.move(180, 450);
  await fallback.page.mouse.wheel(0, 345);
  await fallback.page.waitForFunction((before) => document.querySelector(".reader").scrollTop > before, beforeWheel);
  await settled(fallback.page);
  assert.deepEqual(fallback.errors, []);

  const long = await prepare(browser, base, { mobile: true, paragraphs: 320 });
  await open(long.page, base);
  await mode(long.page, "卷页");
  assert.equal(await long.page.locator(".reader").evaluate(el => el.hasAttribute("data-native-page-scroll")), true);
  await advance(long.page, 1, null);
  const longViewport = long.page.locator(".page");
  const scrollLeft = () => longViewport.evaluate(el => el.scrollLeft);
  const pageStep = await scrollLeft();
  assert.ok(pageStep > 0);
  for (const [dx, expectedPage] of [[-300, 3], [300, 2], [-32, 2]]) {
    const release = await touchDrag(long.page, dx);
    await release();
    // Sample after both the reader's settle and any native inertial motion.
    await long.page.evaluate(() => new Promise(resolve => setTimeout(resolve, 500)));
    assert.deepEqual({ page: await position(long.page), scrollLeft: await scrollLeft() }, {
      page: expectedPage, scrollLeft: (expectedPage - 1) * pageStep,
    }, `one touch drag (${dx}px) must settle with the visible and logical page in sync`);
  }
  // Shift+wheel bypasses Interaction. The browser must not scroll the page itself.
  await long.page.mouse.move(200, 440);
  await long.page.keyboard.down("Shift");
  await long.page.mouse.wheel(0, 2100);
  await long.page.keyboard.up("Shift");
  assert.equal(await longViewport.evaluate(async el => {
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    return el.scrollLeft;
  }), pageStep, "direct horizontal scrolling must not bypass pagination");
  assert.deepEqual(await long.page.evaluate(() => window.turnRenderers), []);
  assert.deepEqual(long.errors, []);

  const wide = await prepare(browser, base, { touch: true, inspectClones: true });
  await wide.page.setViewportSize({ width: 1740, height: 820 });
  await open(wide.page, base);
  const releaseWide = await touchDrag(wide.page, -1230);
  const drag = await wide.page.locator(".reader-frame .book").evaluate(async el => {
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const style = getComputedStyle(el);
    return {
      shift: -new DOMMatrixReadOnly(style.transform).m41,
      step: parseFloat(style.width) + parseFloat(style.columnGap),
    };
  });
  assert.ok(drag.step < 1230, "gesture must be longer than one page step");
  assert.equal(drag.shift, drag.step, "drag preview must stop at the adjacent page");
  await wide.page.screenshot({ path: fileURLToPath(new URL("slide-drag-adjacent.png", artifacts)) });
  await releaseWide();
  await wide.page.waitForFunction(() => /page 2 \/ /.test(document.title));
  assert.deepEqual(wide.errors, []);
  await wide.page.close();

  const complex = await prepare(browser, base, { complex: true });
  await open(complex.page, base);
  await mode(complex.page, "卷页");
  await advance(complex.page, 1, "cover");
  await settled(complex.page);
  assert.deepEqual(complex.errors, []);

  const lists = await prepare(browser, base, {
    mobile: true,
    inspectClones: true,
    prefix: '<ol start="3"><li>第一项</li><li>第二项</li></ol><ul><li>一条说明</li></ul>',
  });
  await open(lists.page, base);
  await mode(lists.page, "卷页");
  const cancelListTurn = await touchDrag(lists.page, -32);
  await stage(lists.page, "cover").waitFor();
  assert.equal(await stage(lists.page, "cover").locator(".cover ol").getAttribute("start"), "3");
  assert.equal(await stage(lists.page, "cover").locator(".cover ul li").textContent(), "一条说明");
  await cancelListTurn();
  await settled(lists.page);
  assert.equal(await position(lists.page), 1);
  assert.deepEqual(lists.errors, []);
  await lists.page.close();

  const scrolled = await prepare(browser, base, {
    mobile: true,
    inspectClones: true,
    prefix: `<pre><code>${"one long code line; ".repeat(40)}</code></pre>`,
  });
  await open(scrolled.page, base);
  const originalFrame = scrolled.page.locator(".reader-frame .atha-structured-overflow");
  for (const style of ["覆盖", "卷页"]) {
    await mode(scrolled.page, style);
    const scrolledX = await originalFrame.evaluate((element) => {
      element.scrollLeft = 120;
      return element.scrollLeft;
    });
    assert.ok(scrolledX > 0, "fixture must scroll horizontally");
    const cancelCodeTurn = await touchDrag(scrolled.page, -32);
    await stage(scrolled.page, "cover").waitFor();
    const cloneOffsets = await stage(scrolled.page, "cover").locator(".atha-structured-overflow")
      .evaluateAll((elements) => elements.map((element) => element.scrollLeft));
    assert.deepEqual(cloneOffsets, [scrolledX, scrolledX], "both mounted clone frames preserve live scroll");
    await cancelCodeTurn();
    await settled(scrolled.page);
    assert.equal(await originalFrame.evaluate((element) => element.scrollLeft), scrolledX);
    assert.equal(await position(scrolled.page), 1);
  }
  assert.deepEqual(scrolled.errors, []);
  await scrolled.page.close();

  const reduced = await prepare(browser, base, { reducedMotion: "reduce" });
  await open(reduced.page, base);
  await mode(reduced.page, "卷页");
  await advance(reduced.page, 1, null);
  assert.deepEqual(await reduced.page.evaluate(() => window.turnRenderers), []);
  assert.deepEqual(reduced.errors, []);
  console.log("Page turns passed: one-page swipes, WebGL, Canvas2D, DOM cover, slide, scroll, cross-section and Locator restore.");
}

const preview = await startPreview({ root: appRoot, preview: { host: "127.0.0.1", port: 0, strictPort: true } });
const base = `http://127.0.0.1:${preview.httpServer.address().port}/`;
let browser;
try {
  await mkdir(artifacts, { recursive: true });
  browser = await chromium.launch({
    executablePath: process.env.CHROMIUM_PATH,
    args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
  });
  await verifyTurns(browser, base);
} finally {
  await browser?.close();
  await new Promise(resolve => preview.httpServer.close(resolve));
}
