#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=env-rust.sh
source "$ROOT_DIR/scripts/env-rust.sh"
# shellcheck source=macos-target.sh
source "$ROOT_DIR/scripts/macos-target.sh"

unset npm_config_devdir 2>/dev/null || true
# Cursor/sandbox may redirect Rust output; keep artifacts under src-tauri/target.
unset CARGO_TARGET_DIR 2>/dev/null || true
export CARGO_TARGET_DIR="$ROOT_DIR/src-tauri/target"
NPM=(env -u npm_config_devdir npm)

macos_resolve_target "$ROOT_DIR"
macos_ensure_rust_target

echo "[build] arch=${MACOS_ARCH_LABEL} rust_target=${RUST_TARGET:-native}"
echo "[build] output app: $APP_BUNDLE"
echo "[build] output dmg: $DIST_DMG"

build_frontend() {
  echo "[build] frontend"
  (cd "$ROOT_DIR" && "${NPM[@]}" run build)
}

build_tauri() {
  echo "[build] tauri bundle (app)"
  local -a tauri_args=(build --bundles app)
  if [[ -z "${RELEASE_CONFIG:-}" ]]; then
    # Local/CI builds: ad-hoc sign; upstream signingIdentity stays in tauri.conf.json for PR.
    tauri_args+=(--config src-tauri/tauri.unsigned.json)
  fi
  if [[ -n "${RUST_TARGET:-}" ]]; then
    tauri_args+=(--target "$RUST_TARGET")
  fi
  (cd "$ROOT_DIR" && "${NPM[@]}" run tauri -- "${tauri_args[@]}")
}

package_dmg() {
  echo "[build] packaging dmg"
  local tmpdir
  tmpdir="$(mktemp -d /tmp/copyosity-dmg.XXXXXX)"
  mkdir -p "$tmpdir/Copyosity"
  cp -R "$APP_BUNDLE" "$tmpdir/Copyosity/Copyosity.app"
  ln -s /Applications "$tmpdir/Copyosity/Applications"
  rm -f "$DIST_DMG"
  hdiutil create -volname Copyosity -srcfolder "$tmpdir/Copyosity" -ov -format UDZO "$DIST_DMG"
  rm -rf "$tmpdir"
}

publish_artifacts() {
  mkdir -p "$DIST_DIR"
  if [[ ! -d "$APP_BUNDLE" ]]; then
    echo "[build] app bundle not found: $APP_BUNDLE" >&2
    exit 1
  fi

  rm -rf "$DIST_APP"
  cp -R "$APP_BUNDLE" "$DIST_APP"
  package_dmg

  echo ""
  echo "========================================"
  echo " macOS build ready (${MACOS_ARCH_LABEL})"
  echo "========================================"
  echo " DMG:  $DIST_DMG"
  echo " APP:  $DIST_APP"
  echo " Raw:  $APP_BUNDLE"
  echo ""
  file "$DIST_APP/Contents/MacOS/copyosity" 2>/dev/null || file "$DIST_APP/Contents/MacOS/"* 2>/dev/null || true
}

main() {
  build_frontend
  build_tauri
  publish_artifacts
}

main "$@"
