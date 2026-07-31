#!/usr/bin/env python3

"""Keep markdown docs small enough for minimal-context agent work."""

from __future__ import annotations

import sys
from pathlib import Path


LIMITS = {
    "docs/ACTIVE.md": 150,
    "docs/INDEX.md": 250,
}

DEFAULT_LIMIT = 400


def normalize(path: Path) -> str:
    return path.as_posix()


def limit_for(path: Path) -> int:
    return LIMITS.get(normalize(path), DEFAULT_LIMIT)


def line_count(path: Path) -> int:
    return len(path.read_text(encoding="utf-8").splitlines())


def main() -> int:
    docs_root = Path("docs")
    if not docs_root.exists():
        print("doc_length_check: ok")
        return 0

    failed = False

    for path in sorted(docs_root.rglob("*.md")):
        count = line_count(path)
        limit = limit_for(path)
        if count > limit:
            print(f"TOO LONG: {normalize(path)} has {count} lines; limit is {limit}.")
            failed = True

    if failed:
        print("Split or summarize oversized documents before continuing.")
        return 1

    print("doc_length_check: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
