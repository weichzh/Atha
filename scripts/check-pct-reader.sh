#!/usr/bin/env bash
# Description: Build, verify, and safely update the PCT-AL10 arm64 reader candidate.

set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
package=com.atha.reader
unsigned_apk="$repo_root/reader/app/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk"
default_apk="$repo_root/reader/app/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-local-test.apk"

adb_bin=
aligned_apk=
signed_apk=
remote_apk=
device=
session=
session_active=0
commit_pid=
evidence_dir=
failure_reason=unexpected_exit
confirmation_package=
installed_before_sha256=
installed_before_certificate=
installed_before_version=
installed_before_first_install_time=
candidate_sha256=
candidate_certificate=
candidate_version=

usage() {
  cat <<'EOF'
Usage:
  scripts/check-pct-reader.sh build
  scripts/check-pct-reader.sh verify [--apk PATH]
  scripts/check-pct-reader.sh install --device SERIAL [--apk PATH]

build creates an optimized arm64 APK, signs it with the configured local
Android test key, and verifies its package, signature, and 16 KiB alignment.

Environment for build:
  ATHA_ANDROID_KEYSTORE_PASSWORD  Required keystore password.
  ATHA_ANDROID_KEYSTORE           Defaults to $HOME/.android/debug.keystore.
  ATHA_ANDROID_KEY_ALIAS          Defaults to androiddebugkey.
  ATHA_ANDROID_KEY_PASSWORD       Defaults to the keystore password.
EOF
}

die() {
  printf '%s\n' "$*" >&2
  exit 1
}

cleanup() {
  local status=$?
  trap - EXIT INT TERM
  set +e

  stop_commit
  if ((session_active)) && [[ -n "$adb_bin" && -n "$device" && -n "$session" ]]; then
    local abandon_output abandon_status abandon_succeeded=false
    abandon_output=$("$adb_bin" -s "$device" shell cmd package install-abandon "$session" 2>&1)
    abandon_status=$?
    if ((abandon_status == 0)) && grep -qx 'Success' <<<"${abandon_output//$'\r'/}"; then
      abandon_succeeded=true
    fi

    local observed_session
    observed_session=$(wait_for_terminal_session)

    local fingerprint fingerprint_status path_count after_sha after_certificate after_version after_first_install
    fingerprint=$(installed_fingerprint "$evidence_dir/installed-observed.apk")
    fingerprint_status=$?
    IFS='|' read -r path_count after_sha after_certificate after_version after_first_install <<<"$fingerprint"

    local observed_package=unavailable
    if ((fingerprint_status == 0)); then
      observed_package=$(classify_installed_package \
        "$path_count" "$after_sha" "$after_certificate" "$after_version" "$after_first_install")
    fi

    local cancellation_verified=false completed_despite_error=false installed_state=unknown
    if [[ "$observed_package" == before ]] &&
      [[ "$observed_session" == absent || "$observed_session" == terminal_failure ]]; then
      cancellation_verified=true
    elif [[ "$observed_package" == before_and_candidate &&
      "$observed_session" == terminal_failure ]]; then
      cancellation_verified=true
    elif [[ "$observed_package" == candidate ]]; then
      completed_despite_error=true
    elif [[ "$observed_package" == before_and_candidate &&
      "$observed_session" == terminal_success ]]; then
      completed_despite_error=true
    fi
    if [[ "$cancellation_verified" == true ]]; then
      installed_state=false
    elif [[ "$completed_despite_error" == true ]]; then
      installed_state=true
    fi

    if [[ -n "$evidence_dir" && -d "$evidence_dir" ]]; then
      {
        printf 'installed=%s\n' "$installed_state"
        printf 'failure_reason=%s\n' "$failure_reason"
        printf 'abandon_command_succeeded=%s\n' "$abandon_succeeded"
        printf 'session_state=%s\n' "$observed_session"
        printf 'package_state=%s\n' "$observed_package"
        printf 'cancellation_verified=%s\n' "$cancellation_verified"
        printf 'completed_despite_error=%s\n' "$completed_despite_error"
        printf 'data_clear_requested=false\n'
      } >"$evidence_dir/result.txt"
    fi

    if [[ "$cancellation_verified" == true ]]; then
      printf 'PackageInstaller cancellation verified.\n' >&2
    elif [[ "$completed_despite_error" == true ]]; then
      printf 'PackageInstaller completed before cancellation; the candidate is installed.\n' >&2
    else
      printf 'PackageInstaller state is unresolved and may still change: session=%s package=%s.\n' \
        "$observed_session" "$observed_package" >&2
    fi
    ((status != 0)) || status=1
  fi
  if [[ -n "$adb_bin" && -n "$device" && -n "$remote_apk" ]]; then
    "$adb_bin" -s "$device" shell rm -f "$remote_apk" >/dev/null 2>&1 || true
  fi
  [[ -z "$aligned_apk" ]] || rm -f -- "$aligned_apk"
  [[ -z "$signed_apk" ]] || rm -f -- "$signed_apk" "${signed_apk}.idsig"
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

load_tools() {
  local sdk=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}
  [[ -n "$sdk" && -d "$sdk" ]] || die 'ANDROID_HOME or ANDROID_SDK_ROOT must name the Android SDK.'

  local build_tools="$sdk/build-tools/${ATHA_ANDROID_BUILD_TOOLS_VERSION:-35.0.0}"
  adb_bin="$sdk/platform-tools/adb"
  aapt2="$build_tools/aapt2"
  zipalign="$build_tools/zipalign"
  apksigner="$build_tools/apksigner"
  readelf="$sdk/ndk/28.2.13676358/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-readelf"

  local tool
  for tool in "$adb_bin" "$aapt2" "$zipalign" "$apksigner" "$readelf"; do
    [[ -x "$tool" ]] || die "Missing Android tool: $tool"
  done
  command -v unzip >/dev/null || die 'unzip is required.'
  command -v mise >/dev/null || die 'mise is required.'
}

certificate_digest() {
  "$apksigner" verify --print-certs "$1" |
    sed -n 's/^Signer #1 certificate SHA-256 digest: //p' |
    head -n 1
}

stop_commit() {
  [[ -n "$commit_pid" ]] || return 0
  if kill -0 "$commit_pid" 2>/dev/null; then
    kill "$commit_pid" 2>/dev/null || true
    local attempts=20
    while ((attempts-- > 0)); do
      kill -0 "$commit_pid" 2>/dev/null || break
      sleep 0.1
    done
    kill -KILL "$commit_pid" 2>/dev/null || true
  fi
  wait "$commit_pid" 2>/dev/null || true
  commit_pid=
}

package_session_state() {
  local dump block compact final_status
  if ! dump=$("$adb_bin" -s "$device" shell dumpsys package 2>/dev/null); then
    printf 'unavailable\n'
    return
  fi
  block=$(tr -d '\r' <<<"$dump" | awk -v id="$session" '
    $0 ~ "^[[:space:]]*Session " id ":$" { found = 1 }
    found { print }
    found && /^[[:space:]]*$/ { exit }
  ')
  if [[ -z "$block" ]]; then
    printf 'absent\n'
    return
  fi

  compact=$(tr -d '[:space:]' <<<"$block")
  if [[ "$compact" == *mDestroyed=true* ]]; then
    if [[ "$compact" =~ mFinalStatus=(-?[0-9]+) ]]; then
      final_status=${BASH_REMATCH[1]}
      if [[ "$final_status" == 1 ]]; then
        printf 'terminal_success\n'
      else
        printf 'terminal_failure\n'
      fi
    else
      printf 'unavailable\n'
    fi
  elif [[ "$compact" == *mCommitted=true* ]]; then
    printf 'pending\n'
  else
    printf 'open\n'
  fi
}

wait_for_terminal_session() {
  local deadline=$((SECONDS + 10)) state
  while :; do
    state=$(package_session_state)
    case "$state" in
      absent | terminal_failure | terminal_success)
        printf '%s\n' "$state"
        return 0
        ;;
    esac
    if ((SECONDS >= deadline)); then
      printf '%s\n' "$state"
      return 1
    fi
    sleep 0.5
  done
}

installed_fingerprint() {
  local destination=$1 paths_output dump
  if ! paths_output=$("$adb_bin" -s "$device" shell pm path "$package" 2>/dev/null); then
    printf 'unknown||||\n'
    return 1
  fi

  local paths=()
  mapfile -t paths < <(sed -n 's/^package://p' <<<"${paths_output//$'\r'/}")
  if ((${#paths[@]} != 1)); then
    printf '%s||||\n' "${#paths[@]}"
    return 1
  fi
  if ! "$adb_bin" -s "$device" pull "${paths[0]}" "$destination" >/dev/null 2>&1; then
    printf '1||||\n'
    return 1
  fi
  if ! dump=$("$adb_bin" -s "$device" shell dumpsys package "$package" 2>/dev/null); then
    printf '1||||\n'
    return 1
  fi

  local sha certificate version first_install
  sha=$(sha256sum "$destination" | cut -d' ' -f1)
  certificate=$(certificate_digest "$destination")
  version=$(sed -n 's/.*versionCode=\([0-9][0-9]*\).*/\1/p' <<<"$dump" | head -n 1)
  first_install=$(sed -n 's/^[[:space:]]*firstInstallTime=//p' <<<"$dump" | head -n 1 | tr -d '\r')
  if [[ -z "$sha" || -z "$certificate" || -z "$version" || -z "$first_install" ]]; then
    printf '1||||\n'
    return 1
  fi
  printf '1|%s|%s|%s|%s\n' "$sha" "$certificate" "$version" "$first_install"
}

classify_installed_package() {
  local path_count=$1 sha=$2 certificate=$3 version=$4 first_install=$5
  [[ "$path_count" == 1 ]] || {
    printf 'unexpected_path_count\n'
    return
  }

  local matches_before=false matches_candidate=false
  if [[ "$sha" == "$installed_before_sha256" &&
    "$certificate" == "$installed_before_certificate" &&
    "$version" == "$installed_before_version" &&
    "$first_install" == "$installed_before_first_install_time" ]]; then
    matches_before=true
  fi
  if [[ "$sha" == "$candidate_sha256" &&
    "$certificate" == "$candidate_certificate" &&
    "$version" == "$candidate_version" &&
    "$first_install" == "$installed_before_first_install_time" ]]; then
    matches_candidate=true
  fi

  if [[ "$matches_before" == true && "$matches_candidate" == true ]]; then
    printf 'before_and_candidate\n'
  elif [[ "$matches_before" == true ]]; then
    printf 'before\n'
  elif [[ "$matches_candidate" == true ]]; then
    printf 'candidate\n'
  else
    printf 'unexpected\n'
  fi
}

verify_apk() {
  local apk=$1
  [[ -f "$apk" ]] || die "APK not found: $apk"

  local badging launch_activity signatures
  badging=$($aapt2 dump badging "$apk")
  grep -q "^package: name='$package'" <<<"$badging" || die 'Unexpected Android package id.'
  launch_activity=$(sed -n "s/^launchable-activity: name='\([^']*\)'.*/\1/p" <<<"$badging" | head -n 1)
  [[ "$launch_activity" == "$package."* ]] || die 'Unexpected Android launch activity.'
  grep -qx "minSdkVersion:'26'" <<<"$badging" || die 'Unexpected Android minSdkVersion.'
  grep -qx "targetSdkVersion:'36'" <<<"$badging" || die 'Unexpected Android targetSdkVersion.'
  grep -qx "native-code: 'arm64-v8a'" <<<"$badging" || die 'APK is not arm64-v8a-only.'

  local permission
  for permission in READ_EXTERNAL_STORAGE WRITE_EXTERNAL_STORAGE MANAGE_EXTERNAL_STORAGE \
    MANAGE_MEDIA ACCESS_MEDIA_LOCATION READ_MEDIA_AUDIO READ_MEDIA_IMAGES READ_MEDIA_VIDEO; do
    if grep -q "android.permission.$permission" <<<"$badging"; then
      die "APK requests broad storage permission android.permission.$permission."
    fi
  done

  "$zipalign" -c -P 16 4 "$apk" >/dev/null || die 'APK is not 16 KiB ZIP aligned.'
  signatures=$($apksigner verify --verbose --print-certs "$apk")
  grep -qx 'Verified using v2 scheme (APK Signature Scheme v2): true' <<<"$signatures" ||
    die 'APK v2 signature verification failed.'
  grep -qx 'Verified using v3 scheme (APK Signature Scheme v3): true' <<<"$signatures" ||
    die 'APK v3 signature verification failed.'

  local libraries=()
  mapfile -t libraries < <(unzip -Z1 "$apk" | awk '/^lib\/arm64-v8a\/[^/]+\.so$/')
  ((${#libraries[@]} > 0)) || die 'APK contains no arm64-v8a shared libraries.'
  local library
  for library in "${libraries[@]}"; do
    if ! unzip -p "$apk" "$library" |
      "$readelf" -lW - |
      awk '
        /^[[:space:]]*LOAD[[:space:]]/ { seen = 1; if ($NF != "0x4000") bad = 1 }
        END { exit (!seen || bad) ? 1 : 0 }
      '; then
      die "ELF LOAD alignment is not 0x4000: $library"
    fi
  done

  printf 'verified=true\n'
  printf 'apk_sha256=%s\n' "$(sha256sum "$apk" | cut -d' ' -f1)"
  printf 'certificate_sha256=%s\n' "$(certificate_digest "$apk")"
  printf 'zipaligned_16k=true\nelf_aligned_16k=true\n'
}

build_apk() {
  local keystore=${ATHA_ANDROID_KEYSTORE:-$HOME/.android/debug.keystore}
  local alias=${ATHA_ANDROID_KEY_ALIAS:-androiddebugkey}
  local keystore_password=${ATHA_ANDROID_KEYSTORE_PASSWORD:-}
  local key_password=${ATHA_ANDROID_KEY_PASSWORD:-$keystore_password}
  [[ -f "$keystore" ]] || die "Android keystore not found: $keystore"
  [[ -n "$keystore_password" ]] || die 'ATHA_ANDROID_KEYSTORE_PASSWORD is required.'

  (
    cd "$repo_root"
    mise exec -- pnpm --dir reader/app tauri android build --target aarch64 --apk --ci
  )
  [[ -f "$unsigned_apk" ]] || die "Unsigned release APK not found: $unsigned_apk"

  mkdir -p "$repo_root/.tmp"
  aligned_apk="$repo_root/.tmp/pct-reader-aligned-$$.apk"
  signed_apk="$repo_root/.tmp/pct-reader-signed-$$.apk"
  "$zipalign" -P 16 -f 4 "$unsigned_apk" "$aligned_apk"

  export ATHA_ANDROID_SIGNING_KEYSTORE_PASSWORD=$keystore_password
  export ATHA_ANDROID_SIGNING_KEY_PASSWORD=$key_password
  "$apksigner" sign \
    --ks "$keystore" \
    --ks-key-alias "$alias" \
    --ks-pass env:ATHA_ANDROID_SIGNING_KEYSTORE_PASSWORD \
    --key-pass env:ATHA_ANDROID_SIGNING_KEY_PASSWORD \
    --v4-signing-enabled false \
    --out "$signed_apk" \
    "$aligned_apk"

  verify_apk "$signed_apk"
  rm -f -- "${default_apk}.idsig"
  mv -f -- "$signed_apk" "$default_apk"
  signed_apk=
  printf 'apk=%s\n' "$default_apk"
}

current_component() {
  local dump component
  if dump=$("$adb_bin" -s "$device" shell dumpsys window 2>/dev/null); then
    component=$(sed -n 's/.*mCurrentFocus=.*u0 \([^ }]*\).*/\1/p' <<<"$dump" | head -n 1 | tr -d '\r')
    if [[ -n "$component" ]]; then
      printf '%s\n' "$component"
      return 0
    fi
  fi
  if dump=$("$adb_bin" -s "$device" shell dumpsys activity activities 2>/dev/null); then
    component=$(sed -n 's/.*mResumedActivity:.*u0 \([^ ]*\).*/\1/p' <<<"$dump" | head -n 1 | tr -d '\r')
    if [[ -n "$component" ]]; then
      printf '%s\n' "$component"
      return 0
    fi
  fi
  return 1
}

confirmation_resolver_package() {
  "$adb_bin" -s "$device" shell cmd package resolve-activity --brief \
    -a android.content.pm.action.CONFIRM_INSTALL 2>/dev/null |
    tr -d '\r' |
    sed -n 's#^\([A-Za-z0-9._]*\)/.*#\1#p' |
    tail -n 1
}

is_confirmation_component() {
  local component_package=${1%%/*}
  case "$component_package" in
    "$confirmation_package" | com.android.packageinstaller | com.android.permissioncontroller | \
      com.android.settings | com.android.systemui | com.huawei.appmarket | com.huawei.systemmanager)
      return 0
      ;;
    *) return 1 ;;
  esac
}

install_apk() {
  local apk=$1
  [[ -n "$device" ]] || die 'install requires --device SERIAL.'
  [[ "$device" =~ ^[A-Za-z0-9._:-]+$ ]] || die 'Invalid Android device serial.'

  local device_line
  device_line=$($adb_bin devices -l | awk -v serial="$device" '$1 == serial { print; exit }')
  [[ -n "$device_line" ]] || die 'The requested Android device is not connected.'
  [[ $(awk '{ print $2 }' <<<"$device_line") == device ]] || die 'The requested Android device is not authorized.'
  [[ $("$adb_bin" -s "$device" shell getprop ro.product.model | tr -d '\r') == PCT-AL10 ]] ||
    die 'The requested device is not PCT-AL10.'
  [[ $("$adb_bin" -s "$device" shell getprop ro.product.cpu.abi | tr -d '\r') == arm64-v8a ]] ||
    die 'The requested device is not arm64-v8a.'
  [[ $("$adb_bin" -s "$device" shell getprop ro.kernel.qemu | tr -d '\r') != 1 ]] ||
    die 'The requested device is an emulator.'

  verify_apk "$apk"

  local installed_paths=()
  mapfile -t installed_paths < <(
    "$adb_bin" -s "$device" shell pm path "$package" |
      sed -n 's/^package://p' |
      tr -d '\r'
  )
  ((${#installed_paths[@]} == 1)) ||
    die 'Safe update requires one already-installed, non-split Atha package.'

  local evidence_rel
  evidence_rel="artifacts/local/audits/pct-reader-install-$(date -u +%Y%m%dT%H%M%SZ)-$$"
  evidence_dir="$repo_root/$evidence_rel"
  mkdir -p "$evidence_dir"
  if ! git -C "$repo_root" check-ignore -q "$evidence_rel"; then
    rmdir "$evidence_dir"
    die 'PCT install evidence directory is not ignored by Git.'
  fi

  local installed_apk="$evidence_dir/installed-before.apk"
  "$adb_bin" -s "$device" pull "${installed_paths[0]}" "$installed_apk" >/dev/null
  installed_before_certificate=$(certificate_digest "$installed_apk")
  candidate_certificate=$(certificate_digest "$apk")
  [[ -n "$installed_before_certificate" && "$candidate_certificate" == "$installed_before_certificate" ]] ||
    die 'Candidate and installed package signatures differ.'

  local installed_dump
  installed_dump=$("$adb_bin" -s "$device" shell dumpsys package "$package")
  installed_before_version=$(sed -n 's/.*versionCode=\([0-9][0-9]*\).*/\1/p' <<<"$installed_dump" | head -n 1)
  installed_before_first_install_time=$(
    sed -n 's/^[[:space:]]*firstInstallTime=//p' <<<"$installed_dump" | head -n 1 | tr -d '\r'
  )
  candidate_version=$($aapt2 dump badging "$apk" | sed -n "s/^package:.*versionCode='\([0-9][0-9]*\)'.*/\1/p")
  installed_before_sha256=$(sha256sum "$installed_apk" | cut -d' ' -f1)
  candidate_sha256=$(sha256sum "$apk" | cut -d' ' -f1)
  [[ -n "$installed_before_version" && -n "$candidate_version" &&
    -n "$installed_before_first_install_time" ]] || die 'Could not read installed package state.'
  ((candidate_version >= installed_before_version)) || die 'Refusing to downgrade the installed package.'

  {
    printf 'package=%s\n' "$package"
    printf 'model=PCT-AL10\n'
    printf 'android_api=%s\n' "$("$adb_bin" -s "$device" shell getprop ro.build.version.sdk | tr -d '\r')"
    printf 'installed_version_code=%s\n' "$installed_before_version"
    printf 'installed_certificate_sha256=%s\n' "$installed_before_certificate"
    printf 'installed_apk_sha256=%s\n' "$installed_before_sha256"
    printf 'installed_first_install_time=%s\n' "$installed_before_first_install_time"
    printf 'candidate_version_code=%s\n' "$candidate_version"
    printf 'candidate_sha256=%s\n' "$candidate_sha256"
  } >"$evidence_dir/before.txt"

  confirmation_package=$(confirmation_resolver_package)
  local baseline_component
  baseline_component=$(current_component) || die 'Could not determine the foreground Android component.'
  if is_confirmation_component "$baseline_component"; then
    die 'Close the Android installer or security screen before starting the update.'
  fi

  remote_apk="/data/local/tmp/atha-reader-candidate-$$.apk"
  "$adb_bin" -s "$device" push "$apk" "$remote_apk" >/dev/null
  local size created
  size=$(stat -c %s "$apk")
  created=$("$adb_bin" -s "$device" shell cmd package install-create \
    --user 0 -r -i com.android.shell --install-reason 4 --full --pkg "$package" -S "$size")
  session=$(sed -n 's/.*\[\([0-9][0-9]*\)\].*/\1/p' <<<"$created")
  [[ -n "$session" ]] || die "PackageInstaller did not create a session: $created"
  session_active=1

  if ! "$adb_bin" -s "$device" shell cmd package install-write "$session" base.apk "$remote_apk" |
    grep -q '^Success:'; then
    failure_reason=install_write_failed
    die 'PackageInstaller did not accept the APK.'
  fi

  local commit_output="$evidence_dir/commit.txt"
  local visible pending=0 timed_out=0 ui_probe_failed=0
  : >"$commit_output"
  "$adb_bin" -s "$device" shell cmd package install-commit "$session" >"$commit_output" 2>&1 &
  commit_pid=$!
  local deadline=$((SECONDS + 45))
  while kill -0 "$commit_pid" 2>/dev/null; do
    if ! visible=$(current_component); then
      ui_probe_failed=1
      break
    fi
    if is_confirmation_component "$visible"; then
      pending=1
      break
    fi
    if ((SECONDS >= deadline)); then
      timed_out=1
      break
    fi
    sleep 0.25
  done

  if ((pending || timed_out || ui_probe_failed)); then
    local failure_message
    if ((pending)); then
      failure_reason=user_action_detected
      failure_message='Huawei installer confirmation appeared; stopping and verifying session and package state.'
    elif ((timed_out)); then
      failure_reason=commit_timeout
      failure_message='PackageInstaller commit timed out; stopping and verifying session and package state.'
    else
      failure_reason=ui_probe_failed
      failure_message='Android foreground-state probing failed; stopping and verifying session and package state.'
    fi
    stop_commit
    die "$failure_message"
  fi

  set +e
  wait "$commit_pid"
  local commit_status=$?
  set -e
  commit_pid=
  if visible=$(current_component) && is_confirmation_component "$visible"; then
    failure_reason=user_action_detected
    die 'Huawei installer confirmation appeared; verifying session and package state.'
  fi
  if ((commit_status != 0)) || ! grep -qx 'Success' "$commit_output"; then
    failure_reason=commit_failed
    die 'PackageInstaller commit failed; inspect the local content-free commit evidence.'
  fi

  local installed_after_path="$evidence_dir/installed-after.apk"
  local after_fingerprint after_path_count after_sha after_certificate after_version after_first_install
  if ! after_fingerprint=$(installed_fingerprint "$installed_after_path"); then
    failure_reason=post_install_probe_failed
    die 'Could not read the installed package after commit.'
  fi
  IFS='|' read -r after_path_count after_sha after_certificate after_version after_first_install \
    <<<"$after_fingerprint"
  [[ "$after_path_count" == 1 ]] || {
    failure_reason=post_install_path_mismatch
    die 'Installed package is not a single base APK after commit.'
  }
  [[ "$after_sha" == "$candidate_sha256" ]] || {
    failure_reason=post_install_hash_mismatch
    die 'Installed APK does not match the verified candidate.'
  }
  [[ "$after_certificate" == "$candidate_certificate" ]] || {
    failure_reason=post_install_signature_mismatch
    die 'Installed package signature changed after commit.'
  }
  [[ "$after_version" == "$candidate_version" ]] || {
    failure_reason=post_install_version_mismatch
    die 'Installed package version does not match the candidate.'
  }
  [[ "$after_first_install" == "$installed_before_first_install_time" ]] || {
    failure_reason=post_install_first_install_time_changed
    die 'Package first-install time changed during the update.'
  }

  local committed_session_state
  if ! committed_session_state=$(wait_for_terminal_session); then
    failure_reason=post_install_session_unresolved
    die 'PackageInstaller session did not reach a terminal state after commit.'
  fi
  [[ "$committed_session_state" == terminal_success || "$committed_session_state" == absent ]] || {
    failure_reason=post_install_session_unresolved
    die 'PackageInstaller session is not terminal after commit.'
  }

  local launch_activity launch_output pid focused focused_package focused_activity
  launch_activity=$(
    $aapt2 dump badging "$apk" |
      sed -n "s/^launchable-activity: name='\([^']*\)'.*/\1/p" |
      head -n 1
  )
  "$adb_bin" -s "$device" shell am force-stop "$package" >/dev/null
  launch_output="$evidence_dir/launch.txt"
  if ! "$adb_bin" -s "$device" shell am start -W -n "$package/$launch_activity" \
    >"$launch_output" 2>&1; then
    failure_reason=post_install_launch_failed
    die 'Installed candidate did not start.'
  fi
  grep -q '^Status: ok' "$launch_output" || {
    failure_reason=post_install_launch_failed
    die 'Installed candidate did not report a successful start.'
  }
  sleep 3
  pid=$("$adb_bin" -s "$device" shell pidof "$package" | tr -d '\r' | cut -d' ' -f1)
  [[ "$pid" =~ ^[0-9]+$ ]] || {
    failure_reason=post_install_process_missing
    die 'Installed candidate did not remain running.'
  }
  focused=$(current_component) || {
    failure_reason=post_install_activity_missing
    die 'Installed candidate has no focused activity.'
  }
  focused_package=${focused%%/*}
  focused_activity=${focused#*/}
  [[ "$focused_package" == "$package" ]] &&
    [[ "$focused_activity" == "$launch_activity" || "$package$focused_activity" == "$launch_activity" ]] || {
    failure_reason=post_install_activity_mismatch
    die 'Installed candidate did not become the focused activity.'
  }

  {
    printf 'installed=true\n'
    printf 'session_state=%s\n' "$committed_session_state"
    printf 'single_base_apk=true\n'
    printf 'apk_sha256_matches_candidate=true\n'
    printf 'version_code_matches_candidate=true\n'
    printf 'certificate_matches_candidate=true\n'
    printf 'first_install_time_preserved=true\n'
    printf 'data_clear_requested=false\n'
    printf 'process_running=true\n'
    printf 'main_activity_focused=true\n'
  } >"$evidence_dir/result.txt"
  session_active=0
  printf 'installed=true\nevidence=%s\n' "$evidence_rel"
}

command=${1:-}
[[ -n "$command" ]] || {
  usage
  exit 2
}
shift

apk=$default_apk
while (($#)); do
  case "$1" in
    --apk)
      (($# >= 2)) || die '--apk requires a path.'
      apk=$(realpath -e -- "$2")
      shift 2
      ;;
    --device)
      (($# >= 2)) || die '--device requires a serial.'
      device=$2
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *) die "Unknown argument: $1" ;;
  esac
done

load_tools
case "$command" in
  build)
    [[ "$apk" == "$default_apk" && -z "$device" ]] || die 'build accepts no arguments.'
    build_apk
    ;;
  verify)
    [[ -z "$device" ]] || die 'verify does not accept --device.'
    verify_apk "$apk"
    ;;
  install)
    install_apk "$apk"
    ;;
  *)
    usage
    exit 2
    ;;
esac
