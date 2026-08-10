#!/usr/bin/env bash
# Description: Run the private formula-heavy EPUB through the Linux GUI reader gate.

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd -- "$script_dir/.." && pwd -P)"
metadata="$repo_root/fixtures/local/logic-heavy-ch095/.atha-reader-sample.json"
minimum_formulas=1000
minimum_pages=10
gesture_warmups=5
gesture_measurements=20
epub=

require_count() {
  local value="$1" minimum="$2" maximum="$3" label="$4"
  if ! [[ "$value" =~ ^[0-9]+$ ]] || ((value < minimum || value > maximum)); then
    printf '%s is out of range.\n' "$label" >&2
    exit 2
  fi
}

while (($#)); do
  case "$1" in
    --epub) epub="${2:?missing EPUB path}"; shift 2 ;;
    --metadata) metadata="${2:?missing metadata path}"; shift 2 ;;
    --minimum-formulas) minimum_formulas="${2:?missing formula minimum}"; shift 2 ;;
    --minimum-pages) minimum_pages="${2:?missing page minimum}"; shift 2 ;;
    --gesture-warmups) gesture_warmups="${2:?missing warmup count}"; shift 2 ;;
    --gesture-measurements) gesture_measurements="${2:?missing measurement count}"; shift 2 ;;
    *) printf 'Unknown argument: %s\n' "$1" >&2; exit 2 ;;
  esac
done

require_count "$minimum_formulas" 1 100000 'Formula minimum'
require_count "$minimum_pages" 1 10000 'Page minimum'
require_count "$gesture_warmups" 0 20 'Gesture warmup count'
require_count "$gesture_measurements" 1 100 'Gesture measurement count'

[[ -f "${epub:-}" && -f "$metadata" ]] || { printf 'Formula fixture is unavailable.\n' >&2; exit 1; }
expected="$(jq -er '.source_sha256 | select(test("^[0-9a-f]{64}$"))' "$metadata")"
jq -e '.entry | strings | select(length > 0)' "$metadata" >/dev/null
[[ "$(sha256sum "$epub" | cut -d' ' -f1)" == "$expected" ]] || {
  printf 'Formula fixture does not match its private metadata.\n' >&2
  exit 1
}

exec bash "$script_dir/check-reader-linux.sh" \
  --formula-epub "$epub" \
  --formula-metadata "$metadata" \
  --minimum-formulas "$minimum_formulas" \
  --minimum-pages "$minimum_pages" \
  --gesture-warmups "$gesture_warmups" \
  --gesture-measurements "$gesture_measurements"
