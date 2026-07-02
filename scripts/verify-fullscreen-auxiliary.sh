#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/src-tauri/target/debug/copyosity"
SWIFT="$ROOT/scripts/verify-fullscreen-auxiliary.swift"

if [[ ! -x "$SWIFT" ]]; then
  chmod +x "$SWIFT"
fi

if [[ ! -x "$BIN" ]]; then
  echo "building debug copyosity..."
  bash "$ROOT/scripts/run-rust.sh" 'cargo build --manifest-path src-tauri/Cargo.toml'
fi

if ! pgrep -x copyosity >/dev/null; then
  echo "starting copyosity..."
  "$BIN" >/tmp/copyosity-verify.log 2>&1 &
  COPYOSITY_PID=$!
  trap 'kill "$COPYOSITY_PID" 2>/dev/null || true' EXIT
  sleep 4
else
  echo "copyosity already running"
fi

echo "opening Settings via tray menu..."
osascript <<'EOF' || true
tell application "System Events"
  repeat with proc in (every process whose background only is false)
    if name of proc contains "copyosity" or name of proc contains "Copyosity" then
      tell proc
        try
          click menu item "Settings" of menu 1 of menu bar item 1 of menu bar 1
          exit repeat
        end try
      end tell
    end if
  end repeat
end tell
EOF

sleep 2
swift "$SWIFT"
