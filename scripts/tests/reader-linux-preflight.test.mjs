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

test('Focus reading checks the reading measure and the two insets independently', linux, () => {
  const check = (record, requested = record.width) => {
    const main = readFileSync(runner, 'utf8');
    const viewport = main.slice(main.indexOf('viewport_width_predicate()'), main.indexOf('set_window_size()'));
    return spawnSync('/bin/bash', ['-ec', `
source "$1"
${viewport}
gdk_backend=wayland
jq -e "$(viewport_width_predicate "$2") and ($focus_geometry_predicate)" >/dev/null
`, 'focus-geometry', ui, String(requested)], { input: JSON.stringify(record), encoding: 'utf8' }).status;
  };
  const desktop = { width: 1280, page: 760, paddingLeft: 32, paddingRight: 32, filled: true, centered: true, overflow: false };
  assert.equal(check(desktop), 0);
  assert.equal(check({ ...desktop, page: 500 }), 1, 'centering an overly narrow desktop column is not sufficient');
  assert.equal(check({ ...desktop, paddingLeft: 16 }), 1, 'a desktop outer gutter cannot substitute for the left inset');
  assert.equal(check({ ...desktop, paddingRight: 48 }), 1, 'a centered page still needs the default right inset');
  assert.equal(check({ ...desktop, width: 360, page: 296 }), 0);
  assert.equal(check({ ...desktop, width: 360, page: 270 }), 1);
  assert.equal(check({ ...desktop, width: 1707, availableWidth: 1707 }, 1600), 0);
  assert.equal(check({ ...desktop, width: 1706, availableWidth: 1707 }, 1600), 1);
});

test('Switching workspace tools rejects a missing or moved position within one chapter', linux, () => {
  const check = (before, after) => spawnSync('/bin/bash', [
    '-ec', 'source "$1"; same_reader_position "$2" "$3"', 'reader-position', ui,
    JSON.stringify(before), JSON.stringify(after)
  ], { encoding: 'utf8' }).status;
  const book = 'a'.repeat(64), start = { section: 'ch1', offset: 221 };
  const state = { book, page: 2, pages: 4, locator: { schema: 1, contentVersion: book, start } };
  assert.equal(check(state, state), 0);
  assert.equal(check(state, { ...state, locator: { ...state.locator, start: { ...start, offset: 422 } } }), 1);
  assert.equal(check(state, { ...state, page: 3 }), 1, 'unchanged stored progress does not excuse a rendered page move');
  assert.equal(check({ locator: null }, { locator: null }), 1, 'two absent locators are not a stable reading position');
});

test('Original-source probe rejects a same-chapter off-page highlight even with the right quote', linux, () => {
  const loaded = spawnSync('/bin/bash', ['-ec', 'source "$1"; printf "%s" "$reader_highlight_script"', 'reader-highlight', ui], { encoding: 'utf8' });
  assert.equal(loaded.status, 0, loaded.stderr);
  const text = '青铜钟下的一只白鹭';
  const domRect = (left, top, width, height) => {
    const bounds = { x: left, y: top, left, top, width, height, right: left + width, bottom: top + height };
    return { ...bounds, toJSON: () => bounds };
  };
  const page = domRect(32, 46, 296, 668);
  const host = { closest: () => ({ getBoundingClientRect: () => page }) };
  const probe = (left, covered = false) => {
    const range = {
      startContainer: { getRootNode: () => ({ host }) }, toString: () => text,
      getClientRects: () => [domRect(left, 74, 133, 22)]
    };
    const document = { querySelector: () => host, elementFromPoint: () => covered ? {} : host };
    return new Function('document', 'CSS', 'innerWidth', 'innerHeight', `
      return function () { ${loaded.stdout} }(['atha-active-annotation'], ${JSON.stringify(text)});
    `)(document, { highlights: new Map([['atha-active-annotation', [range]]]) }, 360, 760);
  };
  assert.equal(probe(51).visible, true);
  const wrongPage = probe(395);
  assert.equal(wrongPage.matches, 1);
  assert.equal(wrongPage.rectCount, 1);
  assert.equal(wrongPage.visible, false);
  assert.equal(probe(51, true).visible, false, 'a covered range is not visible either');
});

for (const caseName of ['pointer', 'formal gesture gate', 'key', 'text', 'click']) test(`Native action timeout stops only its unit without cancellation: ${caseName}`, linux, t => {
  const root = mkdtempSync(join(tmpdir(), 'atha-input-timeout-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const calls = join(root, 'calls');
  mkdirSync(join(root, '.tmp'));
  mkdirSync(join(root, 'run'));
  mkdirSync(join(root, 'other-run'));
  writeFileSync(join(root, 'run', 'isolated-fixture'), 'owned');
  writeFileSync(join(root, 'other-run', 'isolated-fixture'), 'unrelated');
  const source = readFileSync(runner, 'utf8');
  const body = (name, next) => source.slice(source.indexOf(`${name}() {`), source.indexOf(`${next}() {`));
  const publicGate = source.match(/^run_gesture_gate public$/m)?.[0];
  assert(publicGate, 'the formal public gesture gate must run in the current shell');
  const input = {
    pointer: `perform_pointer_action '{"x":10,"y":20}' tap`,
    key: 'webdriver_key n',
    text: 'webdriver_send_keys .editor draft',
    click: 'webdriver_click_element_ref owned-element',
  };
  const invoke = caseName === 'formal gesture gate' ? `
run_gesture_gate() {
  perform_pointer_action '{"x":10,"y":20}' tap || exit 1
}
${publicGate}` : `
status=0
${input[caseName]} || status=$?
exit "$status"`;
  const result = spawnSync('/bin/bash', ['-c', `set -Eeuo pipefail
repo_root=$ATHA_TIMEOUT_TEST_ROOT
run_root=$repo_root/run
unit=atha-timeout-owned
session_id=owned-session
base_url=http://127.0.0.1:1
unit_started=1
native_action_timed_out=0
gdk_backend=wayland
curl() {
  for request_arg in "$@"; do audit_last_request=$request_arg; done
  local payload
  IFS= read -r payload
  printf 'curl:%s:%s\\n' "$audit_last_request" "$(jq -c '.actions[0].actions[1] // .text // null' <<<"$payload")" >> "$repo_root/calls"
  return 28
}
systemctl() { printf 'systemctl:%s\\n' "$*" >> "$repo_root/calls"; }
cleanup_gesture() { printf 'gesture-cancel\\n' >> "$repo_root/calls"; }
wait_for_script() { printf true; }
webdriver_element_id() { printf owned-element; }
${body('webdriver_request', 'webdriver_value')}
${body('webdriver_value', 'webdriver_native_action')}
${body('webdriver_native_action', 'execute_script')}
${body('webdriver_click_element_ref', 'capture_public_ui')}
${body('perform_pointer_action', 'pointer_action_failure_diagnostic')}
${body('pointer_action_failure_diagnostic', 'run_gesture_gate')}
${body('cleanup', 'wait_for_script')}
source "$1"
trap cleanup EXIT
${invoke}
`, 'native-timeout', ui], { env: { ...process.env, ATHA_TIMEOUT_TEST_ROOT: root }, encoding: 'utf8' });
  assert.equal(result.status, caseName === 'formal gesture gate' ? 1 : 28, result.stderr);
  if (caseName === 'pointer' || caseName === 'formal gesture gate') {
    const diagnosis = JSON.parse(result.stderr);
    assert.equal(diagnosis.exitCode, 28);
    assert.equal(diagnosis.probe.probeResponded, false);
  }
  const requests = readFileSync(calls, 'utf8').trim().split('\n');
  const endpoint = { text: 'element/owned-element/value', click: 'element/owned-element/click' }[caseName] ?? 'actions';
  const lastAction = { key: '{"type":"keyUp","value":"n"}', text: '"draft"', click: 'null' }[caseName] ?? '{"type":"pointerDown","button":0}';
  assert.deepEqual(requests, [
    `curl:http://127.0.0.1:1/session/owned-session/${endpoint}:${lastAction}`,
    'systemctl:--user stop atha-timeout-owned.service',
    'systemctl:--user reset-failed atha-timeout-owned.service'
  ], 'do not send cancellation or touch other units after the stalled action');
  assert.equal(existsSync(join(root, 'run')), false, 'isolated run data must still be cleaned');
  assert.equal(readFileSync(join(root, 'other-run', 'isolated-fixture'), 'utf8'), 'unrelated');
});
