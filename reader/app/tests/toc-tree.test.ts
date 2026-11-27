import assert from "node:assert/strict";
import test from "node:test";
import { buildTocTree, initialTocExpansion } from "../src/lib/data/toc-tree.ts";

test("builds real parent/child relationships without changing the manifest order or indices", () => {
  const tree = buildTocTree([
    { label: "Part I", href: "part.xhtml" },
    { label: "Section 1", href: "part.xhtml#first", level: 2 },
    { label: "Deep section", href: "part.xhtml#deep", level: 5 },
    { label: "Section 2", href: "part.xhtml#second", level: 2 },
    { label: "Part II", href: "other.xhtml" },
  ]);
  assert.deepEqual(tree.map(node => node.index), [0, 4]);
  assert.deepEqual(tree[0].children.map(node => node.index), [1, 3]);
  assert.equal(tree[0].children[0].children[0].index, 2);
  assert.deepEqual([...initialTocExpansion(tree, 2)].sort(), [0, 1]);
});

test("keeps old flat caches flat and reveals the active path in long tables", () => {
  const flat = Array.from({ length: 25 }, (_, i) => ({
    label: `第${i + 1}节`,
    href: `${i}.xhtml`,
  }));
  assert.equal(buildTocTree(flat).length, 25);
  const tree = buildTocTree([...flat, { label: "Child", href: "24.xhtml#child", level: 2 }]);
  assert.deepEqual([...initialTocExpansion(tree, 0)], []);
  assert.deepEqual([...initialTocExpansion(tree, 25)], [24]);
});
