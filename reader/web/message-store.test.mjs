import assert from "node:assert/strict";
import test from "node:test";
import { createMessageStore } from "./message-store.mjs";

test("reply refreshes the notes projection before returning to the list", async () => {
  let latestText = "";
  const root = () => ({ messageId: "root", conversationId: "thread", kind: "mark", text: "", latestText, source: {} });
  const store = createMessageStore({
    session: { describe: () => ({ contentVersion: "edition" }) },
    client: {
      roots: async () => [root()],
      reply: async input => { latestText = input.text; return { id: "reply" }; },
    },
    legacy: {},
  });
  await store.reload();
  assert.equal(store.active()[0].latestText, "");
  assert.deepEqual(await store.reply({ text: "New reply" }), { id: "reply" });
  assert.equal(store.active()[0].latestText, "New reply");
});

test("roots failure identifies an already committed write without hiding its result", async () => {
  const rootFailure = new Error("roots failed");
  const writeFailure = new Error("write failed");
  const result = { messageId: "edited", revisionId: "new-revision" };
  let rootsRequested = 0;
  const store = createMessageStore({
    session: { describe: () => ({ contentVersion: "edition" }) },
    client: {
      roots: async () => { rootsRequested += 1; throw rootFailure; },
      reply: async () => { throw writeFailure; },
      revise: async () => result,
      remove: async () => undefined,
    },
    legacy: {},
  });
  await assert.rejects(store.reply({}), (error) => error === writeFailure);
  assert.equal(rootsRequested, 0);
  await assert.rejects(store.revise("edited", "previous-revision", "changed"), (error) =>
    error.code === "message-roots-refresh" && error.committedResult === result && error.cause === rootFailure);
  await assert.rejects(store.deleteMessage("edited", "new-revision"), (error) =>
    error.code === "message-roots-refresh" && Object.hasOwn(error, "committedResult") &&
    error.committedResult === undefined && error.cause === rootFailure);
  assert.equal(rootsRequested, 2);
});

test("late roots read follows the newer projection rather than replacing it", async () => {
  const pending = [];
  const root = (text) => ({
    messageId: "root", conversationId: "thread", kind: "mark", text: "", latestText: text, source: {},
  });
  const store = createMessageStore({
    session: { describe: () => ({ contentVersion: "edition" }) },
    client: {
      roots: () => new Promise((resolve) => pending.push(resolve)),
    },
    legacy: {},
  });
  const older = store.reload();
  const newer = store.reload();
  await Promise.resolve();
  pending[1]([root("new")]);
  await newer;
  pending[0]([root("old")]);
  assert.equal((await older)[0].latestText, "new");
  assert.equal(store.active()[0].latestText, "new");
});

test("root creation includes the document once; a failed refresh does not undo the write", async () => {
  const rich = { schema: 1, document: { type: "doc" } };
  const snapshot = { fragmentHtml: "<p>quote</p>" };
  const calls = [];
  const store = createMessageStore({
    session: { describe: () => ({ contentVersion: "edition" }) },
    locator: { inspect: () => ({ start: { section: "one" } }) },
    client: {
      edition: async () => ({}),
      createRoot: async (draft, doc) => {
        calls.push({ draft, doc });
        return { messageId: "root", conversationId: "thread" };
      },
      roots: async () => { throw Error("refresh failed"); },
    },
    legacy: {},
  });
  const result = await store.add({ canonicalLocator: "locator" }, "note", null, snapshot, rich);
  assert.equal(result.id, "root");
  assert.equal(result.ok, true);
  assert.equal(store.snapshot().lastError, "message-roots-refresh");
  assert.deepEqual(calls, [{ draft: { edition: {}, anchor: { canonicalLocator: "locator", section: "one" }, snapshot, text: "note" }, doc: rich }]);
});
