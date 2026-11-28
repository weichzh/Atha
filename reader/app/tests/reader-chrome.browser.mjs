// Build reader/app first. Use the same Playwright setup as reader-app.browser.mjs.
import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { preview as startPreview } from "vite";

const { chromium } = createRequire(import.meta.url)("playwright");
const appRoot = fileURLToPath(new URL("../", import.meta.url));
const artifacts = new URL("../../../artifacts/local/audits/reader-chrome-v2/", import.meta.url);
const screenshot = (page, name) => page.screenshot({ path: fileURLToPath(new URL(name + ".png", artifacts)) });
const preview = await startPreview({ root: appRoot, preview: { host: "127.0.0.1", port: 0, strictPort: true } });
const base = `http://127.0.0.1:${preview.httpServer.address().port}/`;

async function open(page) {
  await page.goto(base);
  await page.locator('[data-shelf-area="catalog"]').first().click();
  await page.waitForFunction(() => document.querySelector(".reader-root")?.dataset.screenReady === "true" &&
    !document.querySelector(".opening-cover, .opening-back"));
}

async function verticalTouch(page) {
  const cdp = await page.context().newCDPSession(page);
  const box = await page.locator(".reader-frame").boundingBox();
  const y = Math.round(box.y + box.height * 0.46);
  const x = Math.round(box.x + box.width * 0.52);
  const send = (type, delta) => cdp.send("Input.dispatchTouchEvent", {
    type, touchPoints: delta === undefined ? [] : [{ x, y: y + delta }],
  });
  await send("touchStart", 0);
  return {
    move: delta => send("touchMove", delta),
    async finish(cancel = false) {
      await send(cancel ? "touchCancel" : "touchEnd");
      await cdp.detach();
    },
  };
}

async function regionGeometry(page) {
  const box = async selector => page.locator(selector).boundingBox();
  const [top, frame, bottom] = await Promise.all([box(".top-island"), box(".reader-frame"), box(".bottom-island")]);
  assert.ok(top.y + top.height <= frame.y && bottom.y >= frame.y + frame.height,
    JSON.stringify({ top, frame, bottom }));
  assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  return frame;
}

const runningStatus = page => page.locator(".running-foot");
const settings = page => page.getByRole("dialog", { name: "阅读排版设置" });
const more = page => page.getByRole("button", { name: "更多阅读操作" });
const pane = page => page.locator(".pane-b");
const back = page => page.evaluate(() => window.__athaHandleBack());
const lamp = page => page.locator(".bookmark-lamp");
const markedPage = page => page.locator(".running-head svg[aria-label='当前页已加书签']");
const settle = page => page.waitForFunction(() => !document.querySelector(".reader.pulling"));

let browser;
try {
  await mkdir(artifacts, { recursive: true });
  browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH });
  const errors = [];
  const phone = await browser.newPage({
    viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, hasTouch: true, reducedMotion: "reduce",
  });
  const desktop = await browser.newPage({ viewport: { width: 1280, height: 820 }, reducedMotion: "reduce" });
  for (const page of [phone, desktop]) {
    page.setDefaultTimeout(7000);
    page.on("pageerror", error => errors.push(error.message));
  }
  await open(phone);
  assert.match(await runningStatus(phone).textContent(), /本节 \d+\/\d+ 页/);
  assert.match(await phone.locator(".running-foot time").textContent(), /\d{1,2}:\d{2}/);
  assert.equal(await phone.locator(".battery-status").count(), 0);
  const startTitle = await phone.title();
  const startFrame = await phone.locator(".reader-frame").boundingBox();
  await screenshot(phone, "quiet-phone");

  await phone.locator(".running-chapter").tap();
  const directory = phone.getByRole("dialog", { name: "目录、书签与全书搜索" });
  await directory.waitFor();
  const directoryBox = await directory.boundingBox();
  assert.equal(directoryBox.y + directoryBox.height, 844);
  const tabs = await directory.getByRole("tab").all();
  const widths = await Promise.all(tabs.map(tab => tab.boundingBox().then(box => box.width)));
  assert.ok(Math.max(...widths) - Math.min(...widths) < 2);
  await screenshot(phone, "directory-phone");
  assert.equal(await back(phone), true);
  await directory.waitFor({ state: "hidden" });
  await phone.waitForFunction(() => document.activeElement?.matches(".running-chapter"));

  // Move, retreat under the threshold, and release without a save.
  let drag = await verticalTouch(phone);
  await drag.move(55);
  await lamp(phone).waitFor();
  const first = await lamp(phone).locator(".lamp-bulb").boundingBox();
  await drag.move(155);
  await lamp(phone).getByText("松开添加书签").waitFor();
  const armed = await lamp(phone).locator(".lamp-bulb").boundingBox();
  assert.ok(armed.y > first.y + 25, "the lamp must follow the finger");
  assert.equal(await lamp(phone).evaluate(el => el.classList.contains("lit")), true);
  await screenshot(phone, "bookmark-phone");
  await drag.move(34);
  await lamp(phone).getByText("下拉书签").waitFor();
  await drag.finish();
  await settle(phone);
  assert.equal(await markedPage(phone).count(), 0);

  drag = await verticalTouch(phone);
  await drag.move(148);
  await lamp(phone).getByText("松开添加书签").waitFor();
  await drag.finish(true);
  await settle(phone);
  assert.equal(await markedPage(phone).count(), 0);
  drag = await verticalTouch(phone);
  await drag.move(148);
  await drag.finish();
  await markedPage(phone).waitFor();
  drag = await verticalTouch(phone);
  await drag.move(148);
  await lamp(phone).getByText("松开取消书签").waitFor();
  assert.equal(await lamp(phone).evaluate(el => el.classList.contains("lit")), false);
  await drag.finish();
  await markedPage(phone).waitFor({ state: "detached" });
  assert.equal(await phone.title(), startTitle);

  await phone.keyboard.press("h");
  await phone.locator('[data-chrome="classic"]').waitFor();
  assert.deepEqual(await regionGeometry(phone), startFrame);
  const classicActions = phone.locator(".classic-dock-row button > span:first-of-type");
  assert.deepEqual((await classicActions.allTextContents()).map(label => label.trim()), ["目录", "笔记", "进度", "夜间", "样式"]);
  const progress = phone.getByRole("dialog", { name: "阅读进度", exact: true });
  const progressTrigger = phone.getByRole("button", { name: "进度", exact: true });
  assert.equal(await phone.getByRole("slider").count(), 0);
  await screenshot(phone, "classic-phone");
  const beforeProgress = await phone.title();
  await progressTrigger.tap();
  await progress.waitFor();
  assert.deepEqual(await regionGeometry(phone), startFrame);
  await screenshot(phone, "classic-progress-phone");
  await progress.getByRole("button", { name: "下一页" }).tap();
  await phone.waitForFunction(title => document.title !== title, beforeProgress);
  await progress.getByRole("button", { name: "上一页" }).tap();
  await phone.waitForFunction(title => document.title === title, beforeProgress);
  await phone.keyboard.press("Escape");
  await progress.waitFor({ state: "hidden" });
  await phone.waitForFunction(() => document.activeElement?.getAttribute("aria-label") === "进度");
  await phone.getByRole("button", { name: "切换夜间模式" }).tap();
  await phone.waitForSelector('.reader-root[data-reader-theme="dark"]');
  await phone.getByRole("button", { name: "切换日间模式" }).tap();
  await phone.waitForSelector('.reader-root:not([data-reader-theme="dark"])');
  await phone.getByRole("button", { name: "排版与主题设置" }).tap();
  await settings(phone).getByRole("radio", { name: "暖纸" }).tap();
  await phone.waitForSelector('.reader-root[data-reader-theme="paper"]');
  await settings(phone).getByRole("radio", { name: "夜间" }).tap();
  await phone.waitForSelector('.reader-root[data-reader-theme="dark"]');
  await settings(phone).getByRole("button", { name: "关闭设置" }).tap();
  await phone.getByRole("button", { name: "切换日间模式" }).tap();
  await phone.waitForSelector('.reader-root[data-reader-theme="paper"]');
  await phone.getByRole("button", { name: "排版与主题设置" }).tap();
  await settings(phone).getByRole("button", { name: "极简", exact: true }).tap();
  const settingsBox = await settings(phone).boundingBox();
  assert.equal(settingsBox.height, directoryBox.height);
  await screenshot(phone, "settings-phone");
  await settings(phone).getByRole("button", { name: "关闭设置" }).tap();
  await phone.locator('[data-chrome="minimal"]').waitFor();
  assert.deepEqual(await regionGeometry(phone), startFrame);
  const top = phone.locator(".top-island button");
  assert.equal(await top.first().getAttribute("aria-label"), "返回书架");
  assert.match(await top.nth(1).getAttribute("aria-label"), /打开目录$/);
  assert.equal(await top.nth(2).getAttribute("aria-label"), "更多阅读操作");
  await screenshot(phone, "minimal-phone");

  // Android Back closes the anchored actions first, then controls; Escape agrees.
  await more(phone).tap();
  await phone.getByRole("button", { name: "笔记与对话", exact: true }).waitFor();
  await screenshot(phone, "minimal-menu-phone");
  await phone.getByRole("button", { name: /^阅读进度/ }).tap();
  await progress.waitFor();
  assert.equal(await back(phone), true);
  await progress.waitFor({ state: "hidden" });
  await phone.waitForFunction(() => document.activeElement?.getAttribute("aria-label") === "更多阅读操作");
  await more(phone).tap();
  const switchClassic = phone.getByRole("button", { name: "切换经典双栏模式" });
  await switchClassic.tap();
  await phone.locator('[data-chrome="classic"]').waitFor();
  assert.deepEqual(await regionGeometry(phone), startFrame);
  await phone.getByRole("button", { name: "排版与主题设置" }).tap();
  await settings(phone).getByRole("button", { name: "极简", exact: true }).tap();
  await settings(phone).getByRole("button", { name: "关闭设置" }).tap();
  await more(phone).tap();
  assert.equal(await back(phone), true);
  await phone.getByRole("button", { name: "笔记与对话", exact: true }).waitFor({ state: "hidden" });
  assert.equal(await more(phone).getAttribute("aria-expanded"), "false");
  await more(phone).tap();
  await phone.keyboard.press("Escape");
  await phone.waitForFunction(() => document.querySelector('[aria-label="更多阅读操作"]')?.getAttribute("aria-expanded") === "false");

  const chapter = phone.locator(".island-chapter");
  const startChapter = await chapter.textContent();
  await phone.getByRole("button", { name: "下一章" }).tap();
  await phone.waitForFunction((label) => document.querySelector(".island-chapter")?.textContent !== label, startChapter);
  await phone.getByRole("button", { name: "上一章" }).tap();
  await phone.waitForFunction((label) => document.querySelector(".island-chapter")?.textContent === label, startChapter);
  await phone.keyboard.press("h");
  await runningStatus(phone).waitFor();
  drag = await verticalTouch(phone);
  await drag.move(-145);
  await phone.getByText("松开打开笔记", { exact: true }).waitFor();
  await drag.finish();
  await pane(phone).waitFor();
  assert.equal(await back(phone), true);
  await pane(phone).waitFor({ state: "hidden" });
  await runningStatus(phone).waitFor();
  assert.equal(await phone.title(), startTitle);

  // Changing the mode in a scroll book does not force it to repaginate.
  await phone.keyboard.press("h");
  await more(phone).tap();
  await phone.getByRole("button", { name: "排版与主题设置" }).tap();
  await settings(phone).getByRole("tab", { name: "本书覆盖" }).tap();
  await settings(phone).getByRole("button", { name: "连续滚动" }).tap();
  await phone.waitForSelector('.reader[data-reading-mode="scroll"]');
  await settings(phone).getByRole("tab", { name: "全书阅读默认" }).tap();
  await settings(phone).getByRole("button", { name: "经典", exact: true }).tap();
  await settings(phone).getByRole("button", { name: "关闭设置" }).tap();
  await phone.locator('[data-chrome="classic"]').waitFor();
  await phone.keyboard.press("h");
  await runningStatus(phone).waitFor();
  drag = await verticalTouch(phone);
  await drag.move(-145);
  await drag.finish();
  await phone.waitForFunction(() => document.querySelector(".reader")?.scrollTop > 0);
  assert.equal(await lamp(phone).count(), 0);
  assert.equal(await pane(phone).count(), 0);
  assert.equal(await phone.locator(".reader").getAttribute("data-reading-mode"), "scroll");

  // Select a known span through the normal selection listener; exercise the real clipboard
  // in Chromium, then make a subsequent write fail without losing the selected passage.
  await desktop.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await desktop.addInitScript(() => {
    const attach = Element.prototype.attachShadow;
    Element.prototype.attachShadow = function (options) {
      return attach.call(this, this.classList.contains("reader-content-host") ? { ...options, mode: "open" } : options);
    };
  });
  await open(desktop);
  const select = () => desktop.locator(".reader-content-host").evaluate(host => {
    const paragraph = host.shadowRoot.querySelector("p");
    const range = document.createRange();
    range.selectNodeContents(paragraph);
    window.getSelection().removeAllRanges();
    window.getSelection().addRange(range);
    return range.toString().trim();
  });
  const copy = desktop.getByRole("toolbar", { name: "选区操作" }).getByRole("button", { name: "复制" });
  const selectedText = await select();
  assert.ok(selectedText.length > 0);
  await copy.click();
  await desktop.getByRole("status").getByText("已复制").waitFor();
  assert.equal(await desktop.evaluate(() => navigator.clipboard.readText()), selectedText);
  await copy.waitFor({ state: "detached" });
  await screenshot(desktop, "copy-desktop");
  await desktop.evaluate(() => {
    navigator.clipboard.writeText = () => Promise.reject(new DOMException("Fixture denied clipboard", "NotAllowedError"));
  });
  await select();
  await copy.click();
  await desktop.getByRole("status").getByText("未能复制，请重试").waitFor();
  assert.equal(await copy.isVisible(), true);
  await desktop.keyboard.press("Escape");
  await copy.waitFor({ state: "detached" });
  await desktop.getByRole("status").getByText("未能复制，请重试").waitFor({ state: "hidden" });
  await desktop.keyboard.press("h");
  await regionGeometry(desktop);
  await screenshot(desktop, "classic-desktop");
  assert.deepEqual(errors, []);
  console.log("Reader rails, modes, TOC, native/Escape Back, chapter navigation, vertical touch and clipboard passed.");
} finally {
  await browser?.close();
  await new Promise(resolve => preview.httpServer.close(resolve));
}
