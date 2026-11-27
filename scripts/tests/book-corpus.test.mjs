import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtempSync, writeFileSync, mkdirSync, rmSync, symlinkSync, truncateSync, readFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { parseManifest, sourceHash, parseProbe, runSample, parseArguments } from '../check-book-corpus.mjs';

const success = { schema: 1, status: 'passed', phase: 'complete', code: 'ok', sections: 2, resources: 3 };
const output = value => `book_probe=${JSON.stringify(value)}\n`;
const goodProcess = value => ({ status: 0, stdout: output(value), stderr: '' });
const script = fileURLToPath(new URL('../check-book-corpus.mjs', import.meta.url));
function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'atha-corpus-self-check-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const path = join(root, '私有测试标题 with spaces.txt');
  writeFileSync(path, 'Public synthetic text; not a user book.');
  return { root, sample: { id: 'book-001', path, sha256: sourceHash(path) } };
}
function manifest(sample) { return { schema: 1, samples: [sample] }; }

test('Corpus manifest is explicit, bounded and uses anonymous identifiers', t => {
  const { root, sample } = fixture(t);
  assert.deepEqual(parseManifest(JSON.stringify(manifest(sample)), root), [sample]);
  assert.equal(parseManifest(JSON.stringify(manifest({ ...sample, path: 'relative.txt' })), root)[0].path, resolve(root, 'relative.txt'));
  for (const bad of [null, [], {}, { ...manifest(sample), unknown: 1 }, { schema: 1, samples: [] },
    { schema: true, samples: [sample] }, { schema: 1, samples: [sample, sample] },
    manifest({ ...sample, title: 'not allowed' }), manifest({ ...sample, id: 'private-book-title' }),
    manifest({ ...sample, sha256: 'bad' }), manifest({ ...sample, path: '' }),
    manifest({ ...sample, path: 'line\nbreak.txt' }),
    { schema: 1, samples: Array.from({ length: 65 }, (_, i) => ({ ...sample, id: `book-${String(i).padStart(3, '0')}` })) },
  ]) assert.throws(() => parseManifest(JSON.stringify(bad), root), /invalid-corpus-manifest/);
  assert.throws(() => parseManifest('x'.repeat(256 * 1024 + 1), root), /invalid-corpus-manifest/);
});

test('Corpus source preflight rejects missing, linked, empty, unsupported and oversized inputs', t => {
  const { root } = fixture(t);
  const empty = join(root, 'empty.txt'), directory = join(root, 'directory.txt'), large = join(root, 'large.md');
  writeFileSync(empty, ''); mkdirSync(directory); writeFileSync(large, ''); truncateSync(large, 16 * 1024 * 1024 + 1);
  assert.throws(() => sourceHash(join(root, 'missing.txt')), /unavailable-source/);
  assert.throws(() => sourceHash(empty), /unavailable-source/);
  assert.throws(() => sourceHash(directory), /unavailable-source/);
  assert.throws(() => sourceHash(large), /source-too-large/);
  assert.throws(() => sourceHash(join(root, 'book.pdf')), /unsupported-source/);
  if (process.platform !== 'win32') {
    const link = join(root, 'linked.txt'); symlinkSync(empty, link);
    assert.throws(() => sourceHash(link), /unavailable-source/);
  }
});

test('Corpus result requires one complete success report and a successful process', () => {
  assert.equal(parseProbe(goodProcess(success)).status, 'passed');
  for (const processResult of [
    { ...goodProcess(success), status: 1 }, { ...goodProcess(success), status: null },
    { ...goodProcess(success), signal: 'SIGTERM' }, { status: 0, stdout: '' },
    { status: 0, stdout: output(success) + output(success) },
    goodProcess({ ...success, sections: 0 }), goodProcess({ ...success, resources: -1 }),
    goodProcess({ ...success, privatePath: 'unexpected' }), goodProcess({ ...success, phase: 'open' }),
    { status: 0, stdout: 'book_probe={not JSON}\n' },
  ]) assert.equal(parseProbe(processResult).status, 'failed');
  assert.equal(parseProbe({ error: { code: 'ETIMEDOUT' } }).code, 'timeout');
});

test('Corpus child failures never echo private stderr or arbitrary code strings', t => {
  const { sample } = fixture(t);
  const privateMessage = 'PRIVATE-CONTENT-DO-NOT-REPORT';
  const badReport = { ...success, status: 'failed', phase: 'open', code: privateMessage };
  const command = [process.execPath, '-e', `console.error(${JSON.stringify(privateMessage)}); console.log(${JSON.stringify(output(badReport))}); process.exitCode=1;`];
  const result = runSample(sample, command);
  assert.equal(result.status, 'failed');
  assert.equal(result.phase, 'open');
  assert.equal(result.code, 'book-check-failed');
  for (const token of [privateMessage, sample.path, sample.sha256]) assert(!JSON.stringify(result).includes(token));
});

test('Corpus subprocess smoke covers success, nonzero exit, timeout and missing executable', t => {
  const { sample } = fixture(t);
  const print = `console.log(${JSON.stringify(output(success))});`;
  assert.equal(runSample(sample, [process.execPath, '-e', print]).status, 'passed');
  assert.equal(runSample(sample, [process.execPath, '-e', `${print} process.exitCode=2;`]).status, 'failed');
  assert.equal(runSample(sample, [process.execPath, '-e', 'setTimeout(() => {}, 5000)'], 0.1).code, 'timeout');
  assert.equal(runSample(sample, [join(tmpdir(), 'atha-no-such-probe')]).code, 'probe-process-failed');
});

test('Corpus hash mismatch prevents execution and changed source cannot pass', t => {
  const { sample } = fixture(t);
  assert.equal(runSample({ ...sample, sha256: '0'.repeat(64) }, [process.execPath, '-e', 'throw Error("must not execute")']).code, 'hash-mismatch');
  const command = [process.execPath, '-e', `require('node:fs').writeFileSync(process.env.ATHA_CORPUS_SOURCE,'changed synthetic text'); console.log(${JSON.stringify(output(success))});`];
  assert.equal(runSample(sample, command).code, 'source-changed');
});

test('Corpus CLI rejects incomplete arguments and missing private files without leaking paths', t => {
  const { root, sample } = fixture(t);
  assert.equal(parseArguments(['--manifest', 'private.json', '--timeout-seconds', '60']).timeoutSeconds, 60);
  for (const args of [[], ['--manifest'], ['--manifest', 'one', '--manifest', 'two'],
    ['--manifest', 'one', '--timeout-seconds', '0'], ['--manifest', 'one', '--timeout-seconds', '601'],
    ['--manifest', 'one', '--unknown', 'value']]) assert.throws(() => parseArguments(args), /invalid-corpus-arguments/);
  const path = join(root, 'private-list.json');
  writeFileSync(path, JSON.stringify(manifest({ ...sample, path: join(root, 'MISSING-PRIVATE-TITLE.txt') })));
  const result = spawnSync(process.execPath, [script, '--manifest', path], { encoding: 'utf8' });
  assert.equal(result.status, 1);
  assert.equal(JSON.parse(result.stdout).failed, 1);
  for (const token of [root, 'MISSING-PRIVATE-TITLE', sample.sha256]) assert(!(result.stdout + result.stderr).includes(token));
});


test('Corpus timeout removes the isolated child data tree', t => {
  const { root, sample } = fixture(t), receipt = join(root, 'work-root.json');
  const child = `const fs=require('node:fs'); const root=process.env.ATHA_CORPUS_WORK_ROOT; fs.writeFileSync(${JSON.stringify(receipt)}, JSON.stringify(root ?? null)); if(root)fs.writeFileSync(require('node:path').join(root,'private-cache'),'synthetic private cache'); setTimeout(() => {},5000);`;
  const result = runSample(sample, [process.execPath, '-e', child], 0.15);
  assert.equal(result.code, 'timeout');
  const workRoot = JSON.parse(readFileSync(receipt, 'utf8'));
  assert.equal(typeof workRoot, 'string', 'parent must own an isolated cleanup root');
  assert.equal(existsSync(workRoot), false, 'timeout must not leave imported book copies');
});
