#!/usr/bin/env bash
# Smoke test: tray menu stays open on 1st, 2nd, and 3rd click (macOS GUI + Accessibility).
# Agent guardrail: docs/architecture/macos-tray-menu.md — startup log and scheme must match.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=lib/tray-click-check.sh
source "$ROOT/scripts/lib/tray-click-check.sh"
# shellcheck source=env-rust.sh
source "$ROOT/scripts/env-rust.sh"
LOG="/tmp/copyosity-tray-verify.log"
TIMEOUT_SEC="${COPYOSITY_TRAY_VERIFY_TIMEOUT:-25}"

echo "building debug copyosity..."
bash "$ROOT/scripts/run-rust.sh" 'cargo build'

TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/src-tauri/target}"
BIN="$TARGET_DIR/debug/copyosity"
if [[ ! -x "$BIN" ]]; then
	echo "missing binary at $BIN" >&2
	exit 1
fi

pkill -x copyosity 2>/dev/null || true
sleep 1

: >"$LOG"
echo "starting copyosity (tray debug)..."
COPYOSITY_TRAY_DEBUG=1 "$BIN" >>"$LOG" 2>&1 &
APP_PID=$!
trap 'kill "$APP_PID" 2>/dev/null || true; pkill -x copyosity 2>/dev/null || true' EXIT

deadline=$((SECONDS + TIMEOUT_SEC))
while ((SECONDS < deadline)); do
	if grep -q 'startup: hidden main + deferred tray popup ready' "$LOG"; then
		break
	fi
	if ! kill -0 "$APP_PID" 2>/dev/null; then
		echo "copyosity exited early; log:"
		cat "$LOG"
		exit 1
	fi
	sleep 0.25
done

if ! grep -q 'startup: hidden main + deferred tray popup ready' "$LOG"; then
	echo "FAIL: expected tray startup log"
	cat "$LOG"
	exit 1
fi
echo "OK: hidden main + deferred tray popup ready"

sleep 0.2

# §9 verify activation-policy starts as Accessory (no Dock icon, background-only)
policy="$(check_activation_policy)"
case "$policy" in
accessory)
	echo "OK: activation-policy is Accessory at startup"
	;;
regular)
	echo "FAIL: activation-policy is Regular at startup (should be Accessory)"
	exit 1
	;;
*)
	echo "WARN: could not check activation-policy (result=$policy)"
	echo "Grant Accessibility to Terminal/Cursor (System Events) and retry."
	exit 2
	;;
esac

for attempt in 1 2 3 4 5; do
	echo "clicking tray menu (attempt $attempt)..."
	result="$(click_tray_and_check)"
	case "$result" in
	open)
		echo "OK: tray menu stayed open on attempt $attempt"
		;;
	blink)
		echo "FAIL: tray menu blinked on attempt $attempt"
		grep '\[tray\]' "$LOG" || tail -20 "$LOG"
		exit 1
		;;
	*)
		echo "WARN: could not automate tray click (result=$result)"
		echo "Grant Accessibility to Terminal/Cursor (System Events) and retry."
		exit 2
		;;
	esac
	# 0.1 s between clicks 1-3 (tight-loop regression check), then 0.3 s to let AppKit settle
	if ((attempt < 4)); then
		sleep 0.1
	else
		sleep 0.3
	fi
done

echo "verify-tray-startup: passed (5 clicks)"
