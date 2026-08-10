#!/usr/bin/env bash
# Description: Run the workflow contract and documentation self-checks.

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
forbidden='pw''sh|power''shell|[.]ps1'

if grep -EIn "$forbidden" "$script_dir"/*.sh; then
  printf 'Bash verification routes must not invoke PowerShell scripts.\n' >&2
  exit 1
fi

bash "$script_dir/check-docs.sh"
mise exec -- python3 "$script_dir/doc_guard.py" --self-check
