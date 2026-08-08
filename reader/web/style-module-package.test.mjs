import assert from "node:assert/strict";
import test from "node:test";

import { STYLE_MODULE_LIMITS, createStyleModulePackageCodec } from "./style-module-package.mjs";

const sample = Object.freeze({
  id: "quiet-reading",
  name: "安静阅读",
  group: "排版",
  enabled: true,
  css: ".book p { line-height: 1.7; }",
});

function codec() {
  return createStyleModulePackageCodec((css) => {
    if (css.includes("url(")) throw new Error("unsafe-css-resource");
  });
}

test("schema 1 CSS module packages round-trip through one strict boundary", () => {
  const modules = codec().parse(codec().stringify([sample]));
  assert.deepEqual(modules, [sample]);
  assert.equal(Object.isFrozen(modules), true);
  assert.equal(Object.isFrozen(modules[0]), true);
});

test("the package limit covers every locally valid disabled module", () => {
  const modules = Array.from({ length: 32 }, (_, index) => ({
    ...sample,
    id: `disabled-${index}`,
    enabled: false,
    css: "\n".repeat(32768),
  }));
  assert.deepEqual(codec().parse(codec().stringify(modules)), modules);
});

test("CSS module packages reject unknown fields, duplicate IDs, oversize CSS, and unsafe CSS", () => {
  const parse = (modules) => codec().parse(JSON.stringify({ schema: 1, modules }));
  assert.throws(() => parse([{ ...sample, unknown: true }]));
  assert.throws(() => parse([sample, sample]));
  assert.throws(() => parse([{ ...sample, css: "中".repeat(10923) }]));
  assert.throws(() => parse([{ ...sample, css: ".book { background: url(https://invalid); }" }]));
  const oversized = `{"schema":1,"modules":[],"padding":"${"x".repeat(STYLE_MODULE_LIMITS.packageBytes)}"}`;
  assert.throws(
    () => codec().parse(oversized),
    /invalid-style-module-package/,
  );
});
