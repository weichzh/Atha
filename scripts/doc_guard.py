#!/usr/bin/env python3

"""Guard document/code synchronization for local Codex workflow."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path


CODE_PREFIXES = (
    "backend/",
    "src/",
    "app/",
    "lib/",
    "packages/",
    "components/",
    "data/",
    "p0/",
    "scripts/",
    "tests/",
)

CODE_FILES = {
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "package.json",
    "rust-toolchain.toml",
    "vite.config.ts",
    "next.config.js",
    "xmake.lua",
    "CMakeLists.txt",
    "Makefile",
    "makefile",
}

FULL_PATH_DOC_PREFIXES = (
    "docs/specs/",
    "docs/plans/",
    "docs/reviews/",
)


def run_git(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        text=True,
        capture_output=True,
        check=True,
    )


def repo_root() -> Path:
    result = run_git(["rev-parse", "--show-toplevel"])
    return Path(result.stdout.strip())


def normalize(path: str) -> str:
    return path.replace("\\", "/").strip()


def changed_files() -> list[str]:
    result = run_git(["status", "--porcelain=v1", "--untracked-files=all"])
    files: set[str] = set()

    for raw_line in result.stdout.splitlines():
        if not raw_line:
            continue

        path = raw_line[3:]
        if " -> " in path:
            path = path.split(" -> ", 1)[1]

        files.add(normalize(path))

    return sorted(files)


def has_prefix(path: str, prefixes: tuple[str, ...]) -> bool:
    return any(path.startswith(prefix) for prefix in prefixes)


def is_code(path: str) -> bool:
    return path in CODE_FILES or has_prefix(path, CODE_PREFIXES)


def active_flow(root: Path) -> str | None:
    text = (root / "docs/ACTIVE.md").read_text(encoding="utf-8")
    match = re.search(r"^- 流程：(fast|full)\s*$", text, re.MULTILINE)
    return match.group(1) if match else None


def main() -> int:
    root = repo_root()
    changed = changed_files()

    code_changed = any(is_code(path) for path in changed)
    active_changed = "docs/ACTIVE.md" in changed
    flow = active_flow(root)

    if code_changed and (not active_changed or flow not in {"fast", "full"}):
        print("BLOCKED: code changed without an updated ACTIVE flow declaration.")
        print()
        print(f"Repository: {root}")
        print("Changed files:")
        for path in changed:
            print(f" - {path}")
        print()
        print("Update docs/ACTIVE.md with '- 流程：fast' or '- 流程：full'.")
        return 1

    if code_changed and flow == "full":
        missing = [
            prefix
            for prefix in FULL_PATH_DOC_PREFIXES
            if not any(path.startswith(prefix) for path in changed)
        ]
        if missing:
            print("BLOCKED: full-path code changed without spec, plan, and review updates.")
            for prefix in missing:
                print(f" - missing change under {prefix}")
            return 1

    print(f"doc_guard: ok ({flow or 'docs-only'})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
