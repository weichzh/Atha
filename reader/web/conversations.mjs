const SNAPSHOT_LINE_HEIGHTS = Object.freeze({ compact: 1.45, standard: 1.6, comfortable: 1.8 });
const SNAPSHOT_PRESENTATION_KEYS = new Set([
  "schema",
  "theme",
  "brightness",
  "fontSize",
  "fontFamily",
  "density",
  "sourceStyles",
  "userStylesEnabled",
  "userStylesheet",
]);

export function parseSnapshotPresentation(value, prefersDark = false) {
  if (typeof value !== "string" || value.length > 65_536) {
    throw new Error("invalid-message-snapshot");
  }
  let parsed;
  try {
    parsed = JSON.parse(value);
  } catch {
    throw new Error("invalid-message-snapshot");
  }
  if (
    parsed?.schema === 1 &&
    parsed.legacy === true &&
    Object.keys(parsed).length === 2
  ) {
    return Object.freeze({
      theme: prefersDark ? "dark" : "light",
      brightness: 100,
      fontSize: 32,
      fontFamily: "book",
      lineHeightPx: 51.2,
    });
  }
  if (
    !parsed ||
    typeof parsed !== "object" ||
    Array.isArray(parsed) ||
    Object.keys(parsed).some((key) => !SNAPSHOT_PRESENTATION_KEYS.has(key)) ||
    parsed.schema !== 1 ||
    !["system", "light", "paper", "dark"].includes(parsed.theme) ||
    !Number.isInteger(parsed.brightness) ||
    parsed.brightness < 70 ||
    parsed.brightness > 120 ||
    ![24, 32, 40].includes(parsed.fontSize) ||
    !["book", "serif", "sans"].includes(parsed.fontFamily) ||
    !Object.hasOwn(SNAPSHOT_LINE_HEIGHTS, parsed.density) ||
    (Object.hasOwn(parsed, "sourceStyles") && typeof parsed.sourceStyles !== "boolean") ||
    (Object.hasOwn(parsed, "userStylesEnabled") &&
      typeof parsed.userStylesEnabled !== "boolean") ||
    (Object.hasOwn(parsed, "userStylesheet") && typeof parsed.userStylesheet !== "string")
  ) {
    throw new Error("invalid-message-snapshot");
  }
  return Object.freeze({
    theme:
      parsed.theme === "system" ? (prefersDark ? "dark" : "light") : parsed.theme,
    brightness: parsed.brightness,
    fontSize: parsed.fontSize,
    fontFamily: parsed.fontFamily,
    lineHeightPx: parsed.fontSize * SNAPSHOT_LINE_HEIGHTS[parsed.density],
  });
}

export function createConversations({
  store,
  annotations,
  controls,
  closeTools,
  editionId,
  readingSurface,
  returnFocus,
}) {
  let conversation = null;
  let parentId = null;
  let editing = null;

  const action = (label, name, message) => {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = label;
    button.dataset.messageAction = name;
    button.dataset.messageId = message.id;
    return button;
  };

  function report(message, error = false) {
    controls.status.textContent = message;
    controls.status.dataset.error = String(error);
  }

  function close() {
    controls.overlay.hidden = true;
    returnFocus.focus({ preventScroll: true });
  }

  function messageLabel(message, index) {
    if (message.deleted) return `第 ${index + 1} 条消息（已删除）`;
    return message.text || message.source?.selectedText || `第 ${index + 1} 条消息`;
  }

  function resetComposer() {
    editing = null;
    controls.text.value = "";
    controls.cancelEdit.hidden = true;
    parentId = conversation?.messages.find((message) => !message.deleted)?.id || null;
    controls.composerContext.textContent = "回复原文";
    renderReferences();
  }

  function renderReferences() {
    if (!conversation) return;
    controls.references.replaceChildren(
      controls.references.querySelector("legend"),
      ...conversation.messages
        .filter((message) => !message.deleted && message.id !== parentId)
        .map((message, index) => {
          const label = document.createElement("label");
          const input = document.createElement("input");
          input.type = "checkbox";
          input.value = message.id;
          label.append(input, document.createTextNode(messageLabel(message, index).slice(0, 80)));
          return label;
        }),
    );
  }

  function renderConversation() {
    const messages = conversation?.messages || [];
    controls.source.textContent =
      messages.find((message) => message.source)?.source?.selectedText || "原文已删除";
    controls.list.replaceChildren(
      ...messages.map((message, index) => {
        const card = document.createElement("article");
        const body = document.createElement("p");
        const actions = document.createElement("div");
        card.className = "message-card";
        card.dataset.deleted = String(message.deleted);
        card.dataset.messageId = message.id;
        if (message.source && !message.deleted) {
          const quote = document.createElement("blockquote");
          quote.textContent = message.source.selectedText;
          card.append(quote);
        }
        body.textContent = message.deleted ? "这条消息已删除" : message.text || "标注原文";
        actions.className = "message-card-actions";
        if (!message.deleted) {
          actions.append(
            action("回复", "reply", message),
            action(message.text ? "编辑" : "添加笔记", "edit", message),
            action("删除", "delete", message),
          );
          if (message.source) {
            actions.append(
              action("历史引用", "snapshot", message),
              action("跳回原文", "jump", message),
            );
          }
        }
        actions.append(action("修订", "history", message), action("关联", "relations", message));
        card.append(body, actions);
        return card;
      }),
    );
    renderReferences();
  }

  async function reload() {
    conversation = await store.conversation(conversation.id);
    renderConversation();
  }

  async function open(conversationId, messageId = null) {
    try {
      conversation = await store.conversation(conversationId);
      parentId =
        conversation.messages.find((message) => message.id === messageId && !message.deleted)?.id ||
        conversation.messages.find((message) => !message.deleted)?.id ||
        null;
      editing = null;
      controls.text.value = "";
      controls.cancelEdit.hidden = true;
      const parent = conversation.messages.find((message) => message.id === parentId);
      controls.composerContext.textContent =
        !parent || parent === conversation.messages[0]
          ? "回复原文"
          : `回复：${messageLabel(parent, conversation.messages.indexOf(parent)).slice(0, 60)}`;
      controls.overlay.hidden = false;
      controls.overlay.dataset.collapsed = "false";
      controls.collapse.setAttribute("aria-expanded", "true");
      controls.content.hidden = false;
      renderConversation();
      closeTools();
      controls.text.focus();
    } catch {
      report("无法打开阅读对话", true);
    }
  }

  async function showHistory(message) {
    const revisions = await store.revisions(message.id);
    controls.historyTitle.textContent = "修订历史";
    controls.historyContent.replaceChildren(
      ...revisions.map((revision) => {
        const article = document.createElement("article");
        const time = document.createElement("time");
        const body = document.createElement("p");
        time.dateTime = new Date(revision.createdAt).toISOString();
        time.textContent = new Date(revision.createdAt).toLocaleString();
        body.textContent = revision.text || "纯标注";
        article.append(time, body);
        return article;
      }),
    );
    controls.historyDialog.showModal();
  }

  async function showRelations(message) {
    const relations = await store.relationships(message.id);
    controls.historyTitle.textContent = "引用关系";
    const section = (title, ids) => {
      const container = document.createElement("section");
      const heading = document.createElement("h3");
      const list = document.createElement("ul");
      heading.textContent = `${title}（${ids.length}）`;
      for (const id of ids) {
        const item = document.createElement("li");
        const related = conversation.messages.find((candidate) => candidate.id === id);
        if (related) {
          const button = document.createElement("button");
          button.type = "button";
          button.textContent = messageLabel(related, conversation.messages.indexOf(related));
          button.addEventListener("click", () => {
            controls.historyDialog.close();
            controls.list
              .querySelector(`[data-message-id="${CSS.escape(id)}"]`)
              ?.scrollIntoView({ block: "center" });
          });
          item.append(button);
        } else item.textContent = "其他对话中的消息";
        list.append(item);
      }
      if (!ids.length) {
        const empty = document.createElement("li");
        empty.textContent = "无";
        list.append(empty);
      }
      container.append(heading, list);
      return container;
    };
    controls.historyContent.replaceChildren(
      section("引用了", relations.references),
      section("被引用", relations.referencedBy),
    );
    controls.historyDialog.showModal();
  }

  function safeSnapshotFragment(html, resourcePaths) {
    const parsed = new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
    const referenced = new Set();
    if (
      parsed.querySelector(
        "script,iframe,object,embed,form,input,button,select,textarea,video,audio,source,track,style,link,meta,base",
      )
    ) {
      throw new Error("invalid-message-snapshot");
    }
    for (const element of parsed.body.querySelectorAll("*")) {
      const resourceAttribute =
        element.localName === "img" ? "src" : element.localName === "image" ? "href" : null;
      const resourcePath = resourceAttribute && element.getAttribute(resourceAttribute);
      for (const attribute of [...element.attributes]) {
        const name = attribute.name.toLowerCase();
        if (
          name.startsWith("on") ||
          ["srcset", "style"].includes(name) ||
          (name === "href" && resourceAttribute !== "href") ||
          (name === "src" && resourceAttribute !== "src")
        ) {
          throw new Error("invalid-message-snapshot");
        }
      }
      if (resourceAttribute && (!resourcePath || !resourcePaths.has(resourcePath))) {
        throw new Error("invalid-message-snapshot");
      }
      if (resourcePath) referenced.add(resourcePath);
    }
    if (referenced.size !== resourcePaths.size) throw new Error("invalid-message-snapshot");
    return parsed.body;
  }

  function resourceDataUrl(data) {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.addEventListener("load", () => resolve(reader.result), { once: true });
      reader.addEventListener("error", reject, { once: true });
      reader.readAsDataURL(new Blob([new Uint8Array(data.bytes)], { type: data.mediaType }));
    });
  }

  async function renderSnapshot(capture) {
    const presentation = parseSnapshotPresentation(
      capture.snapshot.presentationJson,
      globalThis.matchMedia?.("(prefers-color-scheme: dark)").matches,
    );
    const resources = new Map();
    for (const resource of capture.snapshot.resources) {
      const data = await store.snapshotResource(capture.source.id, resource.path);
      resources.set(resource.path, await resourceDataUrl(data));
    }
    const parsed = safeSnapshotFragment(capture.snapshot.fragmentHtml, new Set(resources.keys()));
    for (const image of parsed.querySelectorAll("img[src], image[href]")) {
      const attribute = image.localName === "img" ? "src" : "href";
      image.setAttribute(attribute, resources.get(image.getAttribute(attribute)));
    }
    const host = document.createElement("div");
    host.dataset.theme = presentation.theme;
    host.style.filter = `brightness(${presentation.brightness / 100})`;
    const shadow = host.attachShadow({ mode: "open" });
    const style = document.createElement("style");
    const css = `${capture.snapshot.bookCss}\n${capture.snapshot.readerCss}\n${capture.snapshot.userCss}`;
    if (/@import|url\s*\(|image-set\s*\(|:host|::part|::slotted/i.test(css)) {
      throw new Error("invalid-message-snapshot");
    }
    style.textContent = `${capture.snapshot.bookCss}\n${capture.snapshot.readerCss.replace(/:root\b/g, ":host")}\n${capture.snapshot.userCss}`;
    const book = document.createElement("article");
    book.className = "book";
    book.dataset.theme = presentation.theme;
    if (presentation.fontFamily !== "book") book.dataset.fontFamily = presentation.fontFamily;
    book.style.fontSize = `${presentation.fontSize}px`;
    book.style.setProperty("--reader-line-height", `${presentation.lineHeightPx}px`);
    book.append(...document.importNode(parsed, true).childNodes);
    shadow.append(style, book);
    controls.snapshotContent.replaceChildren(host);
  }

  async function showSnapshots(message) {
    const captures = await store.sourceCaptures(message.id);
    const select = async (capture) => {
      await renderSnapshot(capture);
      for (const button of controls.snapshotVersions.querySelectorAll("button")) {
        button.setAttribute("aria-current", String(button.dataset.sourceId === capture.source.id));
      }
    };
    controls.snapshotVersions.replaceChildren(
      ...captures.map((capture, index) => {
        const button = document.createElement("button");
        button.type = "button";
        button.dataset.sourceId = capture.source.id;
        button.textContent = capture.current ? "当前引用" : `历史引用 ${index + 1}`;
        button.addEventListener("click", () => select(capture).catch(() => report("历史引用已损坏", true)));
        return button;
      }),
    );
    controls.snapshotDialog.showModal();
    await select(captures.find((capture) => capture.current) || captures.at(-1));
  }

  async function handleAction(button) {
    const message = conversation.messages.find((candidate) => candidate.id === button.dataset.messageId);
    if (!message) return;
    switch (button.dataset.messageAction) {
      case "reply":
        editing = null;
        parentId = message.id;
        controls.text.value = "";
        controls.cancelEdit.hidden = true;
        controls.composerContext.textContent = `回复：${messageLabel(message, 0).slice(0, 60)}`;
        renderReferences();
        controls.text.focus();
        break;
      case "edit":
        editing = message;
        controls.text.value = message.text;
        controls.cancelEdit.hidden = false;
        controls.composerContext.textContent = message.text ? "编辑消息" : "为标注添加笔记";
        controls.text.focus();
        break;
      case "delete":
        await store.deleteMessage(message.id, message.revisionId);
        await reload();
        await annotations.redraw();
        report("已删除");
        break;
      case "history":
        await showHistory(message);
        break;
      case "relations":
        await showRelations(message);
        break;
      case "snapshot":
        await showSnapshots(message);
        break;
      case "jump":
        if ((await annotations.go(message.id)).ok) close();
        break;
    }
  }

  function bind() {
    controls.close.addEventListener("click", close);
    controls.overlay.addEventListener("keydown", (event) => {
      if (event.key === "Escape") close();
    });
    readingSurface.addEventListener("pointerdown", () => (controls.overlay.hidden = true));
    controls.collapse.addEventListener("click", () => {
      const collapsed = controls.overlay.dataset.collapsed !== "true";
      controls.overlay.dataset.collapsed = String(collapsed);
      controls.content.hidden = collapsed;
      controls.collapse.textContent = collapsed ? "展开" : "收起";
      controls.collapse.setAttribute("aria-expanded", String(!collapsed));
    });
    controls.exportButton.addEventListener("click", async () => {
      try {
        if (await store.export(conversation.editionId, conversation.id)) report("已导出阅读对话");
      } catch {
        report("导出失败，请重试", true);
      }
    });
    controls.exportAllButton.addEventListener("click", async () => {
      try {
        if (!(await store.export(editionId, null))) return;
        controls.exportAllButton.textContent = "已导出";
        setTimeout(() => (controls.exportAllButton.textContent = "导出"), 1600);
      } catch {
        controls.exportAllButton.textContent = "导出失败";
        setTimeout(() => (controls.exportAllButton.textContent = "导出"), 1600);
      }
    });
    controls.list.addEventListener("click", (event) => {
      const button = event.target.closest("button[data-message-action]");
      if (button) handleAction(button).catch(() => report("操作失败，请重试", true));
    });
    controls.cancelEdit.addEventListener("click", resetComposer);
    controls.form.addEventListener("submit", async (event) => {
      event.preventDefault();
      const text = controls.text.value.trim();
      if (!text) return;
      try {
        if (editing) await store.revise(editing.id, editing.revisionId, text);
        else {
          if (!parentId) throw new Error("missing-message-parent");
          const referenceIds = [...controls.references.querySelectorAll("input:checked")].map(
            (input) => input.value,
          );
          await store.reply({
            conversationId: conversation.id,
            replyToMessageId: parentId,
            text,
            referenceIds,
          });
        }
        await reload();
        await annotations.redraw();
        resetComposer();
        report("已保存");
      } catch {
        report("保存失败，请重试", true);
      }
    });
  }

  return Object.freeze({ bind, open });
}
