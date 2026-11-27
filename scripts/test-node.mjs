// Unit tests only; browser suites keep their explicit preview commands.
import { readdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const browserTests = new Set([
  'ui-redesign.test.mjs', 'startup-theme.test.mjs',
  'ime-layout.test.mjs', 'reader-workspace.test.mjs',
]);

export function collectNodeTests(repoRoot = root) {
  const files = [];
  for (const directory of ['reader/web', 'reader/app/tests', 'scripts/tests']) {
    for (const entry of readdirSync(resolve(repoRoot, directory), { withFileTypes: true })) {
      if (!entry.isFile() || !/\.test\.(?:mjs|ts)$/.test(entry.name)) continue;
      if (directory === 'reader/app/tests' && browserTests.has(entry.name)) continue;
      files.push(resolve(repoRoot, directory, entry.name));
    }
  }
  if (files.length === 0) throw new Error('No Node tests discovered');
  return files.sort();
}

export function runNodeTests(repoRoot = root, options = [], stdio = 'inherit') {
  const files = collectNodeTests(repoRoot);
  const env = { ...process.env };
  // This is a fresh runner, not a node:test worker inherited from our self-test.
  delete env.NODE_TEST_CONTEXT;
  const result = spawnSync(process.execPath, ['--test', ...options, ...files], {
    cwd: repoRoot, stdio, env,
  });
  if (result.error) throw result.error;
  return result.status ?? 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = runNodeTests(root, process.argv.slice(2)); }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
