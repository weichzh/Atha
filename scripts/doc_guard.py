#!/usr/bin/env python3

"""Guard document/code synchronization for local Codex workflow."""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path


CODE_PREFIXES = (
    "backend/",
    "reader/",
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
    ".mise.toml",
    "Cargo.lock",
    "Cargo.toml",
    "env/example.ps1",
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


def git_output(*args: str) -> bytes:
    return subprocess.check_output(["git", *args])


def repo_root() -> Path:
    # Drop Git's line terminator, not whitespace in the path itself.
    return Path(os.fsdecode(git_output("rev-parse", "--show-toplevel")[:-1]))


def changed_files() -> list[str]:
    # Disable rename detection to enumerate both endpoints. NUL framing preserves
    # literal names (including whitespace, backslashes and " -> ").
    status = git_output("status", "--porcelain=v1", "-z", "--no-renames", "--untracked-files=all")
    return sorted({os.fsdecode(entry[3:]) for entry in status.split(b"\0") if entry})


def commit(ref: str) -> str:
    oid = git_output("rev-parse", "--verify", "--end-of-options", f"{ref}^{{commit}}")
    return oid.decode("ascii").strip()


def committed_paths(commit_range: str) -> list[str]:
    """Compare resolved commits; include both sides of a rename in the policy check."""
    paths = git_output("diff", "--no-renames", "--name-only", "-z", commit_range, "--")
    return sorted(os.fsdecode(path) for path in paths.split(b"\0") if path)


def has_prefix(path: str, prefixes: tuple[str, ...]) -> bool:
    return any(path.startswith(prefix) for prefix in prefixes)


def is_code(path: str) -> bool:
    return path in CODE_FILES or has_prefix(path, CODE_PREFIXES)


def active_changes(root: Path) -> list[str]:
    changes = []
    active = root / "docs/ACTIVE.md"
    for number, line in enumerate(active.read_text(encoding="utf-8").splitlines(), 1):
        marker = re.match(r"[ \t]*[-+*][ \t]+`change`", line)
        if marker is None:
            continue
        route = re.fullmatch(r"：.*→ `([^`]+)`[ \t]*", line[marker.end():])
        if route is None:
            raise ValueError(
                f"malformed active change route at docs/ACTIVE.md:{number}; "
                "use - `change`：Description → `docs/changes/<slug>.md`."
            )
        changes.append(route.group(1))
    return changes


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
    if (
        len(path.parts) != 3
        or path.parts[:2] != ("docs", "changes")
        or path.suffix != ".md"
        or path.as_posix() != relative
    ):
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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", help="compare committed HEAD with this PR base")
    parser.add_argument("--kind", choices=("fast", "change"), help="workflow path for a committed PR diff")
    args = parser.parse_args()
    if args.kind and args.base is None:
        parser.error("--kind requires --base")

    root = repo_root()
    changed = changed_files()
    commit_range = None
    if args.base is not None:
        if changed:
            parser.error("committed-diff mode needs a clean worktree")
        try:
            commit_range = f"{commit(args.base)}...{commit('HEAD')}"
            changed = committed_paths(commit_range)
        except subprocess.CalledProcessError:
            parser.error("cannot compare the specified base with HEAD")

    code_changed = any(is_code(path) for path in changed)
    try:
        changes = active_changes(root)
    except ValueError as error:
        print(f"BLOCKED: {error}", file=sys.stderr)
        return 1

    invalid_changes = [path for path in changes if not accepted_change(root, path)]
    if invalid_changes:
        print("BLOCKED: active change has invalid metadata.")
        for path in invalid_changes:
            print(f" - {path}")
        return 1

    if code_changed and args.kind != "fast" and not any(path in changed for path in changes):
        print("BLOCKED: code changed without an updated accepted change.")
        print()
        print(f"Repository: {root}")
        print("Changed files:")
        for path in changed:
            print(f" - {path}")
        print()
        print("Point docs/ACTIVE.md at a changed accepted/implemented docs/changes/*.md file.")
        return 1

    if commit_range and subprocess.run(
        ["git", "diff", "--check", commit_range, "--"]
    ).returncode:
        return 1

    lane = (args.kind or "change") if code_changed else "docs-only"
    print(f"doc_guard: ok ({lane})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
