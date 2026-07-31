#!/usr/bin/env python3

"""Guard document/code synchronization for local Codex workflow."""

from __future__ import annotations

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

DOC_PREFIXES = (
    "docs/",
    ".agents/",
    ".codex/",
)

DOC_FILES = {
    "AGENTS.md",
    "README.md",
}

REQUIRED_DOC_FILES = {
    "docs/ACTIVE.md",
}

REQUIRED_DOC_PREFIXES = (
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


def is_doc(path: str) -> bool:
    return path in DOC_FILES or has_prefix(path, DOC_PREFIXES)


def is_required_doc(path: str) -> bool:
    return path in REQUIRED_DOC_FILES or has_prefix(path, REQUIRED_DOC_PREFIXES)


def main() -> int:
    root = repo_root()
    changed = changed_files()

    code_changed = any(is_code(path) for path in changed)
    required_doc_changed = any(is_required_doc(path) for path in changed)

    if code_changed and not required_doc_changed:
        print("BLOCKED: code changed but ACTIVE/spec/plan/review docs were not updated.")
        print()
        print(f"Repository: {root}")
        print("Changed files:")
        for path in changed:
            print(f" - {path}")
        print()
        print("Update docs/ACTIVE.md and the relevant spec, plan, or review before continuing.")
        return 1

    print("doc_guard: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
