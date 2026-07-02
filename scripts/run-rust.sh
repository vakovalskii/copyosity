#!/usr/bin/env bash
# Source env-rust.sh once, cd to src-tauri, run a shell command string.
set -euo pipefail

if (($# != 1)); then
  echo "usage: run-rust.sh '<command>'" >&2
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=env-rust.sh
source "$ROOT_DIR/scripts/env-rust.sh"
cd "$ROOT_DIR/src-tauri"
bash -c "$1"
