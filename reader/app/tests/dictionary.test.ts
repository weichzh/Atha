import assert from "node:assert/strict";
import test from "node:test";

import {
  readDictionaryPreferences,
  writeDictionaryPreferences,
} from "../src/dictionary.ts";

test("dictionary settings persist valid values and reject damaged storage", () => {
  const id = "a".repeat(64);
  let stored = "";
  const storage = {
    getItem: () => stored || null,
    setItem: (_key: string, value: string) => {
      stored = value;
    },
  };

  assert.deepEqual(readDictionaryPreferences(storage), { dictionaryId: "", fontScale: 1 });
  assert.equal(writeDictionaryPreferences({ dictionaryId: id, fontScale: 1.5 }, storage), true);
  assert.deepEqual(readDictionaryPreferences(storage), { dictionaryId: id, fontScale: 1.5 });

  stored = JSON.stringify({ schema: 1, dictionaryId: "../outside", fontScale: 9 });
  assert.deepEqual(readDictionaryPreferences(storage), { dictionaryId: "", fontScale: 1 });
  assert.deepEqual(readDictionaryPreferences({ getItem: () => "{" }), {
    dictionaryId: "",
    fontScale: 1,
  });
  assert.equal(
    writeDictionaryPreferences(
      { dictionaryId: id, fontScale: 1 },
      {
        setItem() {
          throw new Error("storage-disabled");
        },
      },
    ),
    false,
  );
});
