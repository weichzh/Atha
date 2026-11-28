import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, symlinkSync, existsSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const helper = fileURLToPath(new URL('../lib/reader-linux-preflight.sh', import.meta.url));
const runner = fileURLToPath(new URL('../check-reader-linux.sh', import.meta.url));
const ui = fileURLToPath(new URL('../lib/reader-linux-ui.sh', import.meta.url));
const linux = { skip: process.platform !== 'linux' };
function setup(t) {
  const root = mkdtempSync(join(tmpdir(), 'atha-driver-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const home = join(root, 'home'), bin = join(root, 'bin'), repo = join(root, 'repo with spaces');
  for (const path of [home, bin, repo]) mkdirSync(path);
  symlinkSync('/usr/bin/realpath', join(bin, 'realpath'));
  const env = { ...process.env, PATH: bin, HOME: home };
  delete env.ATHA_TAURI_DRIVER; delete env.CARGO_HOME;
  function executable(path, body = 'exit 0') {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, '#!/bin/sh\n' + body + '\n', { mode: 0o700 });
    return path;
  }
  function resolveDriver() {
    return spawnSync('/bin/bash', ['--noprofile', '--norc', '-c', 'readonly repo_root="$2" invocation_root="$3"; source "$1"; atha_reader_driver "$2" "$3"', 'driver-test', helper, repo, root], { env, encoding: 'utf8' });
  }
  return { root, home, bin, repo, env, executable, resolveDriver };
}
test('Driver preflight finds user Cargo installation outside PATH', linux, t => {
  const f = setup(t), expected = f.executable(join(f.home, '.cargo/bin/tauri-driver'));
  const result = f.resolveDriver(); assert.equal(result.status, 0, result.stderr); assert.equal(result.stderr, '', 'must coexist with readonly caller variables'); assert.equal(result.stdout.trim(), expected);
});
test('Driver preflight honors explicit relative path and rejects invalid overrides', linux, t => {
  const f = setup(t), expected = f.executable(join(f.root, 'explicit driver'));
  f.executable(join(f.bin, 'tauri-driver'));
  f.env.ATHA_TAURI_DRIVER = 'explicit driver'; assert.equal(f.resolveDriver().stdout.trim(), expected);
  for (const override of ['missing', f.repo]) {
    f.env.ATHA_TAURI_DRIVER = override;
    const result = f.resolveDriver(); assert.notEqual(result.status, 0); assert.match(result.stderr, /not an executable file/);
  }
});
test('Driver preflight prefers PATH and supports custom CARGO_HOME and legacy cache', linux, t => {
  const f = setup(t), legacy = f.executable(join(f.repo, '.tmp/tauri-driver/bin/tauri-driver'));
  assert.equal(f.resolveDriver().stdout.trim(), legacy);
  f.env.CARGO_HOME = join(f.root, 'custom cargo');
  const cargo = f.executable(join(f.env.CARGO_HOME, 'bin/tauri-driver'));
  assert.equal(f.resolveDriver().stdout.trim(), cargo);
  const path = f.executable(join(f.bin, 'tauri-driver'));
  assert.equal(f.resolveDriver().stdout.trim(), path);
});
test('Missing driver fails the full gate before build, fixture writes or GUI startup', linux, t => {
  const f = setup(t), touched = join(f.root, 'unexpected-side-effect');
  for (const command of ['dirname', 'uname']) symlinkSync('/usr/bin/' + command, join(f.bin, command));
  for (const command of ['base64', 'curl', 'jq', 'sha256sum', 'ss', 'systemctl', 'systemd-run', 'WebKitWebDriver', 'mise', 'python3', 'mkdir']) {
    f.executable(join(f.bin, command), `echo touched > '${touched}'; exit 99`);
  }
  f.env.ATHA_TAURI_DRIVER = join(f.root, 'missing-driver');
  for (const flags of [[], ['--software-compositing'], ['--cpu-painting'], ['--cpu-painting', '--software-compositing']]) {
    const result = spawnSync('/bin/bash', [runner, ...flags], { cwd: f.root, env: f.env, encoding: 'utf8' });
    assert.notEqual(result.status, 0); assert.match(result.stderr, /not an executable file/);
    assert.equal(existsSync(touched), false, result.stderr);
  }
});

test('Reading-memory fixture resets after optional reading without loosening the exact assertion', linux, () => {
  const main = readFileSync(runner, 'utf8');
  const call = main.indexOf('  verify_reading_memory\n');
  assert(call > main.indexOf('run_gesture_gate private'), 'optional real reading must precede the isolated memory fixture');
  assert(main.slice(main.lastIndexOf('for width in 360 1000; do', call), call).startsWith('for width in 360 1000; do'),
    'reseed in each reopened memory viewport');
  const source = readFileSync(ui, 'utf8');
  const start = source.indexOf('  statistics=$(jq -cn', source.indexOf('verify_reading_memory()'));
  const end = source.indexOf("  nav_tab '阅读记忆'", start);
  assert(start > 0 && end > start, 'seed the isolated fixture before opening memory');
  const memory = source.slice(start, source.indexOf('\nreader_progress()', end));
  assert(memory.includes('.recent == ["Atha FB2 Gate"]'), 'do not relax the one-book memory contract');
  const seed = source.slice(start, end);
  const result = spawnSync('/bin/bash', ['-c', `set -euo pipefail
public_id=${'1'.repeat(64)}
execute_sync() { printf '%s' "$2" >&2; printf true; }
seed_fixture() {
${seed}}
seed_fixture`], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  const [record] = JSON.parse(result.stderr);
  const statistics = JSON.parse(record);
  assert.equal(statistics.schema, 1);
  assert.equal(statistics.books.length, 1);
  assert.equal(statistics.books[0].contentVersion, '1'.repeat(64));
  assert.equal(statistics.books[0].durationMs, 90000);
});

for (const formalGate of [false, true]) test(`Native action timeout stops only its unit without cancellation in ${formalGate ? 'the formal gesture gate' : 'direct invocation'}`, linux, t => {
  const root = mkdtempSync(join(tmpdir(), 'atha-input-timeout-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const calls = join(root, 'calls');
  mkdirSync(join(root, '.tmp'));
  mkdirSync(join(root, 'run'));
  const source = readFileSync(runner, 'utf8');
  const body = (name, next) => source.slice(source.indexOf(`${name}() {`), source.indexOf(`${next}() {`));
  const publicGate = source.match(/^run_gesture_gate public$/m)?.[0];
  assert(publicGate, 'the formal public gesture gate must run in the current shell');
  const invoke = formalGate ? `
run_gesture_gate() {
  perform_pointer_action '{"x":10,"y":20}' tap || exit 1
}
${publicGate}` : `
status=0
perform_pointer_action '{"x":10,"y":20}' tap || status=$?
exit "$status"`;
  const result = spawnSync('/bin/bash', ['-c', `set -Eeuo pipefail
repo_root=$ATHA_TIMEOUT_TEST_ROOT
run_root=$repo_root/run
unit=atha-timeout-owned
session_id=owned-session
base_url=http://127.0.0.1:1
unit_started=1
pointer_action_timed_out=0
curl() {
  for request_arg in "$@"; do audit_last_request=$request_arg; done
  printf 'curl:%s\\n' "$audit_last_request" >> "$repo_root/calls"
  return 28
}
systemctl() { printf 'systemctl:%s\\n' "$*" >> "$repo_root/calls"; }
cleanup_gesture() { printf 'gesture-cancel\\n' >> "$repo_root/calls"; }
wait_for_script() { printf true; }
pointer_action_failure_diagnostic() { printf 'failure:%s\\n' "$2" >&2; }
${body('webdriver_request', 'webdriver_value')}
${body('webdriver_value', 'execute_script')}
${body('perform_pointer_action', 'pointer_action_failure_diagnostic')}
${body('cleanup', 'wait_for_script')}
trap cleanup EXIT
${invoke}
`], { env: { ...process.env, ATHA_TIMEOUT_TEST_ROOT: root }, encoding: 'utf8' });
  assert.equal(result.status, formalGate ? 1 : 28, result.stderr);
  assert.match(result.stderr, /failure:28/);
  const requests = readFileSync(calls, 'utf8').trim().split('\n');
  assert.equal(requests.filter(line => line.startsWith('curl:')).length, 1, 'do not issue another native command after the stalled action');
  assert(requests.includes('curl:http://127.0.0.1:1/session/owned-session/actions'));
  assert(requests.includes('systemctl:--user stop atha-timeout-owned.service'));
  assert(!requests.includes('gesture-cancel'));
  assert.equal(existsSync(join(root, 'run')), false, 'isolated run data must still be cleaned');
});
