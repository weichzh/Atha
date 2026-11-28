import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");
const url = process.argv[2];
if (!url) throw new Error("Pass the local Vite production preview URL.");

const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROMIUM_PATH });
let page;
try {
  page = await browser.newPage({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 2,
    reducedMotion: "reduce",
  });
  page.setDefaultTimeout(6000);
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.addInitScript(() => {
    // Emulate only the page-side AndroidX message port, not native WebView behavior.
    window.systemBarsMessages = [];
    window.AthaSystemBars = {
      onmessage: null,
      postMessage(message) {
        window.systemBarsMessages.push(message);
      },
    };
    const attach = Element.prototype.attachShadow;
    Element.prototype.attachShadow = function (options) {
      return attach.call(this, this.classList.contains("reader-content-host")
        ? { ...options, mode: "open" } : options);
    };
  });
  const chapter = await readFile(new URL("../public/demo/ch1.xhtml", import.meta.url), "utf8");
  await page.route("**/demo/ch1.xhtml", (route) => route.fulfill({
    contentType: "application/xhtml+xml",
    body: chapter.replace("<body>", `<body>
      <p id="body-line">当前正文选区 <a id="note-ref" href="#note-text" role="doc-noteref">[1]</a></p>
      <aside id="note-text">不换页的脚注正文</aside>`),
  }));

  const safe = { top: 96, right: 36, bottom: 48, left: 24, imeVisible: false };
  const send = (payload) => page.evaluate((data) => {
    window.AthaSystemBars.onmessage({ data });
  }, JSON.stringify(payload));
  const waitForMode = (name) => page.waitForFunction((prefix) =>
    window.systemBarsMessages.filter((message) => message.startsWith(prefix.split(":")[0] + ":"))
      .at(-1)?.startsWith(prefix), name);
  const viewportHeight = () => page.evaluate(() => +parseFloat(
    document.documentElement.style.getPropertyValue("--reader-height"),
  ));
  const withinSafeArea = async (selector) => {
    await page.locator(selector).waitFor();
    const box = await page.locator(selector).boundingBox();
    assert.ok(box && box.y >= 48 && box.x >= 12 &&
      box.x + box.width <= 390 - 18 && box.y + box.height <= 844 - 24, `${selector}: ${JSON.stringify(box)}`);
  };

  await page.goto(url);
  await page.waitForFunction(() => window.systemBarsMessages.includes("subscribe"));
  await waitForMode("bars:visible");
  await send(safe);
  await send({ ...safe, left: -2 });
  await withinSafeArea(".search-pill");
  await withinSafeArea(".nav-shell");
  const shelfSearch = page.getByRole("searchbox");
  await shelfSearch.fill("阅读");
  await send({ ...safe, bottom: 0, imeVisible: true });
  assert.equal(await shelfSearch.evaluate((el) => el.matches(":focus")), true);
  await send(safe);
  assert.equal(await shelfSearch.evaluate((el) => el.matches(":focus")), false);
  assert.equal(await shelfSearch.inputValue(), "阅读");
  await shelfSearch.fill("");
  await send(safe);
  assert.equal(await shelfSearch.evaluate((el) => el.matches(":focus")), true, "stable IME state must not blur");

  const book = page.locator('[data-shelf-area="catalog"]').first();
  await book.click();
  await page.waitForFunction(() =>
    document.querySelector(".reader-root")?.dataset.screenReady === "true" &&
    !document.querySelector(".opening-back, .opening-cover"));
  await waitForMode("bars:immersive");
  const initialHeight = (844 - 48 - 24) * 2;
  assert.equal(await viewportHeight(), initialHeight, "pagination must fit the inset frame, not crop the page");
  await send({ ...safe, top: 104 });
  await page.waitForFunction(() =>
    document.documentElement.style.getPropertyValue("--reader-height") === "1536px");
  await send(safe);
  await page.waitForFunction((height) =>
    document.documentElement.style.getPropertyValue("--reader-height") === height + "px", initialHeight);

  await page.locator("#body-line").evaluate((line) => {
    const node = line.firstChild;
    const range = document.createRange();
    range.setStart(node, 0);
    range.setEnd(node, 7);
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
  });
  await waitForMode("selection:body");
  await page.getByRole("toolbar", { name: "选区操作" }).waitFor();
  await page.waitForFunction(() => Boolean(document.querySelector(".selection-capsule")?.style.top));
  await withinSafeArea(".selection-capsule");
  assert.equal(await page.evaluate(() => window.__athaHandleBack()), true);
  await waitForMode("selection:none");

  await page.locator("#note-ref").click();
  await page.getByRole("dialog", { name: /^脚注/ }).getByText("不换页的脚注正文", { exact: true }).waitFor();
  await waitForMode("bars:visible");
  assert.equal(await page.evaluate(() => window.__athaHandleBack()), true);
  await waitForMode("bars:immersive");

  await page.keyboard.press("h");
  await waitForMode("bars:visible");
  await page.waitForFunction(() => {
    const top = document.querySelector(".top-island");
    return top && top.getBoundingClientRect().top >= 58 - 1;
  });
  await withinSafeArea(".top-island");
  await withinSafeArea(".bottom-island");
  if (process.env.ATHA_INSETS_SCREENSHOT) {
    await page.screenshot({ path: process.env.ATHA_INSETS_SCREENSHOT, animations: "disabled" });
  }
  assert.equal(await viewportHeight(), initialHeight);
  await page.keyboard.press("Control+f");
  const bookSearch = page.getByRole("searchbox", { name: "搜索本书正文" });
  await bookSearch.fill("正文");
  await send({ ...safe, bottom: 0, imeVisible: true });
  await send(safe);
  assert.equal(await bookSearch.evaluate((el) => el.matches(":focus")), false);
  assert.equal(await bookSearch.inputValue(), "正文");
  assert.equal(await page.evaluate(() => window.__athaHandleBack()), true);
  await page.getByRole("dialog", { name: "目录、书签与全书搜索" }).waitFor({ state: "hidden" });

  // Plain editors and controls are outside the search-specific IME blur contract.
  await page.evaluate(() => {
    const editor = document.createElement("textarea");
    editor.setAttribute("aria-label", "IME editor fixture");
    document.body.append(editor);
    editor.focus();
  });
  const editor = page.getByRole("textbox", { name: "IME editor fixture" });
  await send({ ...safe, bottom: 0, imeVisible: true });
  await send(safe);
  assert.equal(await editor.evaluate((el) => el.matches(":focus")), true);
  assert.equal(await page.evaluate(() => window.systemBarsMessages.filter((m) => m === "selection:body").length), 1);
  await editor.evaluate((el) => el.remove());

  // Overlay-close guard is time based; wait for the UI to settle before invoking Back again.
  await page.waitForTimeout(140);
  assert.equal(await page.evaluate(() => window.__athaHandleBack()), true);
  await page.locator(".reader-root").waitFor({ state: "detached" });
  await book.waitFor();
  await page.waitForFunction(() => document.activeElement?.matches('[data-shelf-area="catalog"]'));
  await waitForMode("bars:visible");
  assert.deepEqual(errors, []);
  console.log("Android system bars bridge, IME, selection, footnote and same-document return passed (Chromium mock).");
} catch (error) {
  if (page) {
    console.error(await page.evaluate(() => ({
      readerReady: document.querySelector(".reader-root")?.dataset.screenReady,
      nativeMessages: window.systemBarsMessages,
      errors: document.querySelector('[role="alert"]')?.textContent,
    })));
  }
  throw error;
} finally {
  await browser.close();
}
