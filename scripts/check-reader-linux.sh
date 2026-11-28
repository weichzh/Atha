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
    '  --formula-epub PATH          Add one formula-heavy EPUB (treated as private).' \
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
((shelf_only)) || command -v python3 >/dev/null || die 'python3 is required for the original EPUB fixture'

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
public_fixture=$run_root/fb2-gate.fb2
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
native_action_timed_out=0

webdriver_request() {
  local method=$1 path=$2 body=${3-}
  local arguments=(
    --silent --show-error --fail --max-time 30
    --request "$method"
    --header 'Accept: application/json'
  )
  if [[ -n $body ]]; then
    arguments+=(--header 'Content-Type: application/json' --data-binary @-)
  fi
  curl "${arguments[@]}" "$base_url$path" <<<"$body"
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

webdriver_native_action() {
  local payload=$1 result=0
  webdriver_value POST "/session/$session_id/actions" "$payload" >/dev/null || result=$?
  ((result != 28)) || native_action_timed_out=1
  return "$result"
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
  # A pending native input command can crash WebKitWebDriver when cancelled.
  # Stop this run's unit directly after a transport timeout instead.
  if ((native_action_timed_out == 0)); then
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
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

safe_wait_state() {
  if [[ -z $formula_epub ]]; then
    jq -c . <<<"$1"
  else
    printf '%s' 'withheld for formula fixture privacy'
  fi
}

wait_for_script() {
  local script=$1 predicate=$2 label=$3 script_args=${4:-[]}
  local deadline=$((SECONDS + 30)) value last_value=
  while ((SECONDS < deadline)); do
    if value=$(execute_sync "$script" "$script_args" 2>/dev/null); then
      last_value=$value
      if jq -e "$predicate" <<<"$value" >/dev/null; then
        printf '%s\n' "$value"
        return 0
      fi
      if jq -e '.status == "fail"' <<<"$value" >/dev/null; then
        die "$label (state: $(safe_wait_state "$value"))"
      fi
    fi
    sleep 0.1
  done
  if [[ -n $last_value ]]; then
    die "$label (state: $(safe_wait_state "$last_value"))"
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

viewport_width_predicate() {
  local width=$1
  if [[ $gdk_backend == wayland && $width == 1600 ]]; then
    # GNOME may maximize a 1600x900 minimum when decorations do not fit the
    # work area. Accept only the measured work-area width in the wide layout.
    printf '%s' '((.width >= 1598 and .width <= 1602) or (.width >= 1600 and .width == .availableWidth))'
  else
    printf '(.width >= %d and .width <= %d)' "$((width - 2))" "$((width + 2))"
  fi
}

set_window_size() {
  local width=$1 height=$2 state width_check was_reader saved_book saved_diagnostics viewport_application payload response
  width_check=$(viewport_width_predicate "$width")
  state=$(execute_sync 'return { width: innerWidth, height: innerHeight, availableWidth: screen.availWidth, reader: new URL(location.href).searchParams.has("manifest") };') ||
    die 'could not inspect the current reader viewport'
  if jq -e "$width_check and .height >= $((height - 64)) and .height <= $((height + 2))" \
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

  local cards='.shelf-catalog [data-shelf-open]'
  wait_for_script \
    "return { width: innerWidth, height: innerHeight, availableWidth: screen.availWidth, ready: document.readyState, library: Boolean(document.querySelector('.library-shell')), cards: document.querySelectorAll('$cards').length };" \
    "$width_check and .height >= $((height - 64)) and .height <= $((height + 2)) and .ready == \"complete\" and .library == true and .cards == $expected_books" \
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
    'return { ready: document.readyState, cards: document.querySelectorAll(".shelf-catalog [data-shelf-open]").length, enabled: [...document.querySelectorAll(".shelf-catalog [data-shelf-open]")].filter((button) => !button.disabled).length };' \
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
    'const state = new URL(location.href).searchParams.get("state"); return { status: document.documentElement.dataset.status || null, error: document.documentElement.dataset.error || null, screenReady: document.querySelector(".reader-root")?.dataset.screenReady, reader: new URL(location.href).searchParams.has("manifest"), ready: document.documentElement.hasAttribute("data-reader-ready"), state };' \
    ".reader == true and .ready == true and .screenReady == \"true\" and .state == \"$public_book_key\"" \
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
    'return { ready: document.readyState, library: Boolean(document.querySelector(".library-shell")), cards: document.querySelectorAll(".shelf-catalog [data-shelf-open]").length, reader: new URL(location.href).searchParams.has("manifest") };' \
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
    'return { ready: document.readyState, library: Boolean(document.querySelector(".library-shell")), cards: document.querySelectorAll(".shelf-catalog [data-shelf-open]").length };' \
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
  state=$(fire_tauri_drag_event 'tauri://drag-enter' "$public_fixture") ||
    die 'could not synthesize drag enter'
  jq -e '.listeners == 1 and .emitted == true' <<<"$state" >/dev/null ||
    die "drag enter listener is unavailable (state: $(jq -c . <<<"$state"))"
  wait_for_script \
    'return { overlay: Boolean(document.querySelector(".drop-scrim")), busy: document.querySelector(".library-shell")?.getAttribute("aria-busy") };' \
    '.overlay == true and .busy == "false"' \
    'drag enter did not reveal the drop state' >/dev/null
  state=$(fire_tauri_drag_event 'tauri://drag-leave') || die 'could not synthesize drag leave'
  jq -e '.listeners == 1 and .emitted == true' <<<"$state" >/dev/null ||
    die 'drag leave listener is unavailable'
  wait_for_script \
    'return { overlay: Boolean(document.querySelector(".drop-scrim")) };' \
    '.overlay == false' \
    'drag leave did not clear the drop state' >/dev/null

  state=$(fire_tauri_drag_event 'tauri://drag-drop' "$public_fixture") ||
    die 'could not synthesize duplicate drop'
  jq -e '.listeners == 1 and .emitted == true' <<<"$state" >/dev/null ||
    die 'drag drop listener is unavailable'
  wait_for_script \
    'return { cards: document.querySelectorAll(".shelf-catalog [data-shelf-open]").length, overlay: Boolean(document.querySelector(".drop-scrim")), working: Boolean(document.querySelector(".library-shell[aria-busy=true]")), status: document.querySelector(".toast-banner")?.textContent || "" };' \
    ".cards == $expected_books and .overlay == false and .working == false and .status == \"已导入拖放的书籍\"" \
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
      'return { ready: document.readyState, empty: document.querySelector(".empty-shelf p")?.textContent === "没有符合当前条件的书籍。", cards: document.querySelectorAll(".shelf-catalog [data-shelf-open]").length };' \
      '.ready == "complete" and .empty == true and .cards == 0' \
      'empty library did not become ready' >/dev/null

    fire_tauri_drag_event 'tauri://drag-enter' "$public_fixture" >/dev/null ||
      die 'could not enter the empty-library drop state'
    wait_for_script \
      'return { overlay: Boolean(document.querySelector(".drop-scrim")) };' \
      '.overlay == true' \
      'empty-library drag enter did not reveal the drop state' >/dev/null
    fire_tauri_drag_event 'tauri://drag-leave' >/dev/null ||
      die 'could not leave the empty-library drop state'
    wait_for_script \
      'return { overlay: Boolean(document.querySelector(".drop-scrim")) };' \
      '.overlay == false' \
      'empty-library drag leave did not clear the drop state' >/dev/null
    fire_tauri_drag_event 'tauri://drag-enter' "$public_fixture" >/dev/null ||
      die 'could not re-enter the empty-library drop state'
    fire_tauri_drag_event 'tauri://drag-drop' "$public_fixture" >/dev/null ||
      die 'could not drop into the empty library'
    wait_for_script \
      'const image = document.querySelector(".shelf-catalog .cover-art img"); const bounds = image?.getBoundingClientRect(); const cover = image?.parentElement.getBoundingClientRect(); return { cards: document.querySelectorAll(".shelf-catalog [data-shelf-open]").length, cover: Boolean(image?.complete && image.naturalWidth > 0), framed: Boolean(image?.classList.contains("loaded") && getComputedStyle(image).objectFit === "cover" && bounds.width > 0 && bounds.height > 0 && bounds.width <= cover.width && bounds.height <= cover.height), overlay: Boolean(document.querySelector(".drop-scrim")), working: Boolean(document.querySelector(".library-shell[aria-busy=true]")), status: document.querySelector(".toast-banner")?.textContent || "" };' \
      '.cards == 1 and .cover == true and .framed == true and .overlay == false and .working == false and .status == "已导入拖放的书籍"' \
      'drop did not add a prepared book with its cover to the empty library' >/dev/null
    execute_sync 'document.querySelector(".shelf-catalog .cover-art img")?.dispatchEvent(new Event("error")); return true;' >/dev/null ||
      die 'could not probe cover load failure'
    wait_for_script \
      'return { images: document.querySelectorAll(".shelf-catalog .cover-art img").length, fallback: document.querySelector(".shelf-catalog .cover-glyph")?.textContent || "", title: document.querySelector(".shelf-catalog [data-shelf-open]")?.title || "" };' \
      '.images == 0 and .title != "" and .fallback == (.title | .[0:1] | ascii_upcase)' \
      'cover load failure did not reveal the generated title-glyph fallback' >/dev/null
  fi

  fire_tauri_drag_event 'tauri://drag-drop' "$unsupported_file" >/dev/null ||
    die 'could not synthesize an unsupported drop'
  wait_for_script \
    'const status = document.querySelector(".toast-banner")?.textContent || ""; return { cards: document.querySelectorAll(".shelf-catalog [data-shelf-open]").length, overlay: Boolean(document.querySelector(".drop-scrim")), working: Boolean(document.querySelector(".library-shell[aria-busy=true]")), status, privateName: status.includes("unsupported.pdf") || status.includes("unsupported") || status.includes("/") };' \
    ".cards == $expected_books and .overlay == false and .working == false and .status == \"无法读取所选文件\" and .privateName == false" \
    'unsupported drop did not fail without changing the library' >/dev/null

  execute_sync 'location.reload(); return true;' >/dev/null || die 'could not reset the library after drop checks'
  wait_for_script \
    'const events = ["tauri://drag-enter", "tauri://drag-over", "tauri://drag-drop", "tauri://drag-leave"]; const listeners = globalThis.__internal_unstable_listeners_object_id__ || {}; return { ready: document.readyState, cards: document.querySelectorAll(".shelf-catalog [data-shelf-open]").length, listeners: events.map((event) => Object.getOwnPropertyNames(listeners[event] || {}).length), pathInUrl: location.href.includes("fb2-gate") };' \
    ".ready == \"complete\" and .cards == $expected_books and .listeners == [1,1,1,1] and .pathInUrl == false" \
    'drop listeners leaked or the library URL exposed a path' >/dev/null

  library_entry_evidence=$(jq -n \
    --argjson emptyShelf "$([[ $expected_books == 1 ]] && printf true || printf false)" \
    '{events: ["enter", "leave", "drop"], duplicateStable: true, unsupportedRejected: true, pathBoundary: 32, emptyShelf: $emptyShelf, firstImportCover: $emptyShelf, syntheticTauriEvents: true}')
}

verify_current_shelf() {
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
  set_shelf_query 'Atha FB2 Gate'
  wait_for_script '
return { titles: [...document.querySelectorAll(".book-list [data-shelf-open]")].map(book => book.title) };
' '.titles == ["Atha FB2 Gate"]' 'shelf search did not isolate the public book' >/dev/null
  webdriver_click_element "$book" || die 'could not open the search result'
  wait_for_script '
return { ready: document.querySelector(".reader-root")?.dataset.screenReady === "true",
  opening: Boolean(document.querySelector(".opening-art, .opening-veil, .opening-back")),
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

source "$script_root/lib/reader-linux-ui.sh"

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
    die "reader emitted runtime errors (state: $(safe_wait_state "$errors"))"
}

open_book() {
  local kind=$1 id
  case "$kind" in
    public) id=$public_id ;;
    formula) id=$(jq -er '.id' "$formula_record") ;;
    *) die 'unknown seeded book kind' ;;
  esac
  webdriver_click_element ".shelf-catalog [data-shelf-open=\"$id\"]" ||
    die 'could not open the seeded book'
  wait_for_script \
    'return { status: document.documentElement.dataset.status || null, error: document.documentElement.dataset.error || null, reader: new URL(location.href).searchParams.has("manifest"), ready: document.documentElement.hasAttribute("data-reader-ready"), busy: document.querySelector(".reader-shell")?.getAttribute("aria-busy"), screenReady: document.querySelector(".reader-root")?.dataset.screenReady, opening: Boolean(document.querySelector(".opening-art, .opening-veil, .opening-back")), failed: Boolean(document.querySelector(".reader-root [role=alert]")) };' \
    '.reader == true and .ready == true and .busy == "false" and .screenReady == "true" and .opening == false and .failed == false' \
    'reader did not become ready' >/dev/null
  current_book_kind=$kind
  diagnostics_enabled=0
}

enable_diagnostics() {
  local response
  if [[ ! -f $run_root/kernel-payload.json ]]; then
    local assets=$repo_root/reader module
    # The independent WebView host uses the same flat module composition.
    {
      cat "$script_root/lib/reader-linux-kernel.js"
      for module in content locator style-module-package preferences pagination session navigation \
        interaction content-actions structured-actions reader-state bookmarks search annotation-store \
        annotations message-store conversations diagnostics app; do
        if [[ $module == app ]]; then
          # Keep the standalone kernel's locator and preferences out of the
          # candidate's production reading state used by the following probes.
          sed 's/^const keyPrefix = .*/const keyPrefix = "atha.reader.probe";/' "$assets/web/app.mjs"
        else
          sed 's/^export //' "$assets/web/$module.mjs"
        fi
        printf '\n'
      done
      printf '\nreturn true;\n'
    } >"$run_root/kernel-script.js"
    jq -n --rawfile script "$run_root/kernel-script.js" \
      --rawfile html "$assets/atha-reader.html" --rawfile theme "$assets/theme.css" \
      --rawfile css "$assets/atha-reader.css" '{script:$script,args:[$html,$theme,$css]}' \
      >"$run_root/kernel-payload.json"
  fi
  response=$(curl --silent --show-error --fail --max-time 30 \
    --header 'Content-Type: application/json' --data-binary @"$run_root/kernel-payload.json" \
    "$base_url/session/$session_id/execute/sync") || die 'could not mount the kernel diagnostics in the candidate WebView'
  jq -e '.value == true' <<<"$response" >/dev/null || die 'kernel diagnostics setup did not execute'
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
  webdriver_native_action "$payload" || result=$?
  if ((result != 0)); then
    pointer_action_failure_diagnostic "$action" "$result"
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
        if [[ -z $formula_epub ]]; then
          printf 'gesture_snapshot=%s\n' "$(execute_sync 'return globalThis.__athaReaderDiagnostics.snapshot();' | jq -c '{pages, swipeDragging, resources, session, navigation}')" >&2
        fi
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
  cargo_target=$(mise exec -- cargo metadata --locked --no-deps --format-version 1 | jq -er .target_directory)
  application=$cargo_target/debug/atha-reader-app
  [[ -x $application ]] || die 'Tauri application binary was not produced'
fi

say 'seeding the isolated public FB2 library'
mkdir -p -- "$data_home"
ATHA_FB2_GATE_OUTPUT=$run_root ATHA_FB2_GATE_LIBRARY_ROOT=$library_root \
  mise exec -- cargo test --locked -p atha-backend --test fb2_import \
    writes_fb2_gate_fixture -- --ignored --exact >&2

readonly expected_fixture_sha=155225e7aa977574c5f75559f58ad121004bf714b91e10caeacd774da5550186
[[ $(sha256sum -- "$public_fixture" | awk '{print $1}') == "$expected_fixture_sha" ]] ||
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
printf '%s\n' "$public_fixture" 'fb2-gate.fb2' >>"$privacy_file"
printf '%s\n' \
  '公共记忆中的可跳转笔记' '公共记忆中的缺书笔记' \
  '这条最近回应应出现在笔记索引中' '这条回复用于验证消息之间的回应关系' \
  '正文重点' '第二节正文' '缺失原书引用' \
  '已移出的公共书籍' '公共作者' >>"$privacy_file"
add_record_privacy_tokens "$public_record"

expected_books=1
if [[ -n $formula_epub ]]; then
  say 'seeding the optional formula benchmark'
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
    "$application" "$public_fixture" "$association_extra" \
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
    'const state = new URL(location.href).searchParams.get("state"); return { status: document.documentElement.dataset.status || null, error: document.documentElement.dataset.error || null, screenReady: document.querySelector(".reader-root")?.dataset.screenReady, reader: new URL(location.href).searchParams.has("manifest"), ready: document.documentElement.hasAttribute("data-reader-ready"), state };' \
    ".reader == true and .ready == true and .screenReady == \"true\" and .state == \"$public_book_key\"" \
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
  'return { ready: document.readyState, cards: document.querySelectorAll(".shelf-catalog [data-shelf-open]").length, href: location.href, status: document.querySelector(".toast-banner")?.textContent || null };' \
  ".ready == \"complete\" and .cards == $expected_books" \
  'initial library did not become ready' >/dev/null


say 'verifying native drop entry and the shared library result set'
library_entry_evidence=null
verify_library_entry

say 'running public verify, boundary, and trusted gesture diagnostics'
set_window_size 600 760
open_book public
interface_navigation=$(verify_reader_interface_navigation)
enable_diagnostics
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
say "kernel gesture gate passed: $(jq -r .measurements <<<"$gesture_evidence") measured motion samples"
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
desktop_workspace_evidence=null
verify_desktop_workspace
verify_reader_redesign

# Author the original EPUB in a fresh message store, separate from seeded history.
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
baseline_book_count=$expected_books
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
  --argjson books "$baseline_book_count" \
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
    diagnosticsSurface: "standalone reader kernel in the candidate WebKitGTK view",
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
