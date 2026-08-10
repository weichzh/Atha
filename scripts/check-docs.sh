#!/usr/bin/env bash
# Description: Run the required Atha documentation checks with the project toolchain.

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd -- "$script_dir/.." && pwd -P)"

cd "$repo_root"

mise exec -- python3 scripts/doc_guard.py
mise exec -- python3 scripts/doc_length_check.py
