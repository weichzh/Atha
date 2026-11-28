import assert from "node:assert/strict";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { preview as startPreview } from "vite";
import { verifyConversationEditor } from "./conversation-editor.browser.mjs";
import { verifyReaderWorkspaceNavigation } from "./reader-workspace-navigation.browser.mjs";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(__dirname, "..");
const repoRoot = path.resolve(appRoot, "../..");
const screenshotDir = path.join(repoRoot, "artifacts/local/audits/conversation-editor-v2");

async function assertBorderless(page, selectors) {
  const violations = await page.evaluate((sels) => {
    const hits = [];
    for (const sel of sels) {
      for (const el of document.querySelectorAll(sel)) {
        const st = getComputedStyle(el);
        for (const side of ["borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth"]) {
          const width = parseFloat(st[side] || "0");
          const color = st[side.replace("Width", "Color")];
          const style = st[side.replace("Width", "Style")];
          if (width > 0 && style !== "none" && color !== "rgba(0, 0, 0, 0)" && color !== "transparent") {
            hits.push({ sel, side, width, style, color });
          }
        }
      }
    }
    return hits;
  }, selectors);
  assert.equal(violations.length, 0, `Unexpected borders found: ${JSON.stringify(violations)}`);
}

async function assertNoHorizontalOverflow(page, label) {
  const overflow = await page.evaluate(() => {
    return document.documentElement.scrollWidth > window.innerWidth + 1;
  });
  assert.equal(overflow, false, `Horizontal overflow detected at: ${label}`);
}

async function waitForReaderEntry(page) {
  await page.waitForFunction(() =>
    document.querySelector(".reader-root")?.dataset.screenReady === "true" &&
    !document.querySelector(".opening-cover, .opening-back")
  );
}

async function verifyFootnotePreview(browser, previewUrl) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 820 }, hasTouch: true });
  const release = Promise.withResolvers();
  const requestedPaths = [];
  try {
    await page.addInitScript(() => {
      // Open the reader's shadow root only for this isolated interaction fixture.
      const attach = Element.prototype.attachShadow;
      Element.prototype.attachShadow = function (options) {
        return attach.call(this, this.classList.contains("reader-content-host")
          ? { ...options, mode: "open" } : options);
      };
    });
    const chapter = (name) => fs.readFile(path.join(appRoot, "public/demo", name), "utf8");
    const ch1 = (await chapter("ch1.xhtml")).replace("<body>", `<body>
      <p>书页边注 <a id="note-near" href="#fn-near" role="doc-noteref">[1]</a>
        <a id="note-far" href="ch3.xhtml#fn-far" role="doc-noteref">[2]</a>
        <a id="note-missing" href="unknown.xhtml#fn-missing" role="doc-noteref">[3]</a>
        <a id="note-marker" href="ch2.xhtml#fn-marker" role="doc-noteref">[4]</a>
        <a id="note-image" href="ch3.xhtml#fn-image" role="doc-noteref">[5]</a>
        <a id="ordinary-link" href="ch2.xhtml#sec-2">普通链接</a></p>
      <aside id="fn-near"><p>同章注释 <a href="#note-near" role="doc-backlink">↩</a></p></aside>`);
    const ch2 = (await chapter("ch2.xhtml")).replace("<body>", `<body>
      <p><a id="fn-marker"></a>这段注释的锚点本身没有文字。</p>`);
    const ch3 = (await chapter("ch3.xhtml")).replace("<body>", `<body>
      <aside id="fn-far"><p>跨章注释：书页投影 &lt;原文&gt;。</p>
      <p>读完本段仍可留在原处。</p></aside>
      <aside id="fn-image"><img src="note.gif" alt="脚注插图" width="40" height="30" /></aside>
      <p>插图之后的正文。</p>`);
    const manifest = JSON.parse(await chapter("manifest.json"));
    manifest.resources.push("note.gif");
    await page.route("**/demo/manifest.json", route => route.fulfill({ json: manifest }));
    await page.route("**/demo/note.gif", route => route.fulfill({
      status: 200,
      contentType: "image/gif",
      body: Buffer.from("R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7", "base64"),
    }));
    page.on("request", request => requestedPaths.push(new URL(request.url()).pathname));
    await page.route("**/demo/ch1.xhtml", route =>
      route.fulfill({ status: 200, contentType: "application/xhtml+xml", body: ch1 }));
    await page.route("**/demo/ch2.xhtml", route =>
      route.fulfill({ status: 200, contentType: "application/xhtml+xml", body: ch2 }));
    await page.route("**/demo/ch3.xhtml", async route => {
      await release.promise;
      await route.fulfill({ status: 200, contentType: "application/xhtml+xml", body: ch3 });
    });
    await page.goto(`${previewUrl}?manifest=demo/manifest.json`);
    await page.waitForSelector("html[data-reader-ready]");
    const note = page.getByRole("dialog", { name: /^脚注/ });
    const position = () => page.locator("html").getAttribute("data-section-position");
    const storedLocator = () => page.evaluate(() =>
      localStorage.getItem("atha.reader.progress.a1b2c3d4e5f60718.v1"));
    const originalLocator = await storedLocator();
    assert.ok(originalLocator, "fixture opened at a saved reading position");

    const near = page.locator("#note-near");
    await near.focus();
    await near.press("Enter");
    await note.waitFor();
    assert.equal((await note.locator(".footnote-text").textContent())?.trim(), "同章注释");
    assert.equal(await position(), "1 / 3");
    assert.ok(await note.evaluate(el => el.matches(":modal") && el.classList.contains("anchored")));
    await page.screenshot({
      path: path.join(screenshotDir, "13-reader-footnote-desktop.png"),
      animations: "disabled",
    });
    await page.keyboard.press("ArrowRight");
    assert.equal(await storedLocator(), originalLocator);
    await page.keyboard.press("Escape");
    await note.waitFor({ state: "hidden" });
    assert.ok(await near.evaluate(el => el.getRootNode().activeElement === el));

    await page.setViewportSize({ width: 390, height: 844 });
    await page.waitForFunction(() => Boolean(document.documentElement.dataset.viewportStable));
    const mobileLocator = await storedLocator();
    const missing = page.locator("#note-missing");
    await missing.tap();
    await page.getByRole("status").filter({ hasText: "未找到可预览的脚注" }).waitFor();
    assert.equal(await position(), "1 / 3");
    assert.ok(!requestedPaths.some(url => url.endsWith("/unknown.xhtml")));

    const far = page.locator("#note-far");
    const requested = page.waitForRequest("**/demo/ch3.xhtml");
    await far.tap();
    await requested;
    assert.equal(await page.evaluate(() => window.__athaHandleBack()), true);
    const resolved = page.waitForResponse("**/demo/ch3.xhtml");
    release.resolve();
    await (await resolved).finished();
    // Let the delayed chapter parse; it must not reopen the dismissed preview.
    await page.waitForTimeout(250);
    assert.equal(await note.count(), 0);
    assert.equal(await position(), "1 / 3");
    assert.equal(await storedLocator(), mobileLocator);

    await far.tap();
    await note.waitFor();
    assert.match((await note.locator(".footnote-text").textContent()) ?? "", /跨章注释：书页投影 <原文>/);
    assert.equal(await note.locator(".footnote-text").evaluate(el => el.childElementCount), 0);
    assert.equal(await position(), "1 / 3");
    await assertNoHorizontalOverflow(page, "footnote-preview-390");
    await page.screenshot({
      path: path.join(screenshotDir, "14-reader-footnote-mobile-390.png"),
      animations: "disabled",
    });
    await page.touchscreen.tap(24, 32);
    await note.waitFor({ state: "hidden" });
    assert.equal(await storedLocator(), mobileLocator);

    await far.tap();
    await note.waitFor();
    assert.equal(await page.evaluate(() => window.__athaHandleBack()), true);
    await note.waitFor({ state: "hidden" });

    await far.tap();
    await note.getByRole("button", { name: "跳转原文" }).click();
    await page.waitForFunction(() => document.documentElement.dataset.sectionPosition === "3 / 3");
    await note.waitFor({ state: "hidden" });

    await page.goto(`${previewUrl}?manifest=demo/manifest.json&verify=1`);
    await page.waitForSelector("html[data-reader-ready]");
    await page.locator("#ordinary-link").tap();
    await page.waitForFunction(() => document.documentElement.dataset.sectionPosition === "2 / 3");
    assert.equal(await note.count(), 0);

    // A known target still offers explicit navigation when it has no previewable text.
    for (const [link, destination] of [["#note-marker", "2 / 3"], ["#note-image", "3 / 3"]]) {
      await page.goto(`${previewUrl}?manifest=demo/manifest.json&verify=1`);
      await page.waitForSelector("html[data-reader-ready]");
      await page.locator(link).tap();
      await note.waitFor();
      assert.match((await note.locator(".footnote-text").textContent()) ?? "", /没有可预览的文字/);
      assert.equal(await note.locator("img, svg image, a").count(), 0);
      assert.equal(await position(), "1 / 3");
      await note.getByRole("button", { name: "跳转原文" }).click();
      await page.waitForFunction(expected => document.documentElement.dataset.sectionPosition === expected, destination);
      await note.waitFor({ state: "hidden" });
    }
  } finally {
    release.resolve();
    await page.close();
  }
}

async function verifyShelfSelection(browser, previewUrl) {
  const desktop = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const phone = await browser.newPage({ viewport: { width: 390, height: 844 }, hasTouch: true });
  const group = (page) => page.getByRole("group", { name: "选中书籍操作" });
  const card = (page, title) => page.locator(`.shelf-catalog button[title="${title}"]`);
  const primary = "潮汐与纸页：数字阅读中的对话形态";
  const first = "宋版书刻与江南纸坊考";
  const second = "建筑的现象学与安静空间";
  const third = "海风集：二十四节气散文选";
  const selectionCount = (page, count) => page.getByRole("toolbar", { name: "批量选择" }).getByText(`已选 ${count} 册`, { exact: true }).waitFor();
  const booksAt = (page) => page.evaluate(() => JSON.parse(localStorage.getItem("atha.demo.books.v1")));

  try {
    await desktop.goto(previewUrl);
    await card(desktop, first).click({ button: "right" });
    await selectionCount(desktop, 1);
    assert.equal(await card(desktop, first).getAttribute("aria-pressed"), "true");
    await desktop.keyboard.press("Escape");
    await group(desktop).waitFor({ state: "hidden" });
    await card(desktop, first).press("Shift+F10");
    await selectionCount(desktop, 1);
    await card(desktop, second).click();
    await selectionCount(desktop, 2);
    assert.equal(await group(desktop).getByRole("button", { name: "换封面" }).count(), 0);
    await desktop.screenshot({ path: path.join(screenshotDir, "15-shelf-selection-desktop.png") });
    const search = desktop.getByRole("searchbox");
    await search.fill("江南纸坊");
    await selectionCount(desktop, 1);
    await search.fill("");
    assert.equal(await card(desktop, second).getAttribute("aria-pressed"), "false");
    await group(desktop).getByRole("button", { name: "换封面" }).click();
    assert.ok((await booksAt(desktop)).find(book => book.title === first).hasCustomCover);
    await group(desktop).getByRole("button", { name: "编辑信息" }).click();
    const edit = desktop.getByRole("dialog");
    await edit.getByRole("button", { name: "恢复默认封面" }).click();
    assert.equal((await booksAt(desktop)).find(book => book.title === first).hasCustomCover, false);
    await desktop.keyboard.press("Escape");
    await edit.waitFor({ state: "hidden" });
    await selectionCount(desktop, 1);
    await card(desktop, second).click();
    await group(desktop).getByRole("button", { name: "收藏", exact: true }).click();
    await group(desktop).waitFor({ state: "hidden" });
    assert.ok((await booksAt(desktop)).filter(book => [first, second].includes(book.title)).every(book => book.favorite));

    await desktop.getByRole("button", { name: "清单列表" }).click();
    await desktop.getByRole("button", { name: "选择", exact: true }).click();
    for (const title of [first, second, third]) await card(desktop, title).click();
    await selectionCount(desktop, 3);
    await group(desktop).getByRole("button", { name: "删除", exact: true }).click();
    const confirm = desktop.getByRole("alertdialog");
    await confirm.getByRole("button", { name: "取消", exact: true }).click();
    await confirm.waitFor({ state: "hidden" });
    await selectionCount(desktop, 3);
    assert.ok((await booksAt(desktop)).filter(book => [first, second, third].includes(book.title)).length === 3);

    // Reject the second removal. The third must not run and both must remain selected.
    await desktop.evaluate(() => {
      const write = Storage.prototype.setItem;
      let attempts = 0;
      Storage.prototype.setItem = function(key, value) {
        if (key === "atha.demo.books.v1" && ++attempts === 2) {
          Storage.prototype.setItem = write;
          throw new DOMException("Injected storage failure", "QuotaExceededError");
        }
        return write.call(this, key, value);
      };
    });
    await group(desktop).getByRole("button", { name: "移出", exact: true }).click();
    await confirm.getByRole("button", { name: "移出书架" }).click();
    await desktop.getByText("已移出 1 册，剩余未处理。请重试。").waitFor();
    await selectionCount(desktop, 2);
    assert.equal((await booksAt(desktop)).filter(book => [first, second, third].includes(book.title)).length, 2);
    await group(desktop).getByRole("button", { name: "删除", exact: true }).click();
    await confirm.getByRole("button", { name: "删除本地数据" }).click();
    await group(desktop).waitFor({ state: "hidden" });
    assert.equal((await booksAt(desktop)).filter(book => [first, second, third].includes(book.title)).length, 0);

    await phone.goto(previewUrl);
    await card(phone, primary).scrollIntoViewIfNeeded();
    const bounds = await card(phone, primary).boundingBox();
    const x = bounds.x + bounds.width / 2;
    const y = bounds.y + bounds.height / 2;
    const touch = await phone.context().newCDPSession(phone);
    const deliver = (type, points) => touch.send("Input.dispatchTouchEvent", { type, touchPoints: points });
    await deliver("touchStart", [{ x, y }]);
    await deliver("touchMove", [{ x, y: y - 50 }]);
    await phone.waitForTimeout(475);
    await deliver("touchEnd", []);
    assert.equal(await group(phone).count(), 0, "Scrolling must not activate multi-select");
    await card(phone, primary).scrollIntoViewIfNeeded();
    const moved = await card(phone, primary).boundingBox();
    await deliver("touchStart", [{ x: moved.x + moved.width / 2, y: moved.y + moved.height / 2 }]);
    await selectionCount(phone, 1);
    await deliver("touchEnd", []);
    assert.equal(await card(phone, primary).getAttribute("aria-pressed"), "true");
    assert.equal(await phone.locator(".reader-root").count(), 0);
    await assertNoHorizontalOverflow(phone, "shelf-selection-mobile-390");
    const shelfNav = phone.getByRole("navigation", { name: "主导航" });
    assert.equal(await shelfNav.isVisible(), false, "The action bar replaces mobile navigation while selecting");
    await phone.screenshot({ path: path.join(screenshotDir, "16-shelf-selection-mobile-390.png") });
    assert.equal(await phone.evaluate(() => window.__athaHandleBack()), true);
    await group(phone).waitFor({ state: "hidden" });
    await shelfNav.waitFor();
    await phone.getByRole("button", { name: "选择", exact: true }).click();
    const selectionHeader = phone.getByRole("toolbar", { name: "批量选择" });
    await phone.getByRole("searchbox").fill("潮汐");
    await selectionHeader.getByRole("button", { name: "全选", exact: true }).click();
    await selectionCount(phone, 1);
    assert.equal(await card(phone, primary).getAttribute("aria-pressed"), "true");
    await selectionHeader.getByRole("button", { name: "取消全选" }).click();
    await selectionCount(phone, 0);
    await phone.getByRole("searchbox").fill("");
    await selectionHeader.getByRole("button", { name: "退出选择" }).click();
    await shelfNav.waitFor();
    const lastCard = phone.locator(".shelf-catalog [data-shelf-open]").last();
    await lastCard.scrollIntoViewIfNeeded();
    const deep = await lastCard.boundingBox();
    await deliver("touchStart", [{ x: deep.x + deep.width / 2, y: deep.y + deep.height / 2 }]);
    await selectionCount(phone, 1);
    await deliver("touchEnd", []);
    assert.ok(await phone.evaluate(() => window.scrollY > 200));
    const done = selectionHeader.getByRole("button", { name: "退出选择" });
    const doneBox = await done.boundingBox();
    assert.ok(doneBox.y >= 0 && doneBox.y + doneBox.height <= 160, JSON.stringify(doneBox));
    await done.tap();
    await shelfNav.waitFor();
    await card(phone, primary).tap();
    await waitForReaderEntry(phone);
    assert.equal(await phone.evaluate(() => window.__athaHandleBack()), true);
    await card(phone, primary).waitFor();
  } finally {
    await desktop.close();
    await phone.close();
  }
}

async function verifyShelfResumeAndLayout(browser, previewUrl) {
  const phone = await browser.newPage({ viewport: { width: 390, height: 844 }, hasTouch: true });
  const columns = () => phone.locator(".book-grid").evaluate(grid =>
    getComputedStyle(grid).gridTemplateColumns.trim().split(/\s+/).length);
  const dock = phone.getByRole("complementary", { name: "继续阅读" });
  const history = dock.getByRole("button", { name: "展开最近在读" });
  const drawer = phone.getByRole("dialog", { name: "最近在读" });
  const bar = dock.locator(".resume-bar");
  const nav = phone.getByRole("navigation", { name: "主导航" });
  try {
    await phone.goto(previewUrl);
    await bar.waitFor();
    const mobileBar = await nav.boundingBox();
    assert.equal(mobileBar.x, 0);
    assert.equal(mobileBar.width, 390);
    assert.equal(mobileBar.y + mobileBar.height, 844);
    assert.equal(await columns(), 3);
    assert.ok((await bar.boundingBox()).y + (await bar.boundingBox()).height <= (await nav.boundingBox()).y);
    await phone.locator(".book-grid .cover-art img.loaded").first().waitFor();
    await phone.screenshot({ path: path.join(screenshotDir, "17-shelf-resume-mobile-390.png"), animations: "disabled" });

    await dock.getByRole("button", { name: "收起继续阅读" }).click();
    await bar.waitFor({ state: "hidden" });
    await phone.reload();
    await history.waitFor();
    assert.equal(await bar.count(), 0);
    await history.tap();
    await drawer.waitFor();
    await drawer.locator(".cover-art img.loaded").first().waitFor();
    await phone.screenshot({ path: path.join(screenshotDir, "18-shelf-recent-mobile-390.png"), animations: "disabled" });
    await phone.keyboard.press("Escape");
    await drawer.waitFor({ state: "hidden" });
    assert.equal(await history.evaluate(button => document.activeElement === button), true);
    await history.tap();
    await drawer.waitFor();
    assert.equal(await phone.evaluate(() => window.__athaHandleBack()), true);
    await drawer.waitFor({ state: "hidden" });
    await history.tap();
    await drawer.getByRole("button", { name: "恢复阅读条" }).click();
    await bar.waitFor();

    const search = phone.getByRole("searchbox");
    await search.fill("潮汐");
    await bar.waitFor({ state: "hidden" });
    await history.waitFor();
    await search.fill("");
    await bar.waitFor();

    await phone.getByRole("button", { name: "选择", exact: true }).click();
    await dock.waitFor({ state: "hidden" });
    await phone.getByRole("button", { name: "退出选择" }).click();
    await bar.waitFor();
    await nav.getByRole("tab", { name: "设置" }).click();
    await phone.getByRole("button", { name: /书架布局/ }).click();
    await phone.getByRole("group", { name: "手机封面列数" }).getByRole("button", { name: "2 列" }).click();
    await nav.getByRole("tab", { name: "书架" }).click();
    assert.equal(await columns(), 2);
    await nav.getByRole("tab", { name: "设置" }).click();
    await phone.getByRole("button", { name: /书架布局/ }).click();
    await phone.getByRole("group", { name: "手机封面列数" }).getByRole("button", { name: "4 列" }).click();
    await phone.getByRole("group", { name: "桌面封面密度" }).getByRole("button", { name: "紧凑" }).click();
    await phone.getByLabel("封面下显示书名和作者").uncheck();
    await phone.getByLabel("继续阅读默认收起").check();
    await nav.getByRole("tab", { name: "书架" }).click();
    assert.equal(await columns(), 4);
    assert.equal(await phone.locator(".book-info").count(), 0);
    await assertNoHorizontalOverflow(phone, "shelf-four-columns");
    await phone.reload();
    await history.waitFor();
    assert.equal(await columns(), 4);
    assert.equal(await phone.locator(".book-grid").getAttribute("data-density"), "compact");
    await history.tap();
    await drawer.getByRole("button", { name: /潮汐与纸页/ }).tap();
    await waitForReaderEntry(phone);
    assert.equal(await phone.evaluate(() => window.__athaHandleBack()), true);
    await history.waitFor();
    assert.equal(await history.evaluate(button => document.activeElement === button), true);
    assert.equal(await drawer.count(), 0);
  } finally {
    await phone.close();
  }
}

async function verifySettingsSections(browser, previewUrl) {
  const phone = await browser.newPage({ viewport: { width: 390, height: 844 }, hasTouch: true });
  const index = phone.getByRole("navigation", { name: "设置项目" });
  const search = phone.getByRole("searchbox", { name: "搜索设置" });
  try {
    await phone.addInitScript(() => {
      const key = "atha.ui.prefs.v1";
      if (!localStorage.getItem(key)) localStorage.setItem(key, JSON.stringify({ pageTurnStyle: "scroll" }));
    });
    await phone.goto(previewUrl);
    await phone.getByRole("tab", { name: "设置" }).click();
    await index.waitFor();
    await assertNoHorizontalOverflow(phone, "settings-index-390");
    await phone.screenshot({ path: path.join(screenshotDir, "settings-index-mobile-390.png"), animations: "disabled" });

    assert.equal(await search.count(), 0);
    await phone.getByRole("button", { name: "搜索设置项" }).click();
    assert.equal(await search.evaluate((input) => document.activeElement === input), true);
    await search.fill("3 列");
    assert.deepEqual(await index.locator("[data-setting]").evaluateAll(items => items.map(item => item.dataset.setting)), ["shelf"]);
    await search.fill("跟随系统");
    assert.deepEqual(await index.locator("[data-setting]").evaluateAll(items => items.map(item => item.dataset.setting)), ["appearance"]);
    await search.fill("找不到的设置");
    await index.getByRole("status").waitFor();
    await index.getByRole("button", { name: "清除搜索" }).click();
    assert.equal(await search.inputValue(), "");
    await search.fill("ＭＯＢＩ");
    const dictionary = index.getByRole("button", { name: /离线词典/ });
    assert.equal(await index.locator("[data-setting]").count(), 1);
    await dictionary.click();
    await phone.getByRole("region", { name: "离线词典管理" }).getByRole("button", { name: "导入本地词典" }).waitFor();
    await index.waitFor({ state: "hidden" });
    await phone.getByRole("button", { name: "返回设置", exact: true }).click();
    await index.waitFor();
    assert.equal(await search.inputValue(), "ＭＯＢＩ");
    assert.equal(await dictionary.evaluate((button) => document.activeElement === button), true);

    await search.fill("字号");
    const typography = index.getByRole("button", { name: /全书默认排版/ });
    await typography.click();
    const panel = phone.getByRole("region", { name: "全书阅读默认" });
    await panel.waitFor();
    await assertNoHorizontalOverflow(phone, "settings-detail-390");
    await phone.screenshot({ path: path.join(screenshotDir, "settings-detail-mobile-390.png"), animations: "disabled" });
    await panel.getByRole("button", { name: "恢复默认", exact: true }).click();
    const confirmation = phone.getByRole("alertdialog");
    await confirmation.waitFor();
    assert.equal(await phone.evaluate(() => window.__athaHandleBack()), true);
    await confirmation.waitFor({ state: "hidden" });
    assert.equal(await panel.isVisible(), true);
    assert.equal(await phone.evaluate(() => window.__athaHandleBack()), true);
    await index.waitFor();
    assert.equal(await search.inputValue(), "字号");
    assert.equal(await typography.evaluate((button) => document.activeElement === button), true);

    await search.fill("备份");
    await index.getByRole("button", { name: /本地存储与备份/ }).click();
    const storage = phone.getByRole("region", { name: "本地资料与完整备份" });
    await storage.getByRole("button", { name: "从备份文件恢复" }).waitFor();
    await storage.getByRole("button", { name: /导出 .*完整备份/ }).waitFor();

    assert.equal(await phone.evaluate(() => window.__athaHandleBack()), true);
    await search.fill("手势");
    await index.getByRole("button", { name: /阅读与翻页/ }).click();
    const reading = phone.getByRole("region", { name: "阅读与翻页" });
    const minimal = reading.getByRole("group", { name: "阅读控件" }).getByRole("button", { name: "极简手势" });
    const turn = reading.getByRole("group", { name: "分页动效" });
    const cover = turn.getByRole("button", { name: "覆盖翻页" });
    const pull = reading.getByRole("checkbox", { name: "下拉书签" });
    const swipe = reading.getByRole("checkbox", { name: "上滑打开笔记" });
    assert.equal(await turn.getByRole("button", { name: "左右平移" }).getAttribute("aria-pressed"), "true",
      "legacy scroll token is not a global layout choice");
    assert.equal(await turn.getByRole("button").count(), 3);
    await minimal.click();
    await cover.click();
    await pull.uncheck();
    await swipe.uncheck();
    await assertNoHorizontalOverflow(phone, "reading-controls-390");
    await phone.screenshot({ path: path.join(screenshotDir, "reading-controls-mobile-390.png"), animations: "disabled" });

    await phone.reload();
    await phone.getByRole("tab", { name: "设置" }).click();
    await index.getByRole("button", { name: /阅读与翻页/ }).click();
    assert.equal(await minimal.getAttribute("aria-pressed"), "true");
    assert.equal(await cover.getAttribute("aria-pressed"), "true");
    assert.equal(await pull.isChecked(), false);
    assert.equal(await swipe.isChecked(), false);
    await phone.getByRole("tab", { name: "书架" }).click();
    await phone.locator(".book-open-trigger").first().click();
    await waitForReaderEntry(phone);
    await phone.keyboard.press("h");
    await phone.getByRole("button", { name: "更多阅读操作" }).click();
    await phone.getByRole("button", { name: "排版与主题设置" }).click();
    const readerSettings = phone.getByRole("dialog", { name: "阅读排版设置" });
    await readerSettings.getByRole("tab", { name: "本书覆盖" }).click();
    assert.equal(await readerSettings.getByRole("button", { name: "覆盖", exact: true }).getAttribute("aria-pressed"), "true");
    assert.equal(await phone.locator(".reader").getAttribute("data-reading-mode"), "paged");
  } finally {
    await phone.close();
  }
}

async function verifyGraphiteShell(browser, previewUrl) {
  for (const width of [1440, 360]) {
    const page = await browser.newPage({ viewport: { width, height: 860 }, colorScheme: "light" });
    try {
      await page.goto(previewUrl);
      const navigation = page.getByRole("navigation", { name: "主导航" });
      const settings = navigation.getByRole("tab", { name: "设置" });
      await settings.click();
      const dark = page.getByRole("radio", { name: "黑曜石暗色" });
      if (!(await dark.isVisible())) await page.locator('[data-setting="appearance"]').click();
      const lightBounds = await navigation.boundingBox();
      assert.ok(lightBounds);
      await dark.click();
      await page.waitForFunction(() => document.documentElement.dataset.appTheme === "dark");
      assert.deepEqual(await navigation.boundingBox(), lightBounds, "app theme leaves navigation geometry unchanged");

      const frame = page.locator(".app-frame");
      const navPaint = await navigation.evaluate((node) => {
        const { backdropFilter, backgroundColor } = getComputedStyle(node);
        return { backdropFilter, backgroundColor };
      });
      assert.equal(await frame.evaluate((node) => getComputedStyle(node).colorScheme), "dark");
      assert.equal(await settings.getAttribute("aria-selected"), "true");
      assert.equal(navPaint.backdropFilter, "none");
      assert.equal(navPaint.backgroundColor, width === 360 ? "rgb(45, 44, 46)" : "rgb(39, 39, 41)");
      await assertNoHorizontalOverflow(page, `graphite-settings-${width}`);
      await page.screenshot({ path: path.join(screenshotDir, `graphite-settings-${width}.png`), animations: "disabled" });

      const memory = navigation.getByRole("tab", { name: "阅读记忆" });
      await memory.click();
      await page.locator(".memory-shell").waitFor();
      assert.equal(await frame.evaluate((node) => getComputedStyle(node).backgroundColor), "rgb(21, 21, 22)");
      const library = navigation.getByRole("tab", { name: "书架" });
      await library.click();
      const firstBook = page.locator(".book-open-trigger").first();
      await firstBook.waitFor();
      assert.equal(await page.locator(".favorite-pip").first().evaluate((node) =>
        getComputedStyle(node).color), "rgb(136, 78, 44)", "favorites keep dark ink on their light badges");
      await assertNoHorizontalOverflow(page, `graphite-library-${width}`);
      await page.screenshot({ path: path.join(screenshotDir, `graphite-library-${width}.png`), animations: "disabled" });

      if (width === 360) {
        await page.emulateMedia({ forcedColors: "active" });
        await library.focus();
        await page.keyboard.press("Tab");
        await page.keyboard.press("Shift+Tab");
        assert.equal(await library.evaluate((node) => getComputedStyle(node).outlineStyle), "solid");
        assert.equal(await library.locator(".active-indicator").evaluate((node) => getComputedStyle(node).borderTopWidth), "1px");
        await page.emulateMedia({ forcedColors: "none" });
        await firstBook.click();
        await waitForReaderEntry(page);
        assert.notEqual(await page.locator(".reader-shell").evaluate((node) =>
          getComputedStyle(node).getPropertyValue("--bg-canvas").trim()), "#151516", "reader excludes the shell palette");
      }
    } finally {
      await page.close();
    }
  }
}

async function verifySettingsRemovalDuringBack(browser, previewUrl) {
  const phone = await browser.newPage({ viewport: { width: 390, height: 844 } });
  const errors = [];
  phone.on("pageerror", (error) => errors.push(error.message));
  await phone.addInitScript(() => {
    const dictionary = { id: "d".repeat(64), title: "隔离测试词典", format: "mdict", entryCount: 1, resourceCount: 0, importedAt: 0 };
    const emptyUsage = { totalBytes: 0, booksBytes: 0, cacheBytes: 0, dictionariesBytes: 0, messagesBytes: 0, preferencesBytes: 0 };
    let waitForUsage = false;
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
      transformCallback: () => 1,
      unregisterCallback() {},
      async invoke(command, args) {
        if (command === "list_library_books" || command === "pending_library_book_deletions") return [];
        if (command === "list_local_dictionaries") return [dictionary];
        if (command === "local_data_storage_usage") {
          if (!waitForUsage) return emptyUsage;
          return new Promise((_, reject) => {
            window.finishStatsTest = () => reject(new Error("usage unavailable"));
          });
        }
        if (command === "remove_local_dictionary" && args.dictionaryId === dictionary.id) {
          return new Promise((resolve) => {
            window.finishDictionaryTest = () => {
              waitForUsage = true;
              resolve([]);
            };
          });
        }
        return null;
      },
    };
  });
  try {
    await phone.goto(previewUrl);
    await phone.getByRole("tab", { name: "设置" }).click();
    await phone.getByRole("button", { name: /离线词典/ }).click();
    const panel = phone.getByRole("region", { name: "离线词典管理" });
    await panel.getByRole("button", { name: "移除词典：隔离测试词典" }).click();
    await phone.getByRole("alertdialog").getByRole("button", { name: "移除词典", exact: true }).click();
    await phone.waitForFunction(() => typeof window.finishDictionaryTest === "function");
    assert.equal(await phone.evaluate(() => window.__athaHandleBack()), true);
    assert.equal(await panel.isVisible(), true);
    await phone.evaluate(() => window.finishDictionaryTest());
    await phone.waitForFunction(() => typeof window.finishStatsTest === "function");
    assert.equal(await phone.evaluate(() => window.__athaHandleBack()), true);
    assert.equal(await panel.isVisible(), true);
    await phone.evaluate(() => window.finishStatsTest());
    await phone.getByText("已移除词典《隔离测试词典》，暂时无法刷新存储统计").waitFor();
    assert.equal(await panel.getByRole("button", { name: "移除词典：隔离测试词典" }).count(), 0);
    assert.equal(await phone.evaluate(() => window.__athaHandleBack()), true);
    await phone.getByRole("navigation", { name: "设置项目" }).waitFor();
    assert.deepEqual(errors, []);
  } finally {
    await phone.close();
  }
}

async function verifyCompactShelfToolbar(browser, previewUrl) {
  const phone = await browser.newPage({ viewport: { width: 360, height: 760 }, hasTouch: true });
  const order = phone.getByRole("group", { name: "书架排序" });
  const search = phone.getByRole("searchbox");
  const filters = phone.getByRole("button", { name: "筛选藏书" });
  try {
    await phone.goto(previewUrl);
    await phone.locator(".book-grid .cover-art img.loaded").first().waitFor();
    await assertNoHorizontalOverflow(phone, "compact-shelf-360");
    const searchBox = await search.boundingBox();
    const selectBox = await phone.getByRole("button", { name: "选择", exact: true }).boundingBox();
    assert.ok(Math.abs(searchBox.y + searchBox.height / 2 - selectBox.y - selectBox.height / 2) < 2);
    await phone.getByRole("button", { name: "导入", exact: true }).waitFor();
    assert.ok(await order.evaluate((el) => el.scrollWidth > el.clientWidth));
    assert.ok(await order.evaluate(row => row.scrollLeft <= 2), "only the initial snap inset may be scrolled");
    assert.ok(await filters.evaluate(button => {
      const box = button.getBoundingClientRect();
      return box.width >= 44 && box.left >= 0 && box.right <= innerWidth &&
        button.contains(document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2));
    }), "filter must be discoverable and tappable before scrolling the sort strip");
    await phone.screenshot({ path: path.join(screenshotDir, "shelf-toolbar-360.png"), animations: "disabled" });

    const cards = phone.locator(".shelf-catalog [data-shelf-open]");
    const first = await cards.first().getAttribute("title");
    assert.equal(await order.getByRole("button", { name: "书名 A–Z" }).getAttribute("aria-pressed"), "true");
    await order.getByRole("button", { name: "书名 Z–A" }).press("Enter");
    assert.equal(await order.getByRole("button", { name: "书名 Z–A" }).getAttribute("aria-pressed"), "true");
    assert.notEqual(await cards.first().getAttribute("title"), first);
    await order.getByRole("button", { name: "书名 A–Z" }).click();
    assert.equal(await cards.first().getAttribute("title"), first);
    await order.getByRole("button", { name: "收藏", exact: true }).tap();
    await filters.click();
    assert.equal(await phone.getByRole("switch").getAttribute("aria-checked"), "true");
    assert.equal(await phone.evaluate(() => window.__athaHandleBack()), true);
    await phone.locator(".filter-popover").waitFor({ state: "hidden" });
    await order.getByRole("button", { name: "收藏", exact: true }).tap();
    await search.fill("潮汐");
    assert.equal(await cards.count(), 1);
    assert.equal(await phone.evaluate(() => window.__athaHandleBack()), true);
    assert.equal(await search.inputValue(), "");
    assert.ok(await cards.count() > 1);
    await phone.getByRole("button", { name: "清单列表" }).click();
    await phone.locator(".book-list").waitFor();
    await phone.getByRole("button", { name: "封面网格" }).click();
    await phone.locator(".book-grid").waitFor();

    const quick = order.getByRole("group", { name: "常用分类" }).getByRole("button").first();
    await quick.tap();
    assert.equal(await quick.getAttribute("aria-pressed"), "true");
    assert.ok(await cards.count() > 0);
    const scrollX = await order.evaluate((row) => row.scrollLeft);
    assert.ok(scrollX > 0);
    await cards.first().tap();
    await waitForReaderEntry(phone);
    assert.equal(await phone.evaluate(() => window.__athaHandleBack()), true);
    await quick.waitFor();
    await phone.waitForFunction((x) =>
      Math.abs(document.querySelector(".sort-strip")?.scrollLeft - x) < 2, scrollX);
    assert.equal(await quick.getAttribute("aria-pressed"), "true");
  } finally {
    await phone.close();
  }
}

async function verifyReadingMemoryRecent(browser, previewUrl) {
  const page = await browser.newPage({ viewport: { width: 1100, height: 760 } });
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  // Install the IPC seam before app startup so both shelf and memory use native adapters.
  await page.addInitScript(() => {
    const editionId = "e".repeat(64);
    const locator = JSON.stringify({
      schema: 1, contentVersion: editionId,
      start: { section: "ch1", offset: 0 }, end: { section: "ch1", offset: 4 },
    });
    const root = {
      messageId: "a".repeat(32), rootMessageId: "a".repeat(32),
      conversationId: "c".repeat(32), editionId, title: "隔离记忆夹具", authors: [],
      section: "ch1", selectedText: "原文摘录", text: "", canonicalLocator: locator,
      updatedAt: 1_700_000_000_000,
    };
    const reply = {
      ...root, messageId: "b".repeat(32), text: "跨书检索夹具", updatedAt: root.updatedAt + 60_000,
    };
    let recent = [];
    window.seedTestMemory = () => { recent = [reply, root]; };
    window.testMemoryQueries = [];
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
      transformCallback: () => 1,
      unregisterCallback() {},
      async invoke(command, args) {
        switch (command) {
          case "pending_library_book_deletions":
          case "list_library_books":
            return [];
          case "pending_local_data_restore":
          case "take_startup_import":
          case "plugin:event|unlisten":
            return null;
          case "plugin:event|listen":
            return 1;
          case "reading_memory_search":
            window.testMemoryQueries.push(args.query);
            if (!/\P{White_Space}/u.test(args.query)) return recent;
            if (args.query === "跨书检索") return [reply];
            throw "invalid-message-input";
          default:
            throw new Error(`Unexpected invoke: ${command}`);
        }
      },
    };
  });
  try {
    const tab = page.getByRole("tab", { name: "阅读记忆" });
    const search = page.getByRole("searchbox", { name: "搜索笔记与回复" });
    const results = page.getByRole("region", { name: "记忆检索结果" });
    const ready = page.locator('.memory-hits[aria-busy="false"]');
    const recentTitle = results.getByRole("heading", { name: "最近记忆片段" });
    const notes = results.locator(".hit-note");
    const recentNotes = ["跨书检索夹具", "纯原文标注，尚无后续回应"];

    await page.goto(previewUrl);
    await tab.click();
    await ready.waitFor();
    assert.equal(await recentTitle.count(), 1, "empty queries are labeled as recent memory");
    await results.getByText("还没有阅读记忆。").waitFor();
    assert.equal(await results.getByRole("button", { name: "重试" }).count(), 0);

    await page.evaluate(() => {
      window.seedTestMemory();
      window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true }));
      window.dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true }));
    });
    await notes.nth(1).waitFor();
    assert.deepEqual(await notes.allTextContents(), recentNotes);
    await page.screenshot({
      path: path.join(screenshotDir, "06-memory-recent-ipc-1100.png"),
      animations: "disabled",
    });

    await search.fill("跨书检索");
    await ready.waitFor();
    await results.getByRole("heading", { name: "搜索笔记与回复" }).waitFor();
    assert.deepEqual(await notes.allTextContents(), ["跨书检索夹具"]);
    await page.getByRole("button", { name: "清空搜索" }).click();
    await ready.waitFor();
    await recentTitle.waitFor();
    assert.deepEqual(await notes.allTextContents(), recentNotes);
    assert.equal(await search.inputValue(), "");

    const whitespace = "\u0085\u3000";
    await search.fill(whitespace);
    await ready.waitFor();
    assert.equal(await recentTitle.count(), 1);
    assert.deepEqual(await notes.allTextContents(), recentNotes);
    assert.deepEqual(await page.evaluate(() => window.testMemoryQueries), [
      "", "", "跨书检索", "", whitespace,
    ]);
    assert.deepEqual(errors, []);
  } finally {
    await page.close();
  }
}

async function run() {
  await fs.mkdir(screenshotDir, { recursive: true });

  // 1. Verify production dist/index.html has zero inline scripts (CSP compliance)
  const distHtml = await fs.readFile(path.join(appRoot, "dist/index.html"), "utf8");
  const inlineScriptMatches = [...distHtml.matchAll(/<script\b(?![^>]*\bsrc=)[^>]*>/gi)];
  assert.equal(
    inlineScriptMatches.length,
    0,
    `CSP violation: inline <script> found in dist/index.html: ${inlineScriptMatches.join(", ")}`,
  );

  // Let the OS choose an unused loopback port; no dependency on another preview session.
  const preview = await startPreview({ root: appRoot, preview: { host: "127.0.0.1", port: 0, strictPort: true } });
  let browser;
  try {
    const previewUrl = `http://127.0.0.1:${preview.httpServer.address().port}/`;
    const executablePath = process.env.CHROMIUM_PATH;
    browser = await chromium.launch({
      executablePath,
      headless: true,
      args: ["--no-sandbox", "--disable-gpu"],
    });

    await verifyReadingMemoryRecent(browser, previewUrl);
    await verifyReaderWorkspaceNavigation(browser, previewUrl);
    await verifyConversationEditor(browser, previewUrl, screenshotDir);

    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      deviceScaleFactor: 1,
    });
    const page = await context.newPage();

    const consoleErrors = [];
    const pageErrors = [];
    const failedResponses = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") consoleErrors.push(msg.text());
    });
    page.on("pageerror", (err) => {
      pageErrors.push(err.stack ?? String(err));
    });
    page.on("response", (res) => {
      if (res.status() >= 400) failedResponses.push(`${res.status()} ${res.url()}`);
    });

    // 3. Load Library (Light 1440x900)
    await page.goto(previewUrl, { waitUntil: "networkidle" });
    await page.waitForSelector(".book-card");
    await page.waitForTimeout(450);

    const coverProgress = page.locator(".book-grid .progress-fill").first();
    const progressMotion = await coverProgress.evaluate(async (fill) => {
      const track = fill.parentElement;
      const property = getComputedStyle(fill).transitionProperty;
      const original = fill.style.getPropertyValue("--progress-percent");
      const bar = track.getBoundingClientRect();
      const fraction = () => (fill.getBoundingClientRect().right - bar.left) / bar.width;
      const start = fraction();
      const layoutWidth = fill.offsetWidth;

      fill.style.setProperty("--progress-percent", "100%");
      getComputedStyle(fill).transform;
      await Promise.all(fill.getAnimations().map(animation => animation.finished));
      const complete = fraction();
      const trackUnchanged = track.getBoundingClientRect().width === bar.width &&
        fill.offsetWidth === layoutWidth;

      fill.style.setProperty("--progress-percent", original);
      getComputedStyle(fill).transform;
      await Promise.all(fill.getAnimations().map(animation => animation.finished));
      return { property, expected: Number.parseFloat(original) / 100, start, complete, trackUnchanged };
    });
    assert.equal(progressMotion.property, "transform");
    assert.ok(Math.abs(progressMotion.start - progressMotion.expected) < 0.002);
    assert.ok(Math.abs(progressMotion.complete - 1) < 0.002);
    assert.equal(progressMotion.trackUnchanged, true);
    await page.emulateMedia({ reducedMotion: "reduce" });
    assert.equal(await coverProgress.evaluate(fill => getComputedStyle(fill).transitionDuration), "0s");
    await page.emulateMedia({ reducedMotion: null });

    await assertNoHorizontalOverflow(page, "library-light-1440");
    await assertBorderless(page, [
      ".nav-shell",
      ".search-pill",
      ".book-card",
      ".pill-btn",
      ".recent-card",
    ]);

    await page.screenshot({
      path: path.join(screenshotDir, "01-library-light-1440.png"),
      animations: "disabled",
    });

    // Exercise shelf filter popover, layout toggle, search & C4 continuity
    await page.getByRole("button", { name: /筛选/ }).click();
    await page.waitForSelector(".filter-popover");
    await page.getByRole("switch").click();
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: "清单列表" }).click();
    await page.waitForSelector(".book-list-row");
    await page.getByRole("button", { name: "封面网格" }).click();
    await page.getByRole("button", { name: /重置全部/ }).click();

    // Scroll slightly to verify C4 scroll restoration later
    await page.evaluate(() => window.scrollTo(0, 120));

    // Open primary demo book
    await page.locator('[data-shelf-area="catalog"]').first().click();
    await waitForReaderEntry(page);

    // 4. Pure Reader (1440x900)
    assert.equal(await page.locator(".top-island").count(), 0, "Reader should start distraction-free");
    await assertNoHorizontalOverflow(page, "reader-pure-1440");
    await page.screenshot({
      path: path.join(screenshotDir, "02-reader-pure-1440.png"),
      animations: "disabled",
    });

    // Center click to reveal floating islands
    await page.mouse.click(720, 450);
    await page.waitForSelector(".top-island");
    await page.waitForSelector(".bottom-island");
    await assertBorderless(page, [".top-island", ".bottom-island"]);

    // Test Bookmark toggle & Page Turn
    const bmBtn = page.getByRole("button", { name: /书签/ });
    await bmBtn.click();
    await page.waitForFunction(() => {
      const btn = document.querySelector('button[aria-label*="书签"]');
      return btn?.getAttribute("aria-pressed") === "true";
    });
    await page.getByRole("button", { name: "进度", exact: true }).click();
    await page.getByRole("button", { name: "下一页" }).click();
    await page.waitForTimeout(250);
    await page.getByRole("button", { name: "上一页" }).click();
    await page.getByRole("button", { name: "关闭阅读进度" }).click();
    await page.waitForTimeout(250);

    // Open C2 Reader Settings Dialog + verify Paper theme + CSS Modules lazy CodeMirror
    await page.getByRole("button", { name: "排版与主题设置" }).click();
    await page.waitForSelector(".reader-settings-dialog");
    await page.getByRole("radio", { name: "暖纸" }).click();
    await page.waitForTimeout(200);

    // Inspect CSS Modules tab (lazy CodeMirror 6)
    await page.getByRole("tab", { name: "CSS 模块" }).click();
    await page.getByRole("button", { name: "新建模块" }).click();
    await page.waitForSelector(".cm-editor", { timeout: 10000 });

    // Switch back to Global Defaults tab for clear screenshot 03
    await page.getByRole("tab", { name: "全书阅读默认" }).click();
    await page.waitForTimeout(250);
    await page.screenshot({
      path: path.join(screenshotDir, "03-reader-chrome-settings-1440.png"),
      animations: "disabled",
    });
    await page.keyboard.press("Escape"); // close settings
    await page.waitForTimeout(200);
    await page.mouse.click(720, 450); // hide chrome back to quiet A for selection test
    await page.waitForTimeout(200);

    // 5. Real text selection inside closed Shadow DOM + Floating Pill + Dictionary
    // This fixture's first ordinary paragraph starts about 230px below the
    // page top; anchor the real mouse drag to the paper, not the window.
    const paper = await page.locator(".page").boundingBox();
    await page.mouse.move(paper.x + 120, paper.y + 242);
    await page.mouse.down();
    await page.mouse.move(paper.x + 320, paper.y + 242, { steps: 12 });
    await page.mouse.up();
    await page.waitForSelector(".selection-capsule", { timeout: 5000 });
    await page.waitForTimeout(250);
    await page.screenshot({
      path: path.join(screenshotDir, "04-reader-selection-toolbar-1440.png"),
      animations: "disabled",
    });

    // Dictionary lookup
    await page.locator(".selection-capsule").getByRole("button", { name: "查词" }).click();
    await page.waitForSelector(".dict-card");
    await page.getByRole("button", { name: "关闭词典" }).click();

    // The source must survive focus moving from the book to the editor.
    const newNote = "从原文选区保存的粗体笔记";
    await page.locator(".selection-capsule").getByRole("button", { name: "对话", exact: true }).click();
    await page.waitForSelector(".ProseMirror");
    await page.locator(".ProseMirror").focus();
    await page.keyboard.press("Control+b");
    await page.keyboard.insertText(newNote);
    await page.getByRole("button", { name: "创建对话" }).click();
    await page.waitForFunction((text) => {
      const store = JSON.parse(localStorage.getItem("atha.demo.messages.v1"));
      const root = store.conversations.find((item) => item.messages[0]?.text === text)?.messages[0];
      return Boolean(root?.source?.canonicalLocator && store.revisions[root.id]?.length === 1 &&
        JSON.parse(root.contentJson).content.some((block) =>
          block.content?.some((child) => child.marks?.some((mark) => mark.type === "bold"))));
    }, newNote);
    await page.keyboard.press("Escape");
    await page.waitForSelector(".pane-c", { state: "hidden" });
    await page.locator(".reader-shell").click({ position: { x: 8, y: 8 } });
    await page.keyboard.press("Escape");

    // 6. Open C1 A | B | C Workspace
    await page.keyboard.press("n");
    await page.waitForSelector(".pane-b");
    await page.locator(".note-index-row", { hasText: "真正的阅读并非单向接收" }).click();
    await page.waitForSelector(".pane-c");
    await page.waitForTimeout(450);

    // Verify C1 geometry invariant: A.right <= B.left <= B.right <= C.left (no B | A | C!)
    const abcOrderOk = await page.evaluate(() => {
      const a = document.querySelector(".region-a")?.getBoundingClientRect();
      const b = document.querySelector(".pane-b")?.getBoundingClientRect();
      const c = document.querySelector(".pane-c")?.getBoundingClientRect();
      return Boolean(a && b && c && a.right <= b.left + 2 && b.right <= c.left + 2);
    });
    assert.equal(abcOrderOk, true, "Layout must maintain strict A | B | C order");
    await assertBorderless(page, [".pane-b", ".pane-c", ".note-index-row", ".message-bubble", ".composer-pill"]);

    await page.screenshot({
      path: path.join(screenshotDir, "05-reader-workspace-abc-1440.png"),
      animations: "disabled",
    });

    // Tiptap Composer + Safe Link validation (reject javascript:alert(1), accept https://example.com)
    const prose = page.locator(".ProseMirror");
    await prose.click();
    await page.keyboard.type("测试安全链接：");
    const formatToggle = page.getByRole("button", { name: "排版工具" });
    assert.equal(await formatToggle.getAttribute("aria-expanded"), "false");
    await formatToggle.click();
    await page.getByRole("button", { name: "插入链接" }).click();
    await page.waitForSelector(".link-dialog");
    const linkInput = page.getByRole("textbox", { name: "链接地址" });
    await linkInput.fill("javascript:alert(1)");
    await page.getByRole("button", { name: "确定插入" }).click();
    await page.waitForSelector(".link-err");
    await linkInput.fill("https://example.com");
    await page.getByRole("button", { name: "确定插入" }).click();
    await page.waitForFunction(() => !document.querySelector(".atha-dialog-overlay"));
    await page.waitForTimeout(80);

    // Toggle Markdown <-> Visual
    await page.getByRole("button", { name: "切换 Markdown 源码模式" }).click();
    await page.waitForSelector(".markdown-textarea", { timeout: 10000 });
    await page.getByRole("button", { name: "切换 Markdown 源码模式" }).click();
    await page.waitForSelector(".ProseMirror");

    // Send message
    await page.locator(".send-btn").click();
    await page.waitForTimeout(300);

    // Message Action Menu keyboard nav + AlertDialog Cancel preserves + Confirm deletes
    const lastMore = page.locator(".msg-more-btn").last();
    await lastMore.click();
    await page.waitForSelector('[role="menu"]');
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowUp");
    await page.locator('[role="menuitem"]', { hasText: "删除" }).click();
    await page.waitForSelector(".confirm-dialog");
    const countBeforeCancel = await page.locator(".message-node").count();
    await page.locator(".confirm-dialog").getByRole("button", { name: "取消" }).click();
    await page.waitForTimeout(200);
    assert.equal(await page.locator(".message-node").count(), countBeforeCancel, "Cancel must preserve message");

    await lastMore.click();
    await page.waitForSelector('[role="menu"]');
    await page.locator('[role="menuitem"]', { hasText: "删除" }).click();
    await page.waitForSelector(".confirm-dialog");
    await page.locator(".confirm-dialog").getByRole("button", { name: "确认删除" }).click();
    await page.waitForTimeout(300);
    assert.equal(await page.locator(".message-node").count(), countBeforeCancel - 1, "Confirm delete must remove message");

    // Exercise edits that occur while the real submit handler waits for native IPC.
    await page.evaluate(() => {
      globalThis.__TAURI_INTERNALS__ = {
        invoke(command) {
          if (command !== "message_reply") return Promise.resolve(null);
          return new Promise((resolve) => {
            globalThis.finishTestReply = () => {
              delete globalThis.__TAURI_INTERNALS__;
              resolve({ messageId: "sent", revisionId: "sent" });
            };
          });
        },
      };
    });
    await prose.fill("已发出的部分");
    await page.locator(".send-btn").click();
    await page.waitForFunction(() => typeof globalThis.finishTestReply === "function");
    await prose.fill("发送期间继续输入的草稿");
    await page.evaluate(() => globalThis.finishTestReply());
    await page.locator('.composer-card[aria-busy="false"]').waitFor();
    assert.equal(await prose.innerText(), "发送期间继续输入的草稿");
    await prose.fill("");

    // 7. Dark Mode Reader Workspace (09-reader-workspace-dark-1440.png)
    await page.mouse.click(340, 460); // toggle chrome on center of A
    await page.waitForSelector(".top-island");
    await page.getByRole("button", { name: "排版与主题设置" }).click();
    await page.waitForSelector(".reader-settings-dialog");
    await page.getByRole("radio", { name: "夜间" }).click();
    await page.keyboard.press("Escape");
    await page.waitForTimeout(200);
    await page.mouse.click(340, 460); // hide chrome again so ABC workspace shines cleanly
    await page.waitForTimeout(350);
    await page.screenshot({
      path: path.join(screenshotDir, "09-reader-workspace-dark-1440.png"),
      animations: "disabled",
    });

    // Return to Shelf -> verify C4 continuity
    await page.mouse.click(340, 460);
    await page.waitForSelector(".top-island");
    await page.getByRole("button", { name: "返回书架" }).click();
    await page.waitForSelector(".library-shell");
    await page.waitForTimeout(300);

    // 8. Reading Memory (06-memory-light-1440.png)
    await page.getByRole("tab", { name: "阅读记忆" }).click();
    await page.waitForSelector(".memory-shell");
    await page.waitForTimeout(350);
    await assertNoHorizontalOverflow(page, "memory-light-1440");

    // Test Reading Memory search
    await page.getByRole("searchbox", { name: "搜索笔记与回复" }).fill("快照");
    await page.waitForTimeout(350);
    assert.ok((await page.locator(".hit-row").count()) >= 1, "Reading Memory search should find seeded hits");

    // A real reply saved in the reader is visible on return without retyping the query.
    await page.getByRole("button", { name: "跳回原书" }).first().click();
    await waitForReaderEntry(page);
    await page.waitForSelector(".pane-c");
    const latestReply = "新回复：快照读完不必重搜";
    await page.locator(".pane-c .ProseMirror").fill(latestReply);
    await page.locator(".pane-c .send-btn").click();
    await page.locator(".pane-c .message-node").getByText(latestReply, { exact: true }).waitFor();
    const back = page.locator("#reader-back");
    if (!await back.isVisible()) {
      await page.locator(".reader-shell").focus();
      await page.keyboard.press("h");
    }
    await back.click();
    await page.locator(".memory-shell").getByText(latestReply, { exact: true }).waitFor();
    const memorySearch = page.getByRole("searchbox", { name: "搜索笔记与回复" });
    assert.equal(await memorySearch.inputValue(), "快照");
    await page.screenshot({
      path: path.join(screenshotDir, "06-memory-light-1440.png"),
      animations: "disabled",
    });

    // Native responses can finish in a different order than the user issued queries.
    await page.evaluate(() => {
      const pending = new Map();
      globalThis.testMemoryPending = term => pending.has(term);
      globalThis.finishTestMemory = (term, failed = false) => {
        const result = pending.get(term);
        pending.delete(term);
        if (failed) result.reject(new Error("unavailable"));
        else result.resolve([{
          messageId: term, rootMessageId: term, conversationId: term, editionId: "test-edition",
          title: "合成测试", authors: [], section: "ch1", canonicalLocator: "{}",
          selectedText: term, text: `结果：${term}`, updatedAt: 0,
        }]);
      };
      globalThis.__TAURI_INTERNALS__ = {
        invoke(command, args) {
          if (command !== "reading_memory_search") return Promise.resolve(null);
          return new Promise((resolve, reject) => pending.set(args.query, { resolve, reject }));
        },
      };
    });
    await memorySearch.fill("较早请求");
    await page.waitForFunction(() => globalThis.testMemoryPending("较早请求"));
    await memorySearch.fill("较新请求");
    await page.waitForFunction(() => globalThis.testMemoryPending("较新请求"));
    await page.evaluate(() => globalThis.finishTestMemory("较新请求"));
    await page.getByText("结果：较新请求").waitFor();
    await page.evaluate(() => globalThis.finishTestMemory("较早请求"));
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
    assert.equal(await page.getByText("结果：较早请求").count(), 0);
    assert.equal(await page.getByText("结果：较新请求").count(), 1);

    await memorySearch.fill("失败请求");
    await page.waitForFunction(() => globalThis.testMemoryPending("失败请求"));
    await page.evaluate(() => globalThis.finishTestMemory("失败请求", true));
    await page.getByText("暂时无法读取阅读记忆。").waitFor();
    await page.getByRole("button", { name: "重试" }).click();
    await page.waitForFunction(() => globalThis.testMemoryPending("失败请求"));
    await page.evaluate(() => globalThis.finishTestMemory("失败请求"));
    await page.getByText("结果：失败请求").waitFor();

    await page.evaluate(() => {
      window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true }));
      window.dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true }));
    });
    await page.waitForFunction(() => globalThis.testMemoryPending("失败请求"));
    await page.evaluate(() => {
      globalThis.finishTestMemory("失败请求");
      delete globalThis.__TAURI_INTERNALS__;
      delete globalThis.testMemoryPending;
      delete globalThis.finishTestMemory;
    });
    await memorySearch.fill("快照");
    await page.locator(".memory-shell").getByText(latestReply, { exact: true }).waitFor();

    // Restore an archived inline image from bytes, not from the current book or a remote URL.
    await page.evaluate(() => {
      const key = "atha.demo.messages.v1";
      const store = JSON.parse(localStorage.getItem(key));
      const bytes = Array.from(atob("R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7"), c => c.charCodeAt(0));
      for (const entries of Object.values(store.captures)) {
        for (const capture of entries) {
          capture.snapshot.fragmentHtml += '<img src="Images/archived.gif" alt="历史图像">';
          capture.snapshot.resources.push({
            path: "Images/archived.gif", mediaType: "image/gif", contentHash: "a".repeat(64),
            bytes, byteLength: bytes.length,
          });
        }
      }
      localStorage.setItem(key, JSON.stringify(store));
    });
    await page.getByRole("button", { name: "历史快照" }).first().click();
    const archivedImage = page.locator(".snapshot-shadow-host img[alt='历史图像']").first();
    await archivedImage.waitFor();
    assert.ok(await archivedImage.evaluate(async (image) => {
      await image.decode();
      return image.naturalWidth === 1 && image.src.startsWith("blob:");
    }));
    await page.getByRole("button", { name: "关闭快照" }).click();

    // 9. App Settings (07-settings-light-1440.png)
    await page.getByRole("tab", { name: "设置" }).click();
    await page.waitForSelector(".settings-shell");
    await page.waitForTimeout(350);
    await assertNoHorizontalOverflow(page, "settings-light-1440");
    await page.screenshot({
      path: path.join(screenshotDir, "07-settings-light-1440.png"),
      animations: "disabled",
    });

    // 10. Dark Library (08-library-dark-1440.png)
    await page.getByRole("radio", { name: "黑曜石暗色" }).click();
    await page.getByRole("tab", { name: "书架" }).click();
    await page.waitForSelector(".book-card");
    await page.waitForTimeout(350);
    await page.screenshot({
      path: path.join(screenshotDir, "08-library-dark-1440.png"),
      animations: "disabled",
    });

    // Reset App Theme back to Light for mobile shots
    await page.getByRole("tab", { name: "设置" }).click();
    await page.getByRole("radio", { name: "浅色纸感" }).click();
    await page.getByRole("tab", { name: "书架" }).click();
    await page.waitForSelector(".book-card");

    // 11. Mobile 390x844 (10-library-mobile-390.png & 11-reader-mobile-notes-390.png)
    await page.setViewportSize({ width: 390, height: 844 });
    await page.waitForTimeout(350);
    await assertNoHorizontalOverflow(page, "library-mobile-390");
    await page.screenshot({
      path: path.join(screenshotDir, "10-library-mobile-390.png"),
      animations: "disabled",
    });

    // A canceled turn leaves the same book focused. The next turn reveals paper at cover scale.
    const mobileBook = page.locator('.book-open-trigger').first();
    await mobileBook.click();
    await page.waitForSelector(".opening-cover .cover-art");
    await page.getByRole("button", { name: "返回书架", exact: true }).click();
    await page.waitForFunction(() => !document.querySelector(".opening-cover, .reader-route"));
    assert.ok(await mobileBook.evaluate((el) => el === document.activeElement));
    await mobileBook.click();
    assert.equal(await page.locator("body").getByText(/正在(打开|开启|排版)/).count(), 0);
    const reveal = await page.waitForFunction(() => {
      const camera = document.querySelector("[data-opening-camera]");
      const cover = document.querySelector(".cover-plane");
      if (!camera || !cover) return;
      const bounds = cover.getBoundingClientRect();
      const transform = new DOMMatrixReadOnly(getComputedStyle(camera).transform);
      const basis = Number(camera.closest(".reader-route").style.getPropertyValue("--paper-from-scale"));
      if (Math.abs(transform.a - basis) > 0.001) return;
      return {
        scale: basis,
        paperClip: getComputedStyle(camera.closest(".reader-route")).clipPath,
        spineOffset: Math.abs(transform.e - bounds.left),
        topOffset: Math.abs(transform.f - bounds.top),
      };
    });
    const stage = await reveal.jsonValue();
    assert.ok(stage.scale > 0 && stage.scale < 1, "paper should start smaller than the viewport");
    assert.match(stage.paperClip, /^inset\(/);
    assert.ok(stage.spineOffset < 1 && stage.topOffset < 1, "paper and cover share their origin at 390px");
    await waitForReaderEntry(page);
    // Reset reader theme to paper, then exercise B -> C -> B.

    await page.keyboard.press("h");
    await page.waitForSelector(".top-island");
    await page.getByRole("button", { name: "排版与主题设置" }).click();
    await page.getByRole("radio", { name: "暖纸" }).click();
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");
    await page.waitForTimeout(200);

    await page.keyboard.press("n");
    await page.waitForSelector(".pane-b");
    await page.locator(".note-index-row", { hasText: "真正的阅读并非单向接收" }).click();
    await page.waitForSelector(".pane-c");
    await page.waitForFunction(() => document.querySelectorAll(".pane-b").length === 0);
    assert.equal(await page.locator(".pane-b").count(), 0, "Mobile thread view must replace B with C cleanly");
    await page.waitForTimeout(350);
    await assertNoHorizontalOverflow(page, "reader-mobile-notes-390");
    await page.screenshot({
      path: path.join(screenshotDir, "11-reader-mobile-notes-390.png"),
      animations: "disabled",
    });

    // Test B <- C return on mobile
    await page.getByRole("button", { name: "返回笔记列表" }).click();
    await page.waitForSelector(".pane-b");
    await page.waitForFunction(() => document.querySelectorAll(".pane-c").length === 0);

    // 12. Reduced Motion check
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.locator(".pane-b").getByRole("button", { name: "关闭笔记工作区" }).click();
    await page.waitForTimeout(150);
    assert.equal(await page.locator(".pane-b").count(), 0, "Workspace closes cleanly under reduced motion");

    // Fold a real chapter branch before following one of its nested targets.
    const openToc = async () => {
      const trigger = page.getByRole("button", { name: "目录与全书搜索" });
      if (!await trigger.isVisible()) await page.keyboard.press("h");
      await trigger.click();
      await page.waitForSelector(".toc-viewport");
    };
    await openToc();
    const chapterPosition = () => page.locator("html").getAttribute("data-section-position");
    const startSection = await chapterPosition();
    const collapse = page.getByRole("button", { name: /^折叠：第二章/ });
    const nested = page.getByRole("button", { name: "第二节 · 锚点与快照的双重契约" });
    assert.equal(await collapse.getAttribute("aria-expanded"), "true");
    await collapse.press("Enter");
    assert.equal(await nested.count(), 0);
    assert.equal(await chapterPosition(), startSection);
    await page.getByRole("button", { name: /^展开：第二章/ }).press("Space");
    assert.equal(await nested.isVisible(), true);
    await assertNoHorizontalOverflow(page, "hierarchical-toc-mobile-390");
    await page.screenshot({
      path: path.join(screenshotDir, "12-reader-directory-mobile-390.png"),
      animations: "disabled",
    });
    await nested.click();
    await page.waitForFunction(() => document.documentElement.dataset.sectionPosition === "2 / 3");
    await openToc();
    assert.equal(await nested.getAttribute("aria-current"), "location");
    await page.getByRole("button", { name: "关闭目录" }).click();

    // Direct URL jumps use the saved source anchor, including a later section.
    const memoryTarget = await page.evaluate(() => {
      const store = JSON.parse(localStorage.getItem("atha.demo.messages.v1"));
      const conv = store.conversations.find((item) => item.messages[0]?.source?.section === "ch2");
      const root = conv.messages[0];
      return { version: conv.editionId, conversation: conv.id, root: root.id };
    });
    const query = new URLSearchParams({
      manifest: "demo/manifest.json",
      version: memoryTarget.version,
      "memory-conversation": memoryTarget.conversation,
      "memory-root": memoryTarget.root,
      "memory-message": memoryTarget.root,
    });
    await page.goto(`${previewUrl}?${query}`);
    await page.waitForSelector("html[data-reader-ready]");
    await page.waitForFunction((id) => {
      const raw = JSON.parse(localStorage.getItem(`atha.reader.progress.${id.slice(0, 16)}.v1`));
      return raw && JSON.parse(raw.locator).start.section === "ch2";
    }, memoryTarget.version);

    assert.equal(failedResponses.length, 0, `HTTP >=400 responses: ${failedResponses.join(" | ")}`);
    assert.equal(consoleErrors.length, 0, `Console errors found: ${consoleErrors.join(" | ")}`);
    assert.equal(pageErrors.length, 0, `Page errors found: ${pageErrors.join(" | ")}`);

    await verifyFootnotePreview(browser, previewUrl);
    await verifyShelfSelection(browser, previewUrl);
    await verifyShelfResumeAndLayout(browser, previewUrl);
    await verifySettingsSections(browser, previewUrl);
    await verifyGraphiteShell(browser, previewUrl);
    await verifySettingsRemovalDuringBack(browser, previewUrl);
    await verifyCompactShelfToolbar(browser, previewUrl);
    console.log("ALL BROWSER CHECKS PASSED! Screenshots saved to:", screenshotDir);
  } finally {
    if (browser) await browser.close();
    await preview.close();
  }
}

run().catch((err) => {
  console.error(err);
  process.exit(1);
});
