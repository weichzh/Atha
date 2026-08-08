import assert from "node:assert/strict";
import test from "node:test";

import { dispatchDictionaryLookup } from "./annotations.mjs";

test("dictionary lookup dispatches the selected text", () => {
  const events = [];
  const target = {
    dispatchEvent(event) {
      events.push(event);
      return true;
    },
  };

  assert.equal(dispatchDictionaryLookup({ toString: () => "selected text" }, target), true);
  assert.equal(events.length, 1);
  assert.equal(events[0].type, "atha:dictionary-lookup");
  assert.deepEqual(events[0].detail, { query: "selected text" });
  assert.equal(dispatchDictionaryLookup(null, target), false);
  assert.equal(events.length, 1);
});
