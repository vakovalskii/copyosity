//! macOS tray menu: deferred popup avoids AppKit activation/menu races in Accessory apps.
//!
//! **Agent guardrail:** read [`docs/architecture/macos-tray-menu.md`](../../docs/architecture/macos-tray-menu.md)
//! before editing. Gate: `make verify-tray`.
//!
//! This module is **required** on macOS — do not delete or replace with plain
//! `show_menu_on_left_click(true)`. Partial fixes break either the 1st or 2nd+ click.
//!
//! ## Why deferred popup exists
//!
//! Accessory (menu-bar-only) apps blink when `popUpStatusItemMenu` runs in the same event cycle
//! as activation. Deferring one tick matches long-standing LSUIElement workarounds (warmup
//! activate now, popup on next run-loop turn). Verified 5 clicks in `tauri dev`.
//!
//! ## Known regressions from "cleanup" passes (do not repeat)
//!
//! - Replacing `activateIgnoringOtherApps(true)` in warmup with `activate()` or
//!   `activateIgnoringOtherApps(false)` — breaks the first-click menu popup.
//! - Removing the `highlight(true)` re-assert inside the deferred popup — `mouseUp` clears
//!   highlight before our popup runs, so the status item looks unpressed while the menu is open.
//! - Switching to `show_menu_on_left_click(true)` (plain, no defer) — fixes 1st click but
//!   regresses the 2nd/3rd click (`tray-icon` `performClick` race even with patch).

#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "macos")]
use objc2::MainThreadMarker;
#[cfg(target_os = "macos")]
use objc2_app_kit::NSApplication;

/// Debounce guard: skip scheduling a second deferred popup if one is already in flight.
/// Cleared at the start of the deferred closure. Prevents a rapid double-click from
/// stacking two tasks and producing a stale `highlight(false)` while the menu is still open.
#[cfg(target_os = "macos")]
static TRAY_POPUP_PENDING: AtomicBool = AtomicBool::new(false);

/// Warm up AppKit so the first tray click opens the menu without blinking.
///
/// Must use `activateIgnoringOtherApps(true)` — `activate()` or `false` break the first click.
/// `activateIgnoringOtherApps` was deprecated in macOS 14; no replacement achieves the same
/// one-time warm-up effect for Accessory apps. Suppressed until tray-icon exposes a proper
/// activation API.
#[cfg(target_os = "macos")]
#[allow(deprecated)]
pub fn warmup_app_for_status_item_menu() {
    // §5 docs/architecture/macos-tray-menu.md — must be activateIgnoringOtherApps(true)
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    app.activateIgnoringOtherApps(true);
}

#[cfg(target_os = "macos")]
pub fn set_tray_highlight(tray: &tauri::tray::TrayIcon<tauri::Wry>, highlighted: bool) {
    let _ = tray.with_inner_tray_icon(move |inner| {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let Some(status_item) = inner.ns_status_item() else {
            return;
        };
        let Some(button) = status_item.button(mtm) else {
            return;
        };
        button.highlight(highlighted);
    });
}

#[cfg(target_os = "macos")]
pub fn schedule_tray_menu_popup(tray: tauri::tray::TrayIcon<tauri::Wry>) {
    // Debounce: skip if a popup task is already scheduled (rapid double-click guard).
    if TRAY_POPUP_PENDING.swap(true, Ordering::AcqRel) {
        return;
    }

    let app = tray.app_handle().clone();
    tauri::async_runtime::spawn(async move {
        // §4 One event-cycle defer — imperceptible to users, enough for AppKit.
        // Do not call show_menu() synchronously in the click handler.
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        let _ = app.run_on_main_thread(move || {
            TRAY_POPUP_PENDING.store(false, Ordering::Release);
            let _ = tray.with_inner_tray_icon(|inner| {
                let Some(mtm) = MainThreadMarker::new() else {
                    inner.show_menu();
                    return;
                };
                let Some(status_item) = inner.ns_status_item() else {
                    inner.show_menu();
                    return;
                };
                let Some(button) = status_item.button(mtm) else {
                    inner.show_menu();
                    return;
                };
                // mouseUp clears highlight before our deferred popup; re-assert for the modal menu.
                button.highlight(true);
                inner.show_menu();
                button.highlight(false);
            });
        });
    });
}

#[cfg(not(target_os = "macos"))]
pub fn set_tray_highlight(_tray: &tauri::tray::TrayIcon<tauri::Wry>, _highlighted: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn schedule_tray_menu_popup(_tray: tauri::tray::TrayIcon<tauri::Wry>) {}
