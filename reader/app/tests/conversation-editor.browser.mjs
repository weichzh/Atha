import assert from "node:assert/strict";
import path from "node:path";

export async function verifyConversationEditor(browser, previewUrl, screenshotDir) {
  const rootExcerpt = "真正的阅读并非单向接收";
  const openNotes = async (page) => {
    await page.goto(`${previewUrl}?manifest=demo/manifest.json`);
    await page.waitForSelector("html[data-reader-ready]");
    await page.keyboard.press("n");
    await page.getByRole("complementary", { name: "笔记列表" }).waitFor();
  };
  const snapshot = (page, name) => page.screenshot({
    path: path.join(screenshotDir, name),
    animations: "disabled",
  });
  const desktop = await browser.newPage({ viewport: { width: 1440, height: 900 }, reducedMotion: "reduce" });
  try {
    await openNotes(desktop);
    await desktop.locator(".note-index-row", { hasText: rootExcerpt }).click();
    const thread = desktop.getByRole("region", { name: "当前消息线程" });
    const quote = thread.locator(".source-quote").first();
    assert.match(await quote.innerText(), new RegExp(rootExcerpt));
    assert.equal(await thread.locator(".thread-scroll").evaluate(el => el.scrollTop), 0);
    await snapshot(desktop, "conversation-desktop.png");

    const visual = thread.getByRole("textbox", { name: "写下笔记或回复…" });
    const send = thread.getByRole("button", { name: "发送", exact: true });
    const formats = thread.getByRole("button", { name: "排版工具" });
    await visual.fill("桌面第一行");
    await visual.press("End");
    await visual.press("Shift+Enter");
    await desktop.keyboard.insertText("桌面第二行");
    await visual.dispatchEvent("keydown", { key: "Enter", isComposing: true });
    assert.equal(await thread.getByText("桌面第一行", { exact: true }).count(), 0, "composition must not send");
    assert.equal((await visual.innerText()).trim(), "桌面第一行\n桌面第二行");
    await visual.press("Enter");
    const sent = thread.locator(".message-node", { hasText: "桌面第二行" });
    await sent.waitFor();
    assert.ok(await sent.evaluate(el => {
      const scroll = el.closest(".thread-scroll");
      return el.getBoundingClientRect().bottom <= scroll.getBoundingClientRect().bottom + 1;
    }), "sent reply should be brought into view");
    assert.equal(await sent.getByText(/已编辑/).count(), 0);
    await thread.locator('.composer-card[aria-busy="false"]').waitFor();
    assert.equal((await visual.innerText()).trim(), "");

    await formats.click();
    await thread.getByRole("button", { name: "切换 Markdown 源码模式" }).click();
    const markdown = thread.getByRole("textbox", { name: "Markdown 源码" });
    await markdown.fill("## 分段标题");
    await markdown.press("End");
    await markdown.press("Enter");
    await markdown.press("Enter");
    await desktop.keyboard.insertText("独立段落");
    assert.equal(await markdown.inputValue(), "## 分段标题\n\n独立段落");
    await markdown.press("Control+Enter");
    await thread.getByRole("heading", { name: "分段标题" }).waitFor();
    assert.equal(await formats.getAttribute("aria-expanded"), "false");

    await formats.click();
    await thread.getByRole("button", { name: "展开编辑器" }).click();
    await visual.fill("展开后留在编辑器");
    await visual.press("Enter");
    assert.match(await visual.innerText(), /展开后留在编辑器/);
    assert.equal(await thread.locator(".message-node", { hasText: "展开后留在编辑器" }).count(), 0);
    await thread.getByRole("button", { name: "收起编辑器" }).click();
    await visual.fill("");

    // Clip the selected root from both read projections after a simulated committed reply.
    await desktop.evaluate((excerpt) => {
      const key = "atha.demo.messages.v1";
      const original = localStorage.getItem(key);
      const store = JSON.parse(original);
      store.conversations = store.conversations.filter(c =>
        !c.messages[0].source.selectedText.startsWith(excerpt));
      globalThis.__TAURI_INTERNALS__ = {
        invoke(command) {
          if (command === "message_reply") {
            localStorage.setItem(key, JSON.stringify(store));
            return Promise.resolve({ messageId: "sent", revisionId: "sent" });
          }
          if (command === "message_conversations") {
            localStorage.setItem(key, original);
            delete globalThis.__TAURI_INTERNALS__;
            return Promise.resolve(store.conversations);
          }
          throw new Error(`Unexpected test IPC: ${command}`);
        },
      };
    }, rootExcerpt);
    await visual.fill("成功写入，读回失败");
    await send.click();
    await thread.getByText("这条对话暂时无法读取。").waitFor();
    await thread.getByRole("tab", { name: "本书" }).click();
    assert.equal(await thread.locator(".source-quote, .composer-pill").count(), 0);
    await thread.getByRole("button", { name: "重新读取" }).click();
    await thread.getByRole("tab", { name: "当前", selected: true }).waitFor();
    assert.match(await quote.innerText(), new RegExp(rootExcerpt));
    await visual.waitFor();

    // Replying in the book projection must target the clicked message's conversation.
    await thread.getByRole("tab", { name: "本书" }).click();
    const other = thread.locator(".conversation-block").filter({
      hasNot: desktop.locator(".source-quote", { hasText: rootExcerpt }),
    }).filter({ has: desktop.locator(".msg-more-btn") }).first();
    const otherSource = await other.locator(".source-quote").innerText();
    await other.getByRole("button", { name: "消息操作" }).last().click();
    await desktop.getByRole("menuitem", { name: "回复这条消息" }).click();
    assert.equal(await thread.getByRole("tab", { name: "当前" }).getAttribute("aria-selected"), "true");
    assert.equal(await quote.innerText(), otherSource);
    await visual.fill("回复只进入所选对话");
    await visual.press("Enter");
    const reply = thread.locator(".message-node", { hasText: "回复只进入所选对话" });
    await reply.waitFor();
    await reply.getByRole("button", { name: "消息操作" }).click();
    await desktop.getByRole("menuitem", { name: "编辑正文" }).click();
    assert.equal((await visual.innerText()).trim(), "回复只进入所选对话");
    await visual.fill("同一条回复的新修订");
    await thread.getByRole("button", { name: "保存修订" }).click();
    await thread.locator(".message-node", { hasText: "同一条回复的新修订" }).getByText(/已编辑/).waitFor();
    assert.equal(await reply.count(), 0);
  } finally {
    await desktop.close();
  }

  const narrow = await browser.newPage({
    viewport: { width: 360, height: 760 }, hasTouch: true, isMobile: true, reducedMotion: "reduce",
  });
  try {
    await openNotes(narrow);
    await narrow.evaluate(() => {
      const key = "atha.demo.messages.v1";
      const store = JSON.parse(localStorage.getItem(key));
      const seed = store.conversations[0];
      for (let i = 0; i < 8; i++) {
        const root = structuredClone(seed.messages[0]);
        root.id = `extra-root-${i}`;
        root.updatedAt += (i + 1) * 86400000;
        store.conversations.push({
          ...seed, id: `extra-conversation-${i}`, messages: [root],
        });
      }
      localStorage.setItem(key, JSON.stringify(store));
    });
    await openNotes(narrow);
    const list = narrow.locator(".pane-b");
    const pane = narrow.locator(".pane-c");
    const before = await list.boundingBox();
    const note = list.locator(".note-index-row", { hasText: rootExcerpt }).last();
    const listScroll = list.locator(".pane-b-list");
    await note.scrollIntoViewIfNeeded();
    const savedTop = await listScroll.evaluate(el => el.scrollTop);
    assert.ok(savedTop > 0, "fixture must scroll the note index");
    await note.tap();
    await list.waitFor({ state: "hidden" });
    assert.deepEqual(await pane.boundingBox(), before, "list and thread share one bottom sheet");
    assert.equal(before.y + before.height, 760);
    await pane.getByRole("button", { name: "返回笔记列表" }).tap();
    await pane.waitFor({ state: "hidden" });
    assert.deepEqual(await list.boundingBox(), before);
    await narrow.waitForFunction(top =>
      Math.abs(document.querySelector(".pane-b-list")?.scrollTop - top) <= 1, savedTop);
    await note.tap();
    await list.waitFor({ state: "hidden" });

    const visual = pane.getByRole("textbox", { name: "写下笔记或回复…" });
    await snapshot(narrow, "conversation-narrow.png");
    await visual.fill("触屏第一段");
    await visual.press("End");
    await visual.press("Enter");
    await narrow.keyboard.insertText("触屏第二段");
    assert.equal(await pane.locator(".message-node", { hasText: "触屏第一段" }).count(), 0);
    assert.deepEqual(await visual.locator("p").allTextContents(), ["触屏第一段", "触屏第二段"]);
    const send = pane.getByRole("button", { name: "发送", exact: true });
    const formats = pane.getByRole("button", { name: "排版工具" });
    for (const button of [send, formats, pane.getByRole("button", { name: "返回笔记列表" })]) {
      const box = await button.boundingBox();
      assert.ok(box.width >= 44 && box.height >= 44);
    }

    await formats.tap();
    assert.ok(await pane.evaluate(el => el.scrollWidth <= el.clientWidth + 1),
      "formatting controls must wrap without horizontal scrolling");
    await snapshot(narrow, "conversation-narrow-format.png");
    await send.tap();
    await pane.locator(".message-node", { hasText: "触屏第二段" }).waitFor();
    await pane.getByRole("button", { name: "返回笔记列表" }).tap();
    await pane.waitFor({ state: "hidden" });
    assert.deepEqual(await list.boundingBox(), before);
  } finally {
    await narrow.close();
  }
}
