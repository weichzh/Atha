#!/usr/bin/env bash
# Description: Measure the live PCT reader SurfaceFlinger presentation cadence.
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
device=
duration=10
package=com.atha.reader
swipe=none
transition=any
self_check=false
cdp_port=

die() {
  printf '%s\n' "$*" >&2
  exit 1
}

has_page_step() {
  local direction=$1 section=$2 sections=$3 page=$4 pages=$5
  if ((direction > 0)); then
    ((page + 1 < pages || section + 1 < sections))
  else
    ((page > 0 || section > 0))
  fi
}

single_page_step() {
  local direction=$1 before_section=$2 before_page=$3 before_pages=$4
  local after_section=$5 after_page=$6 after_pages=$7
  if ((direction > 0)); then
    ((
      (after_section == before_section && after_page == before_page + 1) ||
        (after_section == before_section + 1 && before_page == before_pages - 1 && after_page == 0)
    ))
  else
    ((
      (after_section == before_section && after_page == before_page - 1) ||
        (after_section == before_section - 1 && before_page == 0 && after_page == after_pages - 1)
    ))
  fi
}

transition_start() {
  local expected=$1 direction=$2 section=$3 sections=$4 page=$5 pages=$6
  case "$expected" in
    any) has_page_step "$direction" "$section" "$sections" "$page" "$pages" ;;
    same-section)
      if ((direction > 0)); then ((page + 1 < pages)); else ((page > 0)); fi
      ;;
    cross-section)
      if ((direction > 0)); then
        ((page + 1 == pages && section + 1 < sections))
      else
        ((page == 0 && section > 0))
      fi
      ;;
    *) return 1 ;;
  esac
}

surface_present_timestamps() {
  local last=${1:-0}
  awk -v last="$last" '
    NF == 3 && $2 > last && $2 != "9223372036854775807" { print $2 }
  ' | sort -nu
}

self_check_script() {
  has_page_step 1 0 2 0 2
  has_page_step -1 1 2 0 3
  if has_page_step -1 0 2 0 3; then return 1; fi
  single_page_step 1 0 0 3 0 1 3
  single_page_step 1 0 2 3 1 0 4
  single_page_step -1 1 0 4 0 2 3
  if single_page_step 1 0 0 3 0 2 3; then return 1; fi
  transition_start same-section 1 0 2 0 3
  transition_start cross-section 1 0 2 2 3
  transition_start cross-section -1 1 2 0 3
  if transition_start same-section -1 1 2 0 3; then return 1; fi
  metrics=$(printf '%s\n' \
    'Flags,IntendedVsync,Vsync,OldestInputEvent,NewestInputEvent,HandleInputStart,AnimationStart,PerformTraversalsStart,DrawStart,SyncQueued,SyncStart,IssueDrawCommandsStart,SwapBuffers,FrameCompleted,DequeueBufferDuration,QueueBufferDuration,' \
    '0,1000000000,1000000000,0,0,0,0,0,0,0,0,0,0,1010000000,0,0,' \
    '1,1016666667,1016666667,0,0,0,0,0,0,0,0,0,0,1099999999,0,0,' \
    '0,1033333334,1033333334,0,0,0,0,0,0,0,0,0,0,1063333334,0,0,' |
    awk -F, -v threshold=25 '
      /^0,/ && $2 > 0 && $14 > $2 && $14 != "9223372036854775807" {
        duration = ($14 - $2) / 1000000
        count++
        total += duration
        if (duration > maximum) maximum = duration
        if (duration > threshold) slow++
      }
      END { printf "%d %.1f %.1f %d", count, total / count, maximum, slow + 0 }
    ')
  [[ "$metrics" == '2 20.0 30.0 1' ]]
  timestamps=$(printf '%s\n' \
    '16666667' \
    '0 100 0' \
    '0 9223372036854775807 0' \
    '0 40 0' \
    '200 200 9223372036854775807' |
    surface_present_timestamps 50 |
    paste -sd,)
  [[ "$timestamps" == '100,200' ]]
  printf 'self-check=pass\n'
}

while (($#)); do
  case "$1" in
    --device)
      (($# >= 2)) || die 'Missing device serial.'
      device=$2
      shift 2
      ;;
    --duration)
      (($# >= 2)) || die 'Missing duration.'
      duration=$2
      shift 2
      ;;
    --swipe)
      (($# >= 2)) || die 'Missing swipe direction.'
      swipe=$2
      shift 2
      ;;
    --transition)
      (($# >= 2)) || die 'Missing transition kind.'
      transition=$2
      shift 2
      ;;
    --cdp-port)
      (($# >= 2)) || die 'Missing CDP port.'
      cdp_port=$2
      shift 2
      ;;
    --self-check)
      self_check=true
      shift
      ;;
    -h | --help)
      printf 'Usage: scripts/check-pct-reader-fps.sh --device SERIAL [--duration SECONDS] [--swipe none|forward|backward] [--transition any|same-section|cross-section] [--cdp-port PORT]\n'
      printf '       scripts/check-pct-reader-fps.sh --self-check\n'
      exit 0
      ;;
    *) die 'Unknown argument.' ;;
  esac
done

if [[ "$self_check" == true ]]; then
  self_check_script
  exit 0
fi

[[ -n "$device" ]] || die 'Device serial is required.'
if [[ ! "$duration" =~ ^[0-9]+$ ]] || ((duration < 1 || duration > 120)); then
  die 'Duration must be an integer from 1 to 120 seconds.'
fi
[[ "$swipe" == none || "$swipe" == forward || "$swipe" == backward ]] ||
  die 'Swipe must be none, forward, or backward.'
[[ "$transition" == any || "$transition" == same-section || "$transition" == cross-section ]] ||
  die 'Transition must be any, same-section, or cross-section.'
if [[ "$swipe" == none && "$transition" != any ]]; then
  die 'A transition kind requires an automatic swipe.'
fi
if [[ "$swipe" != none ]] && ((duration > 2)); then
  die 'Automatic swipe duration must not exceed 2 seconds on the 60 Hz PCT ring buffer.'
fi

sdk=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}
adb="$sdk/platform-tools/adb"
[[ -x "$adb" ]] || die 'adb is unavailable.'
[[ $($adb -s "$device" shell getprop ro.product.model | tr -d '\r') == PCT-AL10 ]] ||
  die 'The selected device is not PCT-AL10.'

window_dump=$($adb -s "$device" shell dumpsys window | tr -d '\r')
focus=$(grep -m1 'mCurrentFocus=' <<<"$window_dump")
[[ "$focus" == *"$package"* ]] || die 'Atha is not the foreground application.'

layer_list=$($adb -s "$device" shell dumpsys SurfaceFlinger --list | tr -d '\r')
layer=$(grep -F -m 1 -x "$package/$package.MainActivity#0" <<<"$layer_list")
[[ -n "$layer" ]] || die 'The visible Atha SurfaceFlinger layer is unavailable.'

cdp_reader_probe() {
  local operation=$1 direction=$2
  CDP_PORT=$cdp_port CDP_OPERATION=$operation CDP_DIRECTION=$direction \
    mise exec -- node --input-type=module <<'NODE'
const port = process.env.CDP_PORT;
const operation = process.env.CDP_OPERATION;
const direction = process.env.CDP_DIRECTION;

function browserProbe(selectedOperation, selectedDirection) {
  const invalid = (reason) => ({ ok: false, reason });
  const state = () => {
    const diagnostics = globalThis.__athaReaderDiagnostics;
    if (typeof diagnostics?.snapshot !== "function") {
      return invalid("diagnostics-unavailable");
    }
    const snapshot = diagnostics.snapshot();
    const reader = document.querySelector(".reader");
    const position = document.querySelector("#progress-position")?.textContent?.trim() || "";
    const match = /^第\s*(\d+)\/(\d+)\s*节\s*·\s*本节\s*(\d+)\/(\d+)\s*页$/.exec(position);
    if (!reader || !match) return invalid("reader-state-unavailable");
    let locator;
    try {
      locator = JSON.parse(snapshot.navigation?.current);
    } catch {
      return invalid("locator-unavailable");
    }
    const section = Number(match[1]) - 1;
    const sections = Number(match[2]);
    const page = Number(match[3]) - 1;
    const pages = Number(match[4]);
    if (
      document.documentElement.dataset.status !== "pass" ||
      document.documentElement.dataset.sessionState !== "layout-stable" ||
      reader.dataset.readingMode !== "paged" ||
      snapshot.session?.currentIndex !== section ||
      snapshot.session?.sections !== sections ||
      snapshot.pages !== pages ||
      !Number.isInteger(page) ||
      page < 0 ||
      page >= pages ||
      locator?.schema !== 1 ||
      !/^[a-f0-9]{64}$/.test(snapshot.session?.contentVersion || "") ||
      locator?.contentVersion !== snapshot.session.contentVersion ||
      !/^[a-z0-9][a-z0-9._-]{0,63}$/.test(locator?.start?.section || "") ||
      !Number.isInteger(locator?.start?.offset) ||
      locator.start.offset < 0 ||
      locator.start.offset > 2147483647
    ) {
      return invalid("reader-state-unstable");
    }
    return {
      ok: true,
      reader,
      locator: snapshot.navigation.current,
      section,
      sections,
      page,
      pages,
    };
  };

  if (selectedOperation === "prepare") {
    const current = state();
    if (!current.ok) return current;
    if (
      document.visibilityState !== "visible" ||
      document.documentElement.hasAttribute("data-reader-tools") ||
      document.querySelector("dialog[open], details[open]") ||
      !getSelection()?.isCollapsed
    ) {
      return invalid("reader-viewport-obscured");
    }
    const rect = current.reader.getBoundingClientRect();
    if (
      rect.width < 200 ||
      rect.height < 300 ||
      rect.left < 0 ||
      rect.top < 0 ||
      rect.right > innerWidth + 1 ||
      rect.bottom > innerHeight + 1 ||
      !Number.isFinite(devicePixelRatio) ||
      devicePixelRatio <= 0 ||
      screen.width <= 0 ||
      screen.height <= 0
    ) {
      return invalid("reader-viewport-invalid");
    }
    const forward = selectedDirection === "forward";
    const startX = rect.left + rect.width * (forward ? 0.8 : 0.2);
    const endX = rect.left + rect.width * (forward ? 0.2 : 0.8);
    const y = rect.top + rect.height * 0.5;
    const hitsReader = (x) => {
      const target = document.elementFromPoint(x, y);
      return Boolean(target && (target === current.reader || current.reader.contains(target)));
    };
    if (!hitsReader(startX) || !hitsReader(endX)) return invalid("reader-hit-test-failed");
    globalThis.__athaFpsProbe = Object.freeze({
      locator: current.locator,
      section: current.section,
      sections: current.sections,
      page: current.page,
      pages: current.pages,
      direction: selectedDirection,
    });
    return {
      ok: true,
      section: current.section,
      sections: current.sections,
      page: current.page,
      pages: current.pages,
      startX,
      endX,
      y,
      screenX: Number.isFinite(screenX) ? screenX : 0,
      screenY: Number.isFinite(screenY) ? screenY : 0,
      screenWidth: screen.width,
      screenHeight: screen.height,
      scale: devicePixelRatio,
    };
  }

  const before = globalThis.__athaFpsProbe;
  if (!before || before.direction !== selectedDirection) return invalid("probe-state-unavailable");
  const current = state();
  if (!current.ok) return current;
  const sameAsPreviousRead =
    before.lastLocator === current.locator &&
    before.lastSection === current.section &&
    before.lastPage === current.page;
  const pageChanged = current.section !== before.section || current.page !== before.page;
  const settled =
    !document.querySelector(".reader")?.dataset.swipeDragging &&
    pageChanged &&
    sameAsPreviousRead;
  const locatorChanged = current.locator !== before.locator;
  globalThis.__athaFpsProbe = Object.freeze({
    ...before,
    lastLocator: current.locator,
    lastSection: current.section,
    lastPage: current.page,
  });
  return {
    ok: true,
    settled,
    locatorChanged,
    section: current.section,
    sections: current.sections,
    page: current.page,
    pages: current.pages,
  };
}

async function evaluate(url, expression) {
  const socket = new WebSocket(url);
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("cdp-timeout")), 800);
    socket.addEventListener("open", () => {
      clearTimeout(timer);
      resolve();
    }, { once: true });
    socket.addEventListener("error", () => {
      clearTimeout(timer);
      reject(new Error("cdp-connect"));
    }, { once: true });
  });
  try {
    return await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("cdp-timeout")), 15000);
      socket.addEventListener("message", (event) => {
        const message = JSON.parse(event.data);
        if (message.id !== 1) return;
        clearTimeout(timer);
        if (message.error || message.result?.exceptionDetails) reject(new Error("cdp-evaluate"));
        else resolve(message.result?.result?.value);
      });
      socket.send(JSON.stringify({
        id: 1,
        method: "Runtime.evaluate",
        params: { expression, returnByValue: true },
      }));
    });
  } finally {
    socket.close();
  }
}

try {
  const response = await fetch(`http://127.0.0.1:${port}/json/list`, {
    signal: AbortSignal.timeout(800),
  });
  if (!response.ok) throw new Error("cdp-targets");
  const targets = (await response.json()).filter(
    (target) => target.type === "page" && typeof target.webSocketDebuggerUrl === "string",
  );
  if (targets.length !== 1) throw new Error("cdp-target-ambiguous");
  const expression = `(${browserProbe.toString()})(${JSON.stringify(operation)}, ${JSON.stringify(direction)})`;
  const result = await evaluate(targets[0].webSocketDebuggerUrl, expression);
  if (!result?.ok) throw new Error(result?.reason || "reader-probe-unavailable");
  if (operation === "prepare") {
    console.log([
      result.section,
      result.sections,
      result.page,
      result.pages,
      result.startX,
      result.endX,
      result.y,
      result.screenX,
      result.screenY,
      result.screenWidth,
      result.screenHeight,
      result.scale,
    ].join("\t"));
  } else {
    console.log([
      result.settled,
      result.locatorChanged,
      result.section,
      result.sections,
      result.page,
      result.pages,
    ].join("\t"));
  }
} catch (error) {
  console.error(`reader-probe=${error instanceof Error ? error.message : "unavailable"}`);
  process.exitCode = 1;
}
NODE
}

before_section=
before_sections=
before_page=
before_pages=
swipe_start_x=
swipe_end_x=
swipe_y=
if [[ "$swipe" != none ]]; then
  if [[ ! "$cdp_port" =~ ^[0-9]+$ ]] || ((cdp_port <= 0 || cdp_port > 65535)); then
    die 'Automatic swipe requires --cdp-port for the Atha WebView; no swipe was sent.'
  fi
  cdp_pid=$($adb -s "$device" shell pidof "$package" | tr -d '\r' | awk '{ print $1 }')
  cdp_remote=$(
    $adb forward --list |
      awk -v serial="$device" -v local="tcp:$cdp_port" '$1 == serial && $2 == local { print $3 }'
  )
  [[ "$cdp_pid" =~ ^[0-9]+$ && "$cdp_remote" =~ ^localabstract:(huawei_)?webview_devtools_remote_$cdp_pid$ ]] ||
    die 'The CDP port is not forwarded to the current Atha process; no swipe was sent.'
  direction=$([[ "$swipe" == forward ]] && printf 1 || printf -- -1)
  if ! before=$(
    cdp_reader_probe prepare "$swipe"
  ); then
    die 'Automatic swipe requires a stable paged reader and diagnostics state; no swipe was sent.'
  fi
  IFS=$'\t' read -r before_section before_sections before_page before_pages \
    css_start_x css_end_x css_y screen_x screen_y screen_width screen_height screen_scale <<<"$before"
  for value in "$before_section" "$before_sections" "$before_page" "$before_pages"; do
    [[ "$value" =~ ^[0-9]+$ ]] || die 'Invalid reader page state; no swipe was sent.'
  done
  transition_start "$transition" "$direction" "$before_section" "$before_sections" \
    "$before_page" "$before_pages" ||
    die 'The current page does not match the requested transition; no swipe was sent.'

  physical_size=$($adb -s "$device" shell wm size | tr -d '\r' | sed -nE 's/.*: ([0-9]+)x([0-9]+)$/\1 \2/p' | tail -n 1)
  read -r physical_width physical_height <<<"$physical_size"
  [[ "$physical_width" =~ ^[0-9]+$ && "$physical_height" =~ ^[0-9]+$ ]] ||
    die 'The device viewport is unavailable; no swipe was sent.'
  read -r swipe_start_x swipe_end_x swipe_y projected_width projected_height < <(
    awk -v start="$css_start_x" -v finish="$css_end_x" -v y="$css_y" \
      -v origin_x="$screen_x" -v origin_y="$screen_y" -v width="$screen_width" \
      -v height="$screen_height" -v scale="$screen_scale" \
      'BEGIN {
        printf "%d %d %d %d %d\n",
          (origin_x + start) * scale + 0.5,
          (origin_x + finish) * scale + 0.5,
          (origin_y + y) * scale + 0.5,
          width * scale + 0.5,
          height * scale + 0.5
      }'
  )
  if ((
    projected_width < physical_width * 95 / 100 ||
      projected_width > physical_width * 105 / 100 ||
      projected_height < physical_height * 80 / 100 ||
      projected_height > physical_height * 105 / 100 ||
      swipe_start_x <= 0 || swipe_start_x >= physical_width ||
      swipe_end_x <= 0 || swipe_end_x >= physical_width ||
      swipe_y <= 0 || swipe_y >= physical_height
  )); then
    die 'The WebView and device viewports do not match; no swipe was sent.'
  fi
fi

evidence_dir="$repo_root/artifacts/local/audits/atha-reader-gesture-performance/fps-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$evidence_dir"
git -C "$repo_root" check-ignore -q "$evidence_dir/present-timestamps-ns.txt" ||
  die 'The FPS evidence directory is not ignored.'
: >"$evidence_dir/present-timestamps-ns.txt"
printf '%s\n' "$layer" >"$evidence_dir/layer.txt"
package_dump=$($adb -s "$device" shell dumpsys package "$package" | tr -d '\r')
installed_apk_path=$($adb -s "$device" shell pm path "$package" | tr -d '\r' | sed -n 's/^package://p' | head -n 1)
[[ -n "$installed_apk_path" ]] || die 'The installed APK path is unavailable.'
installed_apk_sha256=$($adb -s "$device" shell sha256sum "$installed_apk_path" | awk '{ print $1 }')
[[ "$installed_apk_sha256" =~ ^[a-f0-9]{64}$ ]] || die 'The installed APK digest is unavailable.'
{
  printf 'device_model=PCT-AL10\n'
  printf 'device_sdk=%s\n' "$($adb -s "$device" shell getprop ro.build.version.sdk | tr -d '\r')"
  printf 'device_abi=%s\n' "$($adb -s "$device" shell getprop ro.product.cpu.abi | tr -d '\r')"
  printf 'device_size=%s\n' "$($adb -s "$device" shell wm size | tr -d '\r' | tail -n 1)"
  printf 'device_density=%s\n' "$($adb -s "$device" shell wm density | tr -d '\r' | tail -n 1)"
  printf 'package=%s\n' "$package"
  printf 'installed_apk_sha256=%s\n' "$installed_apk_sha256"
  sed -nE 's/^[[:space:]]*(versionCode=[0-9]+).*/\1/p; s/^[[:space:]]*(versionName=[^[:space:]]+).*/\1/p' \
    <<<"$package_dump" | head -n 2
  $adb -s "$device" shell dumpsys webviewupdate | tr -d '\r' |
    sed -nE 's/^[[:space:]]*Current WebView package \(name, version\): \(([^,]+), ([^)]+)\)$/webview_package=\1\nwebview_version=\2/p' |
    head -n 2
  $adb -s "$device" shell dumpsys battery | tr -d '\r' |
    sed -nE 's/^[[:space:]]*temperature: ([0-9]+)$/battery_temperature_tenths_c=\1/p' |
    head -n 1
  printf 'mode=%s\n' "$([[ "$swipe" == none ]] && printf monitor || printf probe)"
  printf 'duration_seconds=%s\n' "$duration"
  printf 'swipe=%s\n' "$swipe"
  printf 'transition_expected=%s\n' "$transition"
  if [[ "$swipe" != none ]]; then
    printf 'before_state=%s:%s/%s\n' "$before_section" "$before_page" "$before_pages"
  fi
} >"$evidence_dir/metadata.txt"
exec > >(tee "$evidence_dir/summary.txt") 2>&1

$adb -s "$device" shell dumpsys SurfaceFlinger --latency-clear "$layer" >/dev/null
if [[ "$swipe" != none ]]; then
  $adb -s "$device" shell dumpsys gfxinfo "$package" reset >/dev/null
fi
initial_latency=$($adb -s "$device" shell dumpsys SurfaceFlinger --latency "$layer" | tr -d '\r')
refresh_ns=$(head -n 1 <<<"$initial_latency")
if [[ ! "$refresh_ns" =~ ^[0-9]+$ ]] || ((refresh_ns <= 0)); then
  die 'Invalid refresh period.'
fi

deadline_ns=$(( $(date +%s%N) + duration * 1000000000 ))
last_present=$(surface_present_timestamps 0 <<<"$initial_latency" | tail -n 1)
last_present=${last_present:-0}
sample=0
no_presentation_samples=0
ring_full_samples=0
gesture_pid=
if [[ "$swipe" != none ]]; then
  (
    sleep 0.1
    $adb -s "$device" shell input swipe "$swipe_start_x" "$swipe_y" "$swipe_end_x" "$swipe_y" 160 >/dev/null
  ) &
  gesture_pid=$!
fi
sample_sleep=0.5
if [[ "$swipe" != none ]]; then sample_sleep=$duration; fi
while (( $(date +%s%N) < deadline_ns )); do
  sleep "$sample_sleep"
  latency=$($adb -s "$device" shell dumpsys SurfaceFlinger --latency "$layer" | tr -d '\r')
  printf '%s\n' "$latency" >"$evidence_dir/surfaceflinger-latency.txt"
  valid_rows=$(tail -n +2 <<<"$latency" | awk 'NF == 3 && $2 > 0 && $2 != "9223372036854775807" { count++ } END { print count + 0 }')
  if ((valid_rows >= 127)); then
    ring_full_samples=$((ring_full_samples + 1))
  fi
  mapfile -t presented < <(
    surface_present_timestamps "$last_present" <<<"$latency"
  )
  sample=$((sample + 1))
  count=${#presented[@]}
  if ((count == 0)); then
    no_presentation_samples=$((no_presentation_samples + 1))
    printf 'sample=%d presented_frames=0 presented_state=no-new-buffer\n' "$sample"
    continue
  fi
  printf '%s\n' "${presented[@]}" >>"$evidence_dir/present-timestamps-ns.txt"
  last_present=${presented[$((count - 1))]}
  if ((count > 1)); then
    cadence=$(awk -v first="${presented[0]}" -v last="$last_present" -v count="$count" \
      'BEGIN { printf "%.1f", (count - 1) * 1000000000 / (last - first) }')
    printf 'sample=%d presented_frames=%d presentation_update_cadence_fps=%s\n' \
      "$sample" "$count" "$cadence"
  else
    printf 'sample=%d presented_frames=1 presentation_update_cadence_fps=unavailable\n' "$sample"
  fi
done
if [[ -n "$gesture_pid" ]]; then
  wait "$gesture_pid"
fi
if [[ "$swipe" != none ]]; then
  $adb -s "$device" shell dumpsys gfxinfo "$package" framestats | tr -d '\r' >"$evidence_dir/gfxinfo.txt"
fi

if [[ "$swipe" != none ]]; then
  probe_deadline_ns=$(( $(date +%s%N) + 30000000000 ))
  settled=false
  locator_changed=false
  while (( $(date +%s%N) < probe_deadline_ns )); do
    if after=$(cdp_reader_probe read "$swipe" 2>/dev/null); then
      IFS=$'\t' read -r settled locator_changed after_section after_sections after_page after_pages <<<"$after"
      if [[ "$settled" == true && "$locator_changed" == true ]]; then break; fi
    fi
    sleep 0.1
  done
  [[ "$settled" == true && "$locator_changed" == true ]] ||
    die "Automatic swipe did not settle at a distinct locator; raw evidence remains in ${evidence_dir#"$repo_root/"}."
  [[ "$after_sections" == "$before_sections" ]] ||
    die "Automatic swipe changed the book state; raw evidence remains in ${evidence_dir#"$repo_root/"}."
  single_page_step "$direction" "$before_section" "$before_page" "$before_pages" \
    "$after_section" "$after_page" "$after_pages" ||
    die "Automatic swipe was not exactly one page; raw evidence remains in ${evidence_dir#"$repo_root/"}."
  if [[ "$after_section" == "$before_section" ]]; then
    actual_transition=same-section
    [[ "$after_pages" == "$before_pages" ]] ||
      die "Same-section swipe changed the page count; raw evidence remains in ${evidence_dir#"$repo_root/"}."
  else
    actual_transition=cross-section
  fi
  [[ "$transition" == any || "$actual_transition" == "$transition" ]] ||
    die "Automatic swipe used an unexpected transition; raw evidence remains in ${evidence_dir#"$repo_root/"}."
  {
    printf 'transition_actual=%s\n' "$actual_transition"
    printf 'after_state=%s:%s/%s\n' "$after_section" "$after_page" "$after_pages"
  } >>"$evidence_dir/metadata.txt"
fi

sort -nu "$evidence_dir/present-timestamps-ns.txt" -o "$evidence_dir/present-timestamps-ns.txt"
frames=$(wc -l <"$evidence_dir/present-timestamps-ns.txt")

if [[ "$swipe" == none ]]; then
  aggregation=raw-only
  coverage=raw-only
  presentation_update_cadence_fps=unavailable
  presentation_span_ms=unavailable
  p95=unavailable
  maximum=unavailable
  slow_frames=unavailable
else
  aggregation=single-swipe
  coverage=complete
  awk 'NR > 1 { print ($1 - previous) / 1000000 } { previous = $1 }' \
    "$evidence_dir/present-timestamps-ns.txt" >"$evidence_dir/frame-intervals-ms.txt"
  intervals=$(wc -l <"$evidence_dir/frame-intervals-ms.txt")
  if ((intervals > 0)); then
    sort -n "$evidence_dir/frame-intervals-ms.txt" >"$evidence_dir/frame-intervals-sorted-ms.txt"
    p95_index=$(((intervals * 95 + 99) / 100))
    p95=$(sed -n "${p95_index}p" "$evidence_dir/frame-intervals-sorted-ms.txt")
    maximum=$(tail -n 1 "$evidence_dir/frame-intervals-sorted-ms.txt")
    presentation_update_cadence_fps=$(awk 'NR == 1 { first = $1 } { last = $1 } END { printf "%.1f", (NR - 1) * 1000000000 / (last - first) }' \
      "$evidence_dir/present-timestamps-ns.txt")
    presentation_span_ms=$(awk 'NR == 1 { first = $1 } { last = $1 } END { printf "%.1f", (last - first) / 1000000 }' \
      "$evidence_dir/present-timestamps-ns.txt")
    slow_threshold_ms=$(awk -v refresh="$refresh_ns" 'BEGIN { print refresh * 1.5 / 1000000 }')
    slow_frames=$(awk -v threshold="$slow_threshold_ms" '$1 > threshold { count++ } END { print count + 0 }' \
      "$evidence_dir/frame-intervals-ms.txt")
  else
    coverage=missing
    presentation_update_cadence_fps=unavailable
    presentation_span_ms=unavailable
    p95=unavailable
    maximum=unavailable
    slow_frames=unavailable
  fi
fi

app_frames=unavailable
app_frame_mean_ms=unavailable
app_frame_p95_ms=unavailable
app_frame_max_ms=unavailable
app_slow_frames=unavailable
if [[ "$swipe" != none ]]; then
  awk -F, -v threshold="$(awk -v refresh="$refresh_ns" 'BEGIN { print refresh * 1.5 / 1000000 }')" '
    /^0,/ && $2 > 0 && $14 > $2 && $14 != "9223372036854775807" {
      duration = ($14 - $2) / 1000000
      print duration
    }
  ' "$evidence_dir/gfxinfo.txt" >"$evidence_dir/app-frame-durations-ms.txt"
  app_frames=$(wc -l <"$evidence_dir/app-frame-durations-ms.txt")
  if ((app_frames > 0)); then
    sort -n "$evidence_dir/app-frame-durations-ms.txt" >"$evidence_dir/app-frame-durations-sorted-ms.txt"
    app_p95_index=$(((app_frames * 95 + 99) / 100))
    app_frame_mean_ms=$(awk '{ total += $1 } END { printf "%.1f", total / NR }' \
      "$evidence_dir/app-frame-durations-ms.txt")
    app_frame_p95_ms=$(sed -n "${app_p95_index}p" "$evidence_dir/app-frame-durations-sorted-ms.txt")
    app_frame_max_ms=$(tail -n 1 "$evidence_dir/app-frame-durations-sorted-ms.txt")
    app_slow_frames=$(awk -v refresh="$refresh_ns" '$1 > refresh * 1.5 / 1000000 { count++ } END { print count + 0 }' \
      "$evidence_dir/app-frame-durations-ms.txt")
  else
    app_frames=unavailable
  fi
fi

printf 'refresh_hz=%.1f\n' "$(awk -v refresh="$refresh_ns" 'BEGIN { print 1000000000 / refresh }')"
printf 'presented_frames=%d\n' "$frames"
printf 'aggregation=%s\n' "$aggregation"
printf 'presentation_update_cadence_fps=%s\n' "$presentation_update_cadence_fps"
printf 'presentation_span_ms=%s\n' "$presentation_span_ms"
printf 'frame_interval_p95_ms=%s\n' "$p95"
printf 'frame_interval_max_ms=%s\n' "$maximum"
printf 'slow_frames=%s\n' "$slow_frames"
printf 'app_frames=%s\n' "$app_frames"
printf 'app_frame_mean_ms=%s\n' "$app_frame_mean_ms"
printf 'app_frame_p95_ms=%s\n' "$app_frame_p95_ms"
printf 'app_frame_max_ms=%s\n' "$app_frame_max_ms"
printf 'app_slow_frames=%s\n' "$app_slow_frames"
printf 'no_presentation_samples=%d\n' "$no_presentation_samples"
printf 'ring_full_samples=%d\n' "$ring_full_samples"
printf 'coverage=%s\n' "$coverage"
printf 'mode=%s\n' "$([[ "$swipe" == none ]] && printf monitor || printf probe)"
if [[ "$swipe" != none ]]; then
  printf 'probe_validation=single-page locator-changed settled\n'
  printf 'transition_expected=%s\n' "$transition"
  printf 'transition_actual=%s\n' "$actual_transition"
fi
printf 'evidence=%s\n' "${evidence_dir#"$repo_root/"}"
if [[ "$swipe" != none && "$coverage" != complete ]]; then
  die 'Automatic swipe produced fewer than two valid presentation timestamps.'
fi
if [[ "$swipe" != none && "$ring_full_samples" != 0 ]]; then
  die 'Automatic swipe filled the SurfaceFlinger ring; presentation coverage is ambiguous.'
fi
