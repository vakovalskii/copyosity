//! Overlay dismiss: click-outside on macOS without closing on Space switch.
//!
//! Product behavior (intentional):
//! - Outside click dismisses the panel when the global mouse monitor is available.
//! - Switching Space (gesture) suppresses outside-click dismiss briefly so the panel stays open.
//! - When the mouse monitor is active, Cmd+Tab / focus loss does **not** dismiss — users can
//!   move to another Space or app and keep the overlay open to paste into a different target.

/// Grace after panel show during which outside-click dismiss is ignored.
#[cfg(any(target_os = "macos", test))]
pub(crate) const SHOW_DISMISS_GRACE_MS: u64 = 500;

/// Pure geometry helper (Cocoa screen coordinates, bottom-left origin).
#[cfg(any(target_os = "macos", test))]
pub(crate) fn point_in_screen_rect(
    point_x: f64,
    point_y: f64,
    origin_x: f64,
    origin_y: f64,
    width: f64,
    height: f64,
) -> bool {
    point_x >= origin_x
        && point_x <= origin_x + width
        && point_y >= origin_y
        && point_y <= origin_y + height
}

/// Returns true while outside-click dismiss should be suppressed (e.g. after a Space change).
#[cfg(any(target_os = "macos", test))]
pub(crate) fn dismiss_suppressed(now_ms: u64, suppress_until_ms: u64) -> bool {
    now_ms < suppress_until_ms
}

/// Returns true during the short grace window right after the panel is shown.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn within_show_grace(now_ms: u64, last_show_ms: u64, grace_ms: u64) -> bool {
    last_show_ms == 0 || now_ms.saturating_sub(last_show_ms) <= grace_ms
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ptr::{self, NonNull};
    use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{
        NSEvent, NSEventMask, NSWindow, NSWorkspaceActiveSpaceDidChangeNotification,
    };
    use objc2_foundation::{NSNotification, NSNotificationCenter, NSPoint, NSRect};
    use tauri::{AppHandle, Manager};

    use super::{
        dismiss_suppressed, point_in_screen_rect, within_show_grace, SHOW_DISMISS_GRACE_MS,
    };
    use crate::{animated_hide_panel, main_panel_visible, now_ms, LAST_SHOW_MS};

    const SPACE_DISMISS_GRACE_MS: u64 = 900;
    const FOCUS_DISMISS_DELAY_MS: u64 = 450;

    static SUPPRESS_OUTSIDE_DISMISS_UNTIL_MS: AtomicU64 = AtomicU64::new(0);
    static MOUSE_MONITOR_PTR: AtomicPtr<AnyObject> = AtomicPtr::new(ptr::null_mut());
    static MOUSE_MONITOR_ACTIVE: AtomicBool = AtomicBool::new(false);
    static SPACE_GUARD_INSTALLED: AtomicBool = AtomicBool::new(false);

    pub fn install_overlay_dismiss_guards() {
        if SPACE_GUARD_INSTALLED.swap(true, Ordering::AcqRel) {
            return;
        }

        let center = NSNotificationCenter::defaultCenter();
        let block = RcBlock::new(|_notification: NonNull<NSNotification>| {
            SUPPRESS_OUTSIDE_DISMISS_UNTIL_MS.store(
                now_ms().saturating_add(SPACE_DISMISS_GRACE_MS),
                Ordering::Release,
            );
        });
        let observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWorkspaceActiveSpaceDidChangeNotification),
                None,
                None,
                &block,
            )
        };
        // Intentionally leaked for app lifetime — observer must outlive all Space changes.
        std::mem::forget(observer);
        std::mem::forget(block);
    }

    fn suppress_active() -> bool {
        dismiss_suppressed(
            now_ms(),
            SUPPRESS_OUTSIDE_DISMISS_UNTIL_MS.load(Ordering::Acquire),
        )
    }

    pub fn set_outside_click_dismiss(app: &AppHandle, enabled: bool) {
        if enabled {
            start_mouse_monitor(app.clone());
        } else {
            stop_mouse_monitor();
        }
    }

    fn stop_mouse_monitor() {
        let ptr = MOUSE_MONITOR_PTR.swap(ptr::null_mut(), Ordering::AcqRel);
        MOUSE_MONITOR_ACTIVE.store(false, Ordering::Release);
        if ptr.is_null() {
            return;
        }
        let monitor = unsafe { Retained::from_raw(ptr) };
        if let Some(monitor) = monitor {
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
    }

    fn start_mouse_monitor(app: AppHandle) {
        stop_mouse_monitor();

        let block = RcBlock::new(move |_event: NonNull<NSEvent>| {
            if suppress_active() {
                return;
            }
            let last_show = LAST_SHOW_MS.load(Ordering::Relaxed);
            if within_show_grace(now_ms(), last_show, SHOW_DISMISS_GRACE_MS) {
                return;
            }
            if !main_panel_visible(&app) {
                return;
            }
            let Some(window) = app.get_webview_window("main") else {
                return;
            };
            let Ok(ns_ptr) = window.ns_window() else {
                return;
            };
            if ns_ptr.is_null() {
                return;
            }
            let frame = unsafe { (*ns_ptr.cast::<NSWindow>()).frame() };
            let point = NSEvent::mouseLocation();
            if point_in_rect(point, frame) {
                return;
            }
            animated_hide_panel(&app);
        });

        if let Some(monitor) = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
            NSEventMask::LeftMouseDown,
            &block,
        ) {
            let raw = Retained::into_raw(monitor);
            MOUSE_MONITOR_PTR.store(raw, Ordering::Release);
            MOUSE_MONITOR_ACTIVE.store(true, Ordering::Release);
        } else {
            eprintln!("[overlay] global mouse monitor unavailable; using focus-loss fallback");
        }
        // Block must outlive the monitor; dropped with the monitor on stop_mouse_monitor.
        std::mem::forget(block);
    }

    fn point_in_rect(point: NSPoint, rect: NSRect) -> bool {
        point_in_screen_rect(
            point.x,
            point.y,
            rect.origin.x,
            rect.origin.y,
            rect.size.width,
            rect.size.height,
        )
    }

    /// Fallback when the global mouse monitor cannot be installed.
    pub fn handle_focus_lost(app: &AppHandle) {
        // Product decision: when the global mouse monitor is active, do not dismiss on focus
        // loss. Users often switch Space (gesture) or app (Cmd+Tab) to paste into another
        // target while keeping the overlay open. Outside-click still dismisses via the monitor.
        if MOUSE_MONITOR_ACTIVE.load(Ordering::Acquire) {
            return;
        }

        let last_show = LAST_SHOW_MS.load(Ordering::Relaxed);
        if within_show_grace(now_ms(), last_show, SHOW_DISMISS_GRACE_MS) {
            return;
        }

        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(FOCUS_DISMISS_DELAY_MS)).await;
            if suppress_active() {
                return;
            }
            if let Some(window) = app.get_webview_window("main") {
                if window.is_focused().unwrap_or(false) {
                    return;
                }
            }
            if main_panel_visible(&app) {
                animated_hide_panel(&app);
            }
        });
    }
}

#[cfg(target_os = "macos")]
pub use imp::*;

#[cfg(not(target_os = "macos"))]
pub fn handle_focus_lost(_app: &tauri::AppHandle) {}

#[cfg(test)]
mod tests {
    use super::{
        dismiss_suppressed, point_in_screen_rect, within_show_grace, SHOW_DISMISS_GRACE_MS,
    };

    #[test]
    fn point_in_screen_rect_inside_and_on_edges() {
        let rect = (100.0, 200.0, 300.0, 150.0); // origin x,y + width,height
        assert!(point_in_screen_rect(
            100.0, 200.0, rect.0, rect.1, rect.2, rect.3
        ));
        assert!(point_in_screen_rect(
            400.0, 350.0, rect.0, rect.1, rect.2, rect.3
        ));
        assert!(point_in_screen_rect(
            250.0, 275.0, rect.0, rect.1, rect.2, rect.3
        ));
    }

    #[test]
    fn point_in_screen_rect_outside() {
        let rect = (100.0, 200.0, 300.0, 150.0);
        assert!(!point_in_screen_rect(
            99.0, 275.0, rect.0, rect.1, rect.2, rect.3
        ));
        assert!(!point_in_screen_rect(
            401.0, 275.0, rect.0, rect.1, rect.2, rect.3
        ));
        assert!(!point_in_screen_rect(
            250.0, 199.0, rect.0, rect.1, rect.2, rect.3
        ));
        assert!(!point_in_screen_rect(
            250.0, 351.0, rect.0, rect.1, rect.2, rect.3
        ));
    }

    #[test]
    fn dismiss_suppressed_respects_deadline() {
        assert!(dismiss_suppressed(500, 900));
        assert!(!dismiss_suppressed(900, 900));
        assert!(!dismiss_suppressed(901, 900));
    }

    #[test]
    fn within_show_grace_blocks_immediate_dismiss() {
        assert!(within_show_grace(1_000, 0, SHOW_DISMISS_GRACE_MS));
        assert!(within_show_grace(1_000, 800, SHOW_DISMISS_GRACE_MS));
        assert!(within_show_grace(1_300, 800, SHOW_DISMISS_GRACE_MS));
        assert!(!within_show_grace(1_301, 800, SHOW_DISMISS_GRACE_MS));
    }
}
