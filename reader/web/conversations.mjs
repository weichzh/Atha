const SNAPSHOT_LINE_HEIGHTS = Object.freeze({ compact: 1.55, standard: 1.8, comfortable: 2.05 });
const MESSAGE_TIME_FORMAT = new Intl.DateTimeFormat("zh-CN", {
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
});
const MESSAGE_DATE_TIME_FORMAT = new Intl.DateTimeFormat("zh-CN", {
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
});
const SNAPSHOT_PRESENTATION_KEYS = new Set([
  "schema",
  "theme",
  "brightness",
  "fontSize",
  "fontFamily",
  "density",
]);

export function isSnapshotCssSafe(value) {
  return (
    typeof value === "string" &&
    !/@import|(?:url|src|image|image-set)\s*\(|:host|::part|::slotted/i.test(value) &&
    !value.includes("\\")
  );
}

export function parseSnapshotPresentation(value, prefersDark = false) {
  if (typeof value !== "string" || value.length > 4_096) {
    throw new Error("invalid-message-snapshot");
  }
  let parsed;
  try {
    parsed = JSON.parse(value);
  } catch {
    throw new Error("invalid-message-snapshot");
  }
  if (parsed?.schema === 1 && parsed.legacy === true && Object.keys(parsed).length === 2) {
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
    !["light", "paper", "dark"].includes(parsed.theme) ||
    !Number.isInteger(parsed.brightness) ||
    parsed.brightness < 70 ||
    parsed.brightness > 120 ||
    !Number.isInteger(parsed.fontSize) ||
    parsed.fontSize < 16 ||
    parsed.fontSize > 40 ||
    !["book", "serif", "sans"].includes(parsed.fontFamily) ||
    !Object.hasOwn(SNAPSHOT_LINE_HEIGHTS, parsed.density)
  ) {
    throw new Error("invalid-message-snapshot");
  }
  return Object.freeze({
    theme: parsed.theme,
    brightness: parsed.brightness,
    fontSize: parsed.fontSize,
    fontFamily: parsed.fontFamily,
    lineHeightPx: parsed.fontSize * SNAPSHOT_LINE_HEIGHTS[parsed.density],
  });
}

export async function renderSnapshotElement(capture, store) {
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
  if (!isSnapshotCssSafe(css)) throw new Error("invalid-message-snapshot");
  style.textContent = `${capture.snapshot.bookCss}\n${capture.snapshot.readerCss.replace(/:root\b/g, ":host")}\n${capture.snapshot.userCss}`;
  const book = document.createElement("article");
  book.className = "book";
  book.dataset.theme = presentation.theme;
  if (presentation.fontFamily !== "book") book.dataset.fontFamily = presentation.fontFamily;
  book.style.fontSize = `${presentation.fontSize}px`;
  book.style.setProperty("--reader-line-height", `${presentation.lineHeightPx}px`);
  book.append(...document.importNode(parsed, true).childNodes);
  shadow.append(style, book);
  return host;
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
export function messagePreview(message) {
  if (message.deleted) return "这条消息已删除";
  return message.text || message.source?.selectedText || "仅标注原文";
}

export function linkedMessagePreviews(message, messages) {
  const byId = new Map(messages.map((candidate) => [candidate.id, candidate]));
  const seen = new Set();
  const linked = [];
  const add = (id, fallback = null) => {
    if (!id || seen.has(id)) return;
    seen.add(id);
    const local = byId.get(id);
    linked.push({
      id,
      text: local ? messagePreview(local) : fallback?.deleted ? "这条消息已删除" : fallback?.text || "引用消息",
      local: Boolean(local),
    });
  };
  add(message.replyToMessageId);
  for (const preview of message.referencePreviews || []) add(preview.id, preview);
  return linked;
}

export function conversationFeed(conversations, order, compareSources = () => 0) {
  const threads = conversations
    .map((conversation) => ({
      conversation,
      root: conversation.messages.find((message) => message.source && !message.deleted),
    }))
    .filter(({ root }) => root);
  if (order === "time") {
    return threads
      .flatMap(({ conversation, root }) =>
        conversation.messages.map((message, index) => ({
          conversation,
          root,
          message,
          index,
          showSource: true,
        })),
      )
      .sort(
        (left, right) =>
          left.message.createdAt - right.message.createdAt ||
          left.message.id.localeCompare(right.message.id),
      );
  }
  if (order !== "book") throw new Error("invalid-message-order");
  return threads
    .sort(
      (left, right) =>
        compareSources(left.root.source, right.root.source) ||
        left.conversation.id.localeCompare(right.conversation.id),
    )
    .flatMap(({ conversation, root }) =>
      conversation.messages.map((message, index) => ({
        conversation,
        root,
        message,
        index,
        showSource: index === 0,
      })),
    );
}

export function formatMessageTime(value) {
  if (!Number.isFinite(value)) return "";
  return MESSAGE_TIME_FORMAT.format(new Date(value));
}

export function formatMessageFeedTime(value) {
  if (!Number.isFinite(value)) return "";
  return MESSAGE_DATE_TIME_FORMAT.format(new Date(value));
}

export function validConversationTarget(conversation, editionId, expectedRootId = null) {
  const sourceRoot = conversation?.messages?.find(
    (message) => message.source && !message.deleted,
  );
  return Boolean(
    conversation?.editionId === editionId &&
      sourceRoot &&
      (!expectedRootId || sourceRoot.id === expectedRootId),
  );
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
  let reportTimer = null;
  let sheetDrag = null;
  let feedConversations = [];
  let scope = "mark";
  let order = "time";
  let anchorSection = null;
  let scopeGeneration = 0;

  const composer = () => window.athaMessageComposer;

  const action = (label, name, message) => {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = label;
    button.dataset.messageAction = name;
    button.dataset.messageId = message.id;
    return button;
  };

  function report(message, error = false) {
    clearTimeout(reportTimer);
    controls.status.textContent = message;
    controls.status.dataset.error = String(error);
    if (!error && message) {
      reportTimer = setTimeout(() => {
        if (controls.status.textContent === message) controls.status.textContent = "";
      }, 1800);
    }
  }

  function close() {
    scopeGeneration += 1;
    composer()?.collapse();
    controls.overlay.hidden = true;
    returnFocus.focus({ preventScroll: true });
  }

  function setFullscreen(fullscreen) {
    controls.overlay.dataset.fullscreen = String(fullscreen);
    controls.fullscreen.setAttribute("aria-pressed", String(fullscreen));
    controls.fullscreen.setAttribute("aria-label", fullscreen ? "退出全屏对话" : "全屏对话");
    controls.fullscreen.title = fullscreen ? "退出全屏对话" : "全屏对话";
  }

  function messageLabel(message, index) {
    if (message.deleted) return `第 ${index + 1} 条消息（已删除）`;
    return messagePreview(message) || `第 ${index + 1} 条消息`;
  }

  function setComposerContext(message = null, mode = "reply") {
    const root = conversation?.messages.find((candidate) => !candidate.deleted);
    const syncSelectedMessage = () => {
      for (const card of controls.list.querySelectorAll(".message-card")) {
        card.dataset.selected = String(message?.id === card.dataset.messageId && message.id !== root?.id);
      }
    };
    if (mode === "edit") {
      controls.composerContext.hidden = false;
      controls.composerContext.dataset.mode = "edit";
      controls.composerContextText.textContent = message.text ? "编辑消息" : "为标注添加笔记";
      syncSelectedMessage();
      return;
    }
    controls.composerContext.dataset.mode = "reply";
    controls.composerContext.hidden = !message || message.id === root?.id;
    controls.composerContextText.textContent = message ? messagePreview(message) : "";
    syncSelectedMessage();
  }

  function resetComposer() {
    editing = null;
    composer()?.clear();
    parentId = conversation?.messages.find((message) => !message.deleted)?.id || null;
    setComposerContext(conversation?.messages.find((message) => message.id === parentId));
  }

  function openConversationButton(label, conversationId, messageId, className = "") {
    const button = document.createElement("button");
    button.type = "button";
    button.className = className;
    button.textContent = label;
    button.dataset.conversationOpen = conversationId;
    button.dataset.messageId = messageId;
    return button;
  }

  function renderMessageCard(entry) {
    const { conversation: entryConversation, root, message, index, showSource } = entry;
    const messages = entryConversation.messages;
    const aggregate = scope !== "mark";
    const card = document.createElement("article");
    const body = document.createElement("div");
    const footer = document.createElement("footer");
    const time = document.createElement("time");
    const actions = document.createElement("div");
    card.className = "message-card";
    card.dataset.deleted = String(message.deleted);
    card.dataset.selected = String(!aggregate && message.id === parentId && index > 0);
    card.dataset.messageId = message.id;
    if (showSource) {
      const source = openConversationButton(
        root.source.selectedText || "原文已删除",
        entryConversation.id,
        message.id,
        "message-feed-source",
      );
      source.setAttribute("aria-label", `打开标记：${root.source.selectedText || "原文已删除"}`);
      card.append(source);
    }
    for (const preview of linkedMessagePreviews(message, messages)) {
      const quote = document.createElement(preview.local ? "button" : "div");
      const mark = document.createElement("span");
      const text = document.createElement("span");
      quote.className = "message-reference-preview";
      if (preview.local) {
        quote.type = "button";
        quote.dataset.messageTarget = preview.id;
        quote.title = "跳到被引用消息";
      }
      mark.className = "message-quote-mark";
      mark.setAttribute("aria-hidden", "true");
      mark.textContent = "“";
      text.textContent = preview.text;
      quote.append(mark, text);
      card.append(quote);
    }
    body.className = "message-card-body";
    if (message.deleted) body.textContent = messagePreview(message);
    else composer()?.render(body, message.contentJson, messagePreview(message));
    const createdAt = new Date(message.createdAt);
    time.dateTime = Number.isFinite(createdAt.valueOf()) ? createdAt.toISOString() : "";
    time.textContent = `${aggregate ? formatMessageFeedTime(message.createdAt) : formatMessageTime(message.createdAt)}${message.updatedAt > message.createdAt ? " · 已编辑" : ""}`;
    actions.className = "message-card-actions";
    if (aggregate) {
      actions.append(openConversationButton("打开", entryConversation.id, message.id));
      footer.append(time, actions);
      card.append(body, footer);
      return card;
    }
    if (!message.deleted) actions.append(action("回复", "reply", message));
    const more = document.createElement("button");
    const menu = document.createElement("div");
    more.type = "button";
    more.className = "message-more-button";
    more.textContent = "更多";
    more.dataset.messageMenu = message.id;
    more.setAttribute("aria-expanded", "false");
    menu.id = `message-menu-${message.id}`;
    more.setAttribute("aria-controls", menu.id);
    menu.className = "message-menu message-card-menu";
    menu.setAttribute("popover", "auto");
    menu.addEventListener("toggle", () => {
      more.setAttribute("aria-expanded", String(menu.matches(":popover-open")));
    });
    if (!message.deleted) {
      menu.append(action(message.text ? "编辑" : "添加笔记", "edit", message));
      if (message.source) {
        menu.append(
          action("历史引用", "snapshot", message),
          action("跳回原文", "jump", message),
        );
      }
      const remove = action("删除", "delete", message);
      remove.dataset.tone = "danger";
      menu.append(remove);
    }
    menu.append(action("修订历史", "history", message), action("引用关系", "relations", message));
    actions.append(more);
    footer.append(time, actions);
    card.append(body, footer, menu);
    return card;
  }

  function renderConversation() {
    const entries =
      scope === "mark"
        ? (conversation?.messages || []).map((message, index) => ({
            conversation,
            root: conversation.messages.find((candidate) => candidate.source && !candidate.deleted),
            message,
            index,
            showSource: false,
          }))
        : conversationFeed(feedConversations, order, store.compareSources);
    const markCount = scope === "mark" ? 1 : feedConversations.length;
    controls.sourceLabel.textContent =
      scope === "mark" ? "原文引用" : scope === "chapter" ? "本章记录" : "本书记录";
    controls.source.textContent =
      scope === "mark"
        ? entries[0]?.root?.source?.selectedText || "原文已删除"
        : `${markCount} 条标记 · ${entries.length} 条消息`;
    controls.sourceJump.hidden = scope !== "mark";
    controls.form.hidden = scope !== "mark";
    controls.orderControls.hidden = scope === "mark";
    for (const button of controls.scopeButtons) {
      button.setAttribute("aria-pressed", String(button.dataset.messageScope === scope));
    }
    for (const button of controls.orderButtons) {
      button.setAttribute("aria-pressed", String(button.dataset.messageOrder === order));
    }
    if (!entries.length) {
      const empty = document.createElement("p");
      empty.className = "message-feed-empty";
      empty.textContent = scope === "chapter" ? "本章还没有标记" : "本书还没有标记";
      controls.list.replaceChildren(empty);
      return;
    }
    controls.list.replaceChildren(...entries.map(renderMessageCard));
  }

  async function reload() {
    conversation = await store.conversation(conversation.id);
    if (!conversation.messages.some((message) => message.id === parentId && !message.deleted)) {
      parentId = conversation.messages.find((message) => !message.deleted)?.id || null;
    }
    setComposerContext(conversation.messages.find((message) => message.id === parentId));
    renderConversation();
  }

  async function setScope(nextScope) {
    if (!["mark", "chapter", "book"].includes(nextScope)) return;
    scope = nextScope;
    const generation = ++scopeGeneration;
    if (scope === "mark") {
      renderConversation();
      return;
    }
    controls.form.hidden = true;
    controls.orderControls.hidden = false;
    controls.list.textContent = "正在加载聊天记录…";
    try {
      const section = scope === "chapter" ? anchorSection : null;
      const loaded = await store.conversations(editionId, section);
      if (generation !== scopeGeneration) return;
      feedConversations = loaded;
      renderConversation();
    } catch {
      if (generation === scopeGeneration) {
        controls.list.textContent = "无法加载聊天记录";
        report("无法加载聊天记录", true);
      }
    }
  }

  async function open(
    conversationId,
    messageId = null,
    edit = false,
    expectedRootId = null,
    navigationFailed = false,
  ) {
    try {
      scopeGeneration += 1;
      scope = "mark";
      feedConversations = [];
      await window.athaEnsureMessageComposer?.();
      conversation = await store.conversation(conversationId);
      if (!validConversationTarget(conversation, editionId, expectedRootId)) {
        throw new Error("invalid-message-conversation");
      }
      anchorSection = conversation.messages.find((message) => message.source)?.source?.section || null;
      parentId =
        conversation.messages.find((message) => message.id === messageId && !message.deleted)?.id ||
        conversation.messages.find((message) => !message.deleted)?.id ||
        null;
      const parent = conversation.messages.find((message) => message.id === parentId);
      editing = edit ? parent : null;
      if (editing) {
        composer()?.setValue(editing.contentJson, editing.text);
        setComposerContext(editing, "edit");
      } else {
        composer()?.clear();
        setComposerContext(parent);
      }
      controls.overlay.hidden = false;
      setFullscreen(false);
      renderConversation();
      closeTools();
      if (navigationFailed) report("当前原文位置已失效，可查看历史引用", true);
      requestAnimationFrame(() => {
        if (parentId) {
          controls.list
            .querySelector(`[data-message-id="${CSS.escape(parentId)}"]`)
            ?.scrollIntoView({ block: "start" });
        }
        composer()?.focus();
      });
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
        const body = document.createElement("div");
        time.dateTime = new Date(revision.createdAt).toISOString();
        time.textContent = new Date(revision.createdAt).toLocaleString();
        composer()?.render(body, revision.contentJson, revision.text || "纯标注");
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

  async function renderSnapshot(capture) {
    controls.snapshotContent.replaceChildren(await renderSnapshotElement(capture, store));
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
        composer()?.clear();
        setComposerContext(message);
        composer()?.focus();
        break;
      case "edit":
        editing = message;
        composer()?.setValue(message.contentJson, message.text);
        setComposerContext(message, "edit");
        composer()?.focus();
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

  function toggleMessageMenu(button) {
    const menu = document.getElementById(button.getAttribute("aria-controls"));
    if (!menu) return;
    if (menu.matches(":popover-open")) {
      menu.hidePopover();
      return;
    }
    menu.showPopover();
    const anchor = button.getBoundingClientRect();
    const bounds = menu.getBoundingClientRect();
    menu.style.top = `${Math.max(8, Math.min(anchor.bottom + 4, innerHeight - bounds.height - 8))}px`;
    menu.style.left = `${Math.max(8, Math.min(anchor.right - bounds.width, innerWidth - bounds.width - 8))}px`;
  }

  function bind() {
    controls.close.addEventListener("click", close);
    controls.fullscreen.addEventListener("click", () => {
      setFullscreen(controls.overlay.dataset.fullscreen !== "true");
    });
    controls.handle.addEventListener("pointerdown", (event) => {
      if (!event.isPrimary || event.button !== 0) return;
      controls.handle.setPointerCapture(event.pointerId);
      sheetDrag = {
        pointerId: event.pointerId,
        startY: event.clientY,
        startHeight: controls.overlay.getBoundingClientRect().height,
        moved: false,
      };
    });
    controls.handle.addEventListener("pointermove", (event) => {
      if (!sheetDrag || event.pointerId !== sheetDrag.pointerId) return;
      const distance = sheetDrag.startY - event.clientY;
      if (Math.abs(distance) < 4 && !sheetDrag.moved) return;
      sheetDrag.moved = true;
      setFullscreen(false);
      const height = Math.max(280, Math.min(innerHeight, sheetDrag.startHeight + distance));
      controls.overlay.style.setProperty("--message-sheet-height", `${height}px`);
    });
    const finishSheetDrag = (event) => {
      if (!sheetDrag || event.pointerId !== sheetDrag.pointerId) return;
      if (!sheetDrag.moved) setFullscreen(true);
      sheetDrag = null;
    };
    controls.handle.addEventListener("pointerup", finishSheetDrag);
    controls.handle.addEventListener("pointercancel", () => (sheetDrag = null));
    controls.handle.addEventListener("keydown", (event) => {
      if (event.key !== "Enter" && event.key !== " ") return;
      event.preventDefault();
      setFullscreen(true);
    });
    for (const button of controls.scopeButtons) {
      button.addEventListener("click", () => setScope(button.dataset.messageScope));
    }
    for (const button of controls.orderButtons) {
      button.addEventListener("click", () => {
        order = button.dataset.messageOrder;
        if (scope !== "mark") renderConversation();
      });
    }
    controls.sourceJump.addEventListener("click", async () => {
      const sourceMessage = conversation?.messages.find(
        (message) => message.source && !message.deleted,
      );
      if (!sourceMessage) return;
      try {
        if ((await annotations.go(sourceMessage.id)).ok) close();
        else report("无法返回原文", true);
      } catch {
        report("无法返回原文", true);
      }
    });
    controls.overlay.addEventListener("keydown", (event) => {
      if (event.key === "Escape") close();
    });
    readingSurface.addEventListener("pointerdown", () => (controls.overlay.hidden = true));
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
      if (event.target.closest(".message-card-body a")) {
        event.preventDefault();
        return;
      }
      const conversationButton = event.target.closest("button[data-conversation-open]");
      if (conversationButton) {
        open(conversationButton.dataset.conversationOpen, conversationButton.dataset.messageId);
        return;
      }
      const menuButton = event.target.closest("button[data-message-menu]");
      if (menuButton) {
        toggleMessageMenu(menuButton);
        return;
      }
      const target = event.target.closest("button[data-message-target]");
      if (target) {
        const card = controls.list.querySelector(
          `[data-message-id="${CSS.escape(target.dataset.messageTarget)}"]`,
        );
        card?.scrollIntoView({ block: "center", behavior: "smooth" });
        if (card) {
          card.dataset.flash = "true";
          setTimeout(() => delete card.dataset.flash, 1200);
        }
        return;
      }
      const button = event.target.closest("button[data-message-action]");
      if (button) {
        button.closest("[popover]")?.hidePopover();
        handleAction(button).catch(() => report("操作失败，请重试", true));
      }
    });
    controls.cancelEdit.addEventListener("click", resetComposer);
    controls.form.addEventListener("submit", async (event) => {
      event.preventDefault();
      const value = composer()?.value();
      if (!value?.text) return;
      try {
        if (editing) {
          await store.revise(editing.id, editing.revisionId, value.text, value.richText);
        }
        else {
          if (!parentId) throw new Error("missing-message-parent");
          await store.reply({
            conversationId: conversation.id,
            replyToMessageId: parentId,
            text: value.text,
            richText: value.richText,
            referenceIds: [],
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
