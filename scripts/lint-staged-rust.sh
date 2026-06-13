#!/usr/bin/env bash
# Pre-commit Rust hook (via lint-staged).
#
# Speed contract: fmt only on staged paths; clippy uses --lib by default because
# main.rs is a thin wrapper and all logic lives in the library crate. Rust cannot
# lint arbitrary single files in isolation — clippy still analyzes the lib target
# as a whole, but skipping the bin target is meaningfully faster than --all-targets.
# Full `cargo clippy --all-targets` remains in `make check` and release CI.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/src-tauri"

export PATH="${HOME}/.cargo/bin:${PATH}"

if [ "$#" -eq 0 ]; then
  exit 0
fi

cargo fmt -- "$@"

clippy_targets=(--lib)
for f in "$@"; do
  case "$f" in
    */build.rs | */main.rs | */benches/* | */examples/* | */tests/*)
      clippy_targets=(--all-targets)
      break
      ;;
  esac
done

cargo clippy --fix --allow-dirty --allow-staged "${clippy_targets[@]}" -- -D warnings
cargo fmt -- "$@"
