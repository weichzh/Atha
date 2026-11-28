// Windows shelf gate driver. The browser fixture uses the same shelf assertions
// on the production Svelte build, with only the native picker/data adapter replaced.
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { promisify } from "node:util";
import { chromium } from "playwright";
import { preview } from "vite";

const appRoot = fileURLToPath(new URL("../", import.meta.url));
const pickerScript = fileURLToPath(new URL("../../../scripts/lib/submit-library-shelf-picker.ps1", import.meta.url));
const cardSelector = ".shelf-catalog [data-shelf-open]";
const timeout = 10_000;
const loadTimeout = 30_000;
const button = (scope, name) => scope.getByRole("button", { name, exact: true });
const shelf = (page) => page.locator("main.library-shell[aria-busy=false]");
const search = (page) => shelf(page).getByRole("searchbox");
const sort = (page) => shelf(page).getByRole("group", { name: "书架排序" });
const select = (page) => shelf(page).getByRole("group", { name: "选中书籍操作" });
const card = (page, id) => shelf(page).locator(`${cardSelector}[data-shelf-open="${id}"]`);

class ShelfStageError extends Error {
  constructor(stage, cause) {
    super(`Shelf ${stage} failed.`, { cause });
  }
}

async function stage(name, work) {
  try {
    return await work();
  } catch (error) {
    throw new ShelfStageError(name, error);
  }
}

function watchPage(page, checkHttp) {
  const failures = { page: 0, console: 0, http: 0 };
  page.on("pageerror", () => failures.page++);
  page.on("console", (message) => { if (message.type() === "error") failures.console++; });
  if (checkHttp) {
    page.on("response", (response) => {
      if (response.status() >= 400 && !new URL(response.url()).pathname.endsWith("/favicon.ico")) failures.http++;
    });
  }
  return () => assert.deepEqual(failures, { page: 0, console: 0, http: 0 });
}

function configurePage(page) {
  page.setDefaultTimeout(timeout);
}

function waitForOrder(page, ids) {
  return page.waitForFunction(({ selector, ids }) => {
    if (!document.querySelector("main.library-shell[aria-busy=false]")) return false;
    const cards = [...document.querySelectorAll(selector)];
    return cards.length === ids.length && cards.every((el, index) => el.dataset.shelfOpen === ids[index]);
  }, { selector: cardSelector, ids }, { timeout });
}

function waitForAttribute(page, selector, name, value) {
  return page.waitForFunction(({ selector, name, value }) =>
    document.querySelector(selector)?.getAttribute(name) === value, { selector, name, value }, { timeout });
}

async function readBooks(page) {
  const books = await shelf(page).locator(cardSelector).evaluateAll((cards) => cards.map((el) => ({
    id: el.dataset.shelfOpen,
    title: el.title,
    author: el.closest(".book-card")?.querySelector(".book-author")?.textContent.trim() ?? "",
  })));
  for (const { id, title } of books) assert.ok(/^[a-f0-9]{64}$/.test(id) && title);
  return books;
}

async function waitForCount(page, count) {
  await shelf(page).waitFor();
  await page.waitForFunction(({ selector, count }) =>
    document.querySelectorAll(selector).length === count,
    { selector: cardSelector, count }, { timeout: loadTimeout });
}

function expectNoMatches(page) {
  return shelf(page).getByText("没有符合当前条件的书籍。", { exact: true }).waitFor();
}

async function runPicker(page, processId, bookPath, powershell) {
  const controller = new AbortController();
  const picker = promisify(execFile)(powershell, [
    "-NoProfile", "-NonInteractive", "-File", pickerScript,
    "-ProcessId", processId,
  ], {
    env: { ...process.env, ATHA_SHELF_GATE_PICKER_BOOK: bookPath },
    windowsHide: true, signal: controller.signal, timeout: 25_000,
  });
  try {
    await Promise.all([picker, button(shelf(page), "导入").click()]);
  } finally {
    controller.abort();
  }
}

async function importBook(page, knownBooks, pick) {
  await pick();
  await waitForCount(page, knownBooks.length + 1);
  await shelf(page).getByText("已导入新书至书架", { exact: true }).waitFor();
  const books = await readBooks(page);
  assert.ok(knownBooks.every(({ id }) => books.some((book) => book.id === id)));
  const added = books.filter(({ id }) => !knownBooks.some((book) => book.id === id));
  assert.equal(added.length, 1);
  return added[0];
}

export async function exerciseShelf(page, target, control, { native = false } = {}) {
  const ascending = await stage("title order", async () => {
    const books = await readBooks(page);
    assert.equal(books.length, 2);
    assert.notEqual(target.title, control.title, "distinct titles are required for ordering");
    const asc = books.map(({ id }) => id);
    // The browser fixture has fixed titles; the Windows input may be any
    // authored EPUB, so compare the two identities before and after reversing.
    if (!native) assert.deepEqual(asc, [target.id, control.id]);
    assert.equal(await button(sort(page), "书名 A–Z").getAttribute("aria-pressed"), "true");
    await button(sort(page), "书名 Z–A").click();
    await waitForOrder(page, [...asc].reverse());
    assert.equal(await button(sort(page), "书名 Z–A").getAttribute("aria-pressed"), "true");
    await button(shelf(page), "清单列表").click();
    await shelf(page).locator(".book-list.shelf-catalog").waitFor();
    await waitForOrder(page, [...asc].reverse());
    return asc;
  });

  const filterPanel = page.locator(".filter-popover");
  const openFilters = () => button(shelf(page), "筛选藏书").click();
  const closeFilters = async () => {
    await page.keyboard.press("Escape");
    await filterPanel.waitFor({ state: "hidden" });
  };
  const filterChip = (label) => button(shelf(page).locator(".active-conditions"), label);

  await stage("author and latest-added order", async () => {
    if (native) {
      // The EPUB has a named author; the isolated TXT has none. Named authors sort first.
      await button(sort(page), "作者").click();
      await waitForOrder(page, [target.id, control.id]);
    }
    await button(sort(page), "最近添加").click();
    await waitForOrder(page, [control.id, target.id]);
    await button(sort(page), "书名 Z–A").click();
    await waitForOrder(page, [...ascending].reverse());
  });

  await stage("title/author search and reading-state filter", async () => {
    assert.ok(target.author, "an authored EPUB is required");
    const fold = (value) => value.normalize("NFKC").toLocaleLowerCase();
    for (const query of [target.title, target.author.split(" / ")[0]]) {
      await search(page).fill(`no-shelf-match-${Date.now()}`);
      await expectNoMatches(page);
      await waitForOrder(page, []);
      await search(page).fill(query);
      // A caller's EPUB can share a search term with the title of the untagged
      // TXT control. In that case both identities must still be returned.
      const shared = native && fold(query).trim().split(/\s+/u)
        .every((part) => fold(control.title).includes(part));
      await waitForOrder(page, shared ? [...ascending].reverse() : [target.id]);
    }
    await button(shelf(page), "清空搜索").click();
    await waitForOrder(page, [...ascending].reverse());

    await openFilters();
    await filterPanel.getByRole("button", { name: /^未读\s*2$/ }).click();
    await closeFilters();
    await filterChip("阅读状态：未读").waitFor();
    await waitForOrder(page, [...ascending].reverse());
    await filterChip("阅读状态：未读").click();
    await openFilters();
    await filterPanel.getByRole("switch", { name: "仅看收藏书籍" }).click();
    await closeFilters();
    await expectNoMatches(page);
    await waitForOrder(page, []);
    await filterChip("仅看收藏").click();
    await waitForOrder(page, [...ascending].reverse());
  });

  await stage("selection and favorite", async () => {
    await button(shelf(page), "选择").click();
    await card(page, target.id).click();
    await card(page, control.id).click();
    await select(page).getByText("已选 2 册", { exact: true }).waitFor();
    for (const { id } of [target, control]) assert.equal(await card(page, id).getAttribute("aria-pressed"), "true");
    await button(select(page), "退出选择").click();
    await select(page).waitFor({ state: "hidden" });
    assert.equal(await card(page, target.id).getAttribute("aria-pressed"), null);
    await waitForOrder(page, [...ascending].reverse());

    // The keyboard context action replaces the old book menu on this surface.
    await card(page, target.id).press("Shift+F10");
    await select(page).getByText("已选 1 册", { exact: true }).waitFor();
    assert.equal(await card(page, target.id).getAttribute("aria-pressed"), "true");
    await button(select(page), "收藏").click();
    await page.getByText("已收藏 1 册", { exact: true }).waitFor();
    await select(page).waitFor({ state: "hidden" });

    await button(sort(page), "收藏").click();
    await waitForOrder(page, [target.id]);
    await openFilters();
    assert.equal(await filterPanel.getByRole("switch", { name: "仅看收藏书籍" }).getAttribute("aria-checked"), "true");
    await closeFilters();
  });

  await stage("open book and return to shelf", async () => {
    await search(page).fill(target.title);
    await waitForOrder(page, [target.id]);
    if (native) assert.equal(new URL(page.url()).pathname, "/");
    await card(page, target.id).click();
    await page.waitForFunction(() =>
      document.querySelector(".reader-root")?.dataset.screenReady === "true" &&
      !document.querySelector(".opening-art, .opening-veil"), null, { timeout: loadTimeout });
    assert.equal(await page.locator(".reader-route [role=alert]").count(), 0);
    const back = button(page, "返回书架");
    if (!await back.isVisible()) await page.locator(".reader-content-host").click();
    await back.waitFor();
    if (native) {
      const url = new URL(page.url());
      assert.equal(url.origin, "https://tauri.localhost");
      assert.equal(url.pathname, "/index.html");
      assert.equal(url.searchParams.get("version"), target.id);
    }
    await back.click();
    await shelf(page).locator(".book-list.shelf-catalog").waitFor();
    await waitForOrder(page, [target.id]);
    await page.waitForFunction((id) => document.activeElement?.dataset.shelfOpen === id, target.id, { timeout });
    assert.equal(await search(page).inputValue(), target.title);
    assert.equal(await button(sort(page), "书名 Z–A").getAttribute("aria-pressed"), "true");
    assert.equal(await button(sort(page), "收藏").getAttribute("aria-pressed"), "true");
    if (native) assert.equal(page.url(), "https://tauri.localhost/");
  });

  await stage("cancel then confirm removal", async () => {
    await button(shelf(page), "选择").click();
    await card(page, target.id).click();
    await select(page).getByText("已选 1 册", { exact: true }).waitFor();
    const confirmation = page.getByRole("alertdialog", { name: "将书籍移出书架？" });
    await button(select(page), "移出").click();
    await confirmation.waitFor();
    await button(confirmation, "取消").click();
    await confirmation.waitFor({ state: "hidden" });
    await waitForOrder(page, [target.id]);
    assert.equal(await card(page, target.id).getAttribute("aria-pressed"), "true");
    await button(select(page), "移出").click();
    await button(confirmation, "移出书架").click();
    await confirmation.waitFor({ state: "hidden" });
    await select(page).waitFor({ state: "hidden" });
    await expectNoMatches(page);
    await button(shelf(page), "显示全部藏书").click();
    await waitForOrder(page, [control.id]);
    await page.reload();
    await waitForOrder(page, [control.id]);
  });
}

export async function runNative(cdp, processId, bookPath, controlPath, powershell) {
  assert.ok(bookPath, "expected a native EPUB input");
  const browser = await chromium.connectOverCDP(cdp, { timeout });
  try {
    const [context] = browser.contexts();
    assert.ok(context && context.pages().length <= 1, "expected one isolated native webview");
    const page = context.pages()[0] ?? await context.waitForEvent("page", { timeout });
    configurePage(page);
    const assertClean = watchPage(page, false);

    const [target, control] = await stage("native launch and picker imports", async () => {
      await page.waitForURL("https://tauri.localhost/", { waitUntil: "domcontentloaded", timeout });
      assert.equal(await page.evaluate(() => typeof window.__TAURI_INTERNALS__?.invoke), "function");
      await waitForCount(page, 0);
      await expectNoMatches(page);
      const target = await importBook(page, [], () => runPicker(page, processId, bookPath, powershell));
      const control = await importBook(page, [target], () => runPicker(page, processId, controlPath, powershell));
      assert.equal(control.title, path.parse(controlPath).name);
      assert.equal(control.author, "佚名");
      assertClean();
      return [target, control];
    });
    await exerciseShelf(page, target, control, { native: true });
    await stage("native console and page errors", async () => assertClean());
  } finally {
    // A connected Playwright client disconnects; PowerShell owns the process.
    await browser.close();
  }
}

async function verifyResponsive(page, screenshots) {
  const openGeometry = () => page.evaluate(() => {
    const grid = document.querySelector(".book-grid.shelf-catalog");
    const controls = [...document.querySelectorAll(
      ".search-pill, .filter-trigger, .shelf-layout-toggle, .select-trigger, .import-btn",
    )];
    const sizes = controls.map((element) => {
      const rect = element.getBoundingClientRect();
      return rect.width >= 44 && rect.height >= 44;
    });
    return {
      withinViewport: document.documentElement.scrollWidth <= innerWidth + 1,
      controlsValid: sizes.length === 5 && sizes.every(Boolean),
      columns: getComputedStyle(grid).gridTemplateColumns.split(" ").length,
      books: grid.querySelectorAll("[data-shelf-open]").length,
    };
  });
  for (const [width, height] of [[360, 800], [412, 915], [768, 1024], [1280, 900]]) {
    await page.setViewportSize({ width, height });
    const grid = shelf(page).locator(".book-grid.shelf-catalog");
    await grid.waitFor();
    const { withinViewport, controlsValid, columns, books } = await openGeometry();
    assert.ok(withinViewport && controlsValid, `shelf geometry at ${width}x${height}`);
    assert.ok(width < 900 ? columns === 3 : columns >= 4, `grid columns at ${width}x${height}`);
    assert.ok(books > columns, "geometry fixture must have multiple rows");
    await button(shelf(page), "选择").click();
    const last = grid.locator("[data-shelf-open]").last();
    await last.click();
    await select(page).getByText("已选 1 册", { exact: true }).waitFor();
    await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
    const lastBox = await last.boundingBox();
    const actionsBox = await select(page).boundingBox();
    const removeBox = await button(select(page), "移出").boundingBox();
    assert.ok(lastBox.y + lastBox.height <= actionsBox.y + 1, `last row behind actions at ${width}x${height}`);
    assert.ok(removeBox.width >= 44 && removeBox.height >= 44);
    await button(select(page), "退出选择").click();
    await page.evaluate(() => window.scrollTo(0, 0));
    if (width === 360 || width === 1280) {
      await page.screenshot({ path: path.join(screenshots, `library-shelf-${width}x${height}.png`) });
    }
  }
}

async function verifySettings(page) {
  await page.setViewportSize({ width: 360, height: 800 });
  await page.getByRole("tab", { name: "设置", exact: true }).click();
  const settings = page.locator(".settings-shell");
  const settingsBack = () => button(settings, "返回设置").click();
  const section = (id) => settings.locator(`button[data-setting="${id}"]`).click();
  const appearanceKey = "atha.app.appearance.v1";
  const defaultsKey = "atha.reader.application.v1";
  await section("appearance");
  const color = () => page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  const mode = (name) => settings.getByRole("radio", { name, exact: true });
  for (const label of ["跟随系统", "浅色纸感", "黑曜石暗色"]) await mode(label).waitFor();
  await mode("黑曜石暗色").click();
  const dark = await color();
  await mode("浅色纸感").click();
  assert.notEqual(await color(), dark);
  assert.equal(await page.locator("html").getAttribute("data-app-theme"), "light");
  assert.equal(await page.evaluate((key) => JSON.parse(localStorage.getItem(key)).theme, appearanceKey), "light");
  await settingsBack();

  await section("typography");
  await page.evaluate((key) => localStorage.setItem(key, JSON.stringify({
    schema: 1,
    preferences: { theme: "paper", brightness: 90, fontSize: 24, fontFamily: "serif", density: "compact" },
  })), defaultsKey);
  await button(settings, "恢复默认").click();
  const restore = page.getByRole("alertdialog", { name: "恢复全书阅读默认？" });
  await button(restore, "确认恢复").click();
  await restore.waitFor({ state: "hidden" });
  const prefs = await page.evaluate(([appKey, readerKey]) => ({
    app: JSON.parse(localStorage.getItem(appKey)).theme,
    reader: JSON.parse(localStorage.getItem(readerKey)).preferences,
  }), [appearanceKey, defaultsKey]);
  assert.deepEqual(prefs, {
    app: "light",
    reader: { theme: "system", brightness: 100, fontSize: 19, fontFamily: "book", density: "standard" },
  });
  await settingsBack();

  await section("dictionary");
  const scales = settings.getByRole("radiogroup", { name: "词典字号缩放" });
  await scales.waitFor();
  assert.equal(await scales.getByRole("radio").count(), 6);
  await button(settings, "导入本地词典").waitFor();
  await settings.getByRole("button", { name: /^移除词典：/ }).first().waitFor();
  await settingsBack();
  await section("storage");
  await settings.getByText("本机存储占用", { exact: true }).waitFor();
  await button(settings, "导出 .atha-data 完整备份").waitFor();
  await button(settings, "从备份文件恢复").waitFor();
  assert.equal(await settings.locator("main.library-shell, .library-management").count(), 0);
  assert.ok(await settings.evaluate((element) => {
    const bounds = element.getBoundingClientRect();
    return bounds.width > 0 && bounds.left >= 0 && bounds.right <= innerWidth + 1;
  }));
}

async function withFixturePage(browser, setup, work) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, reducedMotion: "reduce" });
  try {
    const page = await context.newPage();
    configurePage(page);
    const assertClean = watchPage(page, true);
    await setup?.(page);
    await work(page);
    assertClean();
  } finally {
    await context.close();
  }
}

export async function runBrowser(browser, screenshots) {
  const server = await preview({ root: appRoot, preview: { host: "127.0.0.1", port: 0, strictPort: true } });
  const url = `http://127.0.0.1:${server.httpServer.address().port}/`;
  try {
    await fs.mkdir(screenshots, { recursive: true });
    await withFixturePage(browser, null, async (page) => {
      await page.goto(url);
      await stage("four-viewport grid and action geometry", () => verifyResponsive(page, screenshots));
      await stage("settings surfaces and independent defaults", () => verifySettings(page));
    });
    await withFixturePage(browser, async (page) => {
      await page.addInitScript(() => {
        if (localStorage.getItem("atha.demo.books.v1") === null) {
          localStorage.setItem("atha.demo.books.v1", JSON.stringify([{
            id: "e".repeat(64), title: "Atha Fixture Target", authors: ["Morgan Fixture"],
            hasCover: false, hasCustomCover: false, importedAt: 1, prepared: true, favorite: false,
          }]));
        }
      });
    }, async (page) => {
      await page.goto(url);
      const [target, control] = await stage("browser demo import", async () => {
        await waitForCount(page, 1);
        const [target] = await readBooks(page);
        const control = await importBook(page, [target], () => button(shelf(page), "导入").click());
        assert.equal(control.title, "新入书斋：数字文献装帧随笔");
        return [target, control];
      });
      await exerciseShelf(page, target, control);
    });
  } finally {
    await new Promise((resolve, reject) => server.httpServer.close((error) => error ? reject(error) : resolve()));
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [mode, cdp, ...args] = process.argv.slice(2);
  try {
    if (mode === "native" && args.length === 3) {
      const [processId, controlPath, powershell] = args;
      const bookPath = process.env.ATHA_SHELF_GATE_EPUB;
      delete process.env.ATHA_SHELF_GATE_EPUB;
      await runNative(cdp, processId, bookPath, controlPath, powershell);
    } else if (mode === "browser" && args.length === 1) {
      const browser = await chromium.connectOverCDP(cdp, { timeout });
      try {
        await runBrowser(browser, args[0]);
        console.log("Chromium production shelf fixture passed (not WebView2).");
      } finally {
        await browser.close();
      }
    } else {
      throw new Error("invalid shelf driver arguments");
    }
  } catch (error) {
    // Playwright diagnostics may contain private EPUB titles, identities, or
    // resource URLs. Expose only the fixed native stage, never the raw cause.
    console.error(mode === "native"
      ? error instanceof ShelfStageError ? error.message : "Native shelf driver could not attach or initialize."
      : error);
    process.exitCode = 1;
  }
}
