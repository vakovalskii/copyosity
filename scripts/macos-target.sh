#!/usr/bin/env bash
# Shared macOS arch / Rust target resolution for build and release scripts.
# shellcheck shell=bash

macos_read_version() {
  local root="$1"
  (cd "$root" && node -e "console.log(require('./src-tauri/tauri.conf.json').version)")
}

# Sets: MACOS_ARCH_LABEL, RUST_TARGET (empty = native), TARGET_DIR, APP_BUNDLE, DMG_BASENAME
macos_resolve_target() {
  local root="$1"
  local arch="${MACOS_ARCH:-auto}"
  local host
  host="$(uname -m)"
  local version
  version="$(macos_read_version "$root")"

  case "$arch" in
    auto)
      case "$host" in
        arm64) MACOS_ARCH_LABEL=aarch64 ;;
        x86_64) MACOS_ARCH_LABEL=x86_64 ;;
        *)
          echo "error: unsupported host architecture: $host" >&2
          return 1
          ;;
      esac
      RUST_TARGET=""
      ;;
    aarch64 | arm64)
      MACOS_ARCH_LABEL=aarch64
      RUST_TARGET=aarch64-apple-darwin
      ;;
    x86_64 | intel | x64)
      MACOS_ARCH_LABEL=x86_64
      RUST_TARGET=x86_64-apple-darwin
      ;;
    universal)
      MACOS_ARCH_LABEL=universal
      RUST_TARGET=universal-apple-darwin
      ;;
    *)
      echo "error: unknown MACOS_ARCH=$arch (use auto, aarch64, x86_64, universal)" >&2
      return 1
      ;;
  esac

  if [[ -n "$RUST_TARGET" ]]; then
    local host_triple
    host_triple="$(rustc -vV 2>/dev/null | awk '/^host: / { print $2 }')"
    if [[ "$host_triple" == "$RUST_TARGET" ]]; then
      RUST_TARGET=""
    fi
  fi

  if [[ -z "$RUST_TARGET" ]]; then
    TARGET_DIR="$root/src-tauri/target/release"
  else
    TARGET_DIR="$root/src-tauri/target/$RUST_TARGET/release"
  fi

  APP_BUNDLE="$TARGET_DIR/bundle/macos/Copyosity.app"
  DMG_BASENAME="Copyosity_${version}_${MACOS_ARCH_LABEL}.dmg"
  DIST_DIR="$root/dist/macos"
  DIST_DMG="$DIST_DIR/$DMG_BASENAME"
  DIST_APP="$DIST_DIR/Copyosity_${MACOS_ARCH_LABEL}.app"
}

macos_ensure_rust_target() {
  if [[ -z "${RUST_TARGET:-}" ]]; then
    return 0
  fi
  if ! rustup target list --installed | grep -q "^${RUST_TARGET}\$"; then
    echo "[macos] installing Rust target: $RUST_TARGET"
    rustup target add "$RUST_TARGET"
  fi
}
