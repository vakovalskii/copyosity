#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="Copyosity"
APP_BUNDLE="$ROOT_DIR/src-tauri/target/release/bundle/macos/$APP_NAME.app"
DMG_PATH="$ROOT_DIR/src-tauri/target/release/bundle/dmg/${APP_NAME}_0.3.0_aarch64.dmg"
IDENTITY="Developer ID Application: Valeriy Kovalsky (A933C2TJXU)"
KEYCHAIN_PROFILE="${KEYCHAIN_PROFILE:-AC_PASSWORD}"
WAIT_FOR_NOTARIZATION="${WAIT_FOR_NOTARIZATION:-0}"

build_app() {
  echo "[release] building app"
  (cd "$ROOT_DIR" && cargo tauri build) || true

  if [[ ! -d "$APP_BUNDLE" ]]; then
    echo "[release] app bundle not found: $APP_BUNDLE" >&2
    exit 1
  fi
}

package_dmg() {
  echo "[release] packaging dmg"
  local tmpdir
  tmpdir="$(mktemp -d /tmp/copyosity-dmg.XXXXXX)"
  mkdir -p "$tmpdir/$APP_NAME"
  cp -R "$APP_BUNDLE" "$tmpdir/$APP_NAME/"
  ln -s /Applications "$tmpdir/$APP_NAME/Applications"

  rm -f "$DMG_PATH"
  hdiutil create \
    -volname "$APP_NAME" \
    -srcfolder "$tmpdir/$APP_NAME" \
    -ov \
    -format UDZO \
    "$DMG_PATH"

  rm -rf "$tmpdir"
}

sign_artifacts() {
  echo "[release] signing app"
  codesign --force --deep --strict --options runtime --sign "$IDENTITY" "$APP_BUNDLE"
  echo "[release] signing dmg"
  codesign --force --sign "$IDENTITY" "$DMG_PATH"
}

verify_artifacts() {
  echo "[release] verifying app signature"
  codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE"
  echo "[release] verifying dmg signature"
  codesign -dv --verbose=2 "$DMG_PATH" >/dev/null
}

submit_notarization() {
  echo "[release] submitting dmg for notarization"
  local output
  if [[ "$WAIT_FOR_NOTARIZATION" == "1" ]]; then
    output="$(xcrun notarytool submit "$DMG_PATH" --keychain-profile "$KEYCHAIN_PROFILE" --wait)"
  else
    output="$(xcrun notarytool submit "$DMG_PATH" --keychain-profile "$KEYCHAIN_PROFILE")"
  fi

  echo "$output"
  local submission_id
  submission_id="$(printf '%s\n' "$output" | awk '/id:/ {print $2}' | tail -n1)"
  if [[ -z "$submission_id" ]]; then
    echo "[release] failed to parse notarization submission id" >&2
    exit 1
  fi

  echo "$submission_id" > "$ROOT_DIR/.last_notarization_id"
  echo "[release] notarization id: $submission_id"
}

staple_and_verify() {
  echo "[release] stapling dmg"
  xcrun stapler staple "$DMG_PATH"
  echo "[release] validating stapled ticket"
  xcrun stapler validate "$DMG_PATH"
  echo "[release] gatekeeper assessment"
  spctl -a -vvv -t install "$DMG_PATH"
}

main() {
  build_app
  package_dmg
  sign_artifacts
  verify_artifacts
  submit_notarization

  if [[ "$WAIT_FOR_NOTARIZATION" == "1" ]]; then
    staple_and_verify
  fi
}

main "$@"
