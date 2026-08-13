#!/usr/bin/env bash
# Description: Verify complete local-data backup, restore, recovery, deletion, and library UI state.

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd -- "$script_dir/.." && pwd -P)"
private_fixtures=

die() {
  printf 'check-local-data: %s\n' "$1" >&2
  exit 1
}

while (($#)); do
  case "$1" in
    --private-fixtures)
      (($# >= 2)) || die '--private-fixtures requires a directory'
      private_fixtures=$(realpath -e -- "$2" 2>/dev/null) || die 'private fixtures are unavailable'
      [[ -d $private_fixtures ]] || die 'private fixtures are unavailable'
      shift 2
      ;;
    -h|--help)
      printf 'Usage: scripts/check-local-data.sh [--private-fixtures PATH]\n'
      exit 0
      ;;
    *) die "unknown option: $1" ;;
  esac
done

command -v mise >/dev/null || die 'mise is required'
command -v node >/dev/null || die 'node is required'
command -v pnpm >/dev/null || die 'pnpm is required'

cd -- "$repo_root"
unset ATHA_PRIVATE_DICTIONARY_ROOT

python3 - <<'PY'
import json
import re
from pathlib import Path

expected = {
    "delete_library_book_data",
    "pending_library_book_deletions",
    "finish_library_book_deletion",
    "backup_local_data",
    "prepare_local_data_restore",
    "commit_local_data_restore",
    "pending_local_data_restore",
    "finish_local_data_restore",
    "rollback_local_data_restore",
    "abort_local_data_restore",
    "local_data_storage_usage",
}
source = Path("reader/app/src-tauri/src/lib.rs").read_text()
handler = re.search(r"(?ms)tauri::generate_handler!\[(.*?)\]\)", source)
if not handler:
    raise SystemExit("local-data gate: invoke handler missing")
registered = re.findall(r"(?:local_data_maintenance::)?([a-z_]+)\s*,?", handler.group(1))
toml = Path("reader/app/src-tauri/permissions/reader.toml").read_text()
blocks = re.findall(r"(?ms)^\[\[permission\]\]\s*(.*?)(?=^\[\[permission\]\]|\Z)", toml)
selected = [block for block in blocks if re.search(r'(?m)^identifier\s*=\s*"allow-library-commands"\s*$', block)]
if len(selected) != 1:
    raise SystemExit("local-data gate: missing unique allow-library-commands block")
match = re.search(r"(?ms)^\s*commands\.allow\s*=\s*\[(.*?)\]", selected[0])
allowed = re.findall(r'"([^"]+)"', match.group(1) if match else "")
capability = json.loads(Path("reader/app/src-tauri/capabilities/main.json").read_text())
if "allow-library-commands" not in capability["permissions"]:
    raise SystemExit("local-data gate: main capability misses allow-library-commands")
for command in sorted(expected):
    if registered.count(command) != 1 or allowed.count(command) != 1:
        raise SystemExit(f"local-data gate: command mapping drift for {command}")
print("local_data_command_acl=true")
PY

mise exec -- cargo fmt -p atha-backend -p atha-reader-app -- --check
mise exec -- cargo test --locked -p atha-backend --test local_data
mise exec -- cargo test --locked -p atha-backend local_data::tests::
mise exec -- cargo test --locked -p atha-backend --test message_reading
node --test reader/app/tests/library.test.ts
pnpm --dir reader/app check
pnpm --dir reader/app build
mise exec -- cargo test --locked -p atha-reader-app

if [[ -n $private_fixtures ]]; then
  env ATHA_PRIVATE_DICTIONARY_ROOT="$private_fixtures" \
    mise exec -- cargo test --locked -p atha-backend --test local_data \
    private_dictionary_round_trip_stays_content_free -- --exact
  printf 'local_data_private_dictionary=true\n'
fi

printf 'local_data_gate=true\n'
