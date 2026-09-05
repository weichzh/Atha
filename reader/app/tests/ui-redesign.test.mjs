// Run against Vite + production preview: mise exec node@24.17.0 -- node reader/app/tests/ui-redesign.test.mjs http://127.0.0.1:1422 http://127.0.0.1:1423
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";

const origin = new URL(process.argv[2] || "http://127.0.0.1:1422");
assert(["localhost", "127.0.0.1"].includes(origin.hostname), "Local fixture only");
const output = resolve("artifacts/local/ui-redesign-20260905");
mkdirSync(output, { recursive: true });
const session = `atha-ui-test-${process.pid}`;
function browser(...args) {
  return execFileSync("agent-browser", ["--session", session, ...args], { encoding: "utf8", timeout: 30000 });
}
function inspect(script) {
  const response = JSON.parse(browser("--json", "eval", script));
  assert(response.success, JSON.stringify(response));
  return response.data.result;
}
function click(selector) { browser("click", selector); }
function settleReader() {
  browser("wait", "--fn", "document.getAnimations().every(animation => animation.playState !== 'running' || animation.effect.getTiming().iterations === Infinity)");
  browser("wait", "--fn", "document.documentElement.dataset.viewportStable === `${document.querySelector('.reader').clientWidth}x${document.querySelector('.reader').clientHeight}`");
}

try {
  browser("open", new URL("/tests/ui-redesign.html", origin).href);
  browser("set", "media", "light");
  browser("wait", ".library-book");
  const readerPreferences = JSON.stringify({
    schema: 1,
    preferences: { theme: "paper", brightness: 90, fontSize: 22, fontFamily: "book", density: "standard" },
  });
  inspect(`localStorage.setItem('atha.reader.application.v1', ${JSON.stringify(readerPreferences)})`);
  for (const width of [360, 1000, 1440]) {
    browser("set", "viewport", String(width), "900");
    assert.equal(inspect("document.querySelectorAll('.library-book').length"), 9);
    assert.equal(inspect("document.documentElement.scrollWidth <= innerWidth"), true);
    assert.equal(inspect(`Array.from(document.querySelectorAll('.library-sections button, .library-import, .library-primary-actions button')).every(el => {
      const r = el.getBoundingClientRect(); return r.width >= 44 && r.height >= 44 && r.left >= 0 && r.right <= innerWidth && r.bottom <= innerHeight;
    })`), true, `Primary targets at ${width}`);
    browser("fill", "input[type=search]", "海岸");
    assert.equal(inspect("document.querySelectorAll('.library-book').length"), 1);
    browser("fill", "input[type=search]", "");
    click('[aria-label="列表视图"]');
    assert.equal(inspect("document.querySelector('.library-shell').classList.contains('library-list-view')"), true);
    click('[aria-label="网格视图"]');
    click('[aria-label="选择书籍"]');
    click(".library-book-open");
    click(".library-delete-action");
    browser("dialog", "dismiss");
    assert.equal(inspect("window.testCommands.includes('delete_library_book_data')"), false);
    click(".library-selection-header > button:last-child");
    browser("screenshot", `${output}/library-${width}.png`);
    click(".library-sections button:last-child");
    click('[data-app-swatch="light"]');
    assert.equal(inspect("document.documentElement.dataset.appTone"), "light");
    assert.deepEqual(inspect("JSON.parse(localStorage.getItem('atha.app.appearance.v1'))"), { schema: 1, theme: "light" });
    assert.equal(inspect("localStorage.getItem('atha.reader.application.v1')"), readerPreferences);
    browser("reload");
    browser("wait", ".library-book");
    assert.equal(inspect("document.documentElement.dataset.appTone"), "light");
    assert.equal(inspect("localStorage.getItem('atha.reader.application.v1')"), readerPreferences);
    click(".library-sections button:last-child");
    browser("screenshot", `${output}/settings-${width}.png`);
    browser("scroll", "down", "2500");
    assert.equal(inspect("document.documentElement.scrollWidth <= innerWidth"), true);
    browser("scroll", "up", "2500");
    click('[data-app-swatch="dark"]');
    assert.equal(inspect("document.documentElement.dataset.appTone"), "dark");
    assert.deepEqual(inspect("JSON.parse(localStorage.getItem('atha.app.appearance.v1'))"), { schema: 1, theme: "dark" });
    browser("screenshot", `${output}/settings-dark-${width}.png`);
    click('[data-app-swatch="system"]');
    assert.deepEqual(inspect("JSON.parse(localStorage.getItem('atha.app.appearance.v1'))"), { schema: 1, theme: "system" });
    assert.equal(inspect("localStorage.getItem('atha.reader.application.v1')"), readerPreferences);
    click(".library-sections button:nth-child(2)");
    browser("fill", ".memory-search-form input", "山");
    click(".memory-search-form button");
    browser("wait", ".memory-result");
    assert.equal(inspect("document.querySelectorAll('.memory-result').length"), 2);
    assert.equal(inspect("document.documentElement.scrollWidth <= innerWidth"), true);
    browser("screenshot", `${output}/memory-${width}.png`);
    click(".library-sections button:first-child");
    browser("focus", ".library-import");
    browser("press", "Tab");
    assert.equal(inspect("getComputedStyle(document.activeElement).outlineStyle !== 'none'"), true);
  }
  assert.deepEqual(JSON.parse(browser("--json", "errors")).data.errors, []);
  console.log("PASS: 360/1000/1440 layout, targets, search, views, selection, delete cancellation, theme reload/restore, reading memory, keyboard focus, no page errors. Mock IPC only; not native acceptance.");
  if (process.argv[3]) {
    // Production preview serves raw styles, unlike Vite's development CSS modules.
    const readerOrigin = new URL(process.argv[3]);
    assert(["localhost", "127.0.0.1"].includes(readerOrigin.hostname));
    const url = new URL("/index.html", readerOrigin);
    url.search = new URLSearchParams({ book: "/tests/reader-design.xhtml", version: "a".repeat(64), state: "reader-design" }).toString();
    browser("open", url.href);
    browser("wait", "--fn", "document.documentElement.hasAttribute('data-reader-ready')");
    const saved = inspect("localStorage.getItem('atha.reader.application.v1')");
    for (const width of [360, 1000, 1600]) {
      browser("set", "viewport", String(width), "1000");
      browser("wait", "--fn", "document.documentElement.dataset.viewportStable === `${document.querySelector('.reader').clientWidth}x${document.querySelector('.reader').clientHeight}`");
      click(".reader");
      settleReader();
      assert.equal(inspect("document.querySelector('#reader-toolbar-progress').textContent === document.querySelector('#progress-position').textContent"), true);
      assert.equal(inspect("document.querySelector('#reader-toolbar-chapter').textContent"), "第 1 节");
      browser("screenshot", `${output}/reader-main-${width}.png`);
      click(".preferences > summary");
      click('[data-preference-for="theme"][data-preference-value="light"]');
      browser("wait", "--fn", "document.querySelector('#theme').value === 'light'");
      settleReader();
      assert.equal(inspect("[...document.querySelectorAll('.reader-quick-settings input, .reader-quick-settings button')].every(el => { const r=el.getBoundingClientRect(); return r.width > 0 && r.top >= 0 && r.bottom <= innerHeight && r.left >= 0 && r.right <= innerWidth; })"), true);
      browser("screenshot", `${output}/reader-quick-${width}.png`);
      click(".settings-menu .panel-close");
      for (const tool of ["directory", "search", "progress"]) {
        if (!inspect("document.documentElement.hasAttribute('data-reader-tools')")) click(".reader");
        click(`.${tool} > summary`);
        settleReader();
        assert.equal(inspect(`document.querySelector('.${tool}-panel').scrollWidth <= document.querySelector('.${tool}-panel').clientWidth`), true, `${tool} fits ${width}`);
        if (tool === "search") {
          assert.equal(inspect("document.querySelector('#search-query').getBoundingClientRect().height >= 44"), true);
          browser("fill", "#search-query", "海岸");
          click("#search-form button[type=submit]");
          browser("wait", "--fn", "document.querySelector('#search-results').options.length > 0");
        }
        browser("screenshot", `${output}/reader-${tool}-${width}.png`);
        if (tool !== "progress" && inspect("document.documentElement.hasAttribute('data-desktop-workspace')")) {
          click(`.${tool} > summary`);
        } else click(`.${tool}-panel [data-close-reader-tools]`);
        settleReader();
      }
      // Panel closure returns focus reading on desktop; the next pass starts there too.
      inspect("document.documentElement.removeAttribute('data-reader-tools')");
    }
    inspect(saved === null ? "localStorage.removeItem('atha.reader.application.v1')" : `localStorage.setItem('atha.reader.application.v1', ${JSON.stringify(saved)})`);
    assert.deepEqual(JSON.parse(browser("--json", "errors")).data.errors, []);
    console.log("PASS: real reader runtime at 360/1000/1600, chapter/progress, quick settings, directory, search results, progress panel, no page errors. Chromium preview only.");
  }
} finally {
  browser("close");
}
