import assert from "node:assert/strict";
import test from "node:test";

import {
  conversationFeed,
  createConversations,
  currentConversationMessages,
  formatMessageTime,
  isSnapshotCssSafe,
  linkedMessagePreviews,
  normalizeConversationOpenOptions,
  parseSnapshotPresentation,
  resolveConversationScrollPlan,
  validConversationTarget,
} from "./conversations.mjs";

test("conversation open options separate scroll and focus from the reply parent", () => {
  const options = {
    scrollTarget: "message",
    targetMessageId: "target",
    replyParentId: "parent",
    edit: false,
    focusMode: "dialog",
    expectedRootId: "root",
    navigationFailed: true,
  };
  assert.deepEqual(normalizeConversationOpenOptions(options), options);
  assert.equal(normalizeConversationOpenOptions({ scrollTarget: "source", replyParentId: "child" }).replyParentId, "child");
  assert.deepEqual(normalizeConversationOpenOptions("child", false, "root", true), {
    ...options,
    targetMessageId: "child",
    replyParentId: "child",
  });
  assert.deepEqual(normalizeConversationOpenOptions("child", true, "root", false), {
    scrollTarget: "source",
    targetMessageId: "child",
    replyParentId: "child",
    edit: true,
    focusMode: "composer",
    expectedRootId: "root",
    navigationFailed: false,
  });
  assert.deepEqual(normalizeConversationOpenOptions(), {
    scrollTarget: "source",
    targetMessageId: null,
    replyParentId: null,
    edit: false,
    focusMode: "dialog",
    expectedRootId: null,
    navigationFailed: false,
  });
});

test("conversation scroll preserves the source turn and locates only live replies", () => {
  const root = { id: "root", source: { selectedText: "原文" }, text: "" };
  const conversation = {
    messages: [root, { id: "child", text: "回复" }, { id: "deleted", deleted: true }],
  };
  const top = { mode: "top", flashMessageId: null };
  const directed = (targetMessageId) => ({ scrollTarget: "message", targetMessageId });
  assert.deepEqual(
    resolveConversationScrollPlan(conversation, { scrollTarget: "source", targetMessageId: "child" }),
    top,
  );
  assert.deepEqual(resolveConversationScrollPlan(conversation, directed(root.id)), top);
  const withRootText = { messages: [{ ...root, text: "笔记" }, ...conversation.messages.slice(1)] };
  assert.deepEqual(resolveConversationScrollPlan(withRootText, directed(root.id)), {
    mode: "top",
    flashMessageId: root.id,
  });
  assert.deepEqual(resolveConversationScrollPlan(conversation, directed("child")), {
    mode: "card",
    messageId: "child",
  });
  for (const target of ["absent", "deleted", null]) {
    assert.deepEqual(resolveConversationScrollPlan(conversation, directed(target)), top);
  }
});

function conversationTestDeferred() {
  let resolve;
  let reject;
  const promise = new Promise((ok, fail) => { resolve = ok; reject = fail; });
  return { promise, resolve, reject };
}

function conversationTestFixture(t, store, closeTools) {
  const focused = [];
  const redraws = [];
  const frames = [];
  let draft = "";
  let richText;
  let version = Symbol();
  class Control {
    dataset = {};
    hidden = false;
    textContent = "";
    children = [];
    attributes = new Map();
    listeners = new Map();
    addEventListener(type, callback) { this.listeners.set(type, callback); }
    emit(type, event = {}) { return this.listeners.get(type)?.(event); }
    setAttribute(name, value) { this.attributes.set(name, value); }
    getAttribute(name) { return this.attributes.get(name) ?? null; }
    append(...children) { this.children.push(...children); }
    replaceChildren(...children) { this.children = children; }
    querySelectorAll(selector) {
      return this.children.filter((element) => element.className === selector.slice(1));
    }
    focus() { focused.push(this); }
  }
  const controls = Object.fromEntries([
    "overlay", "close", "handle", "fullscreen", "list", "sourceLabel", "source",
    "sourceContext", "sourceJump", "form", "orderControls", "composerContext",
    "composerContextText", "status", "exportAllButton", "cancelEdit",
  ].map((name) => [name, new Control()]));
  controls.overlay.hidden = true;
  controls.scopeButtons = ["mark", "chapter", "book"].map((scope) => {
    const button = new Control();
    button.dataset.messageScope = scope;
    return button;
  });
  controls.orderButtons = [];
  const readingSurface = new Control();
  const globals = {
    document: { createElement: () => new Control() },
    window: {
      athaMessageComposer: {
        clear() { draft = ""; richText = undefined; version = Symbol(); },
        draftVersion() { return version; },
        focus() { focused.push("composer"); },
        setValue(_content, text) { draft = text; version = Symbol(); },
        value() { return { text: draft, richText }; },
        render() {},
      },
    },
    requestAnimationFrame: (frame) => frames.push(frame),
    confirm: () => true,
  };
  for (const [name, value] of Object.entries(globals)) {
    const previous = Object.getOwnPropertyDescriptor(globalThis, name);
    Object.defineProperty(globalThis, name, { configurable: true, writable: true, value });
    t.after(() => {
      if (previous) Object.defineProperty(globalThis, name, previous);
      else delete globalThis[name];
    });
  }
  const manager = createConversations({
    store,
    annotations: { redraw() { redraws.push(true); } },
    controls,
    closeTools: () => closeTools?.(manager),
    editionId: "edition",
    readingSurface,
    returnFocus: () => readingSurface,
  });
  manager.bind();
  return {
    manager, controls, focused, redraws, readingSurface,
    flushFrames() { for (const frame of frames.splice(0)) frame(); },
    setDraft(value, rich) { draft = value; richText = rich; version = Symbol(); },
    getDraft: () => draft,
    getRichText: () => richText,
    scope: (name) => controls.scopeButtons.find((button) => button.dataset.messageScope === name).emit("click"),
    hasMessage: (id) => controls.list.children.some((card) => card.dataset.messageId === id),
    submit: () => controls.form.emit("submit", { preventDefault() {} }),
    action(id, name) {
      const button = {
        dataset: { messageId: id, messageAction: name },
        closest(selector) { return selector === "button[data-message-action]" ? this : null; },
      };
      controls.list.emit("click", { target: button });
    },
  };
}

function conversationTestThread(id) {
  return {
    id, editionId: "edition",
    messages: [
      { id: `${id}-root`, source: { selectedText: `${id}-source` }, createdAt: 0 },
      { id: `${id}-child`, text: `${id}-reply`, createdAt: 1, revisionId: "r1" },
    ],
  };
}

function conversationWithSaved(...ids) {
  const thread = conversationTestThread("A");
  for (const id of ids) thread.messages.push({ id, text: id, createdAt: 2 });
  return thread;
}

for (const [write, pendingAt] of [["reply", "save"], ["reply", "reload"], ["delete", "reload"]]) {
  test(`pending ${write} ${pendingAt} leaves the new conversation, draft, and reply context intact`, async (t) => {
    const save = conversationTestDeferred();
    const reload = conversationTestDeferred();
    const reloadStarted = conversationTestDeferred();
    const writes = [];
    let openedA = false;
    const view = conversationTestFixture(t, {
      conversation(id) {
        if (id === "A" && openedA) {
          reloadStarted.resolve();
          return reload.promise;
        }
        if (id === "A") openedA = true;
        return Promise.resolve(conversationTestThread(id));
      },
      reply(data) {
        writes.push(data);
        return data.conversationId === "A" ? save.promise : new Promise(() => {});
      },
      deleteMessage: () => save.promise,
    });
    await view.manager.open("A");
    view.setDraft("A-draft");
    let sending;
    if (write === "reply") sending = view.submit();
    else view.action("A-child", "delete");
    if (pendingAt === "reload") {
      save.resolve();
      await reloadStarted.promise;
    }
    await view.manager.open("B", { scrollTarget: "source", replyParentId: "B-child" });
    view.setDraft("B-draft");
    if (pendingAt === "save") {
      save.resolve();
    }
    reload.resolve(conversationTestThread("A"));
    await sending;
    await new Promise(setImmediate);
    assert.equal(view.controls.source.textContent, "B-source");
    assert.equal(view.getDraft(), "B-draft");
    assert.equal(view.controls.composerContextText.textContent, "B-reply");
    assert.equal(view.controls.status.textContent, "");
    assert.equal(view.redraws.length, 1, "committed changes still refresh the notes list");
    void view.submit();
    assert.equal(writes.at(-1).conversationId, "B");
    assert.equal(writes.at(-1).replyToMessageId, "B-child");
  });
}

test("a reply failure from a previous conversation does not replace the current status", async (t) => {
  const save = conversationTestDeferred();
  const view = conversationTestFixture(t, {
    conversation: async (id) => conversationTestThread(id),
    reply: () => save.promise,
  });
  await view.manager.open("A");
  view.setDraft("A-draft");
  const submitted = view.submit();
  await view.manager.open("B");
  view.setDraft("B-draft");
  save.reject(new Error("failed"));
  await submitted;
  assert.equal(view.controls.status.textContent, "");
  assert.equal(view.getDraft(), "B-draft");
});

test("current → chapter → current keeps the committed draft cleared and applies its in-flight refresh", async (t) => {
  const reload = conversationTestDeferred();
  const reloading = conversationTestDeferred();
  const oldFeed = conversationTestDeferred();
  let readCount = 0;
  let writes = 0;
  const view = conversationTestFixture(t, {
    conversation() {
      if (++readCount === 1) return conversationTestThread("A");
      reloading.resolve();
      return reload.promise;
    },
    conversations: () => oldFeed.promise,
    reply() { writes += 1; },
  });
  await view.manager.open("A");
  view.setDraft("A-first");
  const sending = view.submit();
  await reloading.promise;
  const chapter = view.scope("chapter");
  await view.scope("mark");
  reload.resolve(conversationWithSaved("A-first"));
  await sending;
  oldFeed.resolve([conversationTestThread("A")]);
  await chapter;
  assert.equal(view.controls.form.hidden, false);
  assert.equal(view.hasMessage("A-first"), true);
  assert.equal(view.getDraft(), "");
  await view.submit();
  assert.equal(writes, 1);
});

for (const change of ["next text", "rich formatting only", "same normalized Markdown"]) {
  test(`a saved reply does not clear the newer draft: ${change}`, async (t) => {
    const write = conversationTestDeferred();
    const reload = conversationTestDeferred();
    const reloading = conversationTestDeferred();
    let readCount = 0;
    const sent = [];
    const view = conversationTestFixture(t, {
      conversation() {
        if (++readCount === 1) return conversationTestThread("A");
        reloading.resolve();
        return reload.promise;
      },
      reply(data) { sent.push(data); return write.promise; },
    });
    await view.manager.open("A", { scrollTarget: "source", replyParentId: "A-child" });
    view.setDraft("same");
    const sending = view.submit();
    const nextDraft = change === "next text" ? "next" : "same";
    const format = change === "rich formatting only" ? { marks: ["bold"] } : undefined;
    if (change === "next text") {
      write.resolve();
      await reloading.promise;
    } else {
      // Same plain text: formatting or raw Markdown with an unchanged normalized form.
      view.setDraft(nextDraft, format);
      assert.equal(view.getDraft(), sent[0].text);
      write.resolve();
      await reloading.promise;
    }
    if (change === "next text") view.setDraft(nextDraft);
    reload.resolve(conversationWithSaved("A-saved"));
    await sending;
    assert.equal(view.hasMessage("A-saved"), true);
    assert.equal(view.getDraft(), nextDraft);
    assert.deepEqual(view.getRichText(), format);
    assert.equal(sent[0].replyToMessageId, "A-child");
    if (change !== "next text") assert.equal(view.controls.composerContextText.textContent, "A-reply");
  });
}

test("older thread refresh cannot replace a newer read of the same thread", async (t) => {
  const older = conversationTestDeferred();
  const newer = conversationTestDeferred();
  const firstRead = conversationTestDeferred();
  const secondRead = conversationTestDeferred();
  let reads = 0;
  const view = conversationTestFixture(t, {
    conversation() {
      if (++reads === 1) return conversationTestThread("A");
      if (reads === 2) { firstRead.resolve(); return older.promise; }
      secondRead.resolve();
      return newer.promise;
    },
    reply() {},
  });
  await view.manager.open("A");
  view.setDraft("first");
  const first = view.submit();
  await firstRead.promise;
  view.setDraft("second");
  const second = view.submit();
  await secondRead.promise;
  newer.resolve(conversationWithSaved("A-first", "A-second"));
  await second;
  older.resolve(conversationWithSaved("A-first"));
  await first;
  assert.equal(view.hasMessage("A-first"), true);
  assert.equal(view.hasMessage("A-second"), true);
});

test("reopening the displayed thread on a pending older read keeps a newer committed snapshot", async (t) => {
  const olderOpen = conversationTestDeferred();
  const opening = conversationTestDeferred();
  const write = conversationTestDeferred();
  let reads = 0;
  const view = conversationTestFixture(t, {
    conversation() {
      if (++reads === 1) return conversationTestThread("A");
      if (reads === 2) { opening.resolve(); return olderOpen.promise; }
      return conversationWithSaved("A-saved");
    },
    reply: () => write.promise,
  });
  await view.manager.open("A");
  view.setDraft("first");
  const sending = view.submit();
  const reopening = view.manager.open("A");
  await opening.promise;
  write.resolve();
  await sending;
  olderOpen.resolve(conversationTestThread("A"));
  await reopening;
  assert.equal(view.hasMessage("A-saved"), true);
  assert.equal(view.getDraft(), "");
});

test("save refreshes the active aggregate and does not let its older query overwrite it", async (t) => {
  const save = conversationTestDeferred();
  const oldFeed = conversationTestDeferred();
  const newFeed = conversationTestDeferred();
  const startNewFeed = conversationTestDeferred();
  let reads = 0;
  let feedReads = 0;
  const view = conversationTestFixture(t, {
    conversation: () => ++reads === 1 ? conversationTestThread("A") : conversationWithSaved("A-saved"),
    conversations() {
      if (++feedReads === 1) return oldFeed.promise;
      startNewFeed.resolve();
      return newFeed.promise;
    },
    reply: () => save.promise,
  });
  await view.manager.open("A");
  view.setDraft("A-saved");
  const sending = view.submit();
  const chapter = view.scope("chapter");
  save.resolve();
  await startNewFeed.promise;
  newFeed.resolve([conversationWithSaved("A-saved")]);
  await sending;
  oldFeed.resolve([conversationTestThread("A")]);
  await chapter;
  assert.equal(view.controls.sourceLabel.textContent, "本章对话");
  assert.equal(view.controls.form.hidden, true);
  assert.equal(view.hasMessage("A-saved"), true);
  await view.scope("mark");
  assert.equal(view.hasMessage("A-saved"), true);
  assert.equal(view.getDraft(), "");
});

test("failed replacement of a loading aggregate ends loading without undoing the save", async (t) => {
  const save = conversationTestDeferred();
  const oldFeed = conversationTestDeferred();
  let feeds = 0;
  let reads = 0;
  const view = conversationTestFixture(t, {
    conversation: () => ++reads === 1 ? conversationTestThread("A") : conversationWithSaved("A-saved"),
    async conversations() {
      if (++feeds === 1) return oldFeed.promise;
      throw new Error("new feed failed");
    },
    reply: () => save.promise,
  });
  await view.manager.open("A");
  view.setDraft("A-saved");
  const sending = view.submit();
  const chapter = view.scope("chapter");
  save.resolve();
  await sending;
  oldFeed.resolve([conversationTestThread("A")]);
  await chapter;
  assert.equal(view.controls.form.hidden, true);
  assert.equal(view.controls.list.textContent, "无法加载对话");
  assert.equal(view.getDraft(), "");
  await view.scope("mark");
  assert.equal(view.hasMessage("A-saved"), true);
});

test("only write failure allows retry of the same draft; committed thread refresh failure does not", async (t) => {
  const write = conversationTestDeferred();
  let reads = 0;
  let writes = 0;
  const view = conversationTestFixture(t, {
    async conversation() {
      if (++reads > 1) throw new Error("read failed after commit");
      return conversationTestThread("A");
    },
    reply() { if (++writes === 1) return write.promise; },
  });
  await view.manager.open("A");
  view.setDraft("once");
  const sending = view.submit();
  await view.submit();
  assert.equal(writes, 1, "same draft cannot be re-sent while its write is pending");
  write.reject(new Error("write failed"));
  await sending;
  assert.equal(view.getDraft(), "once");
  assert.match(view.controls.status.textContent, /^保存失败/);
  await view.submit();
  assert.equal(writes, 2);
  assert.equal(view.getDraft(), "");
  assert.match(view.controls.status.textContent, /^消息已保存.*未能刷新对话/);
  await view.submit();
  assert.equal(writes, 2);
});

test("committed notes-index refresh failure preserves the write and an updated edit base", async (t) => {
  const write = conversationTestDeferred();
  const revisions = [];
  const view = conversationTestFixture(t, {
    conversation: () => conversationWithSaved(),
    revise(...args) {
      revisions.push(args);
      if (revisions.length === 1) return write.promise;
      return { revisionId: "r3" };
    },
  });
  await view.manager.open("A", { scrollTarget: "source", replyParentId: "A-child" });
  view.action("A-child", "edit");
  view.setDraft("first edit");
  const editing = view.submit();
  view.setDraft("next edit");
  write.reject(Object.assign(new Error("index failed"), {
    code: "message-roots-refresh",
    committedResult: { revisionId: "r2" },
  }));
  await editing;
  assert.equal(view.getDraft(), "next edit");
  assert.match(view.controls.status.textContent, /^消息已保存.*未能刷新笔记列表/);
  await view.submit();
  assert.deepEqual(revisions.map(([id, revision, text]) => [id, revision, text]), [
    ["A-child", "r1", "first edit"],
    ["A-child", "r2", "next edit"],
  ]);
  assert.equal(view.getDraft(), "");
});

for (const dismissal of ["pointerdown", "cancelPendingOpen"]) {
  test(`${dismissal} invalidates a slow opening without taking focus`, async (t) => {
    const loaded = conversationTestDeferred();
    const fetching = conversationTestDeferred();
    const view = conversationTestFixture(t, {
      conversation() { fetching.resolve(); return loaded.promise; },
    });
    const opening = view.manager.open("A");
    await fetching.promise;
    if (dismissal === "pointerdown") view.readingSurface.emit("pointerdown");
    else view.manager.cancelPendingOpen();
    loaded.resolve(conversationTestThread("A"));
    await opening;
    view.flushFrames();
    assert.equal(view.controls.overlay.hidden, true);
    assert.deepEqual(view.focused, []);
  });
}

test("closing the tools after a committed open does not cancel its first-frame focus", async (t) => {
  const view = conversationTestFixture(t, {
    conversation: async (id) => conversationTestThread(id),
  }, (manager) => manager.cancelPendingOpen());
  await view.manager.open("A");
  view.flushFrames();
  assert.equal(view.controls.overlay.hidden, false);
  assert.deepEqual(view.focused, [view.controls.overlay]);
});

const captured = parseSnapshotPresentation(
  '{"schema":1,"theme":"paper","brightness":90,"fontSize":40,"fontFamily":"serif","density":"comfortable"}',
);
assert.deepEqual(captured, {
  theme: "paper",
  brightness: 90,
  fontSize: 40,
  fontFamily: "serif",
  lineHeightPx: 82,
});
for (const fontSize of [16, 19, 40]) {
  assert.equal(
    parseSnapshotPresentation(
      `{"schema":1,"theme":"paper","brightness":100,"fontSize":${fontSize},"fontFamily":"book","density":"standard"}`,
    ).fontSize,
    fontSize,
  );
}
for (const fontSize of [15, 19.5, 41]) {
  assert.throws(() =>
    parseSnapshotPresentation(
      `{"schema":1,"theme":"paper","brightness":100,"fontSize":${fontSize},"fontFamily":"book","density":"standard"}`,
    ),
  );
}
assert.throws(() =>
  parseSnapshotPresentation(
    '{"schema":1,"theme":"system","brightness":100,"fontSize":32,"fontFamily":"book","density":"standard"}',
  ),
);
assert.throws(() => parseSnapshotPresentation('{"schema":1,"theme":"dark"}'));
assert.throws(() =>
  parseSnapshotPresentation(
    '{"schema":1,"theme":"dark","brightness":100,"fontSize":32,"fontFamily":"book","density":"standard","sourceStyles":true}',
  ),
);
assert.throws(() => parseSnapshotPresentation(" ".repeat(4_097)));
assert.equal(isSnapshotCssSafe("p { color: #222; }"), true);
for (const css of [
  "p { background: url(a.png); }",
  "p { background: src('a.png'); }",
  "p { background: image('a.png'); }",
  "p { background: image-set('a.png' 1x); }",
  String.raw`p { background: u\72l('a.png'); }`,
]) {
  assert.equal(isSnapshotCssSafe(css), false);
}

const targetConversation = {
  editionId: "11".repeat(32),
  messages: [{ id: "aa".repeat(16), source: { selectedText: "原文" }, deleted: false }],
};
assert.equal(
  validConversationTarget(targetConversation, "11".repeat(32), "aa".repeat(16)),
  true,
);
assert.equal(validConversationTarget(targetConversation, "22".repeat(32)), false);
assert.equal(validConversationTarget(targetConversation, "11".repeat(32), "bb".repeat(16)), false);
assert.equal(
  validConversationTarget(
    { ...targetConversation, messages: [{ ...targetConversation.messages[0], deleted: true }] },
    "11".repeat(32),
  ),
  false,
);

const rootMessage = {
  id: "root",
  text: "根消息",
  source: null,
  deleted: false,
  referencePreviews: [{ id: "nested", text: "不应展开的间接引用", deleted: false }],
};
const replyMessage = {
  id: "reply",
  text: "回复正文",
  source: null,
  deleted: false,
  replyToMessageId: "root",
  referencePreviews: [{ id: "external", text: "外部引用", deleted: false }],
};
assert.deepEqual(linkedMessagePreviews(replyMessage, [rootMessage, replyMessage]), [
  { id: "root", text: "根消息", local: true, kind: "reply" },
  { id: "external", text: "外部引用", local: false, kind: "reference" },
]);
assert.deepEqual(linkedMessagePreviews(replyMessage, [rootMessage, replyMessage], "root"), [
  { id: "external", text: "外部引用", local: false, kind: "reference" },
]);
assert.deepEqual(
  currentConversationMessages({
    messages: [
      { id: "source-only", source: { selectedText: "原文" }, text: "", deleted: false },
      { id: "response", source: null, text: "回应", deleted: false },
    ],
  }).map((message) => message.id),
  ["response"],
);
assert.equal(formatMessageTime(new Date(2026, 0, 1, 14, 5).valueOf()), "14:05");

const firstConversation = {
  id: "first",
  messages: [
    { id: "first-root", createdAt: 20, source: { canonicalLocator: "later" }, deleted: false },
    { id: "first-reply", createdAt: 40, source: null, deleted: false },
  ],
};
const secondConversation = {
  id: "second",
  messages: [
    { id: "second-root", createdAt: 10, source: { canonicalLocator: "earlier" }, deleted: false },
    { id: "second-reply", createdAt: 30, source: null, deleted: false },
  ],
};
const conversations = [firstConversation, secondConversation];
assert.deepEqual(
  conversationFeed(conversations, "time").map(({ message }) => message.id),
  ["second-root", "first-root", "second-reply", "first-reply"],
);
const bookFeed = conversationFeed(
  conversations,
  "book",
  (left, right) => left.canonicalLocator.localeCompare(right.canonicalLocator),
);
assert.deepEqual(
  bookFeed.map(({ message }) => message.id),
  ["second-root", "second-reply", "first-root", "first-reply"],
);
assert.deepEqual(
  bookFeed.filter(({ showSource }) => showSource).map(({ message }) => message.id),
  ["second-root", "first-root"],
);
