#!/usr/bin/env bash
# Description: Verify bounded offline dictionaries with optional private output and performance gates.

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd -- "$script_dir/.." && pwd -P)"
private_fixtures=
private_log=
benchmark_root=

usage() {
  printf 'Usage: scripts/check-dictionary-source.sh [--private-fixtures PATH]\n'
}

die() {
  printf '%s\n' "$*" >&2
  exit 1
}

cleanup() {
  [[ -z "$private_log" ]] || rm -f -- "$private_log"
  [[ -z "$benchmark_root" ]] || rm -rf -- "$benchmark_root"
}
trap cleanup EXIT

while (($#)); do
  case "$1" in
    --private-fixtures)
      (($# >= 2)) || die '--private-fixtures requires a directory.'
      private_fixtures=$(realpath -e -- "$2" 2>/dev/null) || die 'Private dictionary fixtures are unavailable.'
      [[ -d "$private_fixtures" ]] || die 'Private dictionary fixtures are unavailable.'
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
done

command -v mise >/dev/null || die 'mise is required.'
cd "$repo_root"

unset ATHA_PRIVATE_DICTIONARY_ROOT ATHA_DICTIONARY_BENCHMARK_ROOT
mise exec -- node --test reader/web/annotations.test.mjs
mise exec -- cargo test --locked -p atha-backend --test dictionary_lookup
mise exec -- cargo test --locked -p atha-backend 'reader::dictionary' --lib
printf 'dictionary_public=true\n'

[[ -n "$private_fixtures" ]] || exit 0
[[ -f "$private_fixtures/dictionary-english-output.json" ]] ||
  die 'Private dictionary evidence is unavailable.'
command -v jq >/dev/null || die 'jq is required for private dictionary verification.'

mkdir -p "$repo_root/.tmp"
private_log=$(mktemp "$repo_root/.tmp/dictionary-gate.XXXXXX")
benchmark_root="$repo_root/.tmp/dictionary-benchmark-gate-$$"

run_private() {
  local failure=$1
  shift
  : >"$private_log"
  if ! env ATHA_PRIVATE_DICTIONARY_ROOT="$private_fixtures" "$@" >"$private_log" 2>&1; then
    die "$failure"
  fi
}

run_private 'Private MDict compatibility verification failed.' \
  mise exec -- cargo test --locked --release -p atha-backend --test dictionary_lookup \
  private_mdict_sample_imports_and_looks_up_without_content_artifacts -- --exact
run_private 'Private Kindle and English output verification failed.' \
  mise exec -- cargo test --locked --release -p atha-backend \
  'reader::dictionary::tests::private_' --lib

: >"$private_log"
if ! env \
  ATHA_PRIVATE_DICTIONARY_ROOT="$private_fixtures" \
  ATHA_DICTIONARY_BENCHMARK_ROOT="$benchmark_root" \
  mise exec -- cargo test --locked --release -p atha-backend \
  'reader::dictionary::tests::private_dictionary_benchmark' \
  --lib -- --ignored --exact --nocapture >"$private_log" 2>&1; then
  die 'Private dictionary benchmark failed.'
fi

benchmark=$(sed -n 's/^dictionary_benchmark=//p' "$private_log" | tail -n 1)
[[ -n "$benchmark" ]] || die 'Private dictionary benchmark emitted no aggregate evidence.'
if ! jq -e '
  .peak_rss_kib > 0 and .peak_rss_kib <= 65536 and
  .kindle_cold_lookup_p95_us <= 500000 and
  .mdict_cold_lookup_p95_us <= 500000 and
  .mdd_cold_lookup_p95_us <= 500000 and
  .kindle_hot_lookup_p95_us <= 100000 and
  .mdict_hot_lookup_p95_us <= 100000 and
  .mdd_hot_lookup_p95_us <= 100000
' >/dev/null 2>&1 <<<"$benchmark"; then
  die 'Private dictionary benchmark exceeded the accepted latency or memory budget.'
fi

printf 'dictionary_private=true\n'
printf 'dictionary_benchmark=%s\n' "$benchmark"
