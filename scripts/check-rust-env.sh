#!/usr/bin/env bash
# Smoke-check that run-rust.sh / Makefile backend targets match env-rust.sh contract.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_RUST="$ROOT_DIR/scripts/run-rust.sh"

wrapper="$("$RUN_RUST" 'printf %s "${RUSTC_WRAPPER:-}"')"

if command -v sccache >/dev/null 2>&1; then
  expected="$(command -v sccache)"
  disabled_wrapper="$(COPYOSITY_DISABLE_SCCACHE=1 "$RUN_RUST" 'printf %s "${RUSTC_WRAPPER:-}"')"

  if [[ "${COPYOSITY_DISABLE_SCCACHE:-0}" != "1" ]]; then
    if [[ "$wrapper" != "$expected" ]]; then
      echo "error: expected RUSTC_WRAPPER=$expected, got: ${wrapper:-<empty>}" >&2
      exit 1
    fi
  fi

  if [[ -n "$disabled_wrapper" ]]; then
    echo "error: COPYOSITY_DISABLE_SCCACHE=1 should not set RUSTC_WRAPPER, got: $disabled_wrapper" >&2
    exit 1
  fi
elif [[ -z "${RUSTC_WRAPPER:-}" && -n "$wrapper" ]]; then
  echo "error: sccache not installed but run-rust.sh set RUSTC_WRAPPER=$wrapper" >&2
  exit 1
fi

echo "check-rust-env: ok (rustc_wrapper=${wrapper:-<none>})"
