#!/usr/bin/env bash
# Generate the Tauri updater manifest (latest.json) from the built macOS updater
# artifact. Upload the resulting latest.json to the GitHub release so installed
# copies can auto-update. Optionally pass release notes as $1.
#
# Prereq: `npm run tauri build` was run with TAURI_SIGNING_PRIVATE_KEY set, so
# src-tauri/target/release/bundle/macos/*.app.tar.gz(.sig) exist.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(python3 -c "import json;print(json.load(open('src-tauri/tauri.conf.json'))['version'])")
NOTES="${1:-Copyosity $VERSION}"
BASE="https://github.com/vakovalskii/copyosity/releases/download/v$VERSION"
MAC_DIR="src-tauri/target/release/bundle/macos"

SIG_FILE=$(ls "$MAC_DIR"/*.app.tar.gz.sig 2>/dev/null | head -n1 || true)
TARGZ_FILE=$(ls "$MAC_DIR"/*.app.tar.gz 2>/dev/null | head -n1 || true)
if [ -z "$SIG_FILE" ] || [ -z "$TARGZ_FILE" ]; then
  echo "ERROR: no updater artifacts in $MAC_DIR (build with TAURI_SIGNING_PRIVATE_KEY set)" >&2
  exit 1
fi
MAC_SIG=$(cat "$SIG_FILE")
TARGZ_NAME=$(basename "$TARGZ_FILE")

python3 - "$VERSION" "$NOTES" "$MAC_SIG" "$BASE/$TARGZ_NAME" > latest.json <<'PY'
import json, sys
version, notes, sig, url = sys.argv[1:5]
print(json.dumps({
    "version": version,
    "notes": notes,
    "platforms": {
        "darwin-aarch64": {"signature": sig, "url": url},
    },
}, indent=2))
PY

echo "wrote latest.json for v$VERSION -> $TARGZ_NAME"
