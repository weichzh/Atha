#!/usr/bin/env bash
# Description: Verify the EPUB importer and one explicit source on Linux.

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd -- "$script_dir/.." && pwd -P)"
epub=
work_root=

die() {
  printf '%s\n' "$*" >&2
  exit 1
}

cleanup() {
  [[ -z "$work_root" ]] || rm -rf -- "$work_root"
}
trap cleanup EXIT

while (($#)); do
  case "$1" in
    --epub)
      (($# >= 2)) || die 'Missing EPUB input.'
      [[ -z "$epub" ]] || die 'EPUB input was provided more than once.'
      epub=$2
      shift 2
      ;;
    -h | --help)
      printf 'Usage: scripts/check-epub-source.sh --epub PATH\n'
      exit 0
      ;;
    *) die 'Unknown argument.' ;;
  esac
done

[[ -n "$epub" && -f "$epub" && -r "$epub" ]] || die 'EPUB input is unavailable.'
command -v mise >/dev/null || die 'mise is required.'
command -v realpath >/dev/null || die 'realpath is required.'
epub="$(realpath -e -- "$epub" 2>/dev/null)" || die 'EPUB input is unavailable.'

mkdir -p "$repo_root/.tmp"
work_root="$(mktemp -d "$repo_root/.tmp/epub-source-gate.XXXXXX")"
library_root="$work_root/library"
mkdir -p "$library_root"

cd "$repo_root"
mise exec -- cargo test --quiet --release --locked \
  -p atha-backend --test epub_import

if ! ATHA_EPUB_GATE_LIBRARY_ROOT="$library_root" ATHA_EPUB_GATE_SOURCE="$epub" \
  mise exec -- cargo test --quiet --release --locked \
    -p atha-backend --test epub_import \
    seeds_private_formula_gui_benchmark -- --ignored --exact \
    >"$work_root/release-import.log" 2>&1; then
  die 'EPUB release import probe failed.'
fi

printf 'epub_import_tests=passed\n'
printf 'release_import_probe=passed\n'
