import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, basename } from 'node:path';
import { collectNodeTests, runNodeTests } from '../test-node.mjs';

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'atha-node-runner-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const dir of ['reader/web', 'reader/app/tests', 'scripts/tests']) mkdirSync(join(root, dir), { recursive: true });
  return root;
}
test('Node runner discovers new unit tests and excludes service-backed browser suites', t => {
  const root = fixture(t);
  for (const path of ['reader/web/next.test.mjs', 'reader/app/tests/next.test.ts', 'scripts/tests/entry.test.mjs', 'reader/app/tests/ime-layout.test.mjs']) writeFileSync(join(root, path), '');
  mkdirSync(join(root, 'reader/web/not-a-file.test.mjs'));
  assert.deepEqual(collectNodeTests(root).map(path => basename(path)), ['next.test.ts', 'next.test.mjs', 'entry.test.mjs']);
});
test('Node runner refuses an empty suite', t => {
  assert.throws(() => collectNodeTests(fixture(t)), /No Node tests/);
});
test('Node runner propagates success and nonzero child test status', t => {
  const root = fixture(t), path = join(root, 'reader/web/probe.test.mjs');
  writeFileSync(path, 'import test from "node:test"; test("pass", () => {});');
  assert.equal(runNodeTests(root, [], 'pipe'), 0);
  writeFileSync(path, 'import test from "node:test"; test("expected child failure", () => { throw Error("controlled-failure"); });');
  assert.notEqual(runNodeTests(root, [], 'pipe'), 0);
});
