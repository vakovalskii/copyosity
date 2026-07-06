#!/usr/bin/env bash
# Smoke test for `make dev` / tauri dev: tray must work on 1st, 2nd, and 3rd click while
# Vite serves http://localhost:1420 (same path as manual `make dev`).
# Agent guardrail: docs/architecture/macos-tray-menu.md — do not change grep target without updating doc.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=lib/tray-click-check.sh
source "$ROOT/scripts/lib/tray-click-check.sh"
LOG="/tmp/copyosity-tray-dev-verify.log"
TIMEOUT_SEC="${COPYOSITY_TRAY_DEV_VERIFY_TIMEOUT:-120}"

pkill -x copyosity 2>/dev/null || true
sleep 1

: >"$LOG"

echo "starting tauri dev (same as make dev)..."
(
	cd "$ROOT"
	COPYOSITY_TRAY_DEBUG=1 env -u npm_config_devdir npm run tauri dev
) >>"$LOG" 2>&1 &
TAURI_PID=$!
VITE_PID=""

cleanup() {
	kill "$TAURI_PID" 2>/dev/null || true
	pkill -x copyosity 2>/dev/null || true
	if [[ -n "$VITE_PID" ]]; then
		kill "$VITE_PID" 2>/dev/null || true
	fi
}
trap cleanup EXIT

deadline=$((SECONDS + TIMEOUT_SEC))
while ((SECONDS < deadline)); do
	tray_ready=0
	vite_ready=0
	grep -q 'startup: hidden main + deferred tray popup ready' "$LOG" && tray_ready=1
	curl -sf "http://localhost:1420" >/dev/null 2>&1 && vite_ready=1
	if ((tray_ready && vite_ready)); then
		VITE_PID="$(lsof -ti tcp:1420 -sTCP:LISTEN 2>/dev/null | head -1 || true)"
		break
	fi
	if ! kill -0 "$TAURI_PID" 2>/dev/null; then
		echo "tauri dev exited early; log:"
		tail -40 "$LOG"
		exit 1
	fi
	sleep 0.25
done

if ! grep -q 'startup: hidden main + deferred tray popup ready' "$LOG"; then
	echo "FAIL: tauri dev never reached tray-ready state"
	tail -40 "$LOG"
	exit 1
fi
if ! curl -sf "http://localhost:1420" >/dev/null 2>&1; then
	echo "FAIL: vite devUrl never became ready on :1420"
	tail -40 "$LOG"
	exit 1
fi
echo "OK: tauri dev tray ready + vite on :1420"

for attempt in 1 2 3 4 5; do
	echo "clicking tray menu in dev (attempt $attempt)..."
	result="$(click_tray_and_check)"
	case "$result" in
	open) echo "OK: dev tray menu stayed open on attempt $attempt" ;;
	blink)
		echo "FAIL: dev tray menu blinked on attempt $attempt"
		grep '\[tray\]' "$LOG" || tail -30 "$LOG"
		exit 1
		;;
	*)
		echo "WARN: could not automate dev tray click (result=$result)"
		echo "Grant Accessibility to Terminal/Cursor (System Events) and retry."
		exit 2
		;;
	esac
	# 0.1 s tight loop for attempts 1-3; 0.3 s to let AppKit settle for 4-5
	if ((attempt < 4)); then
		sleep 0.1
	else
		sleep 0.3
	fi
done

echo "verify-tray-dev: passed (5 clicks under tauri dev)"
