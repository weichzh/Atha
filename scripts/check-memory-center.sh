#!/usr/bin/env bash
# Description: Verify cross-book reading memory queries, projections, deep links, and command access.

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd -- "$script_dir/.." && pwd -P)"

command -v mise >/dev/null || {
  printf 'check-memory-center: mise is required\n' >&2
  exit 1
}
command -v node >/dev/null || {
  printf 'check-memory-center: node is required\n' >&2
  exit 1
}
command -v pnpm >/dev/null || {
  printf 'check-memory-center: pnpm is required\n' >&2
  exit 1
}

cd -- "$repo_root"

python3 - <<'PY'
import json
import re
from pathlib import Path

expected = {
    "reading_memory_search",
    "reading_memory_snapshot_resource",
    "reading_memory_source_captures",
}
lib = Path("reader/app/src-tauri/src/lib.rs").read_text()
handler = re.search(r"(?ms)tauri::generate_handler!\[(.*?)\]\)", lib)
if not handler:
    raise SystemExit("memory gate: invoke handler missing")
registered = re.findall(r"(?:message_commands::)?([a-z_]+)\s*,?", handler.group(1))
permissions = Path("reader/app/src-tauri/permissions/reader.toml").read_text()
blocks = re.findall(
    r"(?ms)^\[\[permission\]\]\s*(.*?)(?=^\[\[permission\]\]|\Z)",
    permissions,
)
selected = [
    block
    for block in blocks
    if re.search(r'(?m)^identifier\s*=\s*"allow-library-commands"\s*$', block)
]
if len(selected) != 1:
    raise SystemExit("memory gate: missing unique allow-library-commands block")
match = re.search(r"(?ms)^\s*commands\.allow\s*=\s*\[(.*?)\]", selected[0])
allowed = re.findall(r'"([^"]+)"', match.group(1) if match else "")
capability = json.loads(Path("reader/app/src-tauri/capabilities/main.json").read_text())
if "allow-library-commands" not in capability["permissions"]:
    raise SystemExit("memory gate: main capability misses allow-library-commands")
commands = Path("reader/app/src-tauri/src/message_commands.rs").read_text()
for command in sorted(expected):
    if registered.count(command) != 1 or allowed.count(command) != 1:
        raise SystemExit(f"memory gate: command mapping drift for {command}")
    body = re.search(
        rf"(?ms)pub\(crate\) async fn {command}\b(.*?)(?=\n#\[tauri::command\]|\Z)",
        commands,
    )
    if not body or "require_library_window(&window)?;" not in body.group(1):
        raise SystemExit(f"memory gate: library origin guard drift for {command}")
print("reading_memory_command_acl=true")
PY

mise exec -- cargo fmt -p atha-backend -p atha-reader-app -- --check
mise exec -- cargo test --locked -p atha-backend --test message_reading
node --test reader/app/tests/library.test.ts
node reader/web/conversations.test.mjs
pnpm --dir reader/app check
pnpm --dir reader/app build
mise exec -- cargo test --locked -p atha-reader-app

printf 'reading_memory_gate=true\n'
