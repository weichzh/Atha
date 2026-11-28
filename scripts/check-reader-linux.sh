#!/usr/bin/env bash
# Description: Exercise the current Linux Tauri reader against an isolated seeded library.
# shellcheck disable=SC2016 # Embedded JavaScript template literals are intentionally single-quoted.

set -Eeuo pipefail

readonly invocation_root=$PWD
script_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
readonly script_root
repo_root=$(cd -- "$script_root/.." && pwd -P)
readonly repo_root

formula_epub=
formula_metadata=
minimum_formulas=1000
minimum_pages=10
gesture_warmups=5
gesture_measurements=20
external_application=
association_launch=0
software_compositing=0
cpu_painting=0
shelf_only=0

usage() {
  printf '%s\n' \
    'Usage: scripts/check-reader-linux.sh [options]' \
    '' \
    'Options:' \
    '  --shelf                     Check the current shelf-to-reader round trip only.' \
    '  --formula-epub PATH          Add one private formula-heavy EPUB.' \
    '  --formula-metadata PATH      JSON sidecar with source_sha256 and entry.' \
    '  --minimum-formulas N         Required formula count (default: 1000).' \
    '  --minimum-pages N            Required formula-section pages (default: 10).' \
    '  --gesture-warmups N          Warmups per gesture scenario (default: 5).' \
    '  --gesture-measurements N     Measurements per scenario (default: 20).' \
    '  --application PATH           Run this candidate executable instead of building debug.' \
    '  --association-launch         Start the candidate with the public fixture path.' \
    '  --software-compositing       Disable accelerated compositing for this isolated run.' \
    '  --cpu-painting               Request CPU Skia painting for this isolated run.' \
    '  -h, --help                   Show this help.'
}

die() {
  printf 'check-reader-linux: %s\n' "$1" >&2
  exit 1
}

say() {
  printf '[reader-linux] %s\n' "$1" >&2
}

need_value() {
  (($# >= 2)) || die "missing value for $1"
}

while (($#)); do
  case "$1" in
    --shelf)
      shelf_only=1
      shift
      ;;
    --formula-epub)
      need_value "$@"
      formula_epub=$2
      shift 2
      ;;
    --formula-metadata)
      need_value "$@"
      formula_metadata=$2
      shift 2
      ;;
    --minimum-formulas)
      need_value "$@"
      minimum_formulas=$2
      shift 2
      ;;
    --minimum-pages)
      need_value "$@"
      minimum_pages=$2
      shift 2
      ;;
    --gesture-warmups)
      need_value "$@"
      gesture_warmups=$2
      shift 2
      ;;
    --gesture-measurements)
      need_value "$@"
      gesture_measurements=$2
      shift 2
      ;;
    --application)
      need_value "$@"
      external_application=$2
      shift 2
      ;;
    --association-launch)
      association_launch=1
      shift
      ;;
    --software-compositing)
      software_compositing=1
      shift
      ;;
    --cpu-painting)
      cpu_painting=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *) die "unknown option: $1" ;;
  esac
done

validate_integer() {
  local name=$1 value=$2 minimum=$3 maximum=$4
  [[ $value =~ ^[0-9]+$ ]] || die "$name must be an integer"
  ((value >= minimum && value <= maximum)) || die "$name must be between $minimum and $maximum"
}

validate_integer minimum-formulas "$minimum_formulas" 1 100000
validate_integer minimum-pages "$minimum_pages" 1 10000
validate_integer gesture-warmups "$gesture_warmups" 0 20
validate_integer gesture-measurements "$gesture_measurements" 1 100

if [[ -n $formula_epub || -n $formula_metadata ]]; then
  [[ -n $formula_epub && -n $formula_metadata ]] ||
    die '--formula-epub and --formula-metadata must be provided together'
fi
((association_launch == 0)) || [[ -n $external_application ]] ||
  die '--association-launch requires --application'
if ((shelf_only)) && [[ -n $formula_epub || $association_launch == 1 ]]; then
  die '--shelf cannot be combined with the formula or file-association gates'
fi

resolve_input_file() {
  local value=$1 candidate
  if [[ $value == /* ]]; then
    candidate=$value
  else
    candidate=$invocation_root/$value
  fi
  [[ -f $candidate ]] || return 1
  realpath -e -- "$candidate"
}

for command in base64 curl jq mise realpath sha256sum ss systemctl systemd-run WebKitWebDriver; do
  command -v "$command" >/dev/null || die "$command is required"
done
[[ $(uname -s) == Linux ]] || die 'this gate requires Linux'

# Fail before builds, fixture writes, or graphical sessions.
source "$script_root/lib/reader-linux-preflight.sh"
driver=$(atha_reader_driver "$repo_root" "$invocation_root") || exit 1
graphics_environment=()
compositing_mode=system-default
painting_mode=system-default
if ((software_compositing)); then
  graphics_environment+=(--setenv=WEBKIT_DISABLE_COMPOSITING_MODE=1)
  compositing_mode=software
fi
if ((cpu_painting)); then
  graphics_environment+=(--setenv=WEBKIT_SKIA_ENABLE_CPU_RENDERING=1)
  painting_mode=cpu
fi

user_environment=$(systemctl --user show-environment) || die 'systemd user manager is unavailable'
grep -q '^XDG_RUNTIME_DIR=' <<<"$user_environment" || die 'systemd user manager lacks XDG_RUNTIME_DIR'
grep -q '^DBUS_SESSION_BUS_ADDRESS=' <<<"$user_environment" ||
  die 'systemd user manager lacks DBUS_SESSION_BUS_ADDRESS'
gdk_backend=${ATHA_GDK_BACKEND:-}
if [[ -n $gdk_backend ]]; then
  [[ $gdk_backend == wayland || $gdk_backend == x11 ]] || die 'ATHA_GDK_BACKEND must be wayland or x11'
  [[ $gdk_backend == wayland ]] && display_variable=WAYLAND_DISPLAY || display_variable=DISPLAY
  grep -q "^$display_variable=" <<<"$user_environment" || die "$gdk_backend display is unavailable"
elif grep -q '^WAYLAND_DISPLAY=' <<<"$user_environment"; then
  gdk_backend=wayland
elif grep -q '^DISPLAY=' <<<"$user_environment"; then
  gdk_backend=x11
else
  die 'systemd user manager has no graphical display'
fi

formula_entry=
formula_source_sha=
if [[ -n $formula_epub ]]; then
  formula_epub=$(resolve_input_file "$formula_epub") || die 'formula EPUB does not exist'
  formula_metadata=$(resolve_input_file "$formula_metadata") || die 'formula metadata does not exist'
  jq -e '
    (.source_sha256 | type == "string" and test("^[0-9a-f]{64}$")) and
    (.entry | type == "string" and length > 0)
  ' "$formula_metadata" >/dev/null || die 'formula metadata is invalid'
  formula_source_sha=$(jq -er '.source_sha256' "$formula_metadata")
  formula_entry=$(jq -er '.entry' "$formula_metadata")
  [[ $(sha256sum -- "$formula_epub" | awk '{print $1}') == "$formula_source_sha" ]] ||
    die 'formula EPUB does not match its metadata'
fi

cd -- "$repo_root"
mkdir -p .tmp
run_root=$(mktemp -d -p "$repo_root/.tmp" reader-linux.XXXXXXXX)
data_home=$run_root/data
library_root=$data_home/com.atha.reader
metrics_file=$run_root/gesture-metrics.jsonl
privacy_file=$run_root/private-tokens.txt
unit="atha-reader-linux-${BASHPID}-${RANDOM}"
session_id=
base_url=
unit_started=0
gesture_active=0
current_book_kind=
diagnostics_enabled=0
authoring_cycle_evidence=null
pointer_action_timed_out=0

webdriver_request() {
  local method=$1 path=$2 body=${3-}
  local arguments=(
    --silent --show-error --fail --max-time 30
    --request "$method"
    --header 'Accept: application/json'
  )
  if [[ -n $body ]]; then
    arguments+=(--header 'Content-Type: application/json' --data "$body")
  fi
  curl "${arguments[@]}" "$base_url$path"
}

webdriver_value() {
  local method=$1 path=$2 body=${3-} response
  response=$(webdriver_request "$method" "$path" "$body") || return "$?"
  if jq -e '
    (.value | type == "object") and
    (.value.error? != null) and
    (.value.message? != null)
  ' <<<"$response" >/dev/null; then
    return 1
  fi
  jq -c '.value' <<<"$response"
}

execute_script() {
  local endpoint=$1 script=$2 script_args=${3:-[]} payload
  payload=$(jq -cn --arg script "$script" --argjson args "$script_args" '{script: $script, args: $args}')
  webdriver_value POST "/session/$session_id/execute/$endpoint" "$payload"
}

execute_sync() {
  execute_script sync "$1" "${2:-[]}"
}

execute_async() {
  execute_script async "$1" "${2:-[]}"
}

webdriver_element_id() {
  local selector=$1 payload response
  payload=$(jq -cn --arg selector "$selector" '{using: "css selector", value: $selector}')
  response=$(webdriver_request POST "/session/$session_id/element" "$payload") || return 1
  jq -er '.value["element-6066-11e4-a52e-4f735466cecf"]' <<<"$response"
}

webdriver_click_element_ref() {
  local element_id=$1
  webdriver_value POST "/session/$session_id/element/$element_id/click" '{}' >/dev/null
}

webdriver_click_element() {
  local element_id
  element_id=$(webdriver_element_id "$1") || return 1
  webdriver_click_element_ref "$element_id"
}

webdriver_send_keys() {
  local selector=$1 text=$2 element_id payload
  element_id=$(webdriver_element_id "$selector") || return 1
  payload=$(jq -cn --arg text "$text" '{text: $text, value: ($text | explode | map([.] | implode))}')
  webdriver_value POST "/session/$session_id/element/$element_id/value" "$payload" >/dev/null
}

capture_public_ui() {
  [[ -z $formula_epub ]] || return 0
  local dimensions image_dir="$repo_root/artifacts/local/reader-linux-ui/$unit"
  wait_for_script \
    'const controls = document.querySelector(".reader-controls"); const style = controls && getComputedStyle(controls); return { settled: (!style || style.visibility === "hidden" || style.opacity === "1") && document.getAnimations().every(animation => animation.playState !== "running" || animation.effect.getTiming().iterations === Infinity) };' \
    '.settled == true' 'reader controls did not finish appearing before screenshot' >/dev/null
  dimensions=$(execute_sync 'return `${innerWidth}x${innerHeight}`;' | jq -r .)
  mkdir -p -- "$image_dir"
  webdriver_value GET "/session/$session_id/screenshot" | jq -r . | base64 --decode >"$image_dir/$1-$dimensions.png"
}

cleanup_gesture() {
  [[ -n $session_id && $gesture_active == 1 ]] || return 0
  local script result
  read -r -d '' script <<'JS' || true
const done = arguments[arguments.length - 1];
globalThis.__athaReaderDiagnostics.cleanupGestureProbe()
  .then((value) => done({ ok: true, value }))
  .catch(() => done({ ok: false }));
JS
  result=$(execute_async "$script" 2>/dev/null) || true
  gesture_active=0
  [[ -n $result ]] && jq -e '.ok == true' <<<"$result" >/dev/null
}

cleanup() {
  local status=$?
  trap - EXIT INT TERM
  set +e
  # A pending native mouse command can crash WebKitWebDriver when cancelled.
  # Stop this run's unit directly after a transport timeout instead.
  if ((pointer_action_timed_out == 0)); then
    cleanup_gesture >/dev/null 2>&1
    if [[ -n $session_id ]]; then
      webdriver_request DELETE "/session/$session_id" >/dev/null 2>&1
    fi
  fi
  if ((unit_started)); then
    systemctl --user stop "$unit.service" >/dev/null 2>&1
  fi
  systemctl --user reset-failed "$unit.service" >/dev/null 2>&1
  rm -rf -- "$run_root"
  rm -rf -- "$repo_root/.tmp/fb2-gate.fb2" "$repo_root/.tmp/fb2-gate-imports"
  exit "$status"
}
trap cleanup EXIT INT TERM

wait_for_script() {
  local script=$1 predicate=$2 label=$3
  local deadline=$((SECONDS + 30)) value last_value=
  while ((SECONDS < deadline)); do
    if value=$(execute_sync "$script" 2>/dev/null); then
      last_value=$value
      if jq -e "$predicate" <<<"$value" >/dev/null; then
        printf '%s\n' "$value"
        return 0
      fi
      if jq -e '.status == "fail"' <<<"$value" >/dev/null; then
        die "$label (state: $(jq -c . <<<"$value"))"
      fi
    fi
    sleep 0.1
  done
  if [[ -n $last_value ]]; then
    die "$label (state: $(jq -c . <<<"$last_value"))"
  fi
  die "$label (WebDriver script unavailable)"
}

pick_free_port() {
  local attempt port
  for ((attempt = 0; attempt < 100; attempt += 1)); do
    port=$((20000 + (RANDOM * 2 + BASHPID + attempt) % 40000))
    if ! ss -H -ltn "sport = :$port" | grep -q .; then
      printf '%s\n' "$port"
      return 0
    fi
  done
  die 'could not reserve a WebDriver port'
}

start_authoring_session() {
  local launch_attempt driver_port native_port attempt driver_ready=0 payload session_response
  session_id=
  for ((launch_attempt = 1; launch_attempt <= 3; launch_attempt += 1)); do
    driver_port=$(pick_free_port)
    native_port=$(pick_free_port)
    while [[ $native_port == "$driver_port" ]]; do native_port=$(pick_free_port); done
    base_url=http://127.0.0.1:$driver_port
    unit="atha-reader-linux-${BASHPID}-${RANDOM}-authoring-${launch_attempt}"

    if systemd-run --user --quiet --collect --unit="$unit" --working-directory="$repo_root" \
      --setenv="XDG_DATA_HOME=$data_home" --setenv="GDK_BACKEND=$gdk_backend" \
      --setenv=ATHA_READER_GUI_GATE=1 \
      "${graphics_environment[@]}" \
      "$driver" --port "$driver_port" --native-port "$native_port"; then
      unit_started=1
      for ((attempt = 0; attempt < 100; attempt += 1)); do
        if curl --silent --fail --max-time 1 "$base_url/status" >/dev/null; then
          driver_ready=1
          break
        fi
        sleep 0.1
      done
    fi
    ((driver_ready)) && break
    systemctl --user stop "$unit.service" >/dev/null 2>&1 || true
    systemctl --user reset-failed "$unit.service" >/dev/null 2>&1 || true
    unit_started=0
  done
  ((driver_ready)) || die 'authoring tauri-driver did not become ready'

  payload=$(jq -cn --arg application "$application" '{
    capabilities: {alwaysMatch: {"tauri:options": {application: $application}}}
  }')
  session_response=$(webdriver_request POST /session "$payload") ||
    die 'tauri-driver could not create the isolated authoring session'
  session_id=$(jq -er '.value.sessionId' <<<"$session_response") ||
    die 'tauri-driver returned no isolated authoring session'
  browser_version=$(jq -r '.value.capabilities.browserVersion // "unknown"' <<<"$session_response")
  webdriver_value POST "/session/$session_id/window/rect" '{"width":800,"height":600}' >/dev/null ||
    die 'could not initialize the isolated authoring window'
  wait_for_script \
    'return { visibility: document.visibilityState, width: innerWidth, height: innerHeight };' \
    '.visibility == "visible" and .width > 0 and .height > 0' \
    'isolated authoring GUI did not expose a usable viewport' >/dev/null
}

set_window_size() {
  local width=$1 height=$2 state was_reader saved_book saved_diagnostics viewport_application payload response
  state=$(execute_sync 'return { width: innerWidth, height: innerHeight, reader: location.pathname.endsWith("/index.html") };') ||
    die 'could not inspect the current reader viewport'
  if jq -e --argjson width "$width" --argjson height "$height" \
    '.width >= ($width - 2) and .width <= ($width + 2) and .height >= ($height - 64) and .height <= ($height + 2)' \
    <<<"$state" >/dev/null; then
    return 0
  fi

  was_reader=$(jq -r '.reader' <<<"$state")
  saved_book=$current_book_kind
  saved_diagnostics=$diagnostics_enabled
  [[ $was_reader != true || -n $saved_book ]] || die 'cannot restore an unidentified reader session'

  webdriver_request DELETE "/session/$session_id" >/dev/null || die 'could not close the Tauri session'
  session_id=
  viewport_application=$(mktemp -p "$run_root" reader-viewport.XXXXXXXX)
  printf '#!/usr/bin/env bash\nexport ATHA_READER_GUI_VIEWPORT=%q\nexec %q "$@"\n' \
    "${width}x${height}" "$application" >"$viewport_application"
  chmod 700 "$viewport_application"
  payload=$(jq -cn --arg application "$viewport_application" '{
    capabilities: {alwaysMatch: {"tauri:options": {application: $application}}}
  }')
  response=$(webdriver_request POST /session "$payload") ||
    die "tauri-driver could not create the ${width}x${height} session"
  session_id=$(jq -er '.value.sessionId' <<<"$response") ||
    die "tauri-driver returned no ${width}x${height} session"

  local cards='.library-book-open'
  if ((shelf_only)); then
    cards='.shelf-catalog [data-shelf-open]'
  fi
  wait_for_script \
    "return { width: innerWidth, height: innerHeight, ready: document.readyState, library: Boolean(document.querySelector('.library-shell')), cards: document.querySelectorAll('$cards').length };" \
    ".width >= $((width - 2)) and .width <= $((width + 2)) and .height >= $((height - 64)) and .height <= $((height + 2)) and .ready == \"complete\" and .library == true and .cards == $expected_books" \
    "reader window did not reach ${width}x${height}" >/dev/null
  if [[ $was_reader == true ]]; then
    open_book "$saved_book"
    ((saved_diagnostics == 0)) || enable_diagnostics
    execute_sync 'window.dispatchEvent(new Event("resize")); return true;' >/dev/null ||
      die 'could not exercise the restored reader resize path'
  fi
}

open_library() {
  local expected_books=$1
  execute_sync "location.assign('tauri://localhost'); return true;" >/dev/null ||
    die 'could not return to the library'
  wait_for_script \
    'return { ready: document.readyState, cards: document.querySelectorAll(".library-book-open").length, enabled: [...document.querySelectorAll(".library-book-open")].filter((button) => !button.disabled).length };' \
    ".ready == \"complete\" and .cards == $expected_books and .enabled == $expected_books" \
    'seeded library did not become ready' >/dev/null
  current_book_kind=
  diagnostics_enabled=0
}

association_library_snapshot() {
  local arguments
  arguments=$(jq -cn --arg public "$public_id" --arg title "$association_extra_title" \
    '[$public, $title]')
  execute_async '
const publicId = arguments[0];
const extraTitle = arguments[1];
const done = arguments[arguments.length - 1];
window.__TAURI_INTERNALS__.invoke("list_library_books")
  .then((books) => done({
    ids: books.map((book) => book.id).sort(),
    count: books.length,
    publicPresent: books.some((book) => book.id === publicId),
    extraMatches: books.filter((book) => book.title === extraTitle).length,
  }))
  .catch(() => done({ ids: [], count: -1, publicPresent: false, extraMatches: 0 }));
' "$arguments"
}

repeat_association_launch() {
  local expected_ids=$1 payload response snapshot
  webdriver_request DELETE "/session/$session_id" >/dev/null ||
    die 'could not close the initial association session'
  session_id=
  sleep 0.5
  payload=$(jq -cn --arg application "$association_wrapper" '{
    capabilities: {alwaysMatch: {"tauri:options": {application: $application}}}
  }')
  response=$(webdriver_request POST /session "$payload") ||
    die 'tauri-driver could not create the repeated association session'
  session_id=$(jq -er '.value.sessionId' <<<"$response") ||
    die 'tauri-driver returned no repeated association session'
  wait_for_script \
    'const state = new URL(location.href).searchParams.get("state"); return { status: document.documentElement.dataset.status || null, error: document.documentElement.dataset.error || null, session: document.documentElement.dataset.sessionState || null, reader: location.pathname.endsWith("/index.html"), ready: document.documentElement.hasAttribute("data-reader-ready"), state };' \
    ".status == \"pass\" and .error == null and .session == \"layout-stable\" and .reader == true and .ready == true and .state == \"$public_book_key\"" \
    'repeated associated launch did not reopen the first public book' >/dev/null
  open_library "$association_book_count"
  snapshot=$(association_library_snapshot) || die 'could not inspect the repeated association library'
  jq -e --argjson count "$association_book_count" --argjson expected "$expected_ids" '
    .count == $count and .publicPresent == true and .extraMatches == 1 and .ids == $expected
  ' <<<"$snapshot" >/dev/null ||
    die 'repeated association launch changed the imported book set'
  association_evidence=$(jq '. + {repeatCountStable: true}' <<<"$association_evidence")
  normalize_association_library
}

verify_ordinary_launch() {
  local payload response
  sleep 0.5
  payload=$(jq -cn --arg application "$application" '{
    capabilities: {alwaysMatch: {"tauri:options": {application: $application}}}
  }')
  response=$(webdriver_request POST /session "$payload") ||
    die 'tauri-driver could not create the ordinary launch session'
  session_id=$(jq -er '.value.sessionId' <<<"$response") ||
    die 'tauri-driver returned no ordinary launch session'
  wait_for_script \
    'return { ready: document.readyState, library: Boolean(document.querySelector(".library-shell")), cards: document.querySelectorAll(".library-book-open").length, reader: location.pathname.endsWith("/index.html") };' \
    ".ready == \"complete\" and .library == true and .cards == $expected_books and .reader == false" \
    'ordinary candidate launch did not enter the library' >/dev/null
  webdriver_request DELETE "/session/$session_id" >/dev/null ||
    die 'could not close the ordinary launch session'
  session_id=
  association_evidence=$(jq '. + {ordinaryLaunchLibrary: true}' <<<"$association_evidence")
}

normalize_association_library() {
  local arguments state
  arguments=$(jq -cn --arg public "$public_id" --arg title "$association_extra_title" \
    '[$public, $title]')
  state=$(execute_async '
const publicId = arguments[0];
const extraTitle = arguments[1];
const done = arguments[arguments.length - 1];
(async () => {
  try {
    const books = await window.__TAURI_INTERNALS__.invoke("list_library_books");
    const extras = books.filter((book) => book.title === extraTitle);
    if (extras.length !== 1) {
      done({ ok: false, before: books.length, matches: extras.length });
      return;
    }
    const after = await window.__TAURI_INTERNALS__.invoke("remove_library_book", {
      id: extras[0].id,
    });
    done({
      ok: true,
      before: books.length,
      matches: extras.length,
      after: after.length,
      publicPresent: after.some((book) => book.id === publicId),
      extraRemoved: !after.some((book) => book.id === extras[0].id),
    });
  } catch {
    done({ ok: false });
  }
})();
' "$arguments") || die 'could not normalize the association fixture'
  jq -e --argjson before "$association_book_count" --argjson after "$expected_books" '
    .ok == true and .before == $before and .matches == 1 and .after == $after and
    .publicPresent == true and .extraRemoved == true
  ' <<<"$state" >/dev/null ||
    die "multi-file association import was incomplete (state: $(jq -c . <<<"$state"))"
  execute_sync 'location.reload(); return true;' >/dev/null ||
    die 'could not reload the normalized association library'
  wait_for_script \
    'return { ready: document.readyState, library: Boolean(document.querySelector(".library-shell")), cards: document.querySelectorAll(".library-book-open").length };' \
    ".ready == \"complete\" and .library == true and .cards == $expected_books" \
    'normalized association library did not become ready' >/dev/null
}

fire_tauri_drag_event() {
  local event=$1 path=${2-} arguments
  arguments=$(jq -cn --arg event "$event" --arg path "$path" '[$event, $path]')
  execute_sync '
const event = arguments[0];
const path = arguments[1];
const listeners = globalThis.__internal_unstable_listeners_object_id__?.[event] || {};
const ids = Object.getOwnPropertyNames(listeners).map(Number);
const emit = globalThis.__internal_unstable_listeners_function_id__;
if (typeof emit !== "function") return { listeners: ids.length, emitted: false };
const payload = event === "tauri://drag-leave"
  ? null
  : { paths: path ? [path] : [], position: { x: 12, y: 12 } };
emit({ event, payload }, ids);
return { listeners: ids.length, emitted: true };
' "$arguments"
}

verify_library_entry() {
  local unsupported_file state removal boundary
  unsupported_file=$run_root/unsupported.pdf
  printf 'not a supported book\n' >"$unsupported_file"
  printf '%s\n' "$unsupported_file" 'unsupported.pdf' >>"$privacy_file"
  wait_for_script \
    'const events = ["tauri://drag-enter", "tauri://drag-over", "tauri://drag-drop", "tauri://drag-leave"]; const listeners = globalThis.__internal_unstable_listeners_object_id__ || {}; return { listeners: events.map((event) => Object.getOwnPropertyNames(listeners[event] || {}).length) };' \
    '.listeners == [1,1,1,1]' \
    'native drop listeners did not become ready' >/dev/null
  state=$(fire_tauri_drag_event 'tauri://drag-enter' "$repo_root/.tmp/fb2-gate.fb2") ||
    die 'could not synthesize drag enter'
  jq -e '.listeners == 1 and .emitted == true' <<<"$state" >/dev/null ||
    die "drag enter listener is unavailable (state: $(jq -c . <<<"$state"))"
  wait_for_script \
    'return { overlay: Boolean(document.querySelector(".library-drop-overlay")), busy: document.querySelector(".library-shell")?.getAttribute("aria-busy") };' \
    '.overlay == true and .busy == "false"' \
    'drag enter did not reveal the drop state' >/dev/null
  state=$(fire_tauri_drag_event 'tauri://drag-leave') || die 'could not synthesize drag leave'
  jq -e '.listeners == 1 and .emitted == true' <<<"$state" >/dev/null ||
    die 'drag leave listener is unavailable'
  wait_for_script \
    'return { overlay: Boolean(document.querySelector(".library-drop-overlay")) };' \
    '.overlay == false' \
    'drag leave did not clear the drop state' >/dev/null

  state=$(fire_tauri_drag_event 'tauri://drag-drop' "$repo_root/.tmp/fb2-gate.fb2") ||
    die 'could not synthesize duplicate drop'
  jq -e '.listeners == 1 and .emitted == true' <<<"$state" >/dev/null ||
    die 'drag drop listener is unavailable'
  wait_for_script \
    'return { cards: document.querySelectorAll(".library-book-open").length, overlay: Boolean(document.querySelector(".library-drop-overlay")), working: Boolean(document.querySelector(".library-work-overlay")), status: document.querySelector(".library-status")?.textContent || "" };' \
    ".cards == $expected_books and .overlay == false and .working == false and .status == \"已加入书架。\"" \
    'duplicate drop did not preserve the library' >/dev/null

  boundary=$(execute_async '
const done = arguments[arguments.length - 1];
const paths = Array.from({ length: 33 }, (_, index) => "/tmp/atha-boundary-" + index + ".epub");
window.__TAURI_INTERNALS__.invoke("import_library_paths", { paths })
  .then(() => done({ rejected: false }))
  .catch((error) => done({ rejected: true, code: String(error) }));
') || die 'could not probe the dropped-path boundary'
  jq -e '.rejected == true and .code == "invalid-library-import"' <<<"$boundary" >/dev/null ||
    die "dropped-path boundary was not enforced (state: $(jq -c . <<<"$boundary"))"

  if ((expected_books == 1)); then
    removal=$(execute_async "
const done = arguments[arguments.length - 1];
window.__TAURI_INTERNALS__.invoke('delete_library_book_data', { id: '$public_id' })
  .then(() => window.__TAURI_INTERNALS__.invoke('finish_library_book_deletion', { id: '$public_id' }))
  .then((books) => done({ ok: true, count: books.length }))
  .catch(() => done({ ok: false }));
") || die 'could not clear the isolated library'
    jq -e '.ok == true and .count == 0' <<<"$removal" >/dev/null ||
      die 'isolated library did not become empty'
    execute_sync 'location.reload(); return true;' >/dev/null || die 'could not reload the empty library'
    wait_for_script \
      'return { ready: document.readyState, empty: Boolean(document.querySelector(".library-empty h2")), cards: document.querySelectorAll(".library-book-open").length };' \
      '.ready == "complete" and .empty == true and .cards == 0' \
      'empty library did not become ready' >/dev/null

    fire_tauri_drag_event 'tauri://drag-enter' "$repo_root/.tmp/fb2-gate.fb2" >/dev/null ||
      die 'could not enter the empty-library drop state'
    wait_for_script \
      'return { overlay: Boolean(document.querySelector(".library-drop-overlay")) };' \
      '.overlay == true' \
      'empty-library drag enter did not reveal the drop state' >/dev/null
    fire_tauri_drag_event 'tauri://drag-leave' >/dev/null ||
      die 'could not leave the empty-library drop state'
    wait_for_script \
      'return { overlay: Boolean(document.querySelector(".library-drop-overlay")) };' \
      '.overlay == false' \
      'empty-library drag leave did not clear the drop state' >/dev/null
    fire_tauri_drag_event 'tauri://drag-enter' "$repo_root/.tmp/fb2-gate.fb2" >/dev/null ||
      die 'could not re-enter the empty-library drop state'
    fire_tauri_drag_event 'tauri://drag-drop' "$repo_root/.tmp/fb2-gate.fb2" >/dev/null ||
      die 'could not drop into the empty library'
    wait_for_script \
      'const image = document.querySelector(".library-cover img"); const bounds = image?.getBoundingClientRect(); const cover = image?.parentElement.getBoundingClientRect(); return { cards: document.querySelectorAll(".library-book-open").length, cover: Boolean(image?.complete && image.naturalWidth > 0), contained: Boolean(image && getComputedStyle(image).objectFit === "contain" && bounds.width > 0 && bounds.height > 0 && bounds.width <= cover.width && bounds.height <= cover.height), overlay: Boolean(document.querySelector(".library-drop-overlay")), working: Boolean(document.querySelector(".library-work-overlay")), status: document.querySelector(".library-status")?.textContent || "" };' \
      '.cards == 1 and .cover == true and .contained == true and .overlay == false and .working == false and .status == "已加入书架。"' \
      'drop did not add a prepared book with its cover to the empty library' >/dev/null
    execute_sync 'document.querySelector(".library-cover img")?.dispatchEvent(new Event("error")); return true;' >/dev/null ||
      die 'could not probe cover load failure'
    wait_for_script \
      'return { images: document.querySelectorAll(".library-cover img").length, fallback: document.querySelector(".library-cover-fallback strong")?.textContent || "", title: document.querySelector(".library-book-title")?.textContent || "" };' \
      '.images == 0 and .title != "" and .fallback == (.title | .[0:1] | ascii_upcase)' \
      'cover load failure did not reveal the generated title-glyph fallback' >/dev/null
  fi

  fire_tauri_drag_event 'tauri://drag-drop' "$unsupported_file" >/dev/null ||
    die 'could not synthesize an unsupported drop'
  wait_for_script \
    'const status = document.querySelector(".library-status")?.textContent || ""; return { cards: document.querySelectorAll(".library-book-open").length, overlay: Boolean(document.querySelector(".library-drop-overlay")), working: Boolean(document.querySelector(".library-work-overlay")), status, privateName: status.includes("unsupported.pdf") || status.includes("unsupported") || status.includes("/") };' \
    ".cards == $expected_books and .overlay == false and .working == false and .status == \"无法读取所选文件\" and .privateName == false" \
    'unsupported drop did not fail without changing the library' >/dev/null

  execute_sync 'location.reload(); return true;' >/dev/null || die 'could not reset the library after drop checks'
  wait_for_script \
    'const events = ["tauri://drag-enter", "tauri://drag-over", "tauri://drag-drop", "tauri://drag-leave"]; const listeners = globalThis.__internal_unstable_listeners_object_id__ || {}; return { ready: document.readyState, cards: document.querySelectorAll(".library-book-open").length, listeners: events.map((event) => Object.getOwnPropertyNames(listeners[event] || {}).length), pathInUrl: location.href.includes("fb2-gate") };' \
    ".ready == \"complete\" and .cards == $expected_books and .listeners == [1,1,1,1] and .pathInUrl == false" \
    'drop listeners leaked or the library URL exposed a path' >/dev/null

  library_entry_evidence=$(jq -n \
    --argjson emptyShelf "$([[ $expected_books == 1 ]] && printf true || printf false)" \
    '{events: ["enter", "leave", "drop"], duplicateStable: true, unsupportedRejected: true, pathBoundary: 32, emptyShelf: $emptyShelf, firstImportCover: $emptyShelf, syntheticTauriEvents: true}')
}

click_shelf_control() {
  local selector=$1 label=$2 arguments
  arguments=$(jq -cn --arg selector "$selector" '[$selector]')
  [[ $(execute_sync '
const button = document.querySelector(arguments[0]);
if (!button || button.disabled || !button.getClientRects().length || getComputedStyle(button).visibility !== "visible") return false;
button.focus({ preventScroll: true });
button.click();
return true;
' "$arguments") == true ]] || die "$label"
}

verify_current_shelf() {
  local input='main[aria-label="书架"] input[type=search]'
  local book='.shelf-catalog [data-shelf-open][title="Atha FB2 Gate"]'
  local point fixture="$run_root/ShelfCheckSecond.txt"
  set_window_size 360 760
  wait_for_script '
return { cards: document.querySelectorAll(".shelf-catalog [data-shelf-open]").length,
  title: document.querySelector(".shelf-catalog [data-shelf-open]")?.title };' \
    '.cards == 1 and .title == "Atha FB2 Gate"' 'current shelf did not load the public book' >/dev/null

  printf '%s\n' 'Synthetic native bookshelf check.' >"$fixture"
  printf '%s\n' "$fixture" 'ShelfCheckSecond' >>"$privacy_file"
  fire_tauri_drag_event 'tauri://drag-drop' "$fixture" >/dev/null || die 'could not submit the isolated import'
  wait_for_script '
return { titles: [...document.querySelectorAll(".shelf-catalog [data-shelf-open]")].map(book => book.title).sort() };
' '.titles == ["Atha FB2 Gate","ShelfCheckSecond"]' 'current shelf did not display the imported book' >/dev/null

  webdriver_click_element 'button[aria-label="筛选藏书"]' || die 'could not open shelf filters'
  webdriver_click_element '#shelf-fav-switch' || die 'could not enable the favorite filter'
  wait_for_script '
return { count: document.querySelectorAll(".shelf-catalog [data-shelf-open]").length,
  empty: Boolean(document.querySelector(".empty-shelf")) };' \
    '.count == 0 and .empty == true' 'favorite filter did not exclude both unstarred books' >/dev/null
  webdriver_click_element '.filter-popover .filter-footer button' || die 'could not clear shelf filters'
  webdriver_click_element 'button[aria-label="筛选藏书"]' || die 'could not close shelf filters'
  webdriver_click_element 'button[aria-label="清单列表"]' || die 'could not switch to the shelf list'
  [[ $(execute_sync '
const sort = [...document.querySelectorAll("[aria-label=书架排序] .sort-chip")]
  .find(button => button.textContent.trim() === "书名 Z–A");
sort?.click();
return Boolean(sort);') == true ]] || die 'could not select descending title order'
  wait_for_script '
return { titles: [...document.querySelectorAll(".book-list [data-shelf-open]")].map(book => book.title),
  overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth };' \
    '.titles == ["ShelfCheckSecond","Atha FB2 Gate"] and .overflow == false' \
    'current shelf list did not sort both books at the narrow width' >/dev/null
  webdriver_send_keys "$input" 'Atha FB2 Gate' || die 'could not type the book query'
  wait_for_script '
return { titles: [...document.querySelectorAll(".book-list [data-shelf-open]")].map(book => book.title) };
' '.titles == ["Atha FB2 Gate"]' 'shelf search did not isolate the public book' >/dev/null
  webdriver_click_element "$book" || die 'could not open the search result'
  wait_for_script '
return { ready: document.querySelector(".reader-root")?.dataset.screenReady === "true",
  opening: Boolean(document.querySelector(".opening-art, .opening-veil")),
  error: Boolean(document.querySelector(".reader-route [role=alert]")) };' \
    '.ready == true and .opening == false and .error == false' 'book did not reach its first reading screen' >/dev/null
  point=$(execute_sync '
const rect = document.querySelector(".reader").getBoundingClientRect();
return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };') ||
    die 'could not locate the reading surface'
  perform_pointer_action "$point" tap || die 'could not reveal reader controls'
  wait_for_script '
return { visible: Boolean(document.querySelector("#reader-back")?.getClientRects().length),
  book: document.querySelector(".island-book")?.textContent?.trim() };' \
    '.visible == true and .book == "Atha FB2 Gate"' 'reader controls did not identify the selected book' >/dev/null
  webdriver_click_element '#reader-back' || die 'could not return using the reader control'
  wait_for_script '
const result = document.querySelector(".book-list [data-shelf-open]");
return { visible: Boolean(result?.getClientRects().length), title: result?.title,
  query: document.querySelector("main[aria-label=书架] input[type=search]")?.value,
  sort: document.querySelector("[aria-label=书架排序] .sort-chip[aria-pressed=true]")?.textContent.trim(),
  focus: document.activeElement === result };' \
    '.visible == true and .title == "Atha FB2 Gate" and .query == .title and .sort == "书名 Z–A" and .focus == true' \
    'return did not restore search, sort, list, and focus' >/dev/null
}

click_book_menu_item() {
  local arguments
  arguments=$(jq -cn --arg label "$1" '[$label]')
  [[ $(execute_sync '
const item = [...document.querySelectorAll(".library-book-menu [role=menuitem]")]
  .find((button) => button.textContent.trim() === arguments[0]);
item?.click();
return Boolean(item);' "$arguments") == true ]] || die "book menu item is unavailable: $1"
}

long_press_shelf_book() {
  local arguments
  arguments=$(jq -cn --arg selector "$1" '[$selector]')
  [[ $(execute_async '
const done = arguments[arguments.length - 1];
const button = document.querySelector(arguments[0]);
if (!button) return done(false);
const rect = button.getBoundingClientRect();
const event = (type) => new PointerEvent(type, {bubbles: true, cancelable: true, pointerId: 91, pointerType: "touch", button: 0, clientX: rect.left + 20, clientY: rect.top + 20});
button.dispatchEvent(event("pointerdown"));
setTimeout(() => {
  button.dispatchEvent(event("pointerup"));
  done(true);
}, 560);' "$arguments") == true ]] || die 'could not long-press a shelf book'
}

choose_shelf_option() {
  local selector=$1 arguments
  click_shelf_control '.shelf-sort' 'could not open the shelf sort and view options'
  click_shelf_control ".shelf-dialog[open] $selector" "shelf option is unavailable: $selector"
  click_shelf_control '.shelf-sort' 'could not reopen the shelf sort and view options'
  arguments=$(jq -cn --arg selector "$selector" '[$selector]')
  [[ $(execute_sync 'return document.querySelector(`.shelf-dialog[open] ${arguments[0]}`)?.getAttribute("aria-pressed") === "true";' "$arguments") == true ]] ||
    die "shelf option did not activate: $selector"
  click_shelf_control '.shelf-dialog[open] button[aria-label="关闭"]' 'could not close the shelf options'
}

set_shelf_query() {
  local arguments
  arguments=$(jq -cn --arg query "$1" '[$query]')
  [[ $(execute_sync '
const input = document.querySelector(".shelf-search-field input");
if (!input || !input.getClientRects().length || input.disabled) return false;
input.value = arguments[0];
input.dispatchEvent(new Event("input", { bubbles: true }));
return true;' "$arguments") == true ]] || die 'could not set the shelf search query'
}

verify_library_layout() {
  local width=$1 height=$2
  choose_shelf_option 'button[aria-label="列表视图"]'
  wait_for_script '
const cards = [...document.querySelectorAll(".library-book")];
const valid = cards.every((card) => {
  const outer = card.querySelector(".library-book-open")?.getBoundingClientRect();
  const cover = card.querySelector(".library-cover")?.getBoundingClientRect();
  const title = card.querySelector(".library-book-title")?.getBoundingClientRect();
  const author = card.querySelector(".library-book-author")?.getBoundingClientRect();
  return Boolean(outer && cover && title && author && cover.left >= outer.left - 1 &&
    cover.right <= title.left + 1 && title.right <= outer.right + 1 &&
    author.right <= outer.right + 1 && title.top <= author.top && cover.bottom <= outer.bottom + 1);
});
const controls = [...document.querySelectorAll(".shelf-mast button, .shelf-mast input, .shelf-catalog-tools button")]
  .filter((control) => control.getClientRects().length);
const clipped = controls.filter((control) => control.scrollWidth > control.clientWidth + 1 || control.scrollHeight > control.clientHeight + 1);
return {
  width: innerWidth, list: document.querySelector(".library-shell")?.classList.contains("library-list-view"),
  cards: cards.length, valid, overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
  controls: controls.length, clippedControls: clipped.map((control) => control.getAttribute("aria-label") || control.textContent.trim())
};' \
    ".width >= $((width - 2)) and .width <= $((width + 2)) and .list == true and .cards == $expected_books and .valid == true and .overflow == false and .controls >= 6 and .clippedControls == []" \
    "library list layout failed at ${width}x${height}" >/dev/null

  choose_shelf_option 'button[aria-label="网格视图"]'
  wait_for_script \
    'const cards = [...document.querySelectorAll(".library-book")].map((card) => card.getBoundingClientRect()); const overlap = cards.some((left, index) => cards.slice(index + 1).some((right) => left.left < right.right && left.right > right.left && left.top < right.bottom && left.bottom > right.top)); return { width: innerWidth, grid: !document.querySelector(".library-shell")?.classList.contains("library-list-view"), cards: cards.length, overlap, overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth };' \
    ".width >= $((width - 2)) and .width <= $((width + 2)) and .grid == true and .cards == $expected_books and .overlap == false and .overflow == false" \
    "library grid layout failed at ${width}x${height}" >/dev/null

  if ((width == 1000)); then
    set_shelf_query 'Atha FB2 Gate'
    wait_for_script \
      'return { cards: document.querySelectorAll(".library-book-open").length, title: document.querySelector(".library-book-title")?.textContent || "" };' \
      '.cards == 1 and .title == "Atha FB2 Gate"' \
      'library search no longer filters the shared result set' >/dev/null
    set_shelf_query ''
    choose_shelf_option 'button[data-shelf-sort="title"]'
    wait_for_script \
      'return { cards: document.querySelectorAll(".library-book-open").length, continuous: Boolean(document.querySelector(".shelf-catalog > .library-grid")), letters: Boolean(document.querySelector(".shelf-jump")) };' \
      ".cards == $expected_books and .continuous == true and .letters == true" \
      'library title sort no longer uses the continuous catalogue' >/dev/null
    choose_shelf_option 'button[data-shelf-sort="progress"]'
    wait_for_script '
const groups = [...document.querySelectorAll(".library-progress-groups > section")];
return {
  labels: groups.map((group) => group.querySelector(".library-group-heading h3")?.textContent),
  counts: groups.map((group) => group.querySelectorAll(".library-book").length),
  publicStarted: Boolean(groups[0]?.querySelector(".library-book-open[title=\"Atha FB2 Gate\"]")),
  cards: document.querySelectorAll(".library-book-open").length
};' \
      ".labels == [\"已开始\",\"未开始\"] and .cards == $expected_books and (.counts | add) == $expected_books and .publicStarted == true" \
      'library progress grouping no longer uses the shared result set' >/dev/null
    choose_shelf_option 'button[data-shelf-sort="title"]'
  fi
}

verify_continuous_shelf() {
  local fixture=$run_root/ShelfGateRoundTrip.txt
  local fixture_id fixture_source card open_book_button state body_point
  printf '%s\n' 'Atha native shelf fixture body marker.' >"$fixture"
  printf '%s\n' "$fixture" 'ShelfGateRoundTrip' 'Atha native shelf fixture body marker.' \
    '原生验收标签' '原生验收主题' '原生验收系列' >>"$privacy_file"
  fire_tauri_drag_event 'tauri://drag-drop' "$fixture" >/dev/null ||
    die 'could not import the isolated shelf round-trip fixture'
  state=$(wait_for_script '
const cards = [...document.querySelectorAll(".library-book")];
const added = cards.find((card) => card.querySelector(".library-book-title")?.textContent === "ShelfGateRoundTrip");
return {cards: cards.length, fixture: added?.dataset.bookId || ""};' \
    ".cards == $((expected_books + 1)) and (.fixture | test(\"^[a-f0-9]{64}$\"))" \
    'the isolated shelf round-trip fixture did not import')
  fixture_id=$(jq -er '.fixture' <<<"$state")
  printf '%s\n' "$fixture_id" >>"$privacy_file"
  fixture_source=$(jq -er '.sourcePath | select(test("^[a-f0-9]{64}\\.txt$"))' "$library_root/Library/$fixture_id.json") ||
    die 'the shelf fixture has no staged source record'
  card=".library-book[data-book-id=\"$fixture_id\"]"
  open_book_button="$card .library-book-open"
  click_shelf_control "$card .shelf-book-actions" 'could not open the fixture book actions'
  click_book_menu_item '收藏'
  wait_for_script \
    'return {favorite: Boolean(document.querySelector(".library-book-open[title=ShelfGateRoundTrip][aria-label*=\"已收藏\"]")), menu: Boolean(document.querySelector(".library-book-menu"))};' \
    '.favorite == true and .menu == false' 'the direct favorite action did not update the fixture' >/dev/null
  click_shelf_control "$card .shelf-book-actions" 'could not reopen the fixture book actions'
  click_book_menu_item '编辑分类'
  [[ $(execute_sync '
const dialog = document.querySelector(".shelf-edit-dialog[open]");
const fields = {tags: "原生验收标签", subjects: "原生验收主题", series: "原生验收系列"};
for (const [name, value] of Object.entries(fields)) {
  const field = dialog?.querySelector(`textarea[name="${name}"]`);
  if (!field) return false;
  field.value = value;
  field.dispatchEvent(new Event("input", { bubbles: true }));
}
return true;') == true ]] || die 'could not edit the fixture classification'
  click_shelf_control '.shelf-edit-dialog[open] button[type=submit]' 'could not submit fixture organization'
  wait_for_script \
    'return {saved: document.querySelector(".library-status")?.textContent === "书籍分类已保存。", open: Boolean(document.querySelector(".shelf-edit-dialog[open]"))};' \
    '.saved == true and .open == false' 'fixture organization did not save' >/dev/null

  execute_sync 'location.reload(); return true;' >/dev/null || die 'could not reload the organized shelf'
  wait_for_script \
    'return {favorite: Boolean(document.querySelector(".library-book-open[title=ShelfGateRoundTrip][aria-label*=\"已收藏\"]")), tag: Boolean(document.querySelector(".shelf-quick button[title=\"标签：原生验收标签\"]"))};' \
    '.favorite == true and .tag == true' 'saved favorite and tag did not survive reload' >/dev/null
  click_shelf_control '.shelf-filter-button' 'could not open shelf filters'
  click_shelf_control '.shelf-dialog[open] [role="switch"]' 'could not enable the favorite filter'
  [[ $(execute_sync '
const tag = [...document.querySelectorAll(".shelf-dialog[open] [aria-label=\"标签\"] .shelf-chip")]
  .find((button) => button.querySelector("span")?.textContent === "原生验收标签");
tag?.click();
return Boolean(tag);') == true ]] || die 'could not enable the saved tag filter'
  click_shelf_control '.shelf-dialog[open] footer .shelf-dialog-confirm' 'could not apply the shelf filters'
  set_shelf_query 'Atha FB2 Gate'
  wait_for_script '
return {
  empty: Boolean(document.querySelector(".library-no-results")),
  cards: document.querySelectorAll(".library-book").length,
  conditions: document.querySelectorAll(".shelf-condition").length
};' '.empty == true and .cards == 0 and .conditions == 3' \
    'search ignored the active shelf conditions' >/dev/null
  click_shelf_control '.library-no-results button' 'could not expand the search to the whole shelf'
  wait_for_script '
return {
  title: document.querySelector(".library-book-title")?.textContent,
  query: document.querySelector(".shelf-search-field input")?.value,
  cards: document.querySelectorAll(".library-book").length,
  conditions: document.querySelectorAll(".shelf-condition").length
};' '.cards == 1 and .title == "Atha FB2 Gate" and .query == .title and .conditions == 1' \
    'expanded search did not keep the keyword and expose the book outside the filters' >/dev/null

  set_shelf_query '原生验收系列'
  click_shelf_control '.shelf-quick button[title="标签：原生验收标签"]' 'could not reapply the saved tag'
  choose_shelf_option 'button[data-shelf-sort="title-desc"]'
  choose_shelf_option 'button[aria-label="列表视图"]'
  [[ $(execute_sync 'return document.querySelectorAll(".library-book").length;') == 1 ]] ||
    die 'search by saved series did not isolate the fixture'
  webdriver_click_element "$open_book_button" || die 'could not open the filtered fixture'
  wait_for_script '
return {
  status: document.documentElement.dataset.status || null,
  ready: document.documentElement.hasAttribute("data-reader-ready"),
  book: new URL(location.href).searchParams.get("state") || ""
};' ".status == \"pass\" and .ready == true and .book == \"${fixture_id:0:16}\"" \
    'filtered fixture did not open its own reader session' >/dev/null
  body_point=$(execute_sync '
const rect = document.querySelector(".reader").getBoundingClientRect();
return {x: (rect.left + rect.right) / 2, y: (rect.top + rect.bottom) / 2};') ||
    die 'could not locate the reader center'
  perform_pointer_action "$body_point" tap || die 'could not reveal reader controls'
  wait_for_script 'const controls = getComputedStyle(document.querySelector(".reader-controls")); return {visible: controls.visibility === "visible" && controls.opacity === "1"};' \
    '.visible == true' 'the official reader back control did not become visible' >/dev/null
  # The reader intentionally keeps rendered book markup in a closed shadow
  # tree. Verify the distinctive body text through the reader's search UI.
  webdriver_click_element '.reader-tool.search > summary' || die 'could not open reader search'
  [[ $(execute_sync '
const form = document.querySelector(".reader-tool.search[open] #search-form");
const input = form?.querySelector("#search-query");
if (!input) return false;
input.value = "native shelf fixture body marker";
form.requestSubmit();
return true;') == true ]] || die 'could not search the opened fixture body'
  wait_for_script '
const items = [...document.querySelectorAll("#search-result-list button")];
return {matches: items.length, exactBody: items.some((item) => item.textContent.includes("Atha native shelf fixture body marker."))};
' '.matches == 1 and .exactBody == true' 'the opened reader did not find the distinctive fixture body text' >/dev/null
  webdriver_click_element '#reader-back' || die 'could not return using the official reader back control'
  wait_for_script '
const button = document.querySelector(".library-book-open[title=ShelfGateRoundTrip]");
return {
  query: document.querySelector(".shelf-search-field input")?.value,
  tag: document.querySelector(".shelf-quick button[title=\"标签：原生验收标签\"]")?.getAttribute("aria-pressed"),
  sort: document.querySelector(".shelf-sort")?.textContent.trim(),
  list: document.querySelector(".library-shell")?.classList.contains("library-list-view"),
  cards: document.querySelectorAll(".library-book").length,
  focused: Boolean(button && document.activeElement === button && !button.disabled),
  recent: Boolean(document.querySelector(".shelf-recent-book[title=ShelfGateRoundTrip]"))
};' '.query == "原生验收系列" and .tag == "true" and .sort == "书名 Z–A" and .list == true and .cards == 1 and .focused == true and .recent == true' \
    'reader return did not restore the shelf query, filter, sort, view, and focus' >/dev/null

  long_press_shelf_book '.shelf-recent-book[title=ShelfGateRoundTrip]'
  wait_for_script '
const menu = document.querySelector(".library-book-menu");
return {name: menu?.getAttribute("aria-label"), items: [...(menu?.querySelectorAll("[role=menuitem]") || [])].map((item) => item.textContent.trim())};
' '.name == "《ShelfGateRoundTrip》操作" and .items == ["打开","选择","取消收藏","编辑分类","更换封面","移出书架","删除本地数据"]' \
    'long press in Recent Reading did not open the same book menu' >/dev/null
  [[ $(execute_sync '
document.activeElement.dispatchEvent(new KeyboardEvent("keydown", {key: "Escape", bubbles: true}));
return !document.querySelector(".library-book-menu");') == true ]] || die 'could not close the recent book menu'

  click_shelf_control '.shelf-clear' 'could not reset the shelf conditions'
  choose_shelf_option 'button[data-shelf-sort="title"]'
  choose_shelf_option 'button[aria-label="网格视图"]'
  click_shelf_control "$card .shelf-book-actions" 'could not reopen the fixture book actions'
  [[ $(execute_sync '
const item = document.querySelector(".library-book-menu button.danger");
if (!item || item.textContent.trim() !== "删除本地数据") return false;
item.click();
return true;') == true ]] || die 'could not trigger deletion confirmation of the isolated fixture'
  wait_for_script '
const dialog = document.querySelector("[data-atha-confirm-dialog]");
const text = dialog?.textContent || "";
return {
  open: Boolean(dialog?.querySelector("[data-atha-confirm-accept]")),
  valid: text.includes("删除这 1 本书的本地文件和阅读状态") && text.includes("笔记、标注和原文快照会保留")
};' '.open == true and .valid == true' 'deletion confirmation dialog for isolated fixture did not appear' >/dev/null
  webdriver_click_element '[data-atha-confirm-accept]' || die 'could not confirm deletion of the isolated fixture'
  wait_for_script '
return {
  cards: document.querySelectorAll(".library-book").length,
  fixture: Boolean(document.querySelector("[data-shelf-open][title=ShelfGateRoundTrip]")),
  conditions: document.querySelectorAll(".shelf-condition").length,
  deleted: document.querySelector(".library-status")?.textContent === "已删除 1 本书的本地数据。"
};' ".cards == $expected_books and .fixture == false and .conditions == 0 and .deleted == true" \
    'the fixture survived local deletion or left the original shelf filtered' >/dev/null
  [[ ! -e $library_root/Library/$fixture_id.json && ! -e $library_root/ImportedBooks/$fixture_id &&
    ! -e $library_root/SourceBooks/$fixture_source ]] ||
    die 'local deletion left fixture library data behind'
  continuous_shelf_evidence='{"savedOrganization":true,"organizationAfterReload":true,"scopedSearch":true,"expandedSearch":true,"realReaderContent":true,"returnedQueryFilterSortViewFocus":true,"recentReadingLongPressMenu":true,"fixtureDeleted":true}'
}

verify_library_settings() {
  local settings theme storage context selection
  [[ $(execute_sync '
const button = [...document.querySelectorAll(".library-sections button")]
  .find((item) => item.textContent.trim() === "设置");
button?.click();
return Boolean(button);') == true ]] || die 'could not enter application settings'
  settings=$(wait_for_script '
const page = document.querySelector(".library-settings");
const controls = [...(page?.querySelectorAll("button, select") || [])];
return {
  heading: page?.querySelector("h1")?.textContent || "",
  labels: [...(page?.querySelectorAll("#local-data-settings-heading + .library-data-actions button") || [])].map((button) => button.textContent.trim()),
  dictionaryActions: [...(page?.querySelectorAll(".dictionary-settings-actions button") || [])].map((button) => button.textContent.trim()),
  readingDefault: Boolean([...page?.querySelectorAll("button") || []].find((button) => button.textContent.trim() === "恢复阅读默认")),
  fontScales: page?.querySelectorAll("select[aria-label=词典释义字号] option").length || 0,
  themeModes: [...(page?.querySelectorAll(".library-theme-control button") || [])].map((button) => button.textContent.trim()),
  themeChecked: page?.querySelector(".library-theme-control [aria-checked=true]")?.textContent.trim() || "",
  controls: controls.every((control) => control.getBoundingClientRect().height >= 40),
  search: Boolean(document.querySelector(".shelf-search-field")),
  legacyMenu: Boolean(document.querySelector(".library-management")),
  overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth
};' \
    '.heading == "设置" and .labels == ["备份资料库","恢复资料库","存储占用"] and .dictionaryActions == ["导入词典","移除当前词典"] and .readingDefault == true and .fontScales == 6 and .themeModes == ["跟随系统","浅色","深色"] and (.themeChecked == "跟随系统" or .themeChecked == "浅色" or .themeChecked == "深色") and .controls == true and .search == false and .legacyMenu == false and .overflow == false' \
    'application settings did not become ready') || die 'could not inspect application settings'
  jq -e '
    .heading == "设置" and .overflow == false and
    .labels == ["备份资料库", "恢复资料库", "存储占用"]
  ' <<<"$settings" >/dev/null || die "application settings are incomplete (state: $(jq -c . <<<"$settings"))"

  [[ $(execute_sync '
globalThis.__athaReaderPreferencesBeforeAppTheme = localStorage.getItem("atha.reader.application.v1");
const button = [...document.querySelectorAll(".library-theme-control button")]
  .find((item) => item.textContent.trim() === "浅色");
button?.click();
return Boolean(button);') == true ]] || die 'could not choose the light application theme'
  theme=$(wait_for_script '
const shell = document.querySelector(".library-shell");
const style = getComputedStyle(shell);
return {
  shell: shell?.dataset.appTheme || "",
  root: document.documentElement.dataset.appTheme || "",
  tone: document.documentElement.dataset.appTone || "",
  stored: JSON.parse(localStorage.getItem("atha.app.appearance.v1") || "null")?.theme || "",
  readerUnchanged: localStorage.getItem("atha.reader.application.v1") === globalThis.__athaReaderPreferencesBeforeAppTheme,
  background: style.backgroundColor,
  color: style.color,
  overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth
};' \
    '.shell == "light" and .root == "light" and .tone == "light" and .stored == "light" and .readerUnchanged == true and .background == "rgb(253, 252, 249)" and .color == "rgb(31, 29, 26)" and .overflow == false' \
    'independent application theme did not become ready') || die 'could not inspect the application theme'
  capture_public_ui settings
  [[ $(execute_sync '
const button = [...document.querySelectorAll(".library-theme-control button")]
  .find((item) => item.textContent.trim() === "跟随系统");
button?.click();
return Boolean(button);') == true ]] || die 'could not restore the system application theme'
  [[ $(execute_sync '
globalThis.__athaAppearanceBeforeReset = localStorage.getItem("atha.app.appearance.v1");
localStorage.setItem("atha.reader.application.v1", JSON.stringify({
  schema: 1,
  preferences: {theme: "paper", brightness: 90, fontSize: 24, fontFamily: "serif", density: "compact"}
}));
const button = [...document.querySelectorAll(".library-settings button")]
  .find((item) => item.textContent.trim() === "恢复阅读默认");
button?.click();
return Boolean(button);') == true ]] || die 'could not trigger reading defaults reset'
  wait_for_script \
    'return { open: Boolean(document.querySelector("[data-atha-confirm-dialog] [data-atha-confirm-accept]")) };' \
    '.open == true' 'reset reading defaults confirmation did not appear' >/dev/null
  webdriver_click_element '[data-atha-confirm-accept]' || die 'could not confirm reading defaults reset'
  wait_for_script '
const dialog = document.querySelector("[data-atha-confirm-dialog]");
const preferences = JSON.parse(localStorage.getItem("atha.reader.application.v1") || "null")?.preferences;
const sameAppearance = localStorage.getItem("atha.app.appearance.v1") === globalThis.__athaAppearanceBeforeReset;
if (!dialog && sameAppearance && JSON.stringify(preferences) === JSON.stringify({theme: "system", brightness: 100, fontSize: 19, fontFamily: "book", density: "standard"})) {
  delete globalThis.__athaAppearanceBeforeReset;
  return { done: true };
}
return { done: false };' '.done == true' 'reading defaults did not reset independently from application appearance' >/dev/null

  [[ $(execute_sync '
const button = [...document.querySelectorAll(".library-data-actions button")]
  .find((item) => item.textContent.includes("存储占用"));
button?.click();
return Boolean(button);') == true ]] || die 'could not open storage usage'
  storage=$(wait_for_script \
    'const dialog = document.querySelector(".library-storage-dialog"); const rows = [...(dialog?.querySelectorAll("dl > div") || [])]; return { open: Boolean(dialog?.open), rows: rows.length, labels: rows.map((row) => row.querySelector("dt")?.textContent || ""), overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth, dialogOverflow: dialog ? dialog.scrollWidth > dialog.clientWidth : true };' \
    '.open == true and .rows == 6 and .overflow == false and .dialogOverflow == false' \
    'storage usage did not become ready')
  jq -e '
    .labels == ["书籍文件", "阅读缓存", "笔记与引用快照", "离线词典", "阅读设置", "合计"]
  ' <<<"$storage" >/dev/null || die "storage usage categories are incomplete (state: $(jq -c . <<<"$storage"))"

  [[ $(execute_sync '
document.querySelector(".library-storage-dialog")?.close();
const library = [...document.querySelectorAll(".library-sections button")]
  .find((item) => item.textContent.trim() === "书架");
library?.click();
return Boolean(library);') == true ]] || die 'could not return to the library shelf'
  [[ $(execute_sync '
const button = document.querySelector(".library-book-open");
const rect = button?.getBoundingClientRect();
if (!button || !rect) return false;
const event = new MouseEvent("contextmenu", {bubbles: true, cancelable: true, button: 2, clientX: rect.left + 20, clientY: rect.top + 20});
return !button.dispatchEvent(event);') == true ]] || die 'could not dispatch a library context menu event'
  context=$(wait_for_script \
    'const menu = document.querySelector(".library-book-menu"); const rect = menu?.getBoundingClientRect(); return { open: Boolean(menu), labels: [...(menu?.querySelectorAll("button") || [])].map((button) => button.textContent.trim()), inViewport: Boolean(rect && rect.left >= 0 && rect.right <= innerWidth && rect.top >= 0 && rect.bottom <= innerHeight) };' \
    '.open == true and .labels == ["打开","选择","收藏","编辑分类","更换封面","移出书架","删除本地数据"] and .inViewport == true' \
    'library context menu did not become ready')
  [[ $(execute_sync 'document.dispatchEvent(new KeyboardEvent("keydown", {key: "Escape", bubbles: true})); return !document.querySelector(".library-book-menu");') == true ]] ||
    die 'could not close the library context menu'
  long_press_shelf_book '.library-book-open'
  wait_for_script \
    'const menu = document.querySelector(".library-book-menu"); return {open: Boolean(menu), labels: [...(menu?.querySelectorAll("[role=menuitem]") || [])].map((button) => button.textContent.trim())};' \
    '.open == true and .labels == ["打开","选择","收藏","编辑分类","更换封面","移出书架","删除本地数据"]' \
    'long press did not open the same library book menu as right click' >/dev/null
  click_book_menu_item '选择'
  selection=$(wait_for_script \
    'const buttons = [...document.querySelectorAll(".library-selection-bar button")]; const rects = buttons.map((button) => button.getBoundingClientRect()); return { count: buttons.length, selected: document.querySelectorAll(".library-book-open[aria-pressed=true]").length, labels: buttons.map((button) => button.textContent.trim()), overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth, clipped: buttons.some((button) => button.scrollWidth > button.clientWidth || button.scrollHeight > button.clientHeight), inViewport: rects.every((rect) => rect.left >= 0 && rect.right <= innerWidth && rect.top >= 0 && rect.bottom <= innerHeight), separated: rects.every((rect, index) => index == 0 || rects[index - 1].right <= rect.left + 1) };' \
    '.count == 3 and .selected == 1 and .labels == ["封面","移出","删除"] and .overflow == false and .clipped == false and .inViewport == true and .separated == true' \
    'library long-press actions do not fit the viewport')
  jq -e '.labels == ["封面", "移出", "删除"] and .selected == 1' <<<"$selection" >/dev/null ||
    die "library long-press actions are incomplete (state: $(jq -c . <<<"$selection"))"
  [[ $(execute_sync '
const button = [...document.querySelectorAll(".library-selection-header button")]
  .find((item) => item.textContent.trim() === "取消");
button?.click();
return Boolean(button);') == true ]] || die 'could not leave library selection'
}

verify_reading_memory() {
  local overview searched snapshot history jump
  overview=$(execute_sync '
const button = [...document.querySelectorAll(".library-sections button")]
  .find((item) => item.textContent.trim() === "阅读记忆");
button?.click();
const controls = [...document.querySelectorAll(".memory-center button, .memory-center input")];
return {
  entered: Boolean(button),
  heading: document.querySelector(".memory-heading h1")?.textContent || "",
  subtitle: document.querySelector(".memory-heading p")?.textContent || "",
  searchHeading: document.querySelector("#memory-search-heading")?.textContent || "",
  searchPlaceholder: document.querySelector(".memory-search-form input")?.placeholder || "",
  recent: document.querySelectorAll(".memory-recent-list > button").length,
  overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
  clipped: controls.some((control) => control.scrollWidth > control.clientWidth || control.scrollHeight > control.clientHeight)
};') || die 'could not enter reading memory'
  jq -e '
    .entered == true and .heading == "阅读记忆" and
    .subtitle == "最近阅读与跨书笔记" and .searchHeading == "搜索笔记与回复" and
    .searchPlaceholder == "搜索原文、笔记与回复" and .recent == 1 and
    .overflow == false and .clipped == false
  ' <<<"$overview" >/dev/null || die "reading memory overview is incomplete (state: $(jq -c . <<<"$overview"))"

  [[ $(execute_sync '
const input = document.querySelector(".memory-search-form input");
const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
setter?.call(input, "公共记忆");
input?.dispatchEvent(new Event("input", { bubbles: true }));
document.querySelector(".memory-search-form")?.requestSubmit();
return Boolean(input);') == true ]] || die 'could not submit reading memory search'
  searched=$(wait_for_script \
    'const rows = [...document.querySelectorAll(".memory-result")]; return { count: rows.length, jumpCount: rows.filter((row) => [...row.querySelectorAll("footer button")].some((button) => button.textContent.includes("跳回原书"))).length, snapshotCount: rows.filter((row) => [...row.querySelectorAll("footer button")].some((button) => button.textContent.includes("历史引用"))).length, missingCount: rows.filter((row) => row.querySelector("footer > span")?.textContent.includes("原书不在资料库")).length, overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth, clipped: [...document.querySelectorAll(".memory-center button")].some((button) => button.scrollWidth > button.clientWidth || button.scrollHeight > button.clientHeight) };' \
    '.count == 2 and .jumpCount == 1 and .snapshotCount == 2 and .missingCount == 1 and .overflow == false and .clipped == false' \
    'reading memory results did not become ready')
  jq -e '.count == 2' <<<"$searched" >/dev/null ||
    die "reading memory search is incomplete (state: $(jq -c . <<<"$searched"))"

  [[ $(execute_sync '
const row = [...document.querySelectorAll(".memory-result")]
  .find((item) => item.querySelector("h3")?.textContent === "Atha FB2 Gate");
const button = [...(row?.querySelectorAll("footer button") || [])]
  .find((item) => item.textContent.includes("历史引用"));
button?.click();
return Boolean(button);') == true ]] || die 'could not open reading memory snapshot'
  snapshot=$(wait_for_script \
    'const dialog = document.querySelector(".memory-snapshot-dialog"); const versions = [...(dialog?.querySelectorAll("nav button") || [])]; const host = dialog?.querySelector(".memory-snapshot-content > div"); return { open: Boolean(dialog?.open), versions: versions.length, current: versions.filter((button) => button.getAttribute("aria-current") === "page").length, rendered: Boolean(host?.shadowRoot?.querySelector(".book")), overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth, dialogOverflow: dialog ? dialog.scrollWidth > dialog.clientWidth : true };' \
    '.open == true and .versions == 2 and .current == 1 and .rendered == true and .overflow == false and .dialogOverflow == false' \
    'current reading memory snapshot did not render')
  jq -e '.rendered == true' <<<"$snapshot" >/dev/null ||
    die "current reading memory snapshot is incomplete (state: $(jq -c . <<<"$snapshot"))"

  [[ $(execute_sync '
const button = [...document.querySelectorAll(".memory-snapshot-dialog nav button")]
  .find((item) => item.textContent.includes("历史引用"));
button?.click();
return Boolean(button);') == true ]] || die 'could not select historical reading memory snapshot'
  history=$(wait_for_script \
    'const dialog = document.querySelector(".memory-snapshot-dialog"); const selected = dialog?.querySelector("nav button[aria-current=page]"); const host = dialog?.querySelector(".memory-snapshot-content > div"); return { historical: Boolean(selected?.textContent.includes("历史引用")), rendered: Boolean(host?.shadowRoot?.querySelector(".book")), overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth };' \
    '.historical == true and .rendered == true and .overflow == false' \
    'historical reading memory snapshot did not render')
  jq -e '.historical == true' <<<"$history" >/dev/null ||
    die "historical reading memory snapshot is incomplete (state: $(jq -c . <<<"$history"))"

  [[ $(execute_sync '
document.querySelector(".memory-snapshot-dialog")?.close();
const row = [...document.querySelectorAll(".memory-result")]
  .find((item) => item.querySelector("h3")?.textContent === "Atha FB2 Gate");
const button = [...(row?.querySelectorAll("footer button") || [])]
  .find((item) => item.textContent.includes("跳回原书"));
button?.click();
return Boolean(button);') == true ]] || die 'could not open a reading memory result'
  jump=$(wait_for_script \
    'const overlay = document.querySelector("#message-conversation"); return { reader: location.pathname.endsWith("/index.html"), ready: document.documentElement.hasAttribute("data-reader-ready"), navigation: document.documentElement.dataset.readingMemoryNavigation || null, conversation: Boolean(overlay && !overlay.hidden), title: document.querySelector("#message-conversation-title")?.textContent || "", scopes: [...document.querySelectorAll("[data-message-scope]")].map((button) => button.textContent.trim()), closeLabel: document.querySelector("#message-conversation-close")?.getAttribute("aria-label") || "", sourceAvailable: Boolean(document.querySelector("#message-conversation-source")?.textContent), error: document.documentElement.dataset.error || null };' \
    '.reader == true and .ready == true and .navigation == "ok" and .conversation == true and .title == "对话" and .scopes == ["当前", "本章", "本书"] and .closeLabel == "返回阅读" and .sourceAvailable == true and .error == null' \
    'reading memory result did not validate and open')
  jq -e '.navigation == "ok"' <<<"$jump" >/dev/null ||
    die "reading memory result is incomplete (state: $(jq -c . <<<"$jump"))"
  execute_sync 'document.querySelector("#message-conversation-close")?.click(); return true;' >/dev/null ||
    die 'could not close the reading memory conversation'
  wait_for_script \
    'return { conversation: !document.querySelector("#message-conversation")?.hidden, focused: document.activeElement === document.querySelector(".reader"), error: document.documentElement.dataset.error || null };' \
    '.conversation == false and .focused == true and .error == null' \
    'reading memory conversation did not return focus to reading' >/dev/null
  open_library "$expected_books"
}

verify_authoring_cycle() {
  local marker marker_json reply_marker reply_marker_json root_id_json reply href probe point selection ready saved composer_evidence composer_ready viewport_evidence='[]'
  local width jump search root_id jump_element jump_element_id book_key progress_key progress_key_json progress_key_args public_id_json memory_section_json book_key_json
  # WebKitWebDriver on X11 sends letter keys without preserving uppercase.
  # These synthetic markers only need to be unique; compare saved text exactly.
  marker="atha-linux-reader-cycle-${BASHPID}-${RANDOM}"
  reply_marker="${marker}-reply"
  marker_json=$(jq -nr --arg value "$marker" '$value | @json')
  reply_marker_json=$(jq -nr --arg value "$reply_marker" '$value | @json')
  book_key=${public_id:0:16}
  progress_key="atha.reader.progress.${book_key}.v1"
  progress_key_json=$(jq -nr --arg value "$progress_key" '$value | @json')
  public_id_json=$(jq -nr --arg value "$public_id" '$value | @json')
  memory_section_json=$(jq -nr --arg value "$memory_section" '$value | @json')
  book_key_json=$(jq -nr --arg value "$book_key" '$value | @json')
  printf '%s\n' "$marker" "$reply_marker" >>"$privacy_file"
  reply=$(jq -nr --arg marker "$reply_marker" '$marker + " " + ("长内容片段。" * 320)')
  (( ${#reply} <= 8000 )) || die 'authoring reply exceeds the composer limit'

  say 'verifying trusted-selection authoring, memory search, and source return'
  open_library "$expected_books"
  set_window_size 360 760
  open_book public
  enable_diagnostics
  href=$(jq -er --arg section "$memory_section" '.sections[] | select(.id == $section) | .href' "$public_manifest") ||
    die 'public authoring section is missing'
  open_gesture_section "$href" false >/dev/null
  probe=$(wait_for_script \
    'return globalThis.__athaReaderDiagnostics?.selectionProbe() || null;' \
    '(.startX | type == "number") and (.startY | type == "number") and (.endX | type == "number") and (.endY | type == "number")' \
    'public text selection geometry did not become ready')
  point=$(jq -c '{x: .startX, y: .startY, endX, endY}' <<<"$probe")
  perform_pointer_action "$point" drag || die 'trusted mouse selection failed'
  selection=$(wait_for_script \
    'const actions = document.querySelector("#selection-actions"); return { visible: Boolean(actions && !actions.hidden) };' \
    '.visible == true' \
    'trusted mouse drag did not select public text')
  webdriver_click_element '#note-selection' || die 'could not open the selected-text note composer'
  ready=$(wait_for_script \
    'const overlay = document.querySelector("#message-conversation"); const editor = overlay?.querySelector("[role=textbox]"); return { open: Boolean(overlay && !overlay.hidden), editor: Boolean(editor), context: Boolean(document.querySelector("#message-composer-context")), error: document.documentElement.dataset.error || null };' \
    '.open == true and .editor == true and .context == true and .error == null' \
    'selected-text note composer did not mount')
  webdriver_send_keys '#message-conversation [role="textbox"]' "$marker" || die 'could not type the public note marker'
  wait_for_script \
    "const editor = document.querySelector('#message-composer [role=textbox]'); return { correct: editor?.textContent === $marker_json, enabled: Boolean(document.querySelector('.message-send-button:not(:disabled)')) };" \
    '.correct == true and .enabled == true' 'the selected-text composer did not receive the note marker' >/dev/null
  webdriver_click_element '#message-composer .message-send-button' || die 'could not save the selected-text note'
  saved=$(wait_for_script \
    "const bodies = [...document.querySelectorAll('#message-conversation-list .message-card-body')]; const root = bodies.find((body) => body.textContent.includes($marker_json)); return { saved: Boolean(root), count: bodies.length, replyContext: Boolean(document.querySelector('#message-composer [role=textbox]')), error: document.documentElement.dataset.error || null };" \
    '.saved == true and .count == 1 and .replyContext == true and .error == null' \
    'public note did not save through the conversation composer')
  root_id=$(execute_sync \
    "const card = [...document.querySelectorAll('#message-conversation-list .message-card')].find((item) => item.dataset.root === 'true' && item.querySelector('.message-card-body')?.textContent.includes($marker_json)); return card?.dataset.messageId || null;" |
    jq -er '.') ||
    die 'could not identify the saved note for privacy-safe evidence'
  [[ $root_id != null && -n $root_id ]] || die 'saved public note has no root message'
  root_id_json=$(jq -nr --arg value "$root_id" '$value | @json')

  webdriver_send_keys '#message-conversation [role="textbox"]' "$reply" || die 'could not type the long public reply'
  for width in 360; do
    composer_evidence=$(wait_for_script \
      'const button = document.querySelector("#message-composer .message-send-button"); const rect = button?.getBoundingClientRect(); const list = document.querySelector("#message-conversation-list"); const body = [...document.querySelectorAll(".message-card-body")].find((item) => item.textContent.length > 1000); return { width: innerWidth, sendVisible: Boolean(rect && rect.width > 0 && rect.left >= 0 && rect.right <= innerWidth && rect.top >= 0 && rect.bottom <= innerHeight), sendEnabled: Boolean(button && !button.disabled), longBody: Boolean(body), overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth, listScrollable: Boolean(list && list.scrollHeight > list.clientHeight) };' \
      '.width >= 358 and .width <= 362 and .sendVisible == true and .sendEnabled == true and .overflow == false' \
      'narrow conversation composer is not reachable')
  done
  webdriver_click_element '#message-composer .message-send-button' || die 'could not send the long public reply'
  wait_for_script \
    "const bodies = [...document.querySelectorAll('#message-conversation-list .message-card-body')]; const reply = bodies.find((body) => body.textContent.includes($reply_marker_json)); return { saved: Boolean(reply), count: bodies.length, length: reply?.textContent.length || 0, error: document.documentElement.dataset.error || null };" \
    '.saved == true and .count == 2 and .length > 1500 and .length < 8000 and .error == null' \
    'long public reply did not save' >/dev/null
  webdriver_click_element '#message-conversation-close' || die 'could not close the authoring conversation'
  wait_for_script \
    'return { closed: document.querySelector("#message-conversation")?.hidden === true, focused: document.activeElement === document.querySelector(".reader"), error: document.documentElement.dataset.error || null };' \
    '.closed == true and .focused == true and .error == null' \
    'closing the authoring conversation did not restore reader focus' >/dev/null

  for width in 360 1000 1600; do
    open_library "$expected_books"
    if [[ $width == 1000 ]]; then set_window_size "$width" 760; fi
    if [[ $width == 1600 ]]; then set_window_size "$width" 900; fi
    execute_sync 'const button = [...document.querySelectorAll(".library-sections button")].find((item) => item.textContent.trim() === "阅读记忆"); button?.click(); return Boolean(button);' >/dev/null ||
      die 'could not open memory search for the new public note'
    webdriver_send_keys '.memory-search-form input' "$marker" || die 'could not enter the private memory query'
    webdriver_click_element '.memory-search-form button[type="submit"]' || die 'could not submit the public memory search'
    search=$(wait_for_script \
      "const rows = [...document.querySelectorAll('.memory-result')]; const books = rows.every((row) => row.querySelector('h3')?.textContent === 'Atha FB2 Gate'); const quote = rows.every((row) => row.querySelector('blockquote')?.textContent.includes('第二节正文')); const root = rows.some((row) => row.querySelector('.memory-result-text')?.textContent.includes($marker_json)); const reply = rows.some((row) => row.querySelector('.memory-result-text')?.textContent.includes($reply_marker_json)); return { count: rows.length, books, quote, root, reply, overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth };" \
      '.count == 2 and .books == true and .quote == true and .root == true and .reply == true and .overflow == false' \
      'memory search did not find both new public messages')
    progress_key_args=$(jq -cn --arg key "$progress_key" '[$key]')
    [[ $(execute_sync 'const key = arguments[0]; localStorage.removeItem(key); return localStorage.getItem(key) === null;' "$progress_key_args") == true ]] ||
      die 'could not clear the isolated public-book progress before memory jump'
    jump_element=$(execute_sync 'const button = [...document.querySelectorAll(".memory-result")].find((row) => row.querySelector(".memory-result-text")?.textContent.includes(arguments[0]))?.querySelector(".memory-jump") || null; button?.scrollIntoView({block: "center", inline: "nearest"}); return button;' "$(jq -cn --arg marker "$reply_marker" '[$marker]')") ||
      die 'could not locate the reply memory jump control'
    jump_element_id=$(jq -er '.["element-6066-11e4-a52e-4f735466cecf"]' <<<"$jump_element") ||
      die 'reply memory jump control is unavailable'
    webdriver_click_element_ref "$jump_element_id" || die 'could not jump back to the public source'
    jump=$(wait_for_script \
      "const source = document.querySelector('#message-conversation-source')?.textContent || ''; const rows = [...document.querySelectorAll('#message-conversation-list .message-card-body')]; const reply = rows.find((body) => body.textContent.includes($reply_marker_json)); const rootPresent = [...document.querySelectorAll('#message-conversation-list .message-card')].some((card) => card.dataset.messageId === $root_id_json); const list = document.querySelector('#message-conversation-list'); const send = document.querySelector('#message-composer .message-send-button'); const rect = send?.getBoundingClientRect(); let progress = null; let progressLocator = null; try { progress = JSON.parse(localStorage.getItem($progress_key_json) || 'null'); progressLocator = JSON.parse(progress?.locator || 'null'); } catch {} if (list) list.scrollTop = list.scrollHeight; return { navigation: document.documentElement.dataset.readingMemoryNavigation || null, routeState: new URL(location.href).searchParams.get('state') || null, readerReady: document.documentElement.hasAttribute('data-reader-ready'), sameEdition: progress?.schema === 1 && progress?.contentVersion === $public_id_json && progressLocator?.contentVersion === $public_id_json, sameSection: progressLocator?.start?.section === $memory_section_json, sourceQuote: source.includes('第二节正文'), rootPresent, reply: Boolean(reply), longReply: (reply?.textContent.length || 0) > 1500, scrollable: Boolean(list && list.scrollHeight > list.clientHeight), scrolled: Boolean(list && list.scrollTop > 0), sendVisible: Boolean(rect && rect.width > 0 && rect.left >= 0 && rect.right <= innerWidth && rect.top >= 0 && rect.bottom <= innerHeight), viewport: innerWidth, error: document.documentElement.dataset.error || null };" \
      ".navigation == \"ok\" and .routeState == $book_key_json and .readerReady == true and .sameEdition == true and .sameSection == true and .sourceQuote == true and .rootPresent == true and .reply == true and .longReply == true and .scrollable == true and .scrolled == true and .sendVisible == true and .error == null" \
      'memory jump did not return to the saved source and scrollable conversation')
    webdriver_send_keys '#message-conversation [role="textbox"]' "${marker}-draft" || die 'could not test the returned composer'
    composer_ready=$(wait_for_script \
      'const button = document.querySelector("#message-composer .message-send-button"); const rect = button?.getBoundingClientRect(); return { visible: Boolean(rect && rect.width > 0 && rect.left >= 0 && rect.right <= innerWidth && rect.top >= 0 && rect.bottom <= innerHeight), enabled: Boolean(button && !button.disabled) };' \
      '.visible == true and .enabled == true' \
      'returned conversation composer could not reach send')
    viewport_evidence=$(jq -cn --arg bookKey "$book_key" --argjson prior "$viewport_evidence" --argjson jump "$jump" --argjson search "$search" --argjson composer "$composer_ready" \
      '$prior + [{width: $jump.viewport, searchHits: $search.count, sourceQuote: $jump.sourceQuote, sameEdition: $jump.sameEdition, sameSection: $jump.sameSection, rootPresent: $jump.rootPresent, routeStateValidated: ($jump.routeState == $bookKey and $jump.readerReady == true), locatorNavigationValidated: ($jump.navigation == "ok"), longReply: $jump.longReply, scrollable: $jump.scrollable, scrolled: $jump.scrolled, sendVisible: $composer.visible, sendEnabled: $composer.enabled}]')
    webdriver_click_element '#message-conversation-close' || die 'could not close the returned conversation'
    wait_for_script \
      'return { closed: document.querySelector("#message-conversation")?.hidden === true, focused: document.activeElement === document.querySelector(".reader") };' \
      '.closed == true and .focused == true' \
      'closing the returned conversation did not restore reader focus' >/dev/null
  done
  authoring_cycle_evidence=$(jq -cn --argjson composer "$composer_evidence" --argjson viewports "$viewport_evidence" \
    '{trustedMouseSelection: true, noteSaved: true, replySent: true, reopened: true, searchHits: 2, jumpToSource: true, composer: {width: $composer.width, sendVisible: $composer.sendVisible, sendEnabled: $composer.sendEnabled}, sameEdition: all($viewports[]; .sameEdition == true), sameSection: all($viewports[]; .sameSection == true), sameRoot: all($viewports[]; .rootPresent == true), routeStateValidated: all($viewports[]; .routeStateValidated == true), locatorNavigationValidated: all($viewports[]; .locatorNavigationValidated == true), focusRestored: true, viewports: $viewports}')
}

verify_desktop_workspace() {
  local baseline resized restored workspace search notes directory keyboard medium narrow tool tool_switch
  local notes_two notes_return notes_wide notes_wide_return medium_notes medium_thread portrait_notes portrait_thread portrait_return terminology
  local baseline_anchor resized_anchor restored_anchor workspace_anchor tool_switch_anchor search_point

  baseline=$(wait_for_script \
    'const frame = document.querySelector(".reader-frame"); const reader = document.querySelector(".reader"); const page = document.querySelector(".page"); const controls = document.querySelector(".reader-controls"); const frameRect = frame?.getBoundingClientRect(); const readerRect = reader?.getBoundingClientRect(); const pageRect = page?.getBoundingClientRect(); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; const controlsStyle = controls ? getComputedStyle(controls) : null; return { desktop: document.documentElement.hasAttribute("data-desktop-workspace"), active: document.documentElement.dataset.workspacePanel || null, openPanels: document.querySelectorAll(".reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]").length, stable: document.documentElement.dataset.viewportStable === expected, full: Boolean(frameRect && readerRect && Math.abs(frameRect.left) <= 1 && Math.abs(frameRect.width - innerWidth) <= 2 && Math.abs(readerRect.left - frameRect.left) <= 1 && Math.abs(readerRect.width - frameRect.width) <= 1), centered: Boolean(pageRect && readerRect && Math.abs((pageRect.left - readerRect.left) - (readerRect.right - pageRect.right)) <= 2), pageWidth: pageRect?.width || 0, pageMargin: reader ? parseFloat(getComputedStyle(reader).getPropertyValue("--page-left-margin")) / devicePixelRatio : 0, controlsHidden: controlsStyle?.visibility === "hidden", overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth, locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null, error: document.documentElement.dataset.error || null };' \
    '.desktop == true and .active == null and .openPanels == 0 and .stable == true and .full == true and .centered == true and .pageWidth >= 700 and .pageWidth <= 761 and .pageMargin >= 31 and .pageMargin <= 33 and .controlsHidden == true and .overflow == false and .locator != null and .error == null' \
    'desktop focus-reading layout did not become ready')
  baseline_anchor=$(jq -r '.locator | fromjson | [.start.section, .start.offset] | @tsv' <<<"$baseline")

  set_window_size 1600 900
  resized=$(wait_for_script \
    'const frame = document.querySelector(".reader-frame"); const reader = document.querySelector(".reader"); const page = document.querySelector(".page"); const frameRect = frame?.getBoundingClientRect(); const readerRect = reader?.getBoundingClientRect(); const pageRect = page?.getBoundingClientRect(); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { desktop: document.documentElement.hasAttribute("data-desktop-workspace"), active: document.documentElement.dataset.workspacePanel || null, openPanels: document.querySelectorAll(".reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]").length, stable: document.documentElement.dataset.viewportStable === expected, full: Boolean(frameRect && readerRect && Math.abs(frameRect.left) <= 1 && Math.abs(frameRect.width - innerWidth) <= 2 && Math.abs(readerRect.left - frameRect.left) <= 1 && Math.abs(readerRect.width - frameRect.width) <= 1), centered: Boolean(pageRect && readerRect && Math.abs((pageRect.left - readerRect.left) - (readerRect.right - pageRect.right)) <= 2), pageWidth: pageRect?.width || 0, locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null, error: document.documentElement.dataset.error || null };' \
    '.desktop == true and .active == null and .openPanels == 0 and .stable == true and .full == true and .centered == true and .pageWidth >= 700 and .pageWidth <= 761 and .locator != null and .error == null' \
    'resized desktop focus-reading layout did not stabilize')
  resized_anchor=$(jq -r '.locator | fromjson | [.start.section, .start.offset] | @tsv' <<<"$resized")
  [[ $resized_anchor == "$baseline_anchor" ]] || die 'desktop resize did not preserve the current locator'

  set_window_size 1280 800
  restored=$(wait_for_script \
    'const frame = document.querySelector(".reader-frame"); const reader = document.querySelector(".reader"); const page = document.querySelector(".page"); const frameRect = frame?.getBoundingClientRect(); const readerRect = reader?.getBoundingClientRect(); const pageRect = page?.getBoundingClientRect(); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { active: document.documentElement.dataset.workspacePanel || null, stable: document.documentElement.dataset.viewportStable === expected, full: Boolean(frameRect && readerRect && Math.abs(frameRect.left) <= 1 && Math.abs(frameRect.width - innerWidth) <= 2 && Math.abs(readerRect.left - frameRect.left) <= 1 && Math.abs(readerRect.width - frameRect.width) <= 1), centered: Boolean(pageRect && readerRect && Math.abs((pageRect.left - readerRect.left) - (readerRect.right - pageRect.right)) <= 2), pageWidth: pageRect?.width || 0, locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null, error: document.documentElement.dataset.error || null };' \
    '.active == null and .stable == true and .full == true and .centered == true and .pageWidth >= 700 and .pageWidth <= 761 and .locator != null and .error == null' \
    'restored desktop focus-reading layout did not stabilize')
  restored_anchor=$(jq -r '.locator | fromjson | [.start.section, .start.offset] | @tsv' <<<"$restored")
  [[ $restored_anchor == "$baseline_anchor" ]] || die 'desktop restore did not preserve the current locator'

  [[ $(execute_sync '
const reader = document.querySelector(".reader");
const rect = reader.getBoundingClientRect();
for (const type of ["pointerdown", "pointerup"]) {
  reader.dispatchEvent(new PointerEvent(type, {pointerId: 41, pointerType: "mouse", isPrimary: true, button: 0, bubbles: true, clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2}));
}
return document.documentElement.hasAttribute("data-reader-tools");') == true ]] ||
    die 'desktop center click did not reveal the reader tools'
  execute_sync 'document.querySelector(".reader-tool.directory > summary")?.click(); return true;' >/dev/null ||
    die 'could not open the desktop directory workspace'
  workspace=$(wait_for_script \
    'const frame = document.querySelector(".reader-frame"); const reader = document.querySelector(".reader"); const page = document.querySelector(".page"); const panel = document.querySelector(".reader-tool.directory > .tool-panel"); const frameRect = frame?.getBoundingClientRect(); const readerRect = reader?.getBoundingClientRect(); const pageRect = page?.getBoundingClientRect(); const panelRect = panel?.getBoundingClientRect(); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { active: document.documentElement.dataset.workspacePanel || null, openPanels: document.querySelectorAll(".reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]").length, stable: document.documentElement.dataset.viewportStable === expected, workspace: Boolean(frameRect && readerRect && panelRect && panelRect.right <= frameRect.left + 1 && Math.abs(panelRect.width - 320) <= 2 && Math.abs(frameRect.right - innerWidth) <= 2 && Math.abs(readerRect.left - frameRect.left) <= 1 && Math.abs(readerRect.width - frameRect.width) <= 1), centered: Boolean(pageRect && readerRect && Math.abs((pageRect.left - readerRect.left) - (readerRect.right - pageRect.right)) <= 2), pageWidth: pageRect?.width || 0, locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null, error: document.documentElement.dataset.error || null };' \
    '.active == "directory" and .openPanels == 1 and .stable == true and .workspace == true and .centered == true and .pageWidth >= 700 and .pageWidth <= 761 and .locator != null and .error == null' \
    'explicit desktop directory workspace did not become ready')
  workspace_anchor=$(jq -r '.locator | fromjson | [.start.section, .start.offset] | @tsv' <<<"$workspace")
  [[ $workspace_anchor == "$baseline_anchor" ]] || die 'opening the desktop workspace changed the current locator'

  for tool in search notes directory; do
    execute_sync "document.querySelector('.reader-tool.$tool > summary')?.click(); return true;" >/dev/null ||
      die "could not click the desktop $tool tool"
    tool_switch=$(wait_for_script \
      'const reader = document.querySelector(".reader"); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { active: document.documentElement.dataset.workspacePanel || null, openPanels: document.querySelectorAll(".reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]").length, stable: document.documentElement.dataset.viewportStable === expected, locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null };' \
      ".active == \"$tool\" and .openPanels == 1 and .stable == true and .locator != null" \
      "desktop $tool tool did not become active without relayout")
  done
  tool_switch_anchor=$(jq -r '.locator | fromjson | [.start.section, .start.offset] | @tsv' <<<"$tool_switch")
  [[ $tool_switch_anchor == "$baseline_anchor" ]] || die 'desktop tool switching changed the current locator'

  [[ $(execute_sync '
const reader = document.querySelector(".reader");
reader?.focus();
const event = new KeyboardEvent("keydown", {key: "f", ctrlKey: true, bubbles: true, cancelable: true});
document.dispatchEvent(event);
return event.defaultPrevented;') == true ]] || die 'desktop search shortcut was not handled'
  search=$(wait_for_script \
    'return { active: document.documentElement.dataset.workspacePanel || null, focused: document.activeElement?.id || null, openPanels: document.querySelectorAll(".reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]").length };' \
    '.active == "search" and .focused == "search-query" and .openPanels == 1' \
    'desktop search shortcut did not focus the search input')

  [[ $(execute_sync '
const input = document.querySelector("#search-query");
const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
setter?.call(input, "第二节正文");
input?.dispatchEvent(new Event("input", {bubbles: true}));
document.querySelector("#search-form")?.requestSubmit();
return Boolean(input);') == true ]] || die 'could not submit desktop book search'
  search=$(wait_for_script \
    'const state = globalThis.__athaReaderDiagnostics?.snapshot().search; return { status: state?.status || null, count: state?.count || 0, options: document.querySelectorAll("#search-results option").length, active: document.documentElement.dataset.workspacePanel || null };' \
    '.status == "complete" and .count > 0 and .options > 0 and .active == "search"' \
    'desktop book search did not return results')
  search_point=$(execute_sync '
const rect = document.querySelector("#search-result-list button")?.getBoundingClientRect();
return rect ? {x: rect.left + rect.width / 2, y: rect.top + rect.height / 2} : null;') ||
    die 'could not locate a desktop search result'
  perform_pointer_action "$search_point" tap || die 'could not open a desktop search result'
  wait_for_script \
    'const search = globalThis.__athaReaderDiagnostics?.snapshot().search; return { visible: Boolean(search?.lastJump?.visible), error: document.documentElement.dataset.error || null };' \
    '.visible == true and .error == null' \
    'desktop search result did not navigate' >/dev/null

  [[ $(execute_sync '
const summary = document.querySelector(".reader-tool.search > summary");
summary?.focus();
const event = new KeyboardEvent("keydown", {key: "ArrowRight", bubbles: true, cancelable: true});
summary?.dispatchEvent(event);
return event.defaultPrevented;') == true ]] || die 'desktop workspace arrow navigation was not handled'
  notes=$(wait_for_script \
    'const panel = document.querySelector(".notes-panel")?.getBoundingClientRect(); const list = document.querySelector(".annotation-list"); const search = document.querySelector("#annotation-filter-query"); const item = document.querySelector(".annotation-item"); return { active: document.documentElement.dataset.workspacePanel || null, focused: document.activeElement === document.querySelector(".reader-tool.notes > summary"), heading: document.querySelector("#annotation-heading")?.textContent || "", search: search?.placeholder || "", searchLabel: search?.getAttribute("aria-label") || "", listLabel: list?.getAttribute("aria-label") || "", count: document.querySelector("#annotations-status")?.textContent || "", items: document.querySelectorAll(".annotation-item-main").length, item: {source: item?.querySelector(".annotation-item-quote")?.textContent || "", response: item?.querySelector(".annotation-item-note")?.textContent || "", time: item?.querySelector(".annotation-item-time")?.textContent || "", types: item?.querySelectorAll(".annotation-item-kind").length || 0, actions: item?.querySelectorAll(".annotation-item-actions").length || 0}, openPanels: document.querySelectorAll(".reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]").length, scroll: list?.scrollTop || 0, viewport: {width: innerWidth, height: innerHeight}, width: panel?.width || 0, left: panel?.left ?? -1, right: panel?.right ?? -1, top: panel?.top ?? -1, bottom: panel?.bottom ?? -1, overflow: Boolean(list && list.scrollWidth > list.clientWidth), locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null };' \
    '.active == "notes" and .focused == true and .heading == "笔记" and .search == "搜索笔记或标注" and .searchLabel == "搜索笔记或标注" and .listLabel == "笔记和标注列表" and (.count | test("^共 [0-9]+ 条$")) and .items > 0 and .item.source == "第二节正文" and .item.response == "这条回复用于验证消息之间的回应关系" and (.item.time | test("^[0-9]{2}:[0-9]{2}$")) and .item.types == 0 and .item.actions == 0 and .openPanels == 1 and .width >= 319 and .width <= 321 and ((.right - .viewport.width) | fabs) <= 2 and ((.left - (.viewport.width - 320)) | fabs) <= 2 and (.top | fabs) <= 1 and ((.bottom - .viewport.height) | fabs) <= 2 and .overflow == false and .locator != null' \
    'desktop notes workspace did not become ready')
  [[ $(execute_sync '
document.querySelector(".annotation-item-main")?.click();
return true;') == true ]] || die 'could not open a desktop message conversation'
  notes_two=$(wait_for_script \
    'const overlay = document.querySelector("#message-conversation"); const notes = document.querySelector(".notes-panel")?.getBoundingClientRect(); const thread = overlay?.getBoundingClientRect(); const composer = document.querySelector("#message-composer")?.getBoundingClientRect(); const frame = document.querySelector(".reader-frame")?.getBoundingClientRect(); const list = document.querySelector(".message-conversation-list")?.getBoundingClientRect(); const cards = [...document.querySelectorAll(".message-card")]; const sourceTurn = document.querySelector(".message-source-turn"); return { viewport: {width: innerWidth, height: innerHeight}, open: Boolean(overlay && !overlay.hidden), title: document.querySelector("#message-conversation-title")?.textContent || "", scopes: [...document.querySelectorAll("[data-message-scope]")].map((button) => button.textContent.trim()), scopeLabel: document.querySelector(".message-segmented")?.getAttribute("aria-label") || "", closeLabel: document.querySelector("#message-conversation-close")?.getAttribute("aria-label") || "", source: sourceTurn?.querySelector(".message-source-turn-text")?.textContent || "", sourceLabel: sourceTurn?.querySelector(".message-source-turn-label")?.textContent || "", sourceHeaderVisible: Boolean(document.querySelector(".message-source-context")?.getClientRects().length), cards: cards.length, rootReplyPreviews: cards[1]?.querySelectorAll(".message-reference-preview").length ?? -1, nestedReplyLabel: cards[2]?.querySelector(".message-reference-label")?.textContent || "", rightAligned: Boolean(list && cards.every((card) => Math.abs(card.getBoundingClientRect().right - (list.right - 20)) <= 2)), composerBottom: composer?.bottom ?? -1, active: document.documentElement.dataset.workspacePanel || null, openPanels: document.querySelectorAll(".reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]").length, selected: document.querySelectorAll(".annotation-item[data-selected=true]").length, notes: notes ? {left: notes.left, right: notes.right, top: notes.top, bottom: notes.bottom} : null, thread: thread ? {left: thread.left, right: thread.right, top: thread.top, bottom: thread.bottom} : null, frame: frame ? {left: frame.left, right: frame.right} : null, locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null, error: document.documentElement.dataset.error || null };' \
    '.open == true and .title == "对话" and .scopes == ["当前", "本章", "本书"] and .scopeLabel == "对话范围" and .closeLabel == "返回笔记" and .source == "第二节正文" and .sourceLabel == "原文" and .sourceHeaderVisible == false and .cards == 3 and .rootReplyPreviews == 0 and .nestedReplyLabel == "回复" and .rightAligned == true and .active == "notes" and .openPanels == 1 and .selected == 1 and (.notes.left | fabs) <= 1 and ((.notes.right - 320) | fabs) <= 2 and (.notes.top | fabs) <= 1 and ((.notes.bottom - .viewport.height) | fabs) <= 2 and ((.thread.left - .notes.right) | fabs) <= 2 and ((.thread.right - .viewport.width) | fabs) <= 2 and (.thread.top | fabs) <= 1 and ((.thread.bottom - .viewport.height) | fabs) <= 2 and ((.composerBottom - .thread.bottom) | fabs) <= 2 and (.frame.left | fabs) <= 2 and ((.frame.right - (.viewport.width - 320)) | fabs) <= 2 and .locator != null and .error == null' \
    'desktop B+C message workspace did not become ready')
  [[ $(execute_sync '
document.querySelector(".message-card [data-message-action=delete]")?.click();
return true;') == true ]] || die 'could not probe message deletion cancellation'
  wait_for_script \
    'return { open: Boolean(document.querySelector("[data-atha-confirm-dialog] [data-atha-confirm-cancel]")) };' \
    '.open == true' 'message deletion confirmation did not appear' >/dev/null
  webdriver_click_element '[data-atha-confirm-cancel]' || die 'could not cancel message deletion'
  wait_for_script \
    'return {dialog: Boolean(document.querySelector("[data-atha-confirm-dialog]")), cards: document.querySelectorAll(".message-card").length, items: document.querySelectorAll(".annotation-item").length};' \
    '.dialog == false and .cards == 3 and .items == 1' \
    'canceling message deletion changed the conversation' >/dev/null
  execute_sync 'document.querySelector("[data-message-scope=chapter]")?.click(); return true;' >/dev/null ||
    die 'could not open the chapter conversation scope'
  terminology=$(wait_for_script \
    'const source = document.querySelector(".message-source-context")?.getBoundingClientRect(); const content = document.querySelector(".message-conversation-content")?.getBoundingClientRect(); const overlay = document.querySelector("#message-conversation")?.getBoundingClientRect(); return { label: document.querySelector("#message-conversation-source-label")?.textContent || "", summary: document.querySelector("#message-conversation-source")?.textContent || "", pressed: document.querySelector("[data-message-scope=chapter]")?.getAttribute("aria-pressed") || "", sourceVisible: Boolean(document.querySelector(".message-source-context")?.getClientRects().length), sourceBottom: source?.bottom ?? -1, contentTop: content?.top ?? -1, contentBottom: content?.bottom ?? -1, overlayBottom: overlay?.bottom ?? -1 };' \
    '.label == "本章对话" and (.summary | test("^[0-9]+ 个对话 · [0-9]+ 条消息$")) and .pressed == "true" and .sourceVisible == true and ((.contentTop - .sourceBottom) | fabs) <= 2 and ((.contentBottom - .overlayBottom) | fabs) <= 2' \
    'chapter conversation terminology did not become ready')
  execute_sync 'document.querySelector("[data-message-scope=book]")?.click(); return true;' >/dev/null ||
    die 'could not open the book conversation scope'
  terminology=$(wait_for_script \
    'return { label: document.querySelector("#message-conversation-source-label")?.textContent || "", summary: document.querySelector("#message-conversation-source")?.textContent || "", pressed: document.querySelector("[data-message-scope=book]")?.getAttribute("aria-pressed") || "" };' \
    '.label == "本书对话" and (.summary | test("^[0-9]+ 个对话 · [0-9]+ 条消息$")) and .pressed == "true"' \
    'book conversation terminology did not become ready')
  execute_sync 'document.querySelector("[data-message-scope=mark]")?.click(); return true;' >/dev/null ||
    die 'could not restore the current conversation scope'
  wait_for_script \
    'return { label: document.querySelector("#message-conversation-source-label")?.textContent || "", pressed: document.querySelector("[data-message-scope=mark]")?.getAttribute("aria-pressed") || "" };' \
    '.label == "原文引用" and .pressed == "true"' \
    'current conversation scope did not return' >/dev/null
  execute_sync 'document.querySelector("#message-conversation-close")?.click(); return true;' >/dev/null ||
    die 'could not close the desktop message conversation'
  notes_return=$(wait_for_script \
    'const overlay = document.querySelector("#message-conversation"); const list = document.querySelector(".annotation-list"); const selected = document.querySelector(".annotation-item[data-selected=true] .annotation-item-main"); return { open: Boolean(overlay && !overlay.hidden), notesOpen: document.querySelector(".reader-tool.notes")?.open || false, active: document.documentElement.dataset.workspacePanel || null, selected: document.querySelectorAll(".annotation-item[data-selected=true]").length, focused: document.activeElement === selected, scroll: list?.scrollTop || 0, locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null, error: document.documentElement.dataset.error || null };' \
    '.open == false and .notesOpen == true and .active == "notes" and .selected == 1 and .focused == true and .locator != null and .error == null' \
    'closing the desktop thread did not restore the notes list')
  [[ $(jq '.scroll' <<<"$notes_return") == "$(jq '.scroll' <<<"$notes")" ]] ||
    die 'closing the desktop thread reset the notes list scroll position'
  [[ $(jq -r '.locator' <<<"$notes_return") == "$(jq -r '.locator' <<<"$notes_two")" ]] ||
    die 'opening and closing the desktop thread changed the current locator'

  webdriver_value POST "/session/$session_id/window/rect" '{"width":1600,"height":900}' >/dev/null ||
    die 'could not widen the active notes workspace'
  wait_for_script \
    'const reader = document.querySelector(".reader"); const frame = document.querySelector(".reader-frame")?.getBoundingClientRect(); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { viewportWidth: innerWidth, active: document.documentElement.dataset.workspacePanel || null, stable: document.documentElement.dataset.viewportStable === expected, frame: frame ? {left: frame.left, right: frame.right} : null, error: document.documentElement.dataset.error || null };' \
    '.viewportWidth >= 1598 and .viewportWidth <= 1602 and .active == "notes" and .stable == true and (.frame.left | fabs) <= 2 and ((.frame.right - (.viewportWidth - 320)) | fabs) <= 2 and .error == null' \
    'wide A+B notes workspace did not stabilize' >/dev/null
  execute_sync 'document.querySelector(".annotation-item[data-selected=true] .annotation-item-main")?.click(); return true;' >/dev/null ||
    die 'could not reopen the selected wide message conversation'
  notes_wide=$(wait_for_script \
    'const overlay = document.querySelector("#message-conversation"); const notes = document.querySelector(".notes-panel")?.getBoundingClientRect(); const thread = overlay?.getBoundingClientRect(); const frame = document.querySelector(".reader-frame")?.getBoundingClientRect(); const reader = document.querySelector(".reader"); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { viewportWidth: innerWidth, open: Boolean(overlay && !overlay.hidden), active: document.documentElement.dataset.workspacePanel || null, selected: document.querySelectorAll(".annotation-item[data-selected=true]").length, stable: document.documentElement.dataset.viewportStable === expected, notes: notes ? {left: notes.left, right: notes.right} : null, thread: thread ? {left: thread.left, right: thread.right, width: thread.width} : null, frame: frame ? {left: frame.left, right: frame.right, width: frame.width} : null, locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null, error: document.documentElement.dataset.error || null };' \
    '.viewportWidth >= 1598 and .viewportWidth <= 1602 and .open == true and .active == "notes" and .selected == 1 and .stable == true and (.frame.left | fabs) <= 2 and ((.frame.right - .notes.left) | fabs) <= 2 and ((.notes.right - .thread.left) | fabs) <= 2 and .frame.width >= 600 and .thread.width >= 440 and .thread.width <= 520 and ((.thread.right - .viewportWidth) | fabs) <= 2 and .locator != null and .error == null' \
    'wide A+B+C message workspace did not become ready')
  [[ $(jq -r '.locator' <<<"$notes_wide") == "$(jq -r '.locator' <<<"$notes_return")" ]] ||
    die 'opening the wide thread changed the current locator'
  capture_public_ui reader-abc
  execute_sync 'document.querySelector("#message-conversation-close")?.click(); return true;' >/dev/null ||
    die 'could not close the wide message conversation'
  notes_wide_return=$(wait_for_script \
    'const overlay = document.querySelector("#message-conversation"); const reader = document.querySelector(".reader"); const frame = document.querySelector(".reader-frame")?.getBoundingClientRect(); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { viewportWidth: innerWidth, open: Boolean(overlay && !overlay.hidden), active: document.documentElement.dataset.workspacePanel || null, stable: document.documentElement.dataset.viewportStable === expected, focused: document.activeElement === document.querySelector(".annotation-item[data-selected=true] .annotation-item-main"), frame: frame ? {left: frame.left, right: frame.right} : null, locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null, error: document.documentElement.dataset.error || null };' \
    '.viewportWidth >= 1598 and .viewportWidth <= 1602 and .open == false and .active == "notes" and .stable == true and .focused == true and (.frame.left | fabs) <= 2 and ((.frame.right - (.viewportWidth - 320)) | fabs) <= 2 and .locator != null and .error == null' \
    'closing the wide thread did not restore A+B')
  [[ $(jq -r '.locator' <<<"$notes_wide_return") == "$(jq -r '.locator' <<<"$notes_wide")" ]] ||
    die 'closing the wide thread changed the current locator'

  webdriver_value POST "/session/$session_id/window/rect" '{"width":1280,"height":800}' >/dev/null ||
    die 'could not restore the active notes workspace width'
  wait_for_script \
    'const reader = document.querySelector(".reader"); const frame = document.querySelector(".reader-frame")?.getBoundingClientRect(); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { viewportWidth: innerWidth, active: document.documentElement.dataset.workspacePanel || null, stable: document.documentElement.dataset.viewportStable === expected, frame: frame ? {left: frame.left, right: frame.right} : null };' \
    '.viewportWidth >= 1278 and .viewportWidth <= 1282 and .active == "notes" and .stable == true and (.frame.left | fabs) <= 2 and ((.frame.right - (.viewportWidth - 320)) | fabs) <= 2' \
    'restored A+B notes workspace did not stabilize' >/dev/null

  execute_sync 'document.querySelector(".reader-tool.directory > summary")?.click(); return true;' >/dev/null ||
    die 'could not switch to the desktop directory'
  directory=$(wait_for_script \
    'return {active: document.documentElement.dataset.workspacePanel || null, count: document.querySelectorAll("#directory-list button").length};' \
    '.active == "directory" and .count > 1' \
    'desktop directory did not become ready')
  directory=$(execute_sync '
const before = globalThis.__athaReaderDiagnostics.snapshot().session.currentIndex;
const target = [...document.querySelectorAll("#directory-list button")]
  .find((button) => !button.disabled && !button.hasAttribute("aria-current"));
target?.click();
const shortcut = new KeyboardEvent("keydown", {key: "f", ctrlKey: true, bubbles: true, cancelable: true});
document.dispatchEvent(shortcut);
return {before, clicked: Boolean(target), shortcut: shortcut.defaultPrevented};') || die 'could not navigate from the desktop directory'
jq -e '.clicked == true and .shortcut == true' <<<"$directory" >/dev/null || die 'desktop directory has no alternate target or search shortcut'
  wait_for_script \
    'return { section: globalThis.__athaReaderDiagnostics?.snapshot().session.currentIndex ?? null, active: document.documentElement.dataset.workspacePanel || null, focused: document.activeElement?.id || null, error: document.documentElement.dataset.error || null };' \
    ".section != $(jq '.before' <<<"$directory") and .active == \"search\" and .focused == \"search-query\" and .error == null" \
    'desktop directory navigation stole focus from a later search shortcut' >/dev/null

  keyboard=$(execute_sync '
const input = document.querySelector("#search-query");
const inputFocused = document.activeElement === input;
const event = new KeyboardEvent("keydown", {key: "Escape", bubbles: true, cancelable: true});
input.dispatchEvent(event);
return {inputFocused, escapeHandled: event.defaultPrevented};') || die 'could not dismiss the desktop workspace'
  jq -e '.inputFocused == true and .escapeHandled == true' <<<"$keyboard" >/dev/null ||
    die 'desktop Escape was not handled'
  wait_for_script \
    'const frame = document.querySelector(".reader-frame"); const reader = document.querySelector(".reader"); const frameRect = frame?.getBoundingClientRect(); const readerRect = reader?.getBoundingClientRect(); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { active: document.documentElement.dataset.workspacePanel || null, tools: document.documentElement.hasAttribute("data-reader-tools"), openPanels: document.querySelectorAll(".reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]").length, stable: document.documentElement.dataset.viewportStable === expected, full: Boolean(frameRect && readerRect && Math.abs(frameRect.left) <= 1 && Math.abs(frameRect.width - innerWidth) <= 2 && Math.abs(readerRect.left - frameRect.left) <= 1 && Math.abs(readerRect.width - frameRect.width) <= 1), focused: document.activeElement === reader, error: document.documentElement.dataset.error || null };' \
    '.active == null and .tools == false and .openPanels == 0 and .stable == true and .full == true and .focused == true and .error == null' \
    'desktop Escape did not restore focus reading' >/dev/null
  keyboard=$(execute_sync '
const before = globalThis.__athaReaderDiagnostics.snapshot();
const event = new KeyboardEvent("keydown", {key: "PageDown", bubbles: true, cancelable: true});
document.querySelector(".reader")?.dispatchEvent(event);
return {pageHandled: event.defaultPrevented, beforePage: before.navigation.current};') || die 'could not dispatch desktop PageDown'
  jq -e '.pageHandled == true' <<<"$keyboard" >/dev/null || die 'desktop PageDown was not handled'
  wait_for_script \
    'const state = globalThis.__athaReaderDiagnostics?.snapshot(); return { current: state?.navigation.current || null, error: document.documentElement.dataset.error || null };' \
    "(.current != $(jq -c '.beforePage' <<<"$keyboard")) and .error == null" \
    'desktop PageDown did not navigate' >/dev/null

  set_window_size 1000 760
  medium=$(wait_for_script \
    'const reader = document.querySelector(".reader"); const page = document.querySelector(".page"); const readerRect = reader?.getBoundingClientRect(); const pageRect = page?.getBoundingClientRect(); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { desktop: document.documentElement.hasAttribute("data-desktop-workspace"), active: document.documentElement.dataset.workspacePanel || null, stable: document.documentElement.dataset.viewportStable === expected, centered: Boolean(pageRect && readerRect && Math.abs((pageRect.left - readerRect.left) - (readerRect.right - pageRect.right)) <= 2), pageWidth: pageRect?.width || 0, pageMargin: reader ? parseFloat(getComputedStyle(reader).getPropertyValue("--page-left-margin")) / devicePixelRatio : 0, overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth, locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null, error: document.documentElement.dataset.error || null };' \
    '.desktop == true and .active == null and .stable == true and .centered == true and .pageWidth >= 700 and .pageWidth <= 761 and .pageMargin >= 31 and .pageMargin <= 33 and .overflow == false and .locator != null and .error == null' \
    'medium reader did not preserve the centered reading measure')
  execute_sync 'document.querySelector(".reader-tool.notes > summary")?.click(); return true;' >/dev/null ||
    die 'could not open the medium notes workspace'
  medium_notes=$(wait_for_script \
    'const notes = document.querySelector(".notes-panel")?.getBoundingClientRect(); const frame = document.querySelector(".reader-frame")?.getBoundingClientRect(); const reader = document.querySelector(".reader"); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { viewport: {width: innerWidth, height: innerHeight}, active: document.documentElement.dataset.workspacePanel || null, stable: document.documentElement.dataset.viewportStable === expected, notes: notes ? {left: notes.left, right: notes.right, top: notes.top, bottom: notes.bottom} : null, frame: frame ? {left: frame.left, right: frame.right} : null, items: document.querySelectorAll(".annotation-item-main").length, locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null, error: document.documentElement.dataset.error || null };' \
    '.active == "notes" and .stable == true and .items > 0 and ((.notes.right - .viewport.width) | fabs) <= 2 and ((.notes.left - (.viewport.width - 320)) | fabs) <= 2 and (.notes.top | fabs) <= 1 and ((.notes.bottom - .viewport.height) | fabs) <= 2 and (.frame.left | fabs) <= 2 and ((.frame.right - .notes.left) | fabs) <= 2 and .locator != null and .error == null' \
    'medium A+B notes workspace did not become ready')
  execute_sync 'document.querySelector(".annotation-item-main")?.click(); return true;' >/dev/null ||
    die 'could not open the medium message conversation'
  medium_thread=$(wait_for_script \
    'const overlay = document.querySelector("#message-conversation"); const notes = document.querySelector(".notes-panel")?.getBoundingClientRect(); const thread = overlay?.getBoundingClientRect(); return { viewport: {width: innerWidth, height: innerHeight}, open: Boolean(overlay && !overlay.hidden), notesOpen: document.querySelector(".reader-tool.notes")?.open || false, active: document.documentElement.dataset.workspacePanel || null, selected: document.querySelectorAll(".annotation-item[data-selected=true]").length, notes: notes ? {right: notes.right, top: notes.top, bottom: notes.bottom} : null, thread: thread ? {left: thread.left, right: thread.right, top: thread.top, bottom: thread.bottom} : null, locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null, error: document.documentElement.dataset.error || null };' \
    '.open == true and .notesOpen == true and .active == "notes" and .selected == 1 and ((.notes.right - 320) | fabs) <= 2 and ((.thread.left - .notes.right) | fabs) <= 2 and ((.thread.right - .viewport.width) | fabs) <= 2 and (.thread.top | fabs) <= 1 and ((.thread.bottom - .viewport.height) | fabs) <= 2 and .locator != null and .error == null' \
    'medium B+C message workspace did not become ready')
  [[ $(jq -r '.locator' <<<"$medium_thread") == "$(jq -r '.locator' <<<"$medium_notes")" ]] ||
    die 'opening the medium thread changed the current locator'
  capture_public_ui reader-bc
  execute_sync 'document.querySelector("#message-conversation-close")?.click(); return true;' >/dev/null ||
    die 'could not close the medium message conversation'
  wait_for_script \
    'return { open: !document.querySelector("#message-conversation")?.hidden, notesOpen: document.querySelector(".reader-tool.notes")?.open || false, focused: document.activeElement === document.querySelector(".annotation-item[data-selected=true] .annotation-item-main") };' \
    '.open == false and .notesOpen == true and .focused == true' \
    'closing the medium thread did not restore the notes list' >/dev/null
  execute_sync '
const target = document.activeElement;
const event = new KeyboardEvent("keydown", {key: "Escape", bubbles: true, cancelable: true});
target?.dispatchEvent(event);
return event.defaultPrevented;' >/dev/null || die 'could not close the medium notes workspace'
  wait_for_script \
    'const reader = document.querySelector(".reader"); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { active: document.documentElement.dataset.workspacePanel || null, openPanels: document.querySelectorAll(".reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]").length, stable: document.documentElement.dataset.viewportStable === expected, focused: document.activeElement === reader };' \
    '.active == null and .openPanels == 0 and .stable == true and .focused == true' \
    'medium notes workspace did not return to focus reading' >/dev/null
  execute_sync '
const control = document.querySelector("#reading-mode");
control.value = "scroll";
control.dispatchEvent(new Event("change", {bubbles: true}));
return true;' >/dev/null || die 'could not switch the medium reader to scroll mode'
  wait_for_script \
    'const reader = document.querySelector(".reader"); const page = document.querySelector(".page"); const readerRect = reader?.getBoundingClientRect(); const pageRect = page?.getBoundingClientRect(); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { mode: reader?.dataset.readingMode || null, stable: document.documentElement.dataset.viewportStable === expected, centered: Boolean(pageRect && readerRect && Math.abs((pageRect.left - readerRect.left) - (readerRect.right - pageRect.right)) <= 2), pageWidth: pageRect?.width || 0, overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth, error: document.documentElement.dataset.error || null };' \
    '.mode == "scroll" and .stable == true and .centered == true and .pageWidth >= 700 and .pageWidth <= 761 and .overflow == false and .error == null' \
    'scroll mode did not preserve the centered reading measure' >/dev/null

  set_window_size 360 760
  narrow=$(wait_for_script \
    'const reader = document.querySelector(".reader"); const page = document.querySelector(".page"); const readerRect = reader?.getBoundingClientRect(); const pageRect = page?.getBoundingClientRect(); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { desktop: document.documentElement.hasAttribute("data-desktop-workspace"), active: document.documentElement.dataset.workspacePanel || null, stable: document.documentElement.dataset.viewportStable === expected, centered: Boolean(pageRect && readerRect && Math.abs((pageRect.left - readerRect.left) - (readerRect.right - pageRect.right)) <= 2), pageWidth: pageRect?.width || 0, pageMargin: reader ? parseFloat(getComputedStyle(reader).getPropertyValue("--page-left-margin")) / devicePixelRatio : 0, overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth, error: document.documentElement.dataset.error || null };' \
    '.desktop == false and .active == null and .stable == true and .centered == true and .pageWidth >= 280 and .pageWidth <= 300 and .pageMargin >= 31 and .pageMargin <= 33 and .overflow == false and .error == null' \
    'narrow reader did not preserve its visual page margins')

  set_window_size 600 760
  wait_for_script \
    'const frame = document.querySelector(".reader-frame"); const reader = document.querySelector(".reader"); const frameRect = frame?.getBoundingClientRect(); const readerRect = reader?.getBoundingClientRect(); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { desktop: document.documentElement.hasAttribute("data-desktop-workspace"), workspace: document.documentElement.dataset.workspacePanel || null, openPanels: document.querySelectorAll(".reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]").length, stable: document.documentElement.dataset.viewportStable === expected, full: Boolean(frameRect && readerRect && Math.abs(frameRect.left) <= 1 && Math.abs(frameRect.width - innerWidth) <= 2 && Math.abs(readerRect.left - frameRect.left) <= 1 && Math.abs(readerRect.width - frameRect.width) <= 1), error: document.documentElement.dataset.error || null };' \
    '.desktop == false and .workspace == null and .openPanels == 0 and .stable == true and .full == true and .error == null' \
    'narrow reader did not restore the mobile tool layout' >/dev/null
  for tool in directory search; do
    execute_sync "
document.documentElement.setAttribute('data-reader-tools', '');
document.querySelector('.reader-tool.$tool > summary')?.click();
return true;" >/dev/null || die "could not open the narrow $tool tool"
    wait_for_script \
      "const details = document.querySelector('.reader-tool.$tool'); const rect = details?.querySelector('.tool-panel')?.getBoundingClientRect(); return { open: Boolean(details?.open), only: document.querySelectorAll('.reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]').length === 1, layout: Boolean(rect && Math.abs(rect.left) <= 1 && Math.abs(rect.width - innerWidth) <= 2 && rect.top > 0 && Math.abs(rect.bottom - innerHeight) <= 2 && Math.abs(rect.height - visualViewport.height * 0.82) <= 2), overflow: Boolean(rect && details.querySelector('.tool-panel').scrollWidth > rect.width + 1) };" \
      '.open == true and .only == true and .layout == true and .overflow == false' \
      "narrow $tool tool no longer uses the mobile layout" >/dev/null
    execute_sync "document.querySelector('.reader-tool.$tool').open = false; document.documentElement.removeAttribute('data-reader-tools'); return true;" >/dev/null ||
      die "could not close the narrow $tool tool"
  done

  set_window_size 360 760
  wait_for_script \
    'const reader = document.querySelector(".reader"); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { desktop: document.documentElement.hasAttribute("data-desktop-workspace"), stable: document.documentElement.dataset.viewportStable === expected, openPanels: document.querySelectorAll(".reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]").length };' \
    '.desktop == false and .stable == true and .openPanels == 0' \
    'portrait reader did not stabilize before opening notes' >/dev/null
  execute_sync '
document.documentElement.setAttribute("data-reader-tools", "");
document.querySelector(".reader-tool.notes > summary")?.click();
return true;' >/dev/null || die 'could not open the portrait notes window'
  portrait_notes=$(wait_for_script \
    'const panel = document.querySelector(".notes-panel")?.getBoundingClientRect(); const list = document.querySelector(".annotation-list"); const controls = [...document.querySelectorAll(".notes-panel button, .notes-panel input, .notes-panel select")]; return { viewport: {width: innerWidth, height: innerHeight}, open: document.querySelector(".reader-tool.notes")?.open || false, heading: document.querySelector("#annotation-heading")?.textContent || "", search: document.querySelector("#annotation-filter-query")?.placeholder || "", listLabel: list?.getAttribute("aria-label") || "", items: document.querySelectorAll(".annotation-item-main").length, panel: panel ? {left: panel.left, right: panel.right, top: panel.top, bottom: panel.bottom, width: panel.width, height: panel.height} : null, scroll: list?.scrollTop || 0, overflow: document.querySelector(".notes-panel")?.scrollWidth > document.querySelector(".notes-panel")?.clientWidth, clipped: controls.some((control) => control.scrollWidth > control.clientWidth || control.scrollHeight > control.clientHeight), locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null };' \
    '.open == true and .heading == "笔记" and .search == "搜索笔记或标注" and .listLabel == "笔记和标注列表" and .items > 0 and ((.panel.left - 12) | fabs) <= 2 and ((.panel.right - (.viewport.width - 12)) | fabs) <= 2 and ((.panel.top - 12) | fabs) <= 2 and ((.panel.bottom - (.viewport.height - 12)) | fabs) <= 2 and ((.panel.width - (.viewport.width - 24)) | fabs) <= 2 and ((.panel.height - (.viewport.height - 24)) | fabs) <= 2 and .overflow == false and .clipped == false and .locator != null' \
    'portrait notes list page did not become ready')
  capture_public_ui reader-notes
  execute_sync 'document.querySelector(".annotation-item-main")?.click(); return true;' >/dev/null ||
    die 'could not open the portrait message thread page'
  portrait_thread=$(wait_for_script \
    'const notes = document.querySelector(".notes-panel")?.getBoundingClientRect(); const overlay = document.querySelector("#message-conversation"); const thread = overlay?.getBoundingClientRect(); const composer = document.querySelector("#message-composer")?.getBoundingClientRect(); const controls = [...(overlay?.querySelectorAll("button") || [])].filter((control) => control.getClientRects().length); const clippedControls = controls.filter((control) => control.scrollWidth > control.clientWidth || control.scrollHeight > control.clientHeight).map((control) => ({className: control.className, text: control.textContent.trim().slice(0, 40), client: [control.clientWidth, control.clientHeight], scroll: [control.scrollWidth, control.scrollHeight]})); return { open: Boolean(overlay && !overlay.hidden), notesOpen: document.querySelector(".reader-tool.notes")?.open || false, title: document.querySelector("#message-conversation-title")?.textContent || "", scopes: [...document.querySelectorAll("[data-message-scope]")].map((button) => button.textContent.trim()), closeLabel: document.querySelector("#message-conversation-close")?.getAttribute("aria-label") || "", selected: document.querySelectorAll(".annotation-item[data-selected=true]").length, notes: notes ? {left: notes.left, right: notes.right, top: notes.top, bottom: notes.bottom} : null, thread: thread ? {left: thread.left, right: thread.right, top: thread.top, bottom: thread.bottom} : null, composerBottom: composer?.bottom ?? -1, fullscreenHidden: document.querySelector("#message-conversation-fullscreen")?.getClientRects().length == 0, overflow: overlay ? overlay.scrollWidth > overlay.clientWidth : true, clipped: clippedControls.length > 0, clippedControls, locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null, error: document.documentElement.dataset.error || null };' \
    '.open == true and .notesOpen == true and .title == "对话" and .scopes == ["当前", "本章", "本书"] and .closeLabel == "返回笔记" and .selected == 1 and ((.thread.left - .notes.left) | fabs) <= 2 and ((.thread.right - .notes.right) | fabs) <= 2 and ((.thread.top - .notes.top) | fabs) <= 2 and ((.thread.bottom - .notes.bottom) | fabs) <= 2 and ((.composerBottom - .thread.bottom) | fabs) <= 2 and .fullscreenHidden == true and .overflow == false and .clipped == false and .locator != null and .error == null' \
    'portrait message thread did not reuse the notes window geometry')
  [[ $(jq -r '.locator' <<<"$portrait_thread") == "$(jq -r '.locator' <<<"$portrait_notes")" ]] ||
    die 'opening the portrait thread changed the current locator'
  capture_public_ui reader-thread
  execute_sync 'document.querySelector("#message-conversation-close")?.click(); return true;' >/dev/null ||
    die 'could not return from the portrait message thread'
  portrait_return=$(wait_for_script \
    'const list = document.querySelector(".annotation-list"); const selected = document.querySelector(".annotation-item[data-selected=true] .annotation-item-main"); return { threadOpen: !document.querySelector("#message-conversation")?.hidden, notesOpen: document.querySelector(".reader-tool.notes")?.open || false, selected: document.querySelectorAll(".annotation-item[data-selected=true]").length, focused: document.activeElement === selected, scroll: list?.scrollTop || 0, locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null };' \
    '.threadOpen == false and .notesOpen == true and .selected == 1 and .focused == true and .locator != null' \
    'portrait thread back did not restore the notes list page')
  [[ $(jq '.scroll' <<<"$portrait_return") == "$(jq '.scroll' <<<"$portrait_notes")" ]] ||
    die 'portrait thread back reset the notes list scroll position'
  [[ $(jq -r '.locator' <<<"$portrait_return") == "$(jq -r '.locator' <<<"$portrait_thread")" ]] ||
    die 'closing the portrait thread changed the current locator'
  execute_sync 'document.querySelector(".notes-panel [data-close-reader-tools]")?.click(); return true;' >/dev/null ||
    die 'could not close the portrait notes window'
  wait_for_script \
    'return { notesOpen: document.querySelector(".reader-tool.notes")?.open || false, tools: document.documentElement.hasAttribute("data-reader-tools"), focused: document.activeElement === document.querySelector(".reader") };' \
    '.notesOpen == false and .tools == false and .focused == true' \
    'portrait notes back did not restore reading' >/dev/null

  [[ $(execute_sync '
document.documentElement.removeAttribute("data-reader-tools");
const reader = document.querySelector(".reader");
const rect = reader.getBoundingClientRect();
const center = new PointerEvent("pointerdown", {pointerId: 51, pointerType: "mouse", isPrimary: true, button: 0, bubbles: true, clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2});
reader.dispatchEvent(center);
reader.dispatchEvent(new PointerEvent("pointerup", {pointerId: 51, pointerType: "mouse", isPrimary: true, button: 0, bubbles: true, clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2}));
return document.documentElement.hasAttribute("data-reader-tools");') == true ]] ||
    die 'narrow center click did not reveal the mobile tools'
  [[ $(execute_sync '
const reader = document.querySelector(".reader");
const rect = reader.getBoundingClientRect();
reader.dispatchEvent(new PointerEvent("pointerdown", {pointerId: 52, pointerType: "mouse", isPrimary: true, button: 0, bubbles: true, clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2}));
reader.dispatchEvent(new PointerEvent("pointerup", {pointerId: 52, pointerType: "mouse", isPrimary: true, button: 0, bubbles: true, clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2}));
return document.documentElement.hasAttribute("data-reader-tools");') == false ]] ||
    die 'narrow center click did not dismiss the mobile tools'

  desktop_workspace_evidence=$(jq -n \
    --argjson searchResults "$(jq '.count' <<<"$search")" \
    --argjson messages "$(jq '.items' <<<"$notes")" \
    --argjson desktopPageWidth "$(jq '.pageWidth' <<<"$baseline")" \
    --argjson workspacePageWidth "$(jq '.pageWidth' <<<"$workspace")" \
    --argjson mediumPageWidth "$(jq '.pageWidth' <<<"$medium")" \
    --argjson narrowPageWidth "$(jq '.pageWidth' <<<"$narrow")" \
    --argjson pageMargin "$(jq '.pageMargin' <<<"$baseline")" \
    --argjson twoPane "$notes_two" \
    --argjson threePane "$notes_wide" \
    --argjson mediumPane "$medium_thread" \
    --argjson portrait "$portrait_thread" \
    '{widths: [360, 600, 1000, 1280, 1600], defaultMode: "focus-reading", workspaceMode: "explicit", tools: ["directory", "search", "notes"], pageWidths: {desktop: $desktopPageWidth, workspace: $workspacePageWidth, medium: $mediumPageWidth, narrow: $narrowPageWidth}, pageMarginCss: $pageMargin, noteLayouts: {portraitPages: $portrait, mediumBC: $mediumPane, desktopBC: $twoPane, wideABC: $threePane}, pagedAndScrollMeasure: true, searchResults: $searchResults, messages: $messages, locatorStable: true, keyboard: true, focusVerified: true, narrowRegression: true}')
}

verify_reader_redesign() {
  local width height result saved point selector before bookmark expected
  saved=$(execute_sync 'return localStorage.getItem("atha.reader.application.v1");')
  execute_sync '
const mode = document.querySelector("#reading-mode");
document.querySelector("#preferences-status").textContent = "";
mode.value = "paged";
mode.dispatchEvent(new Event("change", {bubbles: true}));
return true;' >/dev/null
  wait_for_script \
    'return {mode: document.querySelector(".reader").dataset.readingMode, applied: document.querySelector("#preferences-status").textContent};' \
    '.mode == "paged" and .applied == "已应用"' \
    'paged controls did not become ready after the scroll-mode check' >/dev/null
  for width in 360 1000 1600; do
    height=760
    ((width != 1600)) || height=900
    set_window_size "$width" "$height"
    execute_sync '
document.querySelectorAll(".reader-tool[open]").forEach(tool => { tool.open = false; });
document.documentElement.setAttribute("data-reader-tools", "");
return true;' >/dev/null
    result=$(wait_for_script '
const bar = document.querySelector(".toolbar").getBoundingClientRect();
const page = document.querySelector(".page").getBoundingClientRect();
const chapter = document.querySelector("#reader-toolbar-chapter").textContent;
const progress = document.querySelector("#reader-toolbar-progress").textContent;
return {chapter, synced: chapter === document.querySelector("#progress-chapter").textContent && progress === document.querySelector("#progress-position").textContent, valid: !/NaN/.test(progress), fits: bar.left >= 0 && bar.right <= innerWidth && bar.bottom <= innerHeight && bar.width <= 401 && bar.top >= page.bottom, targets: [...document.querySelectorAll(".toolbar > .reader-tool > summary")].every(el => { const r = el.getBoundingClientRect(); return r.width >= 44 && r.height >= 44; })};' \
      '.chapter != "" and .synced and .valid and .fits and .targets' \
      'reader context or compact toolbar did not fit')
    capture_public_ui reader-main
    bookmark=$(execute_sync 'return document.querySelector("#add-bookmark").getAttribute("aria-pressed");')
    for expected in "$(jq -r 'if . == "true" then "false" else "true" end' <<<"$bookmark")" "$(jq -r . <<<"$bookmark")"; do
      point=$(execute_sync 'const r = document.querySelector("#add-bookmark").getBoundingClientRect(); return {x: r.left + r.width / 2, y: r.top + r.height / 2};')
      perform_pointer_action "$point" tap || die 'could not toggle reader bookmark'
      wait_for_script 'return {pressed: document.querySelector("#add-bookmark").getAttribute("aria-pressed")};' \
        ".pressed == \"$expected\"" 'bookmark did not toggle and restore' >/dev/null
    done
    before=$(execute_sync 'return document.querySelector("#progress-position").textContent;')
    execute_sync 'document.querySelector(".progress > summary").click(); return true;' >/dev/null
    for selector in next previous; do
      point=$(execute_sync 'const r = document.getElementById(arguments[0]).getBoundingClientRect(); return {x: r.left + r.width / 2, y: r.top + r.height / 2};' "[\"$selector\"]")
      perform_pointer_action "$point" tap || die "could not click reader $selector"
      expected=".position != $before"
      [[ $selector != previous ]] || expected=".position == $before"
      wait_for_script 'return {position: document.querySelector("#progress-position").textContent};' \
        "$expected" 'previous/next controls did not navigate and return' >/dev/null
    done
    point=$(execute_sync '
const range = document.querySelector("#progress-range");
const r = range.getBoundingClientRect();
return {x: r.left + 8 + (r.width - 16) * Number(range.value) / Number(range.max), y: r.top + r.height / 2, endX: r.right - 16, endY: r.top + r.height / 2};')
    perform_pointer_action "$point" drag || die 'could not drag reading progress'
    wait_for_script 'return {position: document.querySelector("#progress-position").textContent};' \
      ".position != $before" 'progress drag did not navigate' >/dev/null
    point=$(execute_sync '
const range = document.querySelector("#progress-range");
const r = range.getBoundingClientRect();
return {x: r.left + 8 + (r.width - 16) * Number(range.value) / Number(range.max), y: r.top + r.height / 2, endX: r.left + 2, endY: r.top + r.height / 2};')
    perform_pointer_action "$point" drag || die 'could not restore reading progress'
    wait_for_script 'return {position: document.querySelector("#progress-position").textContent};' \
      ".position == $before" 'progress drag did not return to the opening page' >/dev/null
    capture_public_ui reader-progress
    execute_sync 'document.querySelector(".progress > summary").click(); return true;' >/dev/null
    execute_sync 'document.querySelector(".preferences > summary").click(); return true;' >/dev/null
    wait_for_script '
const panel = document.querySelector(".preferences-panel");
const elements = [document.querySelector("#font-size"), ...document.querySelectorAll("[data-preference-for=theme]")];
const outer = panel.getBoundingClientRect();
return {fits: elements.length === 5 && elements.every(el => { const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0 && r.top >= outer.top && r.bottom <= outer.bottom && r.left >= 0 && r.right <= innerWidth; }), overflow: panel.scrollWidth > panel.clientWidth};' \
      '.fits and (.overflow | not)' 'quick reading settings were not visible' >/dev/null
    capture_public_ui reader-quick-settings
    execute_sync '
const size = document.querySelector("#font-size");
size.value = "24";
size.dispatchEvent(new Event("change", {bubbles: true}));
document.querySelector("[data-preference-for=theme][data-preference-value=dark]").click();
return true;' >/dev/null
    wait_for_script '
const prefs = JSON.parse(localStorage.getItem("atha.reader.application.v1") || "null")?.preferences;
return {saved: prefs?.fontSize === 24 && prefs?.theme === "dark", applied: document.querySelector("#theme").value === "dark" && document.querySelector("#font-size-value").textContent.includes("24")};' \
      '.saved and .applied' 'quick reading settings did not save' >/dev/null
    webdriver_value POST "/session/$session_id/refresh" '{}' >/dev/null
    wait_for_script '
return {ready: document.documentElement.hasAttribute("data-reader-ready"), size: document.querySelector("#font-size")?.value, theme: document.querySelector("#theme")?.value};' \
      '.ready and .size == "24" and .theme == "dark"' 'quick reading settings did not survive reload' >/dev/null
    execute_sync '
const original = arguments[0];
if (original === null) localStorage.removeItem("atha.reader.application.v1");
else localStorage.setItem("atha.reader.application.v1", original);
return true;' "$(jq -cn --argjson saved "$saved" '[$saved]')" >/dev/null
    webdriver_value POST "/session/$session_id/refresh" '{}' >/dev/null
    wait_for_script 'return {ready: document.documentElement.hasAttribute("data-reader-ready")};' \
      '.ready == true' 'restored reader did not become ready' >/dev/null
    [[ $(execute_sync 'return localStorage.getItem("atha.reader.application.v1");') == "$saved" ]] ||
      die 'quick reading settings did not restore the original record'
  done
  say 'reader redesign: 360/1000/1600 chrome, quick controls, save/reload/restore passed'
}

verify_reader_runtime_errors() {
  local errors sentinel
  sentinel=$(execute_sync '
const before = globalThis.__athaReaderRuntimeErrors?.length ?? -1;
console.error("atha-reader-console-probe");
const after = globalThis.__athaReaderRuntimeErrors?.length ?? -1;
globalThis.__athaReaderRuntimeErrors?.pop();
return {before, after};') || die 'could not probe reader console error collection'
  jq -e '.before >= 0 and .after == .before + 1' <<<"$sentinel" >/dev/null ||
    die "reader console errors are not observable (state: $(jq -c . <<<"$sentinel"))"
  errors=$(execute_sync 'return globalThis.__athaReaderRuntimeErrors || null;') ||
    die 'could not inspect reader runtime errors'
  jq -e 'type == "array" and length == 0' <<<"$errors" >/dev/null ||
    die "reader emitted runtime errors (state: $(jq -c . <<<"$errors"))"
}

verify_reader_interface_navigation() {
  local media image_point backdrop_point zoom pinch preview notes keyboard menu closed settings
  media=$(execute_async '
const done = arguments[arguments.length - 1];
Promise.resolve(globalThis.__athaReaderDiagnostics?.mediaPoint("ordinary"))
  .then(async () => {
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    return globalThis.__athaReaderDiagnostics?.mediaPoint("ordinary");
  })
  .then((value) => done({ok: Boolean(value), value}))
  .catch(() => done({ok: false}));') || die 'could not locate the public reader image'
  jq -e '.ok == true and (.value.x | type == "number") and (.value.y | type == "number")' \
    <<<"$media" >/dev/null || die 'public reader image is unavailable for preview navigation'
  perform_pointer_action "$(jq -c '.value' <<<"$media")" tap ||
    die 'could not open the image preview with a trusted pointer'
  wait_for_script \
    'return {open: document.querySelector("#content-dialog")?.open, media: document.querySelector("#content-dialog")?.classList.contains("media-preview")};' \
    '.open == true and .media == true' \
    'image preview did not open' >/dev/null

  [[ $(execute_sync '
const viewport = document.querySelector("#content-dialog-viewport");
const rect = viewport?.getBoundingClientRect();
if (!viewport || !rect) return false;
return !viewport.dispatchEvent(new WheelEvent("wheel", {bubbles: true, cancelable: true, deltaY: -120, clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2}));') == true ]] ||
    die 'preview wheel zoom did not prevent native scrolling'
  zoom=$(wait_for_script \
    'return {label: document.querySelector(".content-dialog-zoom-label")?.textContent || "", scale: document.querySelector("#content-dialog")?.style.getPropertyValue("--content-viewer-scale").trim() || ""};' \
    '.label != "100%" and (.scale | tonumber) > 1' \
    'preview wheel zoom did not update its scale')
  [[ $(execute_sync 'document.querySelector("button[aria-label=\"恢复原始大小\"]")?.click(); return true;') == true ]] ||
    die 'could not reset preview wheel zoom'
  [[ $(execute_sync '
const viewport = document.querySelector("#content-dialog-viewport");
const rect = viewport?.getBoundingClientRect();
if (!viewport || !rect) return false;
const event = (type, touches) => {
  const value = new Event(type, {bubbles: true, cancelable: true});
  Object.defineProperty(value, "touches", {value: touches});
  return viewport.dispatchEvent(value);
};
const y = rect.top + rect.height / 2;
event("touchstart", [{clientX: rect.left + 120, clientY: y}, {clientX: rect.left + 220, clientY: y}]);
const accepted = event("touchmove", [{clientX: rect.left + 90, clientY: y}, {clientX: rect.left + 250, clientY: y}]);
event("touchend", []);
return !accepted;') == true ]] || die 'preview pinch zoom did not prevent native scaling'
  pinch=$(wait_for_script \
    'return {label: document.querySelector(".content-dialog-zoom-label")?.textContent || "", scale: document.querySelector("#content-dialog")?.style.getPropertyValue("--content-viewer-scale").trim() || ""};' \
    '(.scale | tonumber) > 1.5' \
    'preview pinch zoom did not update its scale')
  execute_sync 'document.querySelector("button[aria-label=\"恢复原始大小\"]")?.click(); return true;' >/dev/null ||
    die 'could not reset preview pinch zoom'

  image_point=$(execute_sync '
const rect = document.querySelector("#content-dialog-image")?.getBoundingClientRect();
return rect ? {x: rect.left + rect.width / 2, y: rect.top + rect.height / 2} : null;') ||
    die 'could not locate the preview image'
  perform_pointer_action "$image_point" tap || die 'could not click the preview image'
  preview=$(execute_sync 'return globalThis.__athaReaderDiagnostics.previewState();') ||
    die 'could not inspect the image preview'
  jq -e '.open == true' <<<"$preview" >/dev/null || die 'clicking the preview image closed it'

  backdrop_point=$(execute_sync '
const viewport = document.querySelector("#content-dialog-viewport");
const rect = viewport?.getBoundingClientRect();
if (!rect) return null;
const x = rect.left + 8, y = rect.top + viewport.clientHeight / 2;
return document.elementFromPoint(x, y) === viewport ? {x, y} : null;') ||
    die 'could not locate the image backdrop'
  [[ $backdrop_point != null ]] || die 'image backdrop is covered at its inner padding'
  perform_pointer_action "$backdrop_point" tap || die 'could not click the image backdrop'
  preview=$(wait_for_script \
    'return globalThis.__athaReaderDiagnostics.previewState();' \
    '.open == false and .focusRestored == true' \
    'image backdrop did not close the preview') || die 'image preview navigation failed'

  set_window_size 600 760
  wait_for_script \
    'const reader = document.querySelector(".reader"); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return {width: innerWidth, height: innerHeight, desktop: document.documentElement.hasAttribute("data-desktop-workspace"), stable: document.documentElement.dataset.viewportStable === expected};' \
    '.width >= 598 and .width <= 602 and .height >= 696 and .height <= 762 and .desktop == false and .stable == true' \
    'portrait interface viewport did not stabilize' >/dev/null

  [[ $(execute_sync '
document.documentElement.setAttribute("data-reader-tools", "");
document.querySelector(".reader-tool.notes > summary")?.click();
return true;') == true ]] || die 'could not open notes'
  notes=$(wait_for_script '
const panel = document.querySelector(".notes-panel");
const back = panel?.querySelector("[data-close-reader-tools]");
const backRect = back?.getBoundingClientRect();
return {
  open: document.querySelector(".reader-tool.notes")?.open,
  backIcon: Boolean(back?.querySelector("svg")),
  backVisible: Boolean(backRect && backRect.width >= 44 && backRect.height >= 44),
  exportHidden: document.querySelector("#message-export-all")?.getClientRects().length == 0,
  topbarDictionary: Boolean(document.querySelector(".top-toolbar .reader-tool.dictionary")),
  settingsIcon: Boolean(document.querySelector(".top-toolbar .reader-tool.preferences > summary svg")),
  bottomSettingsIcon: Boolean(document.querySelector(".toolbar .reader-tool.preferences > summary svg")),
  settingsInBottomBar: document.querySelector(".reader-tool.preferences")?.parentElement?.matches(".toolbar") || false,
  overflow: panel ? panel.scrollWidth > panel.clientWidth : true
};' \
    '.open == true and .backIcon == true and .backVisible == true and .exportHidden == true and .topbarDictionary == false and .settingsIcon == false and .bottomSettingsIcon == true and .settingsInBottomBar == true and .overflow == false' \
    'notes navigation did not become ready')

  keyboard=$(execute_async '
const done = arguments[arguments.length - 1];
const root = document.documentElement;
const overlay = document.querySelector("#message-conversation");
const composer = document.querySelector("#message-composer");
const noteDialog = document.querySelector("#annotation-note-dialog");
const noteInput = document.querySelector("#annotation-note");
const noteActions = document.querySelector(".annotation-dialog-actions");
if (!overlay || !composer || !noteDialog || !noteInput || !noteActions || !window.visualViewport) return done({bound: false});
const hidden = overlay.hidden;
const height = root.style.getPropertyValue("--visual-viewport-height");
const top = root.style.getPropertyValue("--visual-viewport-top");
const center = root.style.getPropertyValue("--visual-viewport-center");
const bound = root.hasAttribute("data-visual-viewport") && Math.abs(parseFloat(height) - window.visualViewport.height) < 1 && Math.abs(parseFloat(top) - window.visualViewport.offsetTop) < 1;
overlay.hidden = false;
root.style.setProperty("--visual-viewport-height", "320px");
root.style.setProperty("--visual-viewport-top", "24px");
root.style.setProperty("--visual-viewport-center", "184px");
noteDialog.showModal();
noteInput.focus();
requestAnimationFrame(() => {
  const overlayRect = overlay.getBoundingClientRect();
  const composerRect = composer.getBoundingClientRect();
  const noteRect = noteDialog.getBoundingClientRect();
  const inputRect = noteInput.getBoundingClientRect();
  const actionsRect = noteActions.getBoundingClientRect();
  const messageVisible = overlayRect.top >= 23 && overlayRect.bottom <= 345 && composerRect.bottom <= 345;
  const noteVisible = noteRect.top >= 23 && noteRect.bottom <= 345 && inputRect.bottom <= 345 && actionsRect.bottom <= 345;
  noteDialog.close();
  overlay.hidden = hidden;
  root.style.setProperty("--visual-viewport-height", height);
  root.style.setProperty("--visual-viewport-top", top);
  root.style.setProperty("--visual-viewport-center", center);
  done({bound, messageVisible, noteVisible, overlayBottom: overlayRect.bottom, composerBottom: composerRect.bottom, noteBottom: noteRect.bottom, noteActionsBottom: actionsRect.bottom});
});') || die 'could not inspect the visual viewport editor layouts'
  jq -e '.bound == true and .messageVisible == true and .noteVisible == true' <<<"$keyboard" >/dev/null ||
    die "note editors did not stay in the visual viewport (state: $(jq -c . <<<"$keyboard"))"

  [[ $(execute_sync 'document.querySelector(".notes-actions > summary")?.click(); return true;') == true ]] ||
    die 'could not open the notes actions'
  menu=$(wait_for_script '
const details = document.querySelector(".notes-actions");
const button = document.querySelector("#message-export-all");
const rect = button?.getBoundingClientRect();
return {open: details?.open, label: button?.textContent.trim(), visible: Boolean(rect && rect.width >= 44 && rect.height >= 44)};' \
    '.open == true and .label == "导出笔记" and .visible == true' \
    'notes export action did not become ready')
  closed=$(execute_sync '
document.querySelector(".notes-actions > summary")?.click();
document.querySelector(".notes-panel [data-close-reader-tools]")?.click();
return {
  open: document.querySelector(".reader-tool.notes")?.open,
  focused: document.activeElement === document.querySelector(".reader")
};') || die 'could not return from notes'
  jq -e '.open == false and .focused == true' <<<"$closed" >/dev/null ||
    die 'notes back button did not return to reading'

  settings=$(execute_sync '
document.documentElement.setAttribute("data-reader-tools", "");
const tool = document.querySelector(".toolbar .reader-tool.preferences");
tool?.querySelector("summary")?.click();
const panel = tool?.querySelector(".preferences-panel");
const rect = panel?.getBoundingClientRect();
const result = {open: tool?.open || false, bottom: tool?.parentElement?.matches(".toolbar") || false, visible: Boolean(rect && rect.width > 0 && rect.height > 0)};
tool?.querySelector("summary")?.click();
document.documentElement.removeAttribute("data-reader-tools");
return result;') || die 'could not open reading settings from the bottom bar'
  jq -e '.open == true and .bottom == true and .visible == true' <<<"$settings" >/dev/null ||
    die "reading settings did not open from the bottom bar (state: $(jq -c . <<<"$settings"))"

  jq -cn --argjson preview "$preview" --argjson zoom "$zoom" --argjson pinch "$pinch" \
    --argjson notes "$notes" --argjson keyboard "$keyboard" --argjson menu "$menu" \
    --argjson settings "$settings" \
    '{imageBackdrop: $preview, zoom: {wheel: $zoom, pinch: $pinch}, notes: $notes, keyboard: $keyboard, exportMenu: $menu, settings: $settings, contextualDictionary: true}'
}

open_book() {
  local kind=$1 arguments script
  arguments=$(jq -cn --arg kind "$kind" '[$kind]')
  read -r -d '' script <<'JS' || true
const publicTitle = "Atha FB2 Gate";
const cards = [...document.querySelectorAll(".library-book")];
const card = cards.find((item) => {
  const title = item.querySelector(".library-book-title")?.textContent || "";
  return arguments[0] === "public" ? title === publicTitle : title !== publicTitle;
});
if (!card) return false;
card.querySelector(".library-book-open").click();
return true;
JS
  [[ $(execute_sync "$script" "$arguments") == true ]] || die 'could not open the seeded book'
  wait_for_script \
    'return { status: document.documentElement.dataset.status || null, error: document.documentElement.dataset.error || null, reader: location.pathname.endsWith("/index.html"), ready: document.documentElement.hasAttribute("data-reader-ready"), busy: document.querySelector(".reader-shell")?.getAttribute("aria-busy"), startupHidden: document.querySelector(".reader-startup")?.getAttribute("aria-hidden") };' \
    '.status == "pass" and .reader == true and .ready == true and .busy == "false" and .startupHidden == "true"' \
    'reader did not become ready' >/dev/null
  current_book_kind=$kind
  diagnostics_enabled=0
}

enable_diagnostics() {
  local script
  read -r -d '' script <<'JS' || true
const url = new URL(location.href);
url.searchParams.set("verify-import", "1");
url.searchParams.set("gesture-probe", "1");
url.searchParams.set("probe", "https://example.com/blocked.png");
location.replace(url.href);
return true;
JS
  execute_sync "$script" >/dev/null || die 'could not enable reader diagnostics'
  wait_for_script \
    'const diagnostics = globalThis.__athaReaderDiagnostics; const snapshot = diagnostics?.snapshot(); return { status: document.documentElement.dataset.status || null, error: document.documentElement.dataset.error || null, available: Boolean(diagnostics?.previousBoundaryProbe && diagnostics?.beginGestureProbe && diagnostics?.pendingFormulaQueueProbe), fontStatus: document.fonts.status, pages: snapshot?.pages ?? null, ordinaryImages: snapshot?.ordinaryCount ?? null, session: snapshot ? { state: snapshot.session.state, section: snapshot.session.currentIndex, contentLoads: snapshot.session.contentLoads, stableLayouts: snapshot.session.stableLayouts, releasedSections: snapshot.session.releasedSections } : null, resources: snapshot ? { pending: snapshot.resources.pending, currentPending: snapshot.resources.currentPending, warming: snapshot.resources.warming, warmDurationMs: snapshot.resources.warmDurationMs } : null };' \
    '.status == "pass" and .available == true' \
    'reader verify diagnostics failed' >/dev/null
  diagnostics_enabled=1
}

pending_formula_queue_probe() {
  local script result
  read -r -d '' script <<'JS' || true
const done = arguments[arguments.length - 1];
globalThis.__athaReaderDiagnostics.pendingFormulaQueueProbe()
  .then((value) => done({ ok: true, value }))
  .catch(() => done({ ok: false }));
JS
  result=$(execute_async "$script") || die 'pending-formula queue diagnostic failed'
  jq -e '
    .ok == true and
    .value.formulas == 8 and
    .value.maxInFlight == 3 and
    .value.startedBeforeClose == 3 and
    .value.notStartedAfterClose == 5 and
    .value.abortedInFlight == 3 and
    .value.lateDetachedWrites == 0 and
    .value.reopenedImages == 8 and
    .value.reopenedPending == 0
  ' <<<"$result" >/dev/null ||
    die "pending-formula queue invariant failed (state: $(jq -c '.value' <<<"$result"))"
  jq -c '.value' <<<"$result"
}

open_gesture_section() {
  local href=$1 warm=${2:-true} script arguments result
  [[ $warm == true || $warm == false ]] || die 'gesture warm flag must be true or false'
  arguments=$(jq -cn --arg href "$href" --argjson warm "$warm" '[$href, $warm]')
  read -r -d '' script <<'JS' || true
const done = arguments[arguments.length - 1];
globalThis.__athaReaderDiagnostics.openGestureSection(arguments[0], arguments[1])
  .then((value) => done({ ok: true, value }))
  .catch(() => done({ ok: false }));
JS
  result=$(execute_async "$script" "$arguments") || die 'could not open the diagnostic section'
  jq -e '.ok == true and (.value.section | type == "number") and .value.section > 0' <<<"$result" >/dev/null ||
    die 'diagnostic section is not eligible for a previous-boundary probe'
  printf '%s\n' "$result"
}

scroll_resource_probe() {
  local script result
  read -r -d '' script <<'JS' || true
const done = arguments[arguments.length - 1];
globalThis.__athaReaderDiagnostics.scrollResourceProbe()
  .then((value) => done({ ok: true, value }))
  .catch(() => done({ ok: false }));
JS
  result=$(execute_async "$script") || die 'scroll-resource diagnostic failed'
  jq -e '
    .ok == true and .value.candidate == true and .value.loaded == true and
    .value.pendingAfter < .value.pendingBefore and .value.offsetPreserved == true and
    .value.relayoutPreserved == true
  ' <<<"$result" >/dev/null ||
    die "scroll-resource invariant failed (state: $(jq -c '.value' <<<"$result"))"
  jq -c '.value' <<<"$result"
}

previous_boundary_probe() {
  local script result
  read -r -d '' script <<'JS' || true
const done = arguments[arguments.length - 1];
globalThis.__athaReaderDiagnostics.previousBoundaryProbe()
  .then((value) => done({ ok: true, value }))
  .catch(() => done({ ok: false }));
JS
  result=$(execute_async "$script") || die 'previous-boundary diagnostic failed'
  jq -e '
    .ok == true and .value.moved == true and
    .value.sectionDelta == -1 and
    .value.page == .value.lastContentPage and
    .value.page == (.value.pages - 1) and
    .value.emptyTail.scrollPages > .value.emptyTail.expectedPages and
    .value.emptyTail.pages == .value.emptyTail.expectedPages and
    ((.value.settling // false) == false)
  ' <<<"$result" >/dev/null ||
    die "previous-boundary page invariant failed (state: $(jq -c '.value | {sectionDelta, page, pages, lastContentPage, emptyTail}' <<<"$result"))"
  jq -c '.value | {sectionDelta, page, pages, lastContentPage, emptyTail}' <<<"$result"
}

perform_pointer_action() {
  local point=$1 action=$2 action_id payload result=0
  wait_for_script \
    'return { visibility: document.visibilityState };' \
    '.visibility == "visible"' \
    'Linux GUI target is not visible for trusted pointer input' >/dev/null
  action_id="atha-pointer-${BASHPID}-${RANDOM}"
  if [[ $action == tap ]]; then
    payload=$(jq -cn --arg id "$action_id" --argjson point "$point" '{
      actions: [{
        type: "pointer", id: $id, parameters: {pointerType: "mouse"},
        actions: [
          {type: "pointerMove", duration: 0, origin: "viewport", x: ($point.x | round), y: ($point.y | round)},
          {type: "pointerDown", button: 0},
          {type: "pause", duration: 40},
          {type: "pointerUp", button: 0}
        ]
      }]
    }')
  else
    payload=$(jq -cn --arg id "$action_id" --argjson point "$point" '{
      actions: [{
        type: "pointer", id: $id, parameters: {pointerType: "mouse"},
        actions: (
          [
            {type: "pointerMove", duration: 0, origin: "viewport", x: ($point.x | round), y: ($point.y | round)},
            {type: "pointerDown", button: 0}
          ] +
          [range(1; 11) as $step | {
            type: "pointerMove", duration: 16, origin: "viewport",
            x: ($point.x + (($point.endX - $point.x) * $step / 10) | round),
            y: ($point.y + (($point.endY - $point.y) * $step / 10) | round)
          }] +
          [{type: "pointerUp", button: 0}]
        )
      }]
    }')
  fi
  webdriver_value POST "/session/$session_id/actions" "$payload" >/dev/null || result=$?
  if ((result != 0)); then
    pointer_action_failure_diagnostic "$action" "$result"
    ((result != 28)) || pointer_action_timed_out=1
  else
    webdriver_value DELETE "/session/$session_id/actions" >/dev/null 2>&1 || true
  fi
  return "$result"
}

pointer_action_failure_diagnostic() {
  local action=$1 exit_code=$2 payload response probe='{"probeResponded":false}'
  payload=$(jq -cn '{script: "const dialog = document.querySelector(\"#content-dialog\"); return { readyState: document.readyState, visibility: document.visibilityState, dialogOpen: Boolean(dialog?.open), mediaPreview: Boolean(dialog?.classList.contains(\"media-preview\")), readerReady: document.documentElement.hasAttribute(\"data-reader-ready\"), runtimeErrorCount: Array.isArray(globalThis.__athaReaderRuntimeErrors) ? globalThis.__athaReaderRuntimeErrors.length : null };", args: []}')
  if [[ -n $session_id ]] && response=$(curl --silent --show-error --max-time 2 \
    --request POST --header 'Accept: application/json' --header 'Content-Type: application/json' \
    --data "$payload" "$base_url/session/$session_id/execute/sync" 2>/dev/null); then
    probe=$(jq -c '
      if (.value | type == "object") and (.value.error? == null) then
        .value as $value |
        {
          probeResponded: true,
          readyState: (if ["loading", "interactive", "complete"] | index($value.readyState) then $value.readyState else "unknown" end),
          visibility: (if ["visible", "hidden", "prerender"] | index($value.visibility) then $value.visibility else "unknown" end),
          dialogOpen: ($value.dialogOpen == true),
          mediaPreview: ($value.mediaPreview == true),
          readerReady: ($value.readerReady == true),
          runtimeErrorCount: (if ($value.runtimeErrorCount | type) == "number" and $value.runtimeErrorCount >= 0 and $value.runtimeErrorCount <= 100000 then ($value.runtimeErrorCount | floor) else null end)
        }
      else {probeResponded: false} end
    ' <<<"$response" 2>/dev/null) || probe='{"probeResponded":false}'
  fi
  jq -cn --arg unit "$unit" --arg backend "$gdk_backend" --arg action "$action" \
    --argjson exitCode "$exit_code" --argjson probe "$probe" \
    '{event: "webdriver-actions-failed", unit: $unit, backend: $backend, action: $action, exitCode: $exitCode, probe: $probe}' >&2
}

run_gesture_gate() {
  local book_kind=$1 begin_script finish_script
  local scenario target action mode direction expectation sample total arguments begin finished
  read -r -d '' begin_script <<'JS' || true
const done = arguments[arguments.length - 1];
globalThis.__athaReaderDiagnostics
  .beginGestureProbe(arguments[0], arguments[1], arguments[2], arguments[3])
  .then((value) => done({ ok: true, value }))
  .catch((error) => done({ ok: false, code: error instanceof Error ? error.message : "gesture-setup" }));
JS
  read -r -d '' finish_script <<'JS' || true
const done = arguments[arguments.length - 1];
globalThis.__athaReaderDiagnostics.finishGestureProbe(arguments[0])
  .then((value) => done({ ok: true, value }))
  .catch(() => done({ ok: false }));
JS

  total=$((gesture_warmups + gesture_measurements))
  while IFS=$'\t' read -r scenario target action mode direction expectation; do
    for ((sample = 0; sample < total; sample += 1)); do
      arguments=$(jq -cn \
        --arg target "$target" --arg action "$action" --arg mode "$mode" \
        --argjson direction "$direction" \
        '[$target, $action, $mode, $direction]')
      begin=$(execute_async "$begin_script" "$arguments") || die "gesture setup failed for $scenario"
      if ! jq -e '.ok == true and (.value.id | type == "number")' <<<"$begin" >/dev/null; then
        die "gesture setup failed for $scenario (state: $(jq -c . <<<"$begin"))"
      fi
      gesture_active=1
      perform_pointer_action "$(jq -c '.value' <<<"$begin")" "$action" ||
        die "trusted pointer action failed for $scenario"
      arguments=$(jq -cn --argjson id "$(jq '.value.id' <<<"$begin")" '[$id]')
      finished=$(execute_async "$finish_script" "$arguments") || die "gesture result failed for $scenario"
      if ! jq -e --arg action "$action" --arg expectation "$expectation" '
        .ok == true and
        (.value |
          .targetHit == true and .trusted == true and .settled == true and
          .preview == false and .compatibilityEvents == 0 and
          (if $action == "drag" then .pointerMoves >= 10 else true end) and
          (.timing.releaseToStableMs | type == "number") and
          (if $expectation == "protected" then
            .samePage == true and .scrollDelta > -1 and .scrollDelta < 1 and
            .visualUpdateSamples == 0
          else
            (.timing.inputToFirstVisualMs | type == "number") and
            (if $action == "tap" then
              (.timing.releaseToFirstVisualMs | type == "number")
            else
              .visualUpdateSamples >= ([3, ((.pointerMoves + 1) / 2 | floor)] | max) and
              (.timing.frameP95Ms | type == "number") and
              (.timing.maxFrameMs | type == "number")
            end) and
            (if $expectation == "page" then
              .singlePage == true and
              (if $action == "drag" then .rafTransformSamples >= 6 else true end)
            elif $expectation == "pan" then
              .samePage == true and (.scrollDelta >= 24 or .scrollDelta <= -24)
            else
              false
            end)
          end)
        )
      ' <<<"$finished" >/dev/null; then
        printf 'gesture_result=%s\n' "$(jq -c . <<<"$finished")" >&2
        printf 'gesture_snapshot=%s\n' "$(execute_sync 'return globalThis.__athaReaderDiagnostics.snapshot();' | jq -c '{pages, swipeDragging, resources, session, navigation}')" >&2
        die "gesture semantics failed for $scenario"
      fi
      if ((sample >= gesture_warmups)); then
        jq -c \
          --arg book "$book_kind" --arg scenario "$scenario" --arg action "$action" \
          --arg expectation "$expectation" \
          '.value | {
            book: $book,
            scenario: $scenario,
            action: $action,
            expectation: $expectation,
            trusted,
            touch,
            pointerTypes,
            timing
          }' \
          <<<"$finished" >>"$metrics_file"
      fi
    done
  done <<'SCENARIOS'
ordinary-tap	ordinary	tap	edge	1	page
ordinary-drag	ordinary	drag	edge	1	page
formula-tap	formula	tap	edge	1	page
formula-drag	formula	drag	edge	1	page
formula-vertical	formula	drag	vertical	1	protected
table-tap	table	tap	edge	1	page
table-drag	table	drag	edge	1	page
table-vertical	table	drag	vertical	1	protected
overflow-table-tap	overflow-table	tap	edge	1	page
overflow-table-pan-next	overflow-table	drag	pan	1	pan
overflow-table-pan-previous	overflow-table	drag	pan	-1	pan
overflow-table-edge-next	overflow-table	drag	edge	1	page
overflow-table-edge-previous	overflow-table	drag	edge	-1	page
SCENARIOS
  cleanup_gesture || die 'gesture diagnostic cleanup failed'
}

verify_gesture_performance() {
  jq -s -e '
    map(select(.expectation != "protected")) as $samples |
    ($samples | length > 0) and
    ($samples | all(.[];
      .timing.releaseToStableMs >= 0 and
      (if .action == "drag" then
        .timing.inputToFirstVisualMs >= 0 and .timing.maxFrameMs <= 50
      else
        .timing.releaseToFirstVisualMs >= 0
      end)
    ))
  ' "$metrics_file" >/dev/null || die 'gesture maximum-frame threshold failed'

  if ((gesture_measurements >= 20)); then
    if ! jq -s -e '
      def p95($values):
        ($values | sort) as $ordered |
        $ordered[((($ordered | length) * 0.95 | ceil) - 1)];
      map(select(.expectation != "protected")) |
      sort_by([.book, .scenario]) | group_by([.book, .scenario]) |
      all(.[];
        . as $samples |
        p95([$samples[].timing.releaseToStableMs]) <= 400 and
        (if $samples[0].action == "drag"
          then p95([$samples[].timing.inputToFirstVisualMs]) <= 33.4 and
            p95([$samples[].timing.frameP95Ms]) <= 25
          else p95([$samples[].timing.releaseToFirstVisualMs]) <= 50
        end)
      )
    ' "$metrics_file" >/dev/null; then
      jq -s -c '
        def p95($values):
          ($values | sort) as $ordered |
          $ordered[((($ordered | length) * 0.95 | ceil) - 1)];
        map(select(.expectation != "protected")) |
        sort_by([.book, .scenario]) | group_by([.book, .scenario]) |
        map(. as $samples | {
          book: $samples[0].book,
          scenario: $samples[0].scenario,
          input: (if $samples[0].action == "drag" then p95([$samples[].timing.inputToFirstVisualMs]) else null end),
          tap: (if $samples[0].action == "tap" then p95([$samples[].timing.releaseToFirstVisualMs]) else null end),
          frame: (if $samples[0].action == "drag" then p95([$samples[].timing.frameP95Ms]) else null end),
          settle: p95([$samples[].timing.releaseToStableMs]),
          frameSamples: (if $samples[0].action == "drag" then [$samples[].timing.frameP95Ms] | sort else [] end)
        }) |
        map(select(.settle > 400 or .input > 33.4 or .frame > 25 or .tap > 50))
      ' "$metrics_file" >&2
      die 'gesture P95 threshold failed'
    fi
  fi
}

gesture_summary() {
  jq -s --argjson warmups "$gesture_warmups" --argjson measurements "$gesture_measurements" '
    def p95($values):
      ($values | sort) as $ordered |
      if ($ordered | length) == 0 then null
      else $ordered[((($ordered | length) * 0.95 | ceil) - 1)] end;
    . as $all |
    [$all[] | select(.expectation != "protected")] as $measured |
    ($measured | sort_by([.book, .scenario]) | group_by([.book, .scenario]) |
      map(. as $samples | {
        action: $samples[0].action,
        input: (if $samples[0].action == "drag"
          then p95([$samples[].timing.inputToFirstVisualMs]) else null end),
        tap: (if $samples[0].action == "tap"
          then p95([$samples[].timing.releaseToFirstVisualMs]) else null end),
        frame: (if $samples[0].action == "drag"
          then p95([$samples[].timing.frameP95Ms]) else null end),
        settle: p95([$samples[].timing.releaseToStableMs])
      })) as $scenarioMetrics |
    {
      scenarios: ([$all[].scenario] | unique | length),
      warmupsPerScenario: $warmups,
      measurementsPerScenario: $measurements,
      measurements: ($measured | length),
      requestedPointerType: "mouse",
      trustedPointerEvents: all($all[]; .trusted == true),
      nativeTouchObserved: any($all[]; .touch == true),
      observedPointerTypes: ([$all[].pointerTypes[]?] | unique),
      inputToFirstVisualP95Ms: ([$scenarioMetrics[].input | select(. != null)] | max),
      tapToFirstVisualP95Ms: ([$scenarioMetrics[].tap | select(. != null)] | max),
      dragFrameP95Ms: ([$scenarioMetrics[].frame | select(. != null)] | max),
      maximumFrameMs: ([$measured[] | select(.action == "drag") | .timing.maxFrameMs] | max),
      releaseToStableP95Ms: ([$scenarioMetrics[].settle] | max)
    }
  ' "$metrics_file"
}

add_record_privacy_tokens() {
  local record=$1
  jq -r '.id, .title, .authors[]?' "$record" >>"$privacy_file"
}

check_app_log_privacy() {
  local logs_root=${1:-$library_root}/logs log_count=0 log
  awk 'length >= 8' "$privacy_file" | sort -u >"$privacy_file.filtered"
  while IFS= read -r -d '' log; do
    log_count=$((log_count + 1))
    if grep -Fq -f "$privacy_file.filtered" -- "$log"; then
      die 'AppLog privacy check found seeded-book data'
    fi
  done < <(find "$logs_root" -maxdepth 1 -type f -print0 2>/dev/null)
  ((log_count > 0)) || die 'AppLog privacy check found no log output'
}

if ((!shelf_only)); then
  say 'checking touch interaction ownership'
  mise exec -- node --test reader/web/interaction.test.mjs >&2
fi

if [[ -n $external_application ]]; then
  application=$(resolve_input_file "$external_application") || die 'candidate application does not exist'
  [[ -x $application ]] || die 'candidate application is not executable'
  say 'using the supplied candidate application'
else
  say 'building the current Tauri application'
  mise exec -- pnpm --dir reader/app build >&2
  mise exec -- cargo build --locked -p atha-reader-app >&2
  application=$repo_root/target/debug/atha-reader-app
  [[ -x $application ]] || die 'Tauri application binary was not produced'
fi

say 'seeding the isolated public FB2 library'
mkdir -p -- "$data_home"
ATHA_FB2_GATE_LIBRARY_ROOT=$library_root \
  mise exec -- cargo test --locked -p atha-backend --test fb2_import \
    writes_fb2_gate_fixture -- --ignored --exact >&2

readonly expected_fixture_sha=155225e7aa977574c5f75559f58ad121004bf714b91e10caeacd774da5550186
[[ $(sha256sum .tmp/fb2-gate.fb2 | awk '{print $1}') == "$expected_fixture_sha" ]] ||
  die 'public FB2 fixture identity changed'
shopt -s nullglob
public_records=("$library_root/Library"/*.json)
(( ${#public_records[@]} == 1 )) || die 'public FB2 seed produced an invalid library'
public_record=${public_records[0]}
public_id=$(jq -er '.id' "$public_record") || die 'public FB2 library record is invalid'
public_source_name=$(jq -r '.sourcePath // empty' "$public_record")
public_manifest=$library_root/ImportedBooks/$public_id/.atha-reader.json
jq -e '.schema == 1 and (.sections | length) == 4 and (.resources | length) == 1 and (.toc | length) == 3' \
  "$public_manifest" >/dev/null || die 'public FB2 manifest shape is invalid'
public_boundary_entry=$(jq -er '.sections[1].href' "$public_manifest") ||
  die 'public FB2 manifest has no boundary section'
memory_section=$(jq -er '.sections[2].id' "$public_manifest") ||
  die 'public FB2 manifest has no reading memory section'
if ((!shelf_only)); then
  ATHA_READING_MEMORY_GATE_ROOT=$library_root \
  ATHA_READING_MEMORY_GATE_EDITION=$public_id \
  ATHA_READING_MEMORY_GATE_SECTION=$memory_section \
    mise exec -- cargo test --locked -p atha-backend --test message_reading \
      seeds_reading_memory_gui_fixture -- --ignored --exact >&2
fi
printf '%s\n' "$repo_root/.tmp/fb2-gate.fb2" 'fb2-gate.fb2' >>"$privacy_file"
printf '%s\n' \
  '公共记忆中的可跳转笔记' '公共记忆中的缺书笔记' \
  '这条最近回应应出现在笔记索引中' '这条回复用于验证消息之间的回应关系' \
  '正文重点' '第二节正文' '缺失原书引用' \
  '已移出的公共书籍' '公共作者' >>"$privacy_file"
add_record_privacy_tokens "$public_record"

expected_books=1
if [[ -n $formula_epub ]]; then
  say 'seeding the optional private formula benchmark'
  if ! ATHA_EPUB_GATE_LIBRARY_ROOT=$library_root ATHA_EPUB_GATE_SOURCE=$formula_epub \
    mise exec -- cargo test --locked -p atha-backend --test epub_import \
      seeds_private_formula_gui_benchmark -- --ignored --exact \
      >"$run_root/formula-seed.log" 2>&1; then
    die 'private formula seed failed'
  fi
  all_records=("$library_root/Library"/*.json)
  (( ${#all_records[@]} == 2 )) || die 'private formula seed produced an invalid library'
  formula_record=
  for record in "${all_records[@]}"; do
    if [[ $(jq -er '.id' "$record") != "$public_id" ]]; then
      formula_record=$record
      break
    fi
  done
  [[ -n $formula_record ]] || die 'private formula library record is missing'
  printf '%s\n' "$formula_epub" "$(basename -- "$formula_epub")" \
    "$formula_metadata" "$(basename -- "$formula_metadata")" \
    "$formula_source_sha" "$formula_entry" >>"$privacy_file"
  add_record_privacy_tokens "$formula_record"
  expected_books=2
fi

session_application=$application
if ((association_launch)); then
  say 'preparing a cold-start file-association launch'
  rm -f -- "$public_record"
  if [[ -n $public_source_name ]]; then
    rm -f -- "$library_root/SourceBooks/$public_source_name"
  fi
  rm -rf -- "$library_root/ImportedBooks/$public_id"
  public_book_key=${public_id:0:16}
  association_extra=$run_root/association-extra.md
  association_extra_title=association-extra
  association_invalid=$run_root/association-invalid.md
  printf 'Public multi-file association content.\n' >"$association_extra"
  mkdir -- "$association_invalid"
  printf '%s\n' \
    "$association_extra" "$association_extra_title" 'Public multi-file association content.' \
    "$association_invalid" 'association-invalid.md' >>"$privacy_file"
  association_book_count=$((expected_books + 1))
  association_wrapper=$run_root/launch-associated.sh
  printf '#!/usr/bin/env bash\nexec %q %q %q %q\n' \
    "$application" "$repo_root/.tmp/fb2-gate.fb2" "$association_extra" \
    "$association_invalid" >"$association_wrapper"
  chmod 700 "$association_wrapper"
  session_application=$association_wrapper
fi

say 'starting isolated tauri-driver session'
driver_ready=0
for ((launch_attempt = 1; launch_attempt <= 3; launch_attempt += 1)); do
  driver_port=$(pick_free_port)
  native_port=$(pick_free_port)
  while [[ $native_port == "$driver_port" ]]; do native_port=$(pick_free_port); done
  base_url=http://127.0.0.1:$driver_port
  unit="atha-reader-linux-${BASHPID}-${RANDOM}-${launch_attempt}"

  if systemd-run --user --quiet --collect --unit="$unit" --working-directory="$repo_root" \
    --setenv="XDG_DATA_HOME=$data_home" --setenv="GDK_BACKEND=$gdk_backend" \
    --setenv=ATHA_READER_GUI_GATE=1 \
    "${graphics_environment[@]}" \
    "$driver" --port "$driver_port" --native-port "$native_port"; then
    unit_started=1
    for ((attempt = 0; attempt < 100; attempt += 1)); do
      if curl --silent --fail --max-time 1 "$base_url/status" >/dev/null; then
        driver_ready=1
        break
      fi
      sleep 0.1
    done
  fi
  ((driver_ready)) && break
  systemctl --user stop "$unit.service" >/dev/null 2>&1 || true
  systemctl --user reset-failed "$unit.service" >/dev/null 2>&1 || true
  unit_started=0
done
((driver_ready)) || die 'tauri-driver did not become ready'

session_payload=$(jq -cn --arg application "$session_application" '{
  capabilities: {alwaysMatch: {"tauri:options": {application: $application}}}
}')
session_response=$(webdriver_request POST /session "$session_payload") ||
  die 'tauri-driver could not create a session'
session_id=$(jq -er '.value.sessionId' <<<"$session_response") ||
  die 'tauri-driver returned no session'
browser_version=$(jq -r '.value.capabilities.browserVersion // "unknown"' <<<"$session_response")

webdriver_value POST "/session/$session_id/window/rect" '{"width":800,"height":600}' >/dev/null ||
  die 'could not initialize the trusted-input window'
wait_for_script \
  'return { visibility: document.visibilityState, width: innerWidth, height: innerHeight };' \
  '.visibility == "visible" and .width > 0 and .height > 0' \
  'Linux GUI target did not expose a usable viewport' >/dev/null

association_evidence=null
if ((association_launch)); then
  association_snapshot=
  association_ids=
  wait_for_script \
    'const state = new URL(location.href).searchParams.get("state"); return { status: document.documentElement.dataset.status || null, error: document.documentElement.dataset.error || null, session: document.documentElement.dataset.sessionState || null, reader: location.pathname.endsWith("/index.html"), ready: document.documentElement.hasAttribute("data-reader-ready"), state };' \
    ".status == \"pass\" and .error == null and .session == \"layout-stable\" and .reader == true and .ready == true and .state == \"$public_book_key\"" \
    'associated cold start did not import and open the public book' >/dev/null
  association_evidence=$(jq -n \
    '{coldStart: true, validImported: 2, invalidSkipped: true, openedFirst: true}')
  open_library "$association_book_count"
  association_snapshot=$(association_library_snapshot) ||
    die 'could not inspect the initial association library'
  jq -e --argjson count "$association_book_count" '
    .count == $count and .publicPresent == true and .extraMatches == 1
  ' <<<"$association_snapshot" >/dev/null ||
    die 'multi-file association launch did not import every valid book'
  association_ids=$(jq -c '.ids' <<<"$association_snapshot")
  repeat_association_launch "$association_ids"
fi

read -r -d '' initial_library_commands <<'JS' || true
const done = arguments[arguments.length - 1];
(async () => {
  try {
    const pending = await window.__TAURI_INTERNALS__.invoke("pending_local_data_restore");
    const deletions = await window.__TAURI_INTERNALS__.invoke("pending_library_book_deletions");
    const books = await window.__TAURI_INTERNALS__.invoke("list_library_books");
    done({
      pending: { ok: true, value: pending },
      deletions: { ok: true, count: deletions.length },
      books: { ok: true, count: books.length },
    });
  } catch (error) {
    done({ pending: { ok: false, code: String(error) } });
  }
})();
JS
initial_commands=$(execute_async "$initial_library_commands") || die 'could not probe library commands'
jq -e --argjson books "$expected_books" '
  .pending.ok == true and .pending.value == null and
  .deletions.ok == true and .deletions.count == 0 and
  .books.ok == true and .books.count == $books
' <<<"$initial_commands" >/dev/null ||
  die "initial library commands failed (state: $(jq -c . <<<"$initial_commands"))"

if ((shelf_only)); then
  say 'verifying current native shelf and reader return'
  verify_current_shelf
  check_app_log_privacy
  jq -n --arg browserVersion "$browser_version" --arg display "$gdk_backend" \
    '{gate: "shelf", platform: "Linux", webDriverBrowserVersion: $browserVersion, displayBackend: $display,
      import: "pass", favoriteFilter: "pass",
      listSort: "pass", search: "pass", openFirstScreen: "pass", returnState: "pass", appLogPrivacy: "pass"}'
  exit 0
fi

wait_for_script \
  'return { ready: document.readyState, cards: document.querySelectorAll(".library-book-open").length, href: location.href, status: document.querySelector(".library-status")?.textContent || null };' \
  ".ready == \"complete\" and .cards == $expected_books" \
  'initial library did not become ready' >/dev/null


say 'verifying native drop entry and the shared library result set'
library_entry_evidence=null
verify_library_entry

say 'running public verify, boundary, and trusted gesture diagnostics'
open_book public
enable_diagnostics
open_gesture_section "$public_boundary_entry" >/dev/null
interface_navigation_file=$run_root/interface-navigation.json
verify_reader_interface_navigation >"$interface_navigation_file"
interface_navigation=$(<"$interface_navigation_file")
open_gesture_section "$public_boundary_entry" >/dev/null
pending_formula_queue=$(pending_formula_queue_probe)
public_boundary=$(previous_boundary_probe)
open_gesture_section "$public_boundary_entry" >/dev/null
set_window_size 600 760
run_gesture_gate public
verify_reader_runtime_errors

formula_shape=null
formula_boundary=null
scroll_resource=null
if [[ -n $formula_epub ]]; then
  say 'running private formula verify, boundary, and gesture diagnostics'
  open_library "$expected_books"
  open_book formula
  enable_diagnostics
  open_gesture_section "$formula_entry" false >/dev/null
  scroll_resource=$(scroll_resource_probe)
  formula_opened=$(open_gesture_section "$formula_entry")
  jq -e --argjson formulas "$minimum_formulas" --argjson pages "$minimum_pages" '
    .value.formulas >= $formulas and .value.settledFormulas >= $formulas and
    .value.pages >= $pages and .value.nativePagedScroll == true
  ' <<<"$formula_opened" >/dev/null ||
    die "private formula section did not meet the benchmark shape (state: $(jq -c '.value | {section, formulas, settledFormulas, pages, contentPages, scrollPages, nativePagedScroll}' <<<"$formula_opened"))"
  formula_shape=$(jq -c '.value | {section, formulas, settledFormulas, pages, nativePagedScroll}' <<<"$formula_opened")
  formula_boundary=$(previous_boundary_probe)
  open_gesture_section "$formula_entry" >/dev/null
  run_gesture_gate private
  verify_reader_runtime_errors
fi

verify_gesture_performance
gesture_evidence=$(gesture_summary)
open_library "$expected_books"

for width in 360 1000 1280; do
  height=760
  ((width == 1280)) && height=800
  set_window_size "$width" "$height"
  verify_library_layout "$width" "$height"
done

say 'verifying native shelf organization, scoped search, and reader return'
verify_continuous_shelf

say 'verifying local-data management and reading memory at mobile and desktop widths'
# Real-book gesture runs may add a second recent book. Reset only this isolated fixture.
memory_statistics=$(jq -cn --arg id "$public_id" '{
  schema: 1,
  days: [{date: "2026-08-13", durationMs: 90000}],
  books: [{contentVersion: $id, durationMs: 90000, lastReadDate: "2026-08-13"}]
}')
memory_statistics_args=$(jq -cn --arg value "$memory_statistics" '[$value]')
[[ $(execute_sync '
localStorage.setItem("atha.reader.statistics.v1", arguments[0]);
return true;' "$memory_statistics_args") == true ]] || die 'could not seed reading memory statistics'

for width in 360 1000; do
  set_window_size "$width" 760
  verify_library_settings
  verify_reading_memory
done
set_window_size 600 760

say 'running public desktop workspace and responsive diagnostics'
open_book public
set_window_size 1280 800
origin_rejection=$(execute_async '
const done = arguments[arguments.length - 1];
window.__TAURI_INTERNALS__.invoke("import_library_paths", { paths: ["/tmp/atha-origin.epub"] })
  .then(() => done({ rejected: false }))
  .catch((error) => done({ rejected: true, code: String(error) }));
') || die 'could not probe reader-origin drop rejection'
jq -e '.rejected == true and .code == "invalid-origin"' <<<"$origin_rejection" >/dev/null ||
  die "reader page accepted a library path import (state: $(jq -c . <<<"$origin_rejection"))"
wait_for_script \
  'const frame = document.querySelector(".reader-frame"); const reader = document.querySelector(".reader"); const page = document.querySelector(".page"); const controls = document.querySelector(".reader-controls"); const frameRect = frame?.getBoundingClientRect(); const readerRect = reader?.getBoundingClientRect(); const pageRect = page?.getBoundingClientRect(); const controlsStyle = controls ? getComputedStyle(controls) : null; return {width: innerWidth, desktop: document.documentElement.hasAttribute("data-desktop-workspace"), active: document.documentElement.dataset.workspacePanel || null, openPanels: document.querySelectorAll(".reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]").length, full: Boolean(frameRect && readerRect && Math.abs(frameRect.left) <= 1 && Math.abs(frameRect.width - innerWidth) <= 2 && Math.abs(readerRect.left - frameRect.left) <= 1 && Math.abs(readerRect.width - frameRect.width) <= 1), centered: Boolean(pageRect && readerRect && Math.abs((pageRect.left - readerRect.left) - (readerRect.right - pageRect.right)) <= 2), pageWidth: pageRect?.width || 0, pageMargin: reader ? parseFloat(getComputedStyle(reader).getPropertyValue("--page-left-margin")) / devicePixelRatio : 0, controlsHidden: controlsStyle?.visibility === "hidden", error: document.documentElement.dataset.error || null};' \
  '.desktop == true and .active == null and .openPanels == 0 and .full == true and .centered == true and .pageWidth >= 700 and .pageWidth <= 761 and .pageMargin >= 31 and .pageMargin <= 33 and .controlsHidden == true and .error == null' \
  'normal desktop reader startup did not produce focus reading' >/dev/null
set_window_size 600 760
enable_diagnostics
set_window_size 1280 800
desktop_workspace_evidence=null
verify_desktop_workspace
verify_reader_redesign
verify_reader_runtime_errors

# The reading-memory seed already owns the exact public text anchor used by the
# authoring probe. Run authoring against the same book files with a fresh
# Messages store so this gate creates a new root rather than editing that seed.
webdriver_request DELETE "/session/$session_id" >/dev/null || die 'could not close the baseline Tauri session'
session_id=
systemctl --user stop "$unit.service" >/dev/null || die 'could not stop the baseline tauri-driver'
unit_started=0
systemctl --user reset-failed "$unit.service" >/dev/null 2>&1 || true
check_app_log_privacy "$library_root"

baseline_library_root=$library_root
run_root_real=$(realpath -- "$run_root") || die 'could not resolve the isolated run root'
baseline_library_root_real=$(realpath -- "$baseline_library_root") || die 'could not resolve the baseline fixture root'
[[ $baseline_library_root_real == "$run_root_real"/* ]] || die 'baseline fixture root escaped the isolated run root'
data_home=$run_root/authoring-data
library_root=$data_home/com.atha.reader
mkdir -p -- "$library_root"
authoring_library_root_real=$(realpath -- "$library_root") || die 'could not resolve the authoring fixture root'
[[ $authoring_library_root_real == "$run_root_real"/* ]] || die 'authoring fixture root escaped the isolated run root'
for fixture_dir in Library ImportedBooks SourceBooks; do
  [[ -d $baseline_library_root/$fixture_dir ]] || {
    [[ $fixture_dir == SourceBooks ]] || die "baseline fixture is missing $fixture_dir"
    continue
  }
  [[ ! -e $library_root/$fixture_dir ]] || die 'authoring fixture destination is not empty'
  if find "$baseline_library_root/$fixture_dir" -type l -print -quit | grep -q .; then
    die 'baseline fixture contains a symlink that cannot be copied into authoring data'
  fi
  cp -a -- "$baseline_library_root/$fixture_dir" "$library_root/"
done
[[ ! -e $library_root/Messages ]] || die 'authoring fixture unexpectedly contains message data'
session_application=$application
current_book_kind=
diagnostics_enabled=0
gesture_active=0
say 'starting isolated authoring session with a fresh message store'
start_authoring_session
verify_authoring_cycle

webdriver_request DELETE "/session/$session_id" >/dev/null || die 'could not close the Tauri session'
session_id=
if ((association_launch)); then
  verify_ordinary_launch
fi
systemctl --user stop "$unit.service" >/dev/null || die 'could not stop tauri-driver'
unit_started=0
systemctl --user reset-failed "$unit.service" >/dev/null 2>&1 || true

check_app_log_privacy

jq -n \
  --arg browserVersion "$browser_version" \
  --arg backend "$gdk_backend" --arg compositing "$compositing_mode" \
  --arg painting "$painting_mode" \
  --argjson books "$expected_books" \
  --argjson libraryEntry "$library_entry_evidence" \
  --argjson continuousShelf "$continuous_shelf_evidence" \
  --argjson fileAssociation "$association_evidence" \
  --argjson interfaceNavigation "$interface_navigation" \
  --argjson pendingFormulaQueue "$pending_formula_queue" \
  --argjson desktopWorkspace "$desktop_workspace_evidence" \
  --argjson authoringCycle "$authoring_cycle_evidence" \
  --argjson publicBoundary "$public_boundary" \
  --argjson formulaShape "$formula_shape" \
  --argjson formulaBoundary "$formula_boundary" \
  --argjson scrollResource "$scroll_resource" \
  --argjson gestures "$gesture_evidence" \
  '{
    evidence: "Linux Tauri GUI",
    webview: "WebKitGTK",
    driverBrowserVersion: $browserVersion,
    graphics: {backend: $backend, compositing: $compositing, painting: $painting},
    books: $books,
    libraryEntry: $libraryEntry,
    continuousShelf: $continuousShelf,
    fileAssociation: $fileAssociation,
    interfaceNavigation: $interfaceNavigation,
    verifyDiagnostics: true,
    desktopWorkspace: $desktopWorkspace,
    authoringCycle: $authoringCycle,
    pendingFormulaQueue: $pendingFormulaQueue,
    publicBoundary: $publicBoundary,
    formulaShape: $formulaShape,
    formulaBoundary: $formulaBoundary,
    scrollResource: $scrollResource,
    gestures: $gestures,
    appLogPrivacy: "pass"
  }'
