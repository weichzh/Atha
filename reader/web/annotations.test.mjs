import assert from "node:assert/strict";
import test from "node:test";

import { dispatchDictionaryLookup } from "./annotations.mjs";
import { cloneSelectedRange } from "./content.mjs";

test("shadow selection trusts the range when Chromium misreports isCollapsed", () => {
  const node = {};
  const cloned = {};
  const range = {
    collapsed: false,
    commonAncestorContainer: node,
    cloneRange: () => cloned,
  };
  const book = { contains: (candidate) => candidate === node };
  const selection = {
    isCollapsed: true,
    rangeCount: 1,
    getRangeAt: () => range,
  };

  assert.equal(cloneSelectedRange(book, selection), cloned);
  range.collapsed = true;
  assert.equal(cloneSelectedRange(book, selection), null);
});

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
