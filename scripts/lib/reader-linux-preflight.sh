#!/usr/bin/env bash
# Description: Resolve the existing driver without launching or installing anything.

atha_reader_driver() {
  local driver_repo_root=$1 driver_invocation_root=$2 candidate
  if [[ -n ${ATHA_TAURI_DRIVER:-} ]]; then
    candidate=$ATHA_TAURI_DRIVER
    [[ $candidate == /* ]] || candidate=$driver_invocation_root/$candidate
    if [[ ! -f $candidate || ! -x $candidate ]]; then
      printf 'ATHA_TAURI_DRIVER is not an executable file\n' >&2
      return 1
    fi
    realpath -e -- "$candidate"
    return
  fi
  candidate=$(command -v tauri-driver || true)
  if [[ -n $candidate && -f $candidate && -x $candidate ]]; then
    realpath -e -- "$candidate"
    return
  fi
  for candidate in \
    "${CARGO_HOME:-$HOME/.cargo}/bin/tauri-driver" \
    "$driver_repo_root/.tmp/tauri-driver/bin/tauri-driver"; do
    if [[ -f $candidate && -x $candidate ]]; then
      realpath -e -- "$candidate"
      return
    fi
  done
  printf 'tauri-driver is required; set ATHA_TAURI_DRIVER or install the documented version\n' >&2
  return 1
}
