# shellcheck shell=bash
# Ensure rustup/cargo is on PATH (npm/make may not load interactive shell config).
if [[ -f "${HOME}/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "${HOME}/.cargo/env"
elif [[ -d "${HOME}/.cargo/bin" ]]; then
  export PATH="${HOME}/.cargo/bin:${PATH}"
fi

# Reuse Rust compilation artifacts across dev, check, and release builds when available.
# Applies to Makefile backend targets, with-tauri.sh, and build/release-macos.sh.
# Set COPYOSITY_DISABLE_SCCACHE=1 for a clean rustc path (e.g. reproducible release debugging).
# This is intentionally opt-in by installation: machines without sccache keep Cargo defaults.
if [[ -z "${RUSTC_WRAPPER:-}" && "${COPYOSITY_DISABLE_SCCACHE:-0}" != "1" ]] && command -v sccache >/dev/null 2>&1; then
  export RUSTC_WRAPPER="$(command -v sccache)"
fi
