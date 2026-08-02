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

CHANGE_HEADINGS = (
    "## Problem",
    "## Scope",
    "## Acceptance Criteria",
    "## Files And Steps",
    "## Checks",
    "## Result",
    "## Review",
    "## Evidence And Residual Risks",
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


def active_changes(root: Path) -> list[str]:
    text = (root / "docs/ACTIVE.md").read_text(encoding="utf-8")
    return re.findall(r"^- `change`：.*→ `([^`]+)`\s*$", text, re.MULTILINE)


def accepted_change(root: Path, relative: str) -> bool:
    path = Path(relative)
    if path.is_absolute() or ".." in path.parts or path.parts[:2] != ("docs", "changes"):
        return False
    target = root / path
    if not target.is_file():
        return False
    text = target.read_text(encoding="utf-8")
    status = re.search(r"^## Status\s*$\n+\s*(accepted|implemented)\s*$", text, re.MULTILINE)
    return status is not None and all(heading in text for heading in CHANGE_HEADINGS)


def main() -> int:
    root = repo_root()
    changed = changed_files()

    code_changed = any(is_code(path) for path in changed)
    changes = active_changes(root)

    if code_changed and not any(
        path in changed and accepted_change(root, path) for path in changes
    ):
        print("BLOCKED: code changed without an updated accepted change.")
        print()
        print(f"Repository: {root}")
        print("Changed files:")
        for path in changed:
            print(f" - {path}")
        print()
        print("Point docs/ACTIVE.md at a changed accepted/implemented docs/changes/*.md file.")
        return 1

    print(f"doc_guard: ok ({'change' if code_changed else 'docs-only'})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
