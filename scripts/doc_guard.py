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
    "## Architecture Impact",
    "## Acceptance Criteria",
    "## Files And Steps",
    "## Checks",
    "## Iteration Control",
    "## Result",
    "## Review",
    "## Evidence And Residual Risks",
)

ARCHITECTURE_IMPACT_VALUES = {"none", "present"}
ITERATION_MODES = {"single-pass", "iterative"}


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


def architecture_impact(text: str) -> str | None:
    match = re.search(
        r"^## Architecture Impact\s*$\n+\s*(\S+)\s*$",
        text,
        re.MULTILINE,
    )
    if match is None or match.group(1) not in ARCHITECTURE_IMPACT_VALUES:
        return None
    return match.group(1)


def iteration_mode(text: str) -> str | None:
    sections = re.findall(
        r"^## Iteration Control\s*$\n(?P<body>.*?)(?=^## |\Z)",
        text,
        re.MULTILINE | re.DOTALL,
    )
    if len(sections) != 1:
        return None
    modes = re.findall(
        r"^- Mode:\s*`([^`]+)`\s*$",
        sections[0],
        re.MULTILINE,
    )
    if len(modes) != 1 or modes[0] not in ITERATION_MODES:
        return None
    return modes[0]


def valid_change_text(text: str) -> bool:
    status = re.search(
        r"^## Status\s*$\n+\s*(accepted|implemented)\s*$",
        text,
        re.MULTILINE,
    )
    return (
        status is not None
        and all(heading in text for heading in CHANGE_HEADINGS)
        and architecture_impact(text) is not None
        and iteration_mode(text) is not None
    )


def accepted_change(root: Path, relative: str) -> bool:
    path = Path(relative)
    if path.is_absolute() or ".." in path.parts or path.parts[:2] != ("docs", "changes"):
        return False
    target = root / path
    if not target.is_file():
        return False
    text = target.read_text(encoding="utf-8")
    return valid_change_text(text)


def self_check() -> int:
    body = "\n\n".join(
        (
            "# Self check",
            "## Status\n\naccepted",
            *(f"{heading}\n\nplaceholder" for heading in CHANGE_HEADINGS),
        )
    )
    none = body.replace(
        "## Architecture Impact\n\nplaceholder",
        "## Architecture Impact\n\nnone",
    )
    present = none.replace(
        "## Architecture Impact\n\nnone",
        "## Architecture Impact\n\npresent",
    )
    single_pass = none.replace(
        "## Iteration Control\n\nplaceholder",
        "## Iteration Control\n\n- Mode: `single-pass`",
    )
    iterative = present.replace(
        "## Iteration Control\n\nplaceholder",
        "## Iteration Control\n\n- Mode: `iterative`",
    )
    missing_architecture = single_pass.replace("## Architecture Impact\n\nnone\n\n", "")
    unknown_architecture = single_pass.replace(
        "## Architecture Impact\n\nnone",
        "## Architecture Impact\n\nunknown",
    )
    missing_iteration = none.replace("## Iteration Control\n\nplaceholder\n\n", "")
    unknown_iteration = none.replace(
        "## Iteration Control\n\nplaceholder",
        "## Iteration Control\n\n- Mode: `unknown`",
    )
    duplicate_iteration = single_pass.replace(
        "- Mode: `single-pass`",
        "- Mode: `single-pass`\n- Mode: `iterative`",
    )
    duplicate_iteration_section = single_pass + (
        "\n\n## Iteration Control\n\n- Mode: `iterative`"
    )

    if not valid_change_text(single_pass) or not valid_change_text(iterative):
        raise AssertionError("valid change metadata was rejected")
    if valid_change_text(missing_architecture) or valid_change_text(unknown_architecture):
        raise AssertionError("invalid architecture impact was accepted")
    if (
        valid_change_text(missing_iteration)
        or valid_change_text(unknown_iteration)
        or valid_change_text(duplicate_iteration)
        or valid_change_text(duplicate_iteration_section)
    ):
        raise AssertionError("invalid iteration mode was accepted")
    print("doc_guard_self_check: ok")
    return 0


def main() -> int:
    if sys.argv[1:] == ["--self-check"]:
        return self_check()
    if sys.argv[1:]:
        print("Usage: doc_guard.py [--self-check]", file=sys.stderr)
        return 2

    root = repo_root()
    changed = changed_files()

    code_changed = any(is_code(path) for path in changed)
    changes = active_changes(root)

    invalid_changes = [path for path in changes if not accepted_change(root, path)]
    if invalid_changes:
        print("BLOCKED: active change has invalid metadata.")
        for path in invalid_changes:
            print(f" - {path}")
        return 1

    if code_changed and not any(path in changed for path in changes):
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
