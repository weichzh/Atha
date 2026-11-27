import assert from "node:assert/strict";
import test from "node:test";

import { buildSearchExcerpt } from "./search.mjs";

function assertExcerpt(text, start, end, expected, ...radii) {
  const result = buildSearchExcerpt(text, start, end, ...radii);
  const [before, match, after] = expected;
  assert.deepEqual(result, { before, match, after, text: before + match + after });
  assert.equal(result.before + result.match + result.after, result.text);
}

test("excerpt preserves word boundaries surrounding the matching text", () => {
  assertExcerpt("  first target last  ", 8, 14, ["first ", "target", " last"]);
});

test("excerpt normalizes NUL and runs of whitespace across segment boundaries", () => {
  const text = " \0before\0 \ntwo\t  words\0 \r\n after \0";
  const start = text.indexOf("two");
  assertExcerpt(text, start, start + "two\t  words".length, ["before ", "two words", " after"]);
  assertExcerpt("a \t \0 b", 2, 4, ["a ", "", "b"]);
});

test("excerpt marks the second occurrence using the original text offsets", () => {
  const text = "one\n one again";
  const start = text.lastIndexOf("one");
  assertExcerpt(text, start, start + 3, ["one ", "one", " again"]);
});

test("excerpt retains markup-like input as literal text", () => {
  const text = 'raw <script>alert("&")</script> tail';
  assertExcerpt(text, 4, 31, ["raw ", '<script>alert("&")</script>', " tail"]);
});

test("excerpt bounds context without shortening longer matches", () => {
  const match = "X".repeat(500);
  const text = "0123456789" + match + "abcdefghij";
  assertExcerpt(text, 10, 510, ["789", match, "abcd"], 3, 4);
  assertExcerpt(match, 0, match.length, ["", match, ""]);
});
