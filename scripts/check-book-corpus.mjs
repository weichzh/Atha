// Explicit, content-free corpus checks over the product's LocalLibrary and BookRoot.
import { constants, openSync, closeSync, fstatSync, lstatSync, readFileSync, readSync, existsSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, extname, isAbsolute, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const sourceLimits = new Map([
  ['.epub', 512], ['.cbz', 512], ['.fbz', 512], ['.fb2', 64],
  ['.mobi', 256], ['.azw', 256], ['.azw3', 256],
  ['.md', 16], ['.markdown', 16], ['.txt', 512],
]);
const phases = new Set(['setup', 'stage', 'open', 'manifest', 'section', 'resource', 'cover', 'duplicate', 'reopen', 'complete']);
const MiB = 1024 * 1024;
function record(value, keys) {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}

export function parseManifest(text, directory) {
  try {
    if (Buffer.byteLength(text) > 256 * 1024) throw Error();
    const manifest = JSON.parse(text);
    if (!record(manifest, ['schema', 'samples']) || manifest.schema !== 1
      || !Array.isArray(manifest.samples) || manifest.samples.length < 1 || manifest.samples.length > 64) throw Error();
    const ids = new Set();
    return manifest.samples.map(sample => {
      if (!record(sample, ['id', 'path', 'sha256']) || typeof sample.id !== 'string'
        || !/^book-[0-9]{3}$/.test(sample.id) || ids.has(sample.id)
        || typeof sample.path !== 'string' || sample.path.length < 1 || sample.path.length > 4096
        || /[\x00-\x1f\x7f]/.test(sample.path)
        || typeof sample.sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(sample.sha256)) throw Error();
      ids.add(sample.id);
      return { id: sample.id, path: resolve(directory, sample.path), sha256: sample.sha256 };
    });
  } catch { throw new Error('invalid-corpus-manifest'); }
}

export function sourceHash(path) {
  const limit = sourceLimits.get(extname(path).toLowerCase());
  if (!limit) throw new Error('unsupported-source');
  let fd;
  try {
    if (!lstatSync(path).isFile()) throw new Error('unavailable-source');
    fd = openSync(path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0) | (constants.O_NONBLOCK ?? 0));
    const stat = fstatSync(fd);
    if (!stat.isFile() || stat.size === 0) throw new Error('unavailable-source');
    if (stat.size > limit * MiB) throw new Error('source-too-large');
    const hash = createHash('sha256'), buffer = Buffer.alloc(64 * 1024);
    let total = 0;
    for (;;) {
      const count = readSync(fd, buffer, 0, buffer.length, null);
      if (!count) break;
      total += count;
      if (total > limit * MiB) throw new Error('source-too-large');
      hash.update(buffer.subarray(0, count));
    }
    return hash.digest('hex');
  } catch (error) {
    if (['unavailable-source', 'source-too-large'].includes(error.message)) throw error;
    throw new Error('unavailable-source');
  } finally { if (fd !== undefined) closeSync(fd); }
}

export function parseProbe(result) {
  if (result.error || result.signal) {
    return { status: 'failed', phase: 'process', code: result.error?.code === 'ETIMEDOUT' ? 'timeout' : 'probe-process-failed' };
  }
  try {
    const lines = String(result.stdout ?? '').split(/\r?\n/).filter(line => line.startsWith('book_probe='));
    if (lines.length !== 1) throw Error();
    const value = JSON.parse(lines[0].slice('book_probe='.length));
    if (!record(value, ['schema', 'status', 'phase', 'code', 'sections', 'resources']) || value.schema !== 1
      || !['passed', 'failed'].includes(value.status) || !phases.has(value.phase)
      || !Number.isInteger(value.sections) || value.sections < 0 || value.sections > 2000
      || !Number.isInteger(value.resources) || value.resources < 0 || value.resources > 10000) throw Error();
    if (value.status === 'passed') {
      if (result.status !== 0 || value.phase !== 'complete' || value.code !== 'ok' || value.sections === 0) throw Error();
      return { status: 'passed', phase: 'complete', code: 'ok', sections: value.sections, resources: value.resources };
    }
    // Backend errors are fixed codes, but never echo arbitrary child strings.
    return { status: 'failed', phase: value.phase, code: 'book-check-failed' };
  } catch { return { status: 'failed', phase: 'process', code: 'invalid-probe-result' }; }
}

export function runSample(sample, probeCommand, timeoutSeconds = 90) {
  let workRoot, outcome;
  try {
    if (sourceHash(sample.path) !== sample.sha256) return { id: sample.id, status: 'failed', phase: 'source', code: 'hash-mismatch' };
    const temporary = resolve(root, '.tmp');
    mkdirSync(temporary, { recursive: true });
    if (!lstatSync(temporary).isDirectory()) throw new Error('invalid-test-root');
    workRoot = mkdtempSync(resolve(temporary, 'corpus-run-'));
    const env = { ...process.env, ATHA_CORPUS_SOURCE: sample.path, ATHA_CORPUS_WORK_ROOT: workRoot, RUST_BACKTRACE: '0' };
    delete env.NODE_TEST_CONTEXT;
    const result = spawnSync(probeCommand[0], probeCommand.slice(1), {
      cwd: root, env, encoding: 'utf8', timeout: timeoutSeconds * 1000,
      maxBuffer: 4 * MiB, windowsHide: true,
    });
    outcome = parseProbe(result);
    if (sourceHash(sample.path) !== sample.sha256) outcome = { status: 'failed', phase: 'source', code: 'source-changed' };
  } catch (error) {
    const code = ['unsupported-source', 'unavailable-source', 'source-too-large', 'invalid-test-root'].includes(error.message)
      ? error.message : 'probe-process-failed';
    outcome = { status: 'failed', phase: 'source', code };
  } finally {
    // The child cannot run Rust Drop after a hard timeout. Only remove the
    // directory this invocation created, never a manifest-provided data root.
    if (workRoot) {
      try { rmSync(workRoot, { recursive: true, force: true }); }
      catch { outcome = { status: 'failed', phase: 'cleanup', code: 'cleanup-failed' }; }
    }
  }
  return { id: sample.id, ...outcome };
}

export function buildProbe() {
  const env = { ...process.env };
  const cargoHome = env.CARGO_HOME || resolve(env.HOME || env.USERPROFILE, '.cargo');
  const cargo = resolve(cargoHome, 'bin', process.platform === 'win32' ? 'cargo.exe' : 'cargo');
  const command = existsSync(cargo) ? cargo : 'cargo';
  const built = spawnSync(command, ['test', '--release', '--locked', '--offline', '-p', 'atha-backend',
    '--test', 'book_corpus', '--no-run', '--message-format=json'], {
    cwd: root, env, encoding: 'utf8', timeout: 600_000, maxBuffer: 4 * MiB, windowsHide: true,
  });
  if (built.error || built.status !== 0) throw new Error('corpus-build-failed');
  const artifacts = String(built.stdout).split(/\r?\n/).flatMap(line => {
    try {
      const item = JSON.parse(line);
      return item.reason === 'compiler-artifact' && item.target?.name === 'book_corpus'
        && item.profile?.test === true && typeof item.executable === 'string' ? [item.executable] : [];
    } catch { return []; }
  });
  if (artifacts.length !== 1 || !isAbsolute(artifacts[0])) throw new Error('corpus-build-failed');
  return [artifacts[0], '--ignored', '--exact', 'private_book_probe', '--nocapture'];
}

export function parseArguments(args) {
  let manifest, timeoutSeconds = 90;
  for (let i = 0; i < args.length; i += 2) {
    if (args[i] === '--manifest' && manifest === undefined && args[i + 1]) manifest = resolve(args[i + 1]);
    else if (args[i] === '--timeout-seconds' && /^[0-9]+$/.test(args[i + 1] ?? '')) timeoutSeconds = Number(args[i + 1]);
    else throw new Error('invalid-corpus-arguments');
  }
  if (!manifest || !Number.isInteger(timeoutSeconds) || timeoutSeconds < 1 || timeoutSeconds > 600) throw new Error('invalid-corpus-arguments');
  return { manifest, timeoutSeconds };
}

export function main(args) {
  if (args.length === 1 && args[0] === '--help') {
    console.log('Usage: node scripts/check-book-corpus.mjs --manifest PATH [--timeout-seconds 90]\nExplicit local files only; isolated importer checks, not GUI or whole-library acceptance.');
    return 0;
  }
  try {
    const { manifest, timeoutSeconds } = parseArguments(args);
    const stat = lstatSync(manifest);
    if (!stat.isFile() || stat.size > 256 * 1024) throw new Error('invalid-corpus-manifest');
    const samples = parseManifest(readFileSync(manifest, 'utf8'), dirname(manifest));
    // Reject unavailable or changed inputs before spending time on compilation.
    const ready = [], outcomes = [];
    for (const sample of samples) {
      try {
        if (sourceHash(sample.path) !== sample.sha256) throw new Error('hash-mismatch');
        ready.push(sample);
      } catch (error) {
        outcomes.push({ id: sample.id, status: 'failed', phase: 'source', code: error.message });
      }
    }
    if (ready.length) {
      const command = buildProbe();
      for (const sample of ready) outcomes.push(runSample(sample, command, timeoutSeconds));
    }
    outcomes.sort((a, b) => a.id.localeCompare(b.id));
    const passed = outcomes.filter(item => item.status === 'passed').length;
    console.log(JSON.stringify({ schema: 1, evidence: 'isolated-importer', total: samples.length,
      passed, failed: samples.length - passed, results: outcomes }, null, 2));
    return passed === samples.length ? 0 : 1;
  } catch (error) {
    const code = ['invalid-corpus-arguments', 'invalid-corpus-manifest', 'corpus-build-failed'].includes(error.message)
      ? error.message : 'corpus-input-unavailable';
    console.error(JSON.stringify({ schema: 1, status: 'failed', code }));
    return 1;
  }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) process.exitCode = main(process.argv.slice(2));
