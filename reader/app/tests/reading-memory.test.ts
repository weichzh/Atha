import assert from "node:assert/strict";
import test from "node:test";
import { buildDemoMessageSeed } from "../src/lib/data/demo-seed.ts";
import { getReadingMemoryClient } from "../src/lib/data/message-adapter.ts";

test("demo recent memory uses message identity for ties without changing nonempty ordering", async (t) => {
  const seed = buildDemoMessageSeed();
  const conversations = seed.conversations.slice(0, 3).map((conversation, index) => ({
    ...conversation,
    messages: [{
      ...conversation.messages[0],
      text: "同一查询",
      updatedAt: index === 2 ? 2000 : 1000,
    }],
  }));
  const [first, second, newest] = conversations.map(conversation => conversation.messages[0].id);
  seed.conversations = conversations.reverse();
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: { localStorage: { getItem: () => JSON.stringify(seed) } },
  });
  t.after(() => { Reflect.deleteProperty(globalThis, "window"); });

  const client = getReadingMemoryClient();
  const ids = async (query: string) => (await client.search(query)).map(hit => hit.messageId);
  const recent = [newest, ...[first, second].sort()];
  assert.deepEqual(await ids(""), recent);
  assert.deepEqual(await ids("\u0085\u3000"), recent);
  assert.deepEqual(await ids("同一查询"), [newest, second, first]);
});
