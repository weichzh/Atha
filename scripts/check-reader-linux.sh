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

usage() {
  printf '%s\n' \
    'Usage: scripts/check-reader-linux.sh [options]' \
    '' \
    'Options:' \
    '  --formula-epub PATH          Add one private formula-heavy EPUB.' \
    '  --formula-metadata PATH      JSON sidecar with source_sha256 and entry.' \
    '  --minimum-formulas N         Required formula count (default: 1000).' \
    '  --minimum-pages N            Required formula-section pages (default: 10).' \
    '  --gesture-warmups N          Warmups per gesture scenario (default: 5).' \
    '  --gesture-measurements N     Measurements per scenario (default: 20).' \
    '  --application PATH           Run this candidate executable instead of building debug.' \
    '  --association-launch         Start the candidate with the public fixture path.' \
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

for command in curl jq mise realpath sha256sum ss systemctl systemd-run WebKitWebDriver; do
  command -v "$command" >/dev/null || die "$command is required"
done
[[ $(uname -s) == Linux ]] || die 'this gate requires Linux'

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
  response=$(webdriver_request "$method" "$path" "$body") || return 1
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
  cleanup_gesture >/dev/null 2>&1
  if [[ -n $session_id ]]; then
    webdriver_request DELETE "/session/$session_id" >/dev/null 2>&1
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

  wait_for_script \
    'return { width: innerWidth, height: innerHeight, ready: document.readyState, library: Boolean(document.querySelector(".library-shell")), cards: document.querySelectorAll(".library-book-open").length };' \
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
window.__TAURI_INTERNALS__.invoke('remove_library_book', { id: '$public_id' })
  .then((books) => done({ ok: true, count: books.length }))
  .catch(() => done({ ok: false }));
") || die 'could not empty the isolated library'
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
      'return { cards: document.querySelectorAll(".library-book-open").length, overlay: Boolean(document.querySelector(".library-drop-overlay")), working: Boolean(document.querySelector(".library-work-overlay")), status: document.querySelector(".library-status")?.textContent || "" };' \
      '.cards == 1 and .overlay == false and .working == false and .status == "已加入书架。"' \
      'drop did not add a book to the empty library' >/dev/null
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
    '{events: ["enter", "leave", "drop"], duplicateStable: true, unsupportedRejected: true, pathBoundary: 32, emptyShelf: $emptyShelf, syntheticTauriEvents: true}')
}

verify_library_layout() {
  local width=$1 height=$2 state
  state=$(execute_sync '
document.querySelector("button[aria-label=\"列表视图\"]")?.click();
return true;
') || die 'could not switch to the library list view'
  [[ $state == true ]] || die 'library list control is unavailable'
  state=$(wait_for_script \
    'const cards = [...document.querySelectorAll(".library-book")]; const valid = cards.every((card) => { const button = card.querySelector(".library-book-open"); const cover = card.querySelector(".library-cover"); const title = card.querySelector(".library-book-title"); const author = card.querySelector(".library-book-author"); const outer = button?.getBoundingClientRect(); const coverRect = cover?.getBoundingClientRect(); const titleRect = title?.getBoundingClientRect(); const authorRect = author?.getBoundingClientRect(); return Boolean(outer && coverRect && titleRect && authorRect && coverRect.left >= outer.left - 1 && coverRect.right <= titleRect.left + 1 && titleRect.right <= outer.right + 1 && authorRect.right <= outer.right + 1 && titleRect.top <= authorRect.top && coverRect.bottom <= outer.bottom + 1); }); const controls = [...document.querySelectorAll(".library-header button, .library-header input")]; const clippedControls = controls.filter((control) => control.scrollWidth > control.clientWidth + 1 || control.scrollHeight > control.clientHeight + 1); return { width: innerWidth, list: document.querySelector(".library-shell")?.classList.contains("library-list-view"), pressed: document.querySelector("button[aria-label=\"列表视图\"]")?.getAttribute("aria-pressed"), cards: cards.length, valid, overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth, clipped: clippedControls.length > 0, clippedControls: clippedControls.map((control) => ({ label: control.getAttribute("aria-label") || control.textContent.trim(), client: [control.clientWidth, control.clientHeight], scroll: [control.scrollWidth, control.scrollHeight] })) };' \
    ".width >= $((width - 2)) and .width <= $((width + 2)) and .list == true and .pressed == \"true\" and .cards == $expected_books and .valid == true and .overflow == false and .clipped == false" \
    "library list layout failed at ${width}x${height}")

  execute_sync 'document.querySelector("button[aria-label=\"网格视图\"]")?.click(); return true;' >/dev/null ||
    die 'could not switch to the library grid view'
  wait_for_script \
    'const cards = [...document.querySelectorAll(".library-book")].map((card) => card.getBoundingClientRect()); const overlap = cards.some((left, index) => cards.slice(index + 1).some((right) => left.left < right.right && left.right > right.left && left.top < right.bottom && left.bottom > right.top)); return { width: innerWidth, grid: !document.querySelector(".library-shell")?.classList.contains("library-list-view"), pressed: document.querySelector("button[aria-label=\"网格视图\"]")?.getAttribute("aria-pressed"), cards: cards.length, overlap, overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth };' \
    ".width >= $((width - 2)) and .width <= $((width + 2)) and .grid == true and .pressed == \"true\" and .cards == $expected_books and .overlap == false and .overflow == false" \
    "library grid layout failed at ${width}x${height}" >/dev/null

  if ((width == 1000)); then
    execute_sync '
const input = document.querySelector(".library-search-field input");
const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
setter?.call(input, "Atha FB2 Gate");
input?.dispatchEvent(new Event("input", { bubbles: true }));
return Boolean(input);
' >/dev/null || die 'could not search the library'
    wait_for_script \
      'return { cards: document.querySelectorAll(".library-book-open").length, title: document.querySelector(".library-book-title")?.textContent || "" };' \
      '.cards == 1 and .title == "Atha FB2 Gate"' \
      'library search no longer filters the shared result set' >/dev/null
    execute_sync '
const input = document.querySelector(".library-search-field input");
const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
setter?.call(input, "");
input?.dispatchEvent(new Event("input", { bubbles: true }));
[...document.querySelectorAll(".library-views button")].find((button) => button.textContent.trim() === "书名")?.click();
return true;
' >/dev/null || die 'could not restore and sort the library'
    wait_for_script \
      'return { cards: document.querySelectorAll(".library-book-open").length, selected: [...document.querySelectorAll(".library-views button")].find((button) => button.textContent.trim() === "书名")?.getAttribute("aria-current") || null };' \
      ".cards == $expected_books and .selected == \"page\"" \
      'library title sort no longer uses the shared result set' >/dev/null
    execute_sync '
[...document.querySelectorAll(".library-views button")].find((button) => button.textContent.trim() === "进度")?.click();
return true;
' >/dev/null || die 'could not open the progress grouping'
    wait_for_script \
      'return { groups: [...document.querySelectorAll(".library-group-heading h2")].map((heading) => heading.textContent), cards: document.querySelectorAll(".library-book-open").length };' \
      ".groups == [\"在读\",\"未开始\"] and .cards == $expected_books" \
      'library progress grouping no longer uses the shared result set' >/dev/null
    execute_sync '
[...document.querySelectorAll(".library-views button")].find((button) => button.textContent.trim() === "默认")?.click();
return true;
' >/dev/null || die 'could not restore the default library view'
  fi
}

verify_library_management() {
  local menu storage selection
  menu=$(execute_sync '
const details = document.querySelector(".library-management");
details?.querySelector("summary")?.click();
return {
  open: Boolean(details?.open),
  labels: [...(details?.querySelectorAll(".library-management-menu button") || [])].map((button) => button.textContent.trim()),
  overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth
};') || die 'could not inspect library management'
  jq -e '
    .open == true and .overflow == false and
    .labels == ["备份资料库", "恢复资料库", "存储占用"]
  ' <<<"$menu" >/dev/null || die "library management is incomplete (state: $(jq -c . <<<"$menu"))"

  [[ $(execute_sync '
const button = [...document.querySelectorAll(".library-management-menu button")]
  .find((item) => item.textContent.includes("存储占用"));
button?.click();
return Boolean(button);') == true ]] || die 'could not open storage usage'
  storage=$(wait_for_script \
    'const dialog = document.querySelector(".library-storage-dialog"); const rows = [...(dialog?.querySelectorAll("dl > div") || [])]; return { open: Boolean(dialog?.open), rows: rows.length, labels: rows.map((row) => row.querySelector("dt")?.textContent || ""), overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth, dialogOverflow: dialog ? dialog.scrollWidth > dialog.clientWidth : true };' \
    '.open == true and .rows == 6 and .overflow == false and .dialogOverflow == false' \
    'storage usage did not become ready')
  jq -e '
    .labels == ["书籍文件", "阅读缓存", "消息与快照", "离线词典", "阅读设置", "合计"]
  ' <<<"$storage" >/dev/null || die "storage usage categories are incomplete (state: $(jq -c . <<<"$storage"))"

  [[ $(execute_sync '
document.querySelector(".library-storage-dialog")?.close();
const button = [...document.querySelectorAll(".library-primary-actions button")]
  .find((item) => item.textContent.includes("选择"));
button?.click();
return Boolean(button);') == true ]] || die 'could not enter library selection'
  selection=$(wait_for_script \
    'const buttons = [...document.querySelectorAll(".library-selection-bar button")]; const rects = buttons.map((button) => button.getBoundingClientRect()); return { count: buttons.length, labels: buttons.map((button) => button.textContent.trim()), overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth, clipped: buttons.some((button) => button.scrollWidth > button.clientWidth || button.scrollHeight > button.clientHeight), inViewport: rects.every((rect) => rect.left >= 0 && rect.right <= innerWidth && rect.top >= 0 && rect.bottom <= innerHeight), separated: rects.length == 2 && rects[0].right <= rects[1].left };' \
    '.count == 2 and .overflow == false and .clipped == false and .inViewport == true and .separated == true' \
    'library deletion actions do not fit the viewport')
  jq -e '.labels == ["移出书架", "删除本地数据"]' <<<"$selection" >/dev/null ||
    die "library deletion actions are incomplete (state: $(jq -c . <<<"$selection"))"
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
  recent: document.querySelectorAll(".memory-recent-list > button").length,
  overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
  clipped: controls.some((control) => control.scrollWidth > control.clientWidth || control.scrollHeight > control.clientHeight)
};') || die 'could not enter reading memory'
  jq -e '
    .entered == true and .heading == "阅读记忆" and .recent == 1 and
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
    'const overlay = document.querySelector("#message-conversation"); return { reader: location.pathname.endsWith("/index.html"), ready: document.documentElement.hasAttribute("data-reader-ready"), navigation: document.documentElement.dataset.readingMemoryNavigation || null, conversation: Boolean(overlay && !overlay.hidden), sourceAvailable: Boolean(document.querySelector("#message-conversation-source")?.textContent), error: document.documentElement.dataset.error || null };' \
    '.reader == true and .ready == true and .navigation == "ok" and .conversation == true and .sourceAvailable == true and .error == null' \
    'reading memory result did not validate and open')
  jq -e '.navigation == "ok"' <<<"$jump" >/dev/null ||
    die "reading memory result is incomplete (state: $(jq -c . <<<"$jump"))"
  open_library "$expected_books"
}

verify_desktop_workspace() {
  local baseline resized restored search notes directory keyboard tool tool_switch
  local baseline_anchor resized_anchor restored_anchor tool_switch_anchor

  baseline=$(wait_for_script \
    'const frame = document.querySelector(".reader-frame"); const panel = document.querySelector(".reader-tool.directory > .tool-panel"); const frameRect = frame?.getBoundingClientRect(); const panelRect = panel?.getBoundingClientRect(); const readerRect = document.querySelector(".reader")?.getBoundingClientRect(); return { desktop: document.documentElement.hasAttribute("data-desktop-workspace"), active: document.documentElement.dataset.workspacePanel || null, openPanels: document.querySelectorAll(".reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]").length, layout: Boolean(frameRect && panelRect && readerRect && panelRect.right <= frameRect.left + 1 && Math.abs(frameRect.right - innerWidth) <= 2 && Math.abs(readerRect.left - frameRect.left) <= 1 && Math.abs(readerRect.width - frameRect.width) <= 1), overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth || (panel ? panel.scrollWidth > panel.clientWidth : true), locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null, error: document.documentElement.dataset.error || null };' \
    '.desktop == true and .active == "directory" and .openPanels == 1 and .layout == true and .overflow == false and .locator != null and .error == null' \
    'desktop directory workspace did not become ready')
  baseline_anchor=$(jq -r '.locator | fromjson | [.start.section, .start.offset] | @tsv' <<<"$baseline")

  set_window_size 1600 900
  resized=$(wait_for_script \
    'const frame = document.querySelector(".reader-frame"); const reader = document.querySelector(".reader"); const panel = document.querySelector(".reader-tool.directory > .tool-panel"); const frameRect = frame?.getBoundingClientRect(); const readerRect = reader?.getBoundingClientRect(); const panelRect = panel?.getBoundingClientRect(); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { desktop: document.documentElement.hasAttribute("data-desktop-workspace"), active: document.documentElement.dataset.workspacePanel || null, stable: document.documentElement.dataset.viewportStable === expected, layout: Boolean(frameRect && readerRect && panelRect && panelRect.right <= frameRect.left + 1 && Math.abs(frameRect.right - innerWidth) <= 2 && Math.abs(readerRect.left - frameRect.left) <= 1 && Math.abs(readerRect.width - frameRect.width) <= 1), locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null, error: document.documentElement.dataset.error || null };' \
    '.desktop == true and .active == "directory" and .stable == true and .layout == true and .locator != null and .error == null' \
    'resized desktop workspace did not stabilize')
  resized_anchor=$(jq -r '.locator | fromjson | [.start.section, .start.offset] | @tsv' <<<"$resized")
  [[ $resized_anchor == "$baseline_anchor" ]] || die 'desktop resize did not preserve the current locator'

  set_window_size 1280 800
  restored=$(wait_for_script \
    'const frame = document.querySelector(".reader-frame"); const reader = document.querySelector(".reader"); const panel = document.querySelector(".reader-tool.directory > .tool-panel"); const frameRect = frame?.getBoundingClientRect(); const readerRect = reader?.getBoundingClientRect(); const panelRect = panel?.getBoundingClientRect(); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { stable: document.documentElement.dataset.viewportStable === expected, layout: Boolean(frameRect && readerRect && panelRect && panelRect.right <= frameRect.left + 1 && Math.abs(frameRect.right - innerWidth) <= 2 && Math.abs(readerRect.left - frameRect.left) <= 1 && Math.abs(readerRect.width - frameRect.width) <= 1), locator: globalThis.__athaReaderDiagnostics?.snapshot().navigation.current || null, error: document.documentElement.dataset.error || null };' \
    '.stable == true and .layout == true and .locator != null and .error == null' \
    'restored desktop workspace did not stabilize')
  restored_anchor=$(jq -r '.locator | fromjson | [.start.section, .start.offset] | @tsv' <<<"$restored")
  [[ $restored_anchor == "$baseline_anchor" ]] || die 'desktop restore did not preserve the current locator'

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
  [[ $(execute_sync '
const results = document.querySelector("#search-results");
results.selectedIndex = 0;
document.querySelector("#go-search-result")?.click();
return results.options.length > 0;') == true ]] || die 'could not open a desktop search result'
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
    'return { active: document.documentElement.dataset.workspacePanel || null, focused: document.activeElement === document.querySelector(".reader-tool.notes > summary"), items: document.querySelectorAll(".annotation-item-main").length, openPanels: document.querySelectorAll(".reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]").length };' \
    '.active == "notes" and .focused == true and .items > 0 and .openPanels == 1' \
    'desktop notes workspace did not become ready')
  [[ $(execute_sync '
document.querySelector(".annotation-item-main")?.click();
return true;') == true ]] || die 'could not open a desktop message conversation'
  wait_for_script \
    'const overlay = document.querySelector("#message-conversation"); return { open: Boolean(overlay && !overlay.hidden), source: Boolean(document.querySelector("#message-conversation-source")?.textContent), error: document.documentElement.dataset.error || null };' \
    '.open == true and .source == true and .error == null' \
    'desktop message conversation did not open' >/dev/null
  execute_sync 'document.querySelector("#message-conversation-close")?.click(); return true;' >/dev/null ||
    die 'could not close the desktop message conversation'

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

  keyboard=$(wait_for_script \
    'const input = document.querySelector("#search-query"); const inputFocused = document.activeElement === input; const event = new KeyboardEvent("keydown", {key: "Escape", bubbles: true, cancelable: true}); input.dispatchEvent(event); const before = globalThis.__athaReaderDiagnostics.snapshot(); const pageEvent = new KeyboardEvent("keydown", {key: "PageDown", bubbles: true, cancelable: true}); document.querySelector(".reader")?.dispatchEvent(pageEvent); return {inputFocused, focused: document.activeElement === document.querySelector(".reader"), escapeHandled: event.defaultPrevented, pageHandled: pageEvent.defaultPrevented, beforeSection: before.session.currentIndex, beforePage: before.navigation.current};' \
    '.inputFocused == true and .focused == true and .escapeHandled == true and .pageHandled == true' \
    'desktop escape or page keyboard navigation was not handled')
  wait_for_script \
    'const state = globalThis.__athaReaderDiagnostics?.snapshot(); return { current: state?.navigation.current || null, error: document.documentElement.dataset.error || null };' \
    "(.current != $(jq -c '.beforePage' <<<"$keyboard")) and .error == null" \
    'desktop PageDown did not navigate' >/dev/null

  set_window_size 600 760
  wait_for_script \
    'const frame = document.querySelector(".reader-frame"); const reader = document.querySelector(".reader"); const frameRect = frame?.getBoundingClientRect(); const readerRect = reader?.getBoundingClientRect(); const expected = reader ? `${reader.clientWidth}x${reader.clientHeight}` : null; return { desktop: document.documentElement.hasAttribute("data-desktop-workspace"), workspace: document.documentElement.dataset.workspacePanel || null, openPanels: document.querySelectorAll(".reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]").length, stable: document.documentElement.dataset.viewportStable === expected, full: Boolean(frameRect && readerRect && Math.abs(frameRect.left) <= 1 && Math.abs(frameRect.width - innerWidth) <= 2 && Math.abs(readerRect.left - frameRect.left) <= 1 && Math.abs(readerRect.width - frameRect.width) <= 1), error: document.documentElement.dataset.error || null };' \
    '.desktop == false and .workspace == null and .openPanels == 0 and .stable == true and .full == true and .error == null' \
    'narrow reader did not restore the mobile tool layout' >/dev/null
  for tool in directory search notes; do
    execute_sync "
document.documentElement.setAttribute('data-reader-tools', '');
document.querySelector('.reader-tool.$tool > summary')?.click();
return true;" >/dev/null || die "could not open the narrow $tool tool"
    wait_for_script \
      "const details = document.querySelector('.reader-tool.$tool'); const rect = details?.querySelector('.tool-panel')?.getBoundingClientRect(); const fullscreen = '$tool' !== 'search'; return { open: Boolean(details?.open), only: document.querySelectorAll('.reader-tool.directory[open], .reader-tool.search[open], .reader-tool.notes[open]').length === 1, layout: Boolean(rect && Math.abs(rect.left) <= 1 && Math.abs(rect.width - innerWidth) <= 2 && (fullscreen ? Math.abs(rect.top) <= 1 && Math.abs(rect.bottom - innerHeight) <= 2 : rect.top > 0 && rect.bottom <= innerHeight - 47)), overflow: Boolean(rect && details.querySelector('.tool-panel').scrollWidth > rect.width + 1) };" \
      '.open == true and .only == true and .layout == true and .overflow == false' \
      "narrow $tool tool no longer uses the mobile layout" >/dev/null
    execute_sync "document.querySelector('.reader-tool.$tool').open = false; document.documentElement.removeAttribute('data-reader-tools'); return true;" >/dev/null ||
      die "could not close the narrow $tool tool"
  done
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
    '{widths: [1280, 1600], tools: ["directory", "search", "notes"], searchResults: $searchResults, messages: $messages, locatorStable: true, keyboard: true, focusVerified: true, narrowRegression: true}')
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
  webdriver_value DELETE "/session/$session_id/actions" >/dev/null 2>&1 || true
  return "$result"
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
  local logs_root=$library_root/logs log_count=0 log
  awk 'length >= 8' "$privacy_file" | sort -u >"$privacy_file.filtered"
  while IFS= read -r -d '' log; do
    log_count=$((log_count + 1))
    if grep -Fq -f "$privacy_file.filtered" -- "$log"; then
      die 'AppLog privacy check found seeded-book data'
    fi
  done < <(find "$logs_root" -maxdepth 1 -type f -print0 2>/dev/null)
  ((log_count > 0)) || die 'AppLog privacy check found no log output'
}

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
ATHA_READING_MEMORY_GATE_ROOT=$library_root \
ATHA_READING_MEMORY_GATE_EDITION=$public_id \
ATHA_READING_MEMORY_GATE_SECTION=$memory_section \
  mise exec -- cargo test --locked -p atha-backend --test message_reading \
    seeds_reading_memory_gui_fixture -- --ignored --exact >&2
printf '%s\n' "$repo_root/.tmp/fb2-gate.fb2" 'fb2-gate.fb2' >>"$privacy_file"
printf '%s\n' \
  '公共记忆中的可跳转笔记' '公共记忆中的缺书笔记' \
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

driver=${ATHA_TAURI_DRIVER:-}
if [[ -n $driver ]]; then
  [[ -x $driver ]] || die 'ATHA_TAURI_DRIVER is not executable'
  driver=$(realpath -e -- "$driver")
elif command -v tauri-driver >/dev/null; then
  driver=$(command -v tauri-driver)
elif [[ -x $repo_root/.tmp/tauri-driver/bin/tauri-driver ]]; then
  driver=$repo_root/.tmp/tauri-driver/bin/tauri-driver
else
  die 'tauri-driver is required'
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
  '.width > 0 and .height > 0' \
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

wait_for_script \
  'return { ready: document.readyState, cards: document.querySelectorAll(".library-book-open").length, href: location.href, status: document.querySelector(".library-status")?.textContent || null };' \
  ".ready == \"complete\" and .cards == $expected_books" \
  'initial library did not become ready' >/dev/null

memory_statistics=$(jq -cn --arg id "$public_id" '{
  schema: 1,
  days: [{date: "2026-08-13", durationMs: 90000}],
  books: [{contentVersion: $id, durationMs: 90000, lastReadDate: "2026-08-13"}]
}')
memory_statistics_args=$(jq -cn --arg value "$memory_statistics" '[$value]')
[[ $(execute_sync '
localStorage.setItem("atha.reader.statistics.v1", arguments[0]);
return true;' "$memory_statistics_args") == true ]] || die 'could not seed reading memory statistics'

say 'verifying native drop entry and the shared library result set'
library_entry_evidence=null
verify_library_entry

say 'running public verify, boundary, and trusted gesture diagnostics'
open_book public
enable_diagnostics
pending_formula_queue=$(pending_formula_queue_probe)
open_gesture_section "$public_boundary_entry" >/dev/null
public_boundary=$(previous_boundary_probe)
open_gesture_section "$public_boundary_entry" >/dev/null
webdriver_value POST "/session/$session_id/window/rect" '{"width":600,"height":760}' >/dev/null ||
  die 'could not prepare the trusted gesture viewport'
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

say 'verifying local-data management and reading memory at mobile and desktop widths'
for width in 360 1000; do
  set_window_size "$width" 760
  verify_library_management
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
  'const frame = document.querySelector(".reader-frame")?.getBoundingClientRect(); const panel = document.querySelector(".directory-panel")?.getBoundingClientRect(); return {width: innerWidth, desktop: document.documentElement.hasAttribute("data-desktop-workspace"), active: document.documentElement.dataset.workspacePanel || null, layout: Boolean(frame && panel && panel.right <= frame.left + 1 && Math.abs(frame.right - innerWidth) <= 2), error: document.documentElement.dataset.error || null};' \
  '.desktop == true and .active == "directory" and .layout == true and .error == null' \
  'normal desktop reader startup did not produce the workspace' >/dev/null
set_window_size 600 760
enable_diagnostics
set_window_size 1280 800
desktop_workspace_evidence=null
verify_desktop_workspace
verify_reader_runtime_errors

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
  --argjson books "$expected_books" \
  --argjson libraryEntry "$library_entry_evidence" \
  --argjson fileAssociation "$association_evidence" \
  --argjson pendingFormulaQueue "$pending_formula_queue" \
  --argjson desktopWorkspace "$desktop_workspace_evidence" \
  --argjson publicBoundary "$public_boundary" \
  --argjson formulaShape "$formula_shape" \
  --argjson formulaBoundary "$formula_boundary" \
  --argjson scrollResource "$scroll_resource" \
  --argjson gestures "$gesture_evidence" \
  '{
    evidence: "Linux Tauri GUI",
    webview: ("WebKitGTK " + $browserVersion),
    books: $books,
    libraryEntry: $libraryEntry,
    fileAssociation: $fileAssociation,
    verifyDiagnostics: true,
    desktopWorkspace: $desktopWorkspace,
    pendingFormulaQueue: $pendingFormulaQueue,
    publicBoundary: $publicBoundary,
    formulaShape: $formulaShape,
    formulaBoundary: $formulaBoundary,
    scrollResource: $scrollResource,
    gestures: $gestures,
    appLogPrivacy: "pass"
  }'
