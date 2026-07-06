#!/usr/bin/env bash
# Shared tray-click automation helper for verify-tray-*.sh scripts.
# Agent guardrail: docs/architecture/macos-tray-menu.md §4 — smoke targets and timing.
#
# Exports: click_tray_and_check
#   stdout: "open"  — menu appeared and stayed open (blink-free click)
#           "blink" — menu opened then closed immediately (regression)
#           any other string — automation unavailable (caller should exit 2)
#
# Requirements: Accessibility for System Events must be granted to the calling
# terminal or IDE. Grant in System Settings → Privacy & Security → Accessibility.

# Check the macOS activation policy of the copyosity process.
# stdout: "accessory" | "regular" | "error: ..." (caller should exit 2 on error)
#
# Implementation: `background only` of a System Events process is true for
# Accessory/LSUIElement apps (no Dock icon) and false for Regular apps.
# Agent guardrail: docs/architecture/macos-tray-menu.md §9 — startup must be Accessory;
# Settings promotes to Regular; hide restores to Accessory via maybe_restore_accessory.
check_activation_policy() {
	local result
	result="$(osascript 2>/dev/null <<'APPLESCRIPT'
tell application "System Events"
	try
		set proc to first process whose name is "copyosity"
		if background only of proc then
			return "accessory"
		else
			return "regular"
		end if
	on error errMsg number errNum
		return "error(" & errNum & "): " & errMsg
	end try
end tell
APPLESCRIPT
	)"
	printf '%s' "${result:-no-output}"
}

click_tray_and_check() {
	local result
	result="$(osascript 2>/dev/null <<'APPLESCRIPT'
tell application "System Events"
	try
		tell process "copyosity"
			-- menu bar 2 is the extra (status-bar) menu bar; item 1 = our status item
			set mbi to menu bar item 1 of menu bar 2
			click mbi
			delay 0.4
			-- If the menu opened and stayed, menu 1 of mbi will exist
			if exists menu 1 of mbi then
				-- dismiss cleanly before returning
				key code 53 -- Escape
				delay 0.1
				return "open"
			else
				return "blink"
			end if
		end tell
	on error errMsg number errNum
		return "error(" & errNum & "): " & errMsg
	end try
end tell
APPLESCRIPT
	)"
	printf '%s' "${result:-no-output}"
}
