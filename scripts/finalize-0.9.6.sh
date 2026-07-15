#!/bin/bash
set -euo pipefail

cd /Users/v.kovalskii/copyosity

IDENTITY="Developer ID Application: Valeriy Kovalsky (A933C2TJXU)"
PROFILE="copyosity"
KEY="$(cat .tauri/copyosity-updater.key)"
OUT="/private/tmp/claude-502/-Users-v-kovalskii-copyosity/22612add-21ac-4177-a118-4c828696e9cd/scratchpad/release-0.9.6"
mkdir -p "$OUT"

finalize_arch () {
  local APP="$1" ARCHLABEL="$2" TARBALL="$3"
  local NAME="Copyosity-${ARCHLABEL}"
  local STAGE="$OUT/stage-$ARCHLABEL"
  local DMG="$OUT/${NAME}_0.9.6.dmg"

  echo "########## $ARCHLABEL ##########"
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
  # AppleDouble `._*` entries (from codesign/staple xattrs) that the updater's
  # Rust tar unpacker chokes on ("failed to unpack ._Copyosity.app") — which
  # silently broke auto-update install. Keep the tarball free of them.
  ( cd "$TDIR" && COPYFILE_DISABLE=1 tar --no-mac-metadata --no-xattrs -czf "$OUT/$TARBALL" Copyosity.app )
  rm -rf "$TDIR"
  npx tauri signer sign -k "$KEY" -p "" "$OUT/$TARBALL"

  echo ">> $ARCHLABEL done: $DMG , $OUT/$TARBALL"
}

finalize_arch "src-tauri/target/release/bundle/macos/Copyosity.app" "aarch64" "Copyosity_aarch64.app.tar.gz"
finalize_arch "src-tauri/target/x86_64-apple-darwin/release/bundle/macos/Copyosity.app" "x64" "Copyosity_x64.app.tar.gz"

echo "ALL DONE"
ls -la "$OUT"
