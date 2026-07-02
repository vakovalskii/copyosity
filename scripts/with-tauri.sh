#!/usr/bin/env bash
set -euo pipefail

dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./env-rust.sh
source "$dir/env-rust.sh"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found. Install Rust: https://rustup.rs" >&2
  exit 1
fi

exec tauri "$@"
