#!/usr/bin/env bash
# Description: Run local checks without starting GUI sessions or reading private fixtures.
set -euo pipefail
script_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
repo_root=$(cd -- "$script_root/.." && pwd -P)
if [[ $# == 1 && $1 == --help ]]; then
  printf 'Usage: scripts/check-fast.sh [--base REF [--kind fast|change]]\nRuns syntax, docs, Node, Svelte, Rust workspace tests and production frontend build.\nWith --base, the workflow check compares committed HEAD to the PR base.\nGUI and private samples use their explicit gates.\n'
  exit 0
fi
command -v mise >/dev/null || { printf 'mise is required.\n' >&2; exit 1; }
# Prefer rustup so rust-toolchain.toml is honored in a minimal SSH environment.
cargo_bin=${CARGO_HOME:-$HOME/.cargo}/bin
if [[ -x $cargo_bin/rustup && -x $cargo_bin/cargo ]]; then export PATH="$cargo_bin:$PATH"; fi
cd -- "$repo_root"
bash "$script_root/check-workflow.sh" "$@"
# The committed path checks its PR diff in doc_guard; retain the local diff check otherwise.
if [[ $# == 0 ]]; then
  git diff --check
fi
bash -n "$script_root/check-fast.sh" "$script_root/check-reader-linux.sh" "$script_root/lib/reader-linux-preflight.sh"
mise exec -- node --check "$script_root/test-node.mjs"
mise exec -- cargo fmt --all -- --check
mise exec -- pnpm --dir reader/app test
mise exec -- pnpm --dir reader/app check
mise exec -- cargo test --workspace --locked --offline
mise exec -- pnpm --dir reader/app build
printf 'fast_checks=passed\n'
