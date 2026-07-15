#!/usr/bin/env bash
# Finalize a macOS release: build DMG, notarize + staple, and produce signed,
# CLEAN updater tarballs for both arches. Assumes both .app bundles are already
# built and Developer-ID signed (see scripts/build-macos.sh with RELEASE_CONFIG=1).
#
# Usage: scripts/finalize.sh <version> [out-dir]
#   out-dir defaults to dist/updater/<version> (git-ignored).
#
# Prereqs: .tauri/copyosity-updater.key present; notarytool keychain profile
# (default "copyosity", override with KEYCHAIN_PROFILE). Run under
# `dangerouslyDisableSandbox` on this machine (hdiutil/notarytool/stapler).
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:?usage: finalize.sh <version> [out-dir]}"
OUT="${2:-$PWD/dist/updater/$VERSION}"
IDENTITY="Developer ID Application: Valeriy Kovalsky (A933C2TJXU)"
PROFILE="${KEYCHAIN_PROFILE:-copyosity}"
KEY="$(cat .tauri/copyosity-updater.key)"
mkdir -p "$OUT"

finalize_arch () {
  local APP="$1" ARCHLABEL="$2" TARBALL="$3"
  local NAME="Copyosity-${ARCHLABEL}"
  local STAGE="$OUT/stage-$ARCHLABEL"
  local DMG="$OUT/${NAME}_${VERSION}.dmg"

  echo "########## $ARCHLABEL ##########"
  [ -d "$APP" ] || { echo "ERROR: app bundle not found: $APP (build it first)" >&2; exit 1; }
  rm -rf "$STAGE"; mkdir -p "$STAGE"
  cp -R "$APP" "$STAGE/Copyosity.app"
  ln -s /Applications "$STAGE/Applications"

  echo ">> building DMG"
  rm -f "$OUT/tmp-$ARCHLABEL.dmg" "$DMG"
  hdiutil makehybrid -hfs -hfs-volume-name "Copyosity" -o "$OUT/tmp-$ARCHLABEL.dmg" "$STAGE"
  hdiutil convert "$OUT/tmp-$ARCHLABEL.dmg" -format UDZO -o "$DMG"
  rm -f "$OUT/tmp-$ARCHLABEL.dmg"

  echo ">> signing DMG"
  codesign --force --timestamp --sign "$IDENTITY" "$DMG"

  echo ">> notarizing DMG"
  xcrun notarytool submit "$DMG" --keychain-profile "$PROFILE" --wait

  echo ">> stapling DMG + app"
  xcrun stapler staple "$DMG"
  xcrun stapler staple "$APP"

  echo ">> regenerating updater tarball from stapled app"
  local TDIR="$OUT/tar-$ARCHLABEL"
  rm -rf "$TDIR"; mkdir -p "$TDIR"
  cp -R "$APP" "$TDIR/Copyosity.app"
  # COPYFILE_DISABLE + --no-mac-metadata/--no-xattrs: macOS tar otherwise embeds
  # AppleDouble `._*` entries that the updater's Rust unpacker chokes on.
  ( cd "$TDIR" && COPYFILE_DISABLE=1 tar --no-mac-metadata --no-xattrs -czf "$OUT/$TARBALL" Copyosity.app )
  rm -rf "$TDIR"
  npx tauri signer sign -k "$KEY" -p "" "$OUT/$TARBALL"

  echo ">> $ARCHLABEL done: $DMG , $OUT/$TARBALL"
}

finalize_arch "src-tauri/target/release/bundle/macos/Copyosity.app" "aarch64" "Copyosity_aarch64.app.tar.gz"
finalize_arch "src-tauri/target/x86_64-apple-darwin/release/bundle/macos/Copyosity.app" "x64" "Copyosity_x64.app.tar.gz"

echo "ALL DONE — artifacts in $OUT"
ls -la "$OUT"
