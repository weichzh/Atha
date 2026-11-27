import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(__dirname, "..");
const repoRoot = path.resolve(appRoot, "../..");
const screenshotDir = path.join(repoRoot, "artifacts/local/audits/borderless-reader-20261126");
const PREVIEW_URL = "http://127.0.0.1:4174/";

async function waitForServer(url, timeoutMs = 15000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(url);
      if (res.ok) return;
    } catch {
      // retry
    }
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(`Preview server did not start within ${timeoutMs}ms`);
}

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

  // 2. Spawn vite preview
  const preview = spawn("pnpm", ["preview"], {
    cwd: appRoot,
    stdio: "pipe",
    env: { ...process.env },
  });

  let browser;
  try {
    await waitForServer(PREVIEW_URL);
    const executablePath = process.env.CHROMIUM_PATH;
    browser = await chromium.launch({
      executablePath,
      headless: true,
      args: ["--no-sandbox", "--disable-gpu"],
    });

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
      pageErrors.push(String(err));
    });
    page.on("response", (res) => {
      if (res.status() >= 400) failedResponses.push(`${res.status()} ${res.url()}`);
    });

    // 3. Load Library (Light 1440x900)
    await page.goto(PREVIEW_URL, { waitUntil: "networkidle" });
    await page.waitForSelector(".book-card");
    await page.waitForTimeout(450);

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
    await page.getByRole("button", { name: "下一页" }).click();
    await page.waitForTimeout(250);
    await page.getByRole("button", { name: "上一页" }).click();
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
    await page.mouse.move(460, 292);
    await page.mouse.down();
    await page.mouse.move(660, 292, { steps: 12 });
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
    await assertBorderless(page, [".pane-b", ".pane-c", ".note-index-row", ".msg-body-card", ".composer-card"]);

    await page.screenshot({
      path: path.join(screenshotDir, "05-reader-workspace-abc-1440.png"),
      animations: "disabled",
    });

    // Tiptap Composer + Safe Link validation (reject javascript:alert(1), accept https://example.com)
    const prose = page.locator(".ProseMirror");
    await prose.click();
    await page.keyboard.type("测试安全链接：");
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
    await page.waitForFunction(() => document.querySelector(".send-btn")?.textContent.includes("发送中") === false);
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
    await page.screenshot({
      path: path.join(screenshotDir, "06-memory-light-1440.png"),
      animations: "disabled",
    });

    // Test Reading Memory search
    await page.getByRole("searchbox", { name: "搜索笔记与回复" }).fill("快照");
    await page.waitForTimeout(350);
    assert.ok((await page.locator(".hit-row").count()) >= 1, "Reading Memory search should find seeded hits");

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
    await page.goto(`${PREVIEW_URL}?${query}`);
    await page.waitForSelector("html[data-reader-ready]");
    await page.waitForFunction((id) => {
      const raw = JSON.parse(localStorage.getItem(`atha.reader.progress.${id.slice(0, 16)}.v1`));
      return raw && JSON.parse(raw.locator).start.section === "ch2";
    }, memoryTarget.version);

    assert.equal(failedResponses.length, 0, `HTTP >=400 responses: ${failedResponses.join(" | ")}`);
    assert.equal(consoleErrors.length, 0, `Console errors found: ${consoleErrors.join(" | ")}`);
    assert.equal(pageErrors.length, 0, `Page errors found: ${pageErrors.join(" | ")}`);

    console.log("ALL BROWSER CHECKS PASSED! 12 screenshots saved to:", screenshotDir);
  } finally {
    if (browser) await browser.close();
    preview.kill("SIGTERM");
  }
}

run().catch((err) => {
  console.error(err);
  process.exit(1);
});
