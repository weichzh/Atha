#!/usr/bin/env bash
# Description: Exercise the current Linux Tauri reader against an isolated seeded library.

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

open_library() {
  local expected_books=$1
  execute_sync "location.assign('tauri://localhost'); return true;" >/dev/null ||
    die 'could not return to the library'
  wait_for_script \
    'return { ready: document.readyState, cards: document.querySelectorAll(".library-book-open").length, enabled: [...document.querySelectorAll(".library-book-open")].filter((button) => !button.disabled).length };' \
    ".ready == \"complete\" and .cards == $expected_books and .enabled == $expected_books" \
    'seeded library did not become ready' >/dev/null
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

perform_touch_action() {
  local point=$1 action=$2 action_id payload result=0
  action_id="atha-touch-${BASHPID}-${RANDOM}"
  if [[ $action == tap ]]; then
    payload=$(jq -cn --arg id "$action_id" --argjson point "$point" '{
      actions: [{
        type: "pointer", id: $id, parameters: {pointerType: "touch"},
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
        type: "pointer", id: $id, parameters: {pointerType: "touch"},
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
  .catch(() => done({ ok: false }));
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
      jq -e '.ok == true and (.value.id | type == "number")' <<<"$begin" >/dev/null ||
        die "gesture setup failed for $scenario"
      gesture_active=1
      perform_touch_action "$(jq -c '.value' <<<"$begin")" "$action" ||
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
      requestedPointerType: "touch",
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

say 'building the current Tauri application'
mise exec -- pnpm --dir reader/app build
mise exec -- cargo build --locked -p atha-reader-app
application=$repo_root/target/debug/atha-reader-app
[[ -x $application ]] || die 'Tauri application binary was not produced'

say 'seeding the isolated public FB2 library'
mkdir -p -- "$data_home"
ATHA_FB2_GATE_LIBRARY_ROOT=$library_root \
  mise exec -- cargo test --locked -p atha-backend --test fb2_import \
    writes_fb2_gate_fixture -- --ignored --exact

readonly expected_fixture_sha=155225e7aa977574c5f75559f58ad121004bf714b91e10caeacd774da5550186
[[ $(sha256sum .tmp/fb2-gate.fb2 | awk '{print $1}') == "$expected_fixture_sha" ]] ||
  die 'public FB2 fixture identity changed'
shopt -s nullglob
public_records=("$library_root/Library"/*.json)
(( ${#public_records[@]} == 1 )) || die 'public FB2 seed produced an invalid library'
public_record=${public_records[0]}
public_id=$(jq -er '.id' "$public_record") || die 'public FB2 library record is invalid'
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
    seeds_reading_memory_gui_fixture -- --ignored --exact
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
if grep -q '^WAYLAND_DISPLAY=' <<<"$user_environment"; then
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

session_payload=$(jq -cn --arg application "$application" '{
  capabilities: {alwaysMatch: {"tauri:options": {application: $application}}}
}')
session_response=$(webdriver_request POST /session "$session_payload") ||
  die 'tauri-driver could not create a session'
session_id=$(jq -er '.value.sessionId' <<<"$session_response") ||
  die 'tauri-driver returned no session'
browser_version=$(jq -r '.value.capabilities.browserVersion // "unknown"' <<<"$session_response")

webdriver_value POST "/session/$session_id/window/rect" '{"width":600,"height":760}' >/dev/null ||
  die 'could not size the reader window'
display_state=$(execute_sync \
  'return { visibility: document.visibilityState, width: screen.width, height: screen.height };') ||
  die 'could not inspect the Linux GUI display'
jq -e '.visibility == "visible" and .width > 0 and .height > 0' <<<"$display_state" >/dev/null ||
  die 'Linux GUI target has no active display; connect the GNOME desktop and retry.'

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

say 'verifying local-data management and reading memory at mobile and desktop widths'
for width in 360 1000; do
  webdriver_value POST "/session/$session_id/window/rect" \
    "{\"width\":$width,\"height\":760}" >/dev/null || die 'could not resize the library window'
  verify_library_management
  verify_reading_memory
done
webdriver_value POST "/session/$session_id/window/rect" '{"width":600,"height":760}' >/dev/null ||
  die 'could not restore the reader window size'

say 'running public verify, boundary, and gesture diagnostics'
open_book public
enable_diagnostics
pending_formula_queue=$(pending_formula_queue_probe)
open_gesture_section "$public_boundary_entry" >/dev/null
public_boundary=$(previous_boundary_probe)
open_gesture_section "$public_boundary_entry" >/dev/null
run_gesture_gate public

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
fi

verify_gesture_performance
gesture_evidence=$(gesture_summary)

webdriver_request DELETE "/session/$session_id" >/dev/null || die 'could not close the Tauri session'
session_id=
systemctl --user stop "$unit.service" >/dev/null || die 'could not stop tauri-driver'
unit_started=0
systemctl --user reset-failed "$unit.service" >/dev/null 2>&1 || true

check_app_log_privacy

jq -n \
  --arg browserVersion "$browser_version" \
  --argjson books "$expected_books" \
  --argjson pendingFormulaQueue "$pending_formula_queue" \
  --argjson publicBoundary "$public_boundary" \
  --argjson formulaShape "$formula_shape" \
  --argjson formulaBoundary "$formula_boundary" \
  --argjson scrollResource "$scroll_resource" \
  --argjson gestures "$gesture_evidence" \
  '{
    evidence: "Linux Tauri GUI",
    webview: ("WebKitGTK " + $browserVersion),
    books: $books,
    verifyDiagnostics: true,
    pendingFormulaQueue: $pendingFormulaQueue,
    publicBoundary: $publicBoundary,
    formulaShape: $formulaShape,
    formulaBoundary: $formulaBoundary,
    scrollResource: $scrollResource,
    gestures: $gestures,
    appLogPrivacy: "pass"
  }'
