//! macOS window/panel presentation — float above fullscreen apps on all Spaces.
//!
//! Agent guardrail: docs/architecture/macos-tray-menu.md §3, §7 — hidden overlay uses level 3
//! until shown; fullscreen auxiliary (24) only when overlay/settings are presented.

use crate::palette_window::{
    center_in_work_area, is_dot_logical_size, window_center, PALETTE_DOT_SIZE, PALETTE_MIN_HEIGHT,
    PALETTE_MIN_WIDTH,
};

/// Same level as the main overlay panel (`NSMainMenuWindowLevel`).
#[cfg(target_os = "macos")]
pub const FULLSCREEN_AUXILIARY_LEVEL: i64 = 24;

/// Hidden auxiliary panels stay below status-bar menu popups (first tray click).
/// Agent guardrail: docs/architecture/macos-tray-menu.md — do not raise hidden main to 24 at startup.
#[cfg(target_os = "macos")]
pub const HIDDEN_AUXILIARY_LEVEL: i64 = 3;

#[cfg(target_os = "macos")]
pub fn fullscreen_auxiliary_collection_behavior() -> objc2_app_kit::NSWindowCollectionBehavior {
    use tauri_nspanel::CollectionBehavior;

    CollectionBehavior::new()
        .can_join_all_spaces()
        .full_screen_auxiliary()
        .into()
}

#[cfg(target_os = "macos")]
pub fn configure_hidden_auxiliary_panel(panel: &dyn tauri_nspanel::Panel) {
    // §3 docs/architecture/macos-tray-menu.md — level 3 while hidden (after overlay hide)
    panel.set_level(HIDDEN_AUXILIARY_LEVEL);
    panel.set_collection_behavior(fullscreen_auxiliary_collection_behavior());
    panel.set_hides_on_deactivate(false);
    panel.set_floating_panel(true);
}

#[cfg(target_os = "macos")]
pub fn configure_fullscreen_auxiliary_panel(panel: &dyn tauri_nspanel::Panel) {
    panel.set_level(FULLSCREEN_AUXILIARY_LEVEL);
    panel.set_collection_behavior(fullscreen_auxiliary_collection_behavior());
    panel.set_hides_on_deactivate(false);
    panel.set_floating_panel(true);
}

#[cfg(target_os = "macos")]
pub fn present_fullscreen_auxiliary_panel(panel: &dyn tauri_nspanel::Panel) {
    configure_fullscreen_auxiliary_panel(panel);
    panel.show_and_make_key();
}

#[cfg(target_os = "macos")]
pub fn apply_hidden_auxiliary_webview(window: &tauri::WebviewWindow) {
    // §3 Called from ensure_main_overlay_window at startup — keeps main below tray menu popups.
    use objc2_app_kit::NSWindow;

    let Ok(raw) = window.ns_window() else {
        return;
    };
    unsafe {
        let ns_window = &*raw.cast::<NSWindow>();
        ns_window.setCollectionBehavior(fullscreen_auxiliary_collection_behavior());
        ns_window.setLevel(HIDDEN_AUXILIARY_LEVEL as isize);
        ns_window.setHidesOnDeactivate(false);
    }
}

#[cfg(target_os = "macos")]
pub fn apply_fullscreen_auxiliary_webview(window: &tauri::WebviewWindow) {
    use objc2_app_kit::NSWindow;

    let Ok(raw) = window.ns_window() else {
        return;
    };
    unsafe {
        let ns_window = &*raw.cast::<NSWindow>();
        ns_window.setCollectionBehavior(fullscreen_auxiliary_collection_behavior());
        ns_window.setLevel(FULLSCREEN_AUXILIARY_LEVEL as isize);
        ns_window.setHidesOnDeactivate(false);
    }
}

#[cfg(target_os = "macos")]
pub fn present_fullscreen_auxiliary_webview(window: &tauri::WebviewWindow) {
    use objc2_app_kit::NSWindow;

    apply_fullscreen_auxiliary_webview(window);
    let Ok(raw) = window.ns_window() else {
        return;
    };
    unsafe {
        let ns_window = &*raw.cast::<NSWindow>();
        ns_window.orderFrontRegardless();
        ns_window.makeKeyAndOrderFront(None);
    }
}

#[cfg(target_os = "macos")]
fn palette_run_on_main<R: Send + 'static>(
    app: &tauri::AppHandle,
    f: impl FnOnce() -> R + Send + 'static,
) -> Result<R, String> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = tx.send(f());
    })
    .map_err(|e| format!("main_thread: {e}"))?;
    rx.recv().map_err(|_| "main_thread: closed".to_string())
}

#[cfg(target_os = "macos")]
fn palette_win_op<T>(result: Result<T, impl std::fmt::Display>, op: &str) -> Result<T, String> {
    result.map_err(|e| format!("{op}: {e}"))
}

#[cfg(target_os = "macos")]
pub fn palette_set_dot_mode(
    app: &tauri::AppHandle,
    minimized: bool,
    restore_width: f64,
    restore_height: f64,
) -> Result<(), String> {
    use tauri::{LogicalSize, Manager};
    use tauri_nspanel::ManagerExt;

    let app = app.clone();
    palette_run_on_main(&app.clone(), move || {
        let Some(win) = app.get_webview_window("command_palette") else {
            return Err("command palette window not found".to_string());
        };
        let panel = app.get_webview_panel("command_palette").ok();
        if minimized {
            let dot = LogicalSize::new(PALETTE_DOT_SIZE, PALETTE_DOT_SIZE);
            palette_win_op(win.set_min_size(Some(dot)), "set_min_size")?;
            palette_win_op(win.set_size(dot), "set_size")?;
            palette_win_op(win.set_resizable(false), "set_resizable")?;
            if let Some(panel) = panel.as_ref() {
                panel.set_content_size(PALETTE_DOT_SIZE, PALETTE_DOT_SIZE);
                panel.set_movable_by_window_background(true);
            }
            // AppKit caches the drop shadow shape from the window's previous
            // (much larger) frame; without invalidating it, a leftover
            // rectangular shadow lingers around the small round dot.
            set_window_shadow(&win, false);
        } else {
            // Capture the screen the dot is on *before* resizing — after
            // set_size the window spans a different frame and current_monitor
            // can jump to another display.
            let monitor = monitor_for_window(&win);
            palette_win_op(
                win.set_min_size(Some(LogicalSize::new(
                    PALETTE_MIN_WIDTH,
                    PALETTE_MIN_HEIGHT,
                ))),
                "set_min_size",
            )?;
            palette_win_op(
                win.set_size(LogicalSize::new(restore_width, restore_height)),
                "set_size",
            )?;
            palette_win_op(win.set_resizable(true), "set_resizable")?;
            if let Some(panel) = panel.as_ref() {
                panel.set_movable_by_window_background(false);
            }
            if let Some(monitor) = monitor {
                center_window_on_monitor(&win, &monitor, restore_width, restore_height)?;
            }
            set_window_shadow(&win, true);
        }
        Ok(())
    })?
}

/// Toggle the native window drop shadow and force AppKit to recompute it
/// immediately. Must run after size changes, otherwise the shadow shape from
/// the previous frame can persist as a visible ghost outline.
#[cfg(target_os = "macos")]
fn set_window_shadow(win: &tauri::WebviewWindow, has_shadow: bool) {
    use objc2_app_kit::NSWindow;

    let Ok(raw) = win.ns_window() else {
        return;
    };
    unsafe {
        let ns_window = &*raw.cast::<NSWindow>();
        ns_window.setHasShadow(has_shadow);
        ns_window.invalidateShadow();
    }
}

/// Monitor that contains `win` (where the dot was dragged). Never hops to the
/// primary display — restore centers the agent overlay on this screen.
#[cfg(target_os = "macos")]
fn monitor_for_window(win: &tauri::WebviewWindow) -> Option<tauri::Monitor> {
    if let Ok(Some(m)) = win.current_monitor() {
        return Some(m);
    }
    let pos = win.outer_position().ok()?;
    let size = win.outer_size().ok()?;
    let (cx, cy) = window_center(
        f64::from(pos.x),
        f64::from(pos.y),
        f64::from(size.width),
        f64::from(size.height),
    );
    let monitors = win.available_monitors().ok()?;
    for m in monitors {
        let mp = m.position();
        let ms = m.size();
        let left = f64::from(mp.x);
        let top = f64::from(mp.y);
        let right = left + f64::from(ms.width);
        let bottom = top + f64::from(ms.height);
        if cx >= left && cx < right && cy >= top && cy < bottom {
            return Some(m);
        }
    }
    win.primary_monitor().ok().flatten()
}

/// Center `win` on `monitor` using known logical restore dimensions (do not
/// read outer_size right after set_size — it can still reflect the old frame).
#[cfg(target_os = "macos")]
fn center_window_on_monitor(
    win: &tauri::WebviewWindow,
    monitor: &tauri::Monitor,
    logical_width: f64,
    logical_height: f64,
) -> Result<(), String> {
    use tauri::PhysicalPosition;

    let scale = monitor.scale_factor();
    let width = (logical_width * scale).round() as i32;
    let height = (logical_height * scale).round() as i32;
    let work_area = monitor.work_area();
    let (x, y) = center_in_work_area(
        work_area.position.x,
        work_area.position.y,
        work_area.size.width,
        work_area.size.height,
        width,
        height,
    );
    palette_win_op(
        win.set_position(PhysicalPosition::new(x, y)),
        "set_position",
    )
}

#[cfg(target_os = "macos")]
pub fn palette_is_dot_mode(app: &tauri::AppHandle) -> Result<bool, String> {
    use tauri::Manager;

    let app = app.clone();
    palette_run_on_main(&app.clone(), move || {
        let win = app
            .get_webview_window("command_palette")
            .ok_or_else(|| "command palette window not found".to_string())?;
        let size = win.inner_size().map_err(|e| e.to_string())?;
        let scale = win.scale_factor().map_err(|e| e.to_string())?;
        let logical_width = f64::from(size.width) / scale;
        let logical_height = f64::from(size.height) / scale;
        Ok(is_dot_logical_size(logical_width, logical_height))
    })?
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use objc2_app_kit::NSWindowCollectionBehavior;

    #[test]
    fn fullscreen_auxiliary_behavior_includes_required_flags() {
        let behavior = fullscreen_auxiliary_collection_behavior();
        assert!(behavior.contains(NSWindowCollectionBehavior::CanJoinAllSpaces));
        assert!(behavior.contains(NSWindowCollectionBehavior::FullScreenAuxiliary));
    }

    #[test]
    fn fullscreen_auxiliary_level_matches_main_menu() {
        assert_eq!(FULLSCREEN_AUXILIARY_LEVEL, 24);
    }

    #[test]
    fn hidden_auxiliary_level_stays_below_status_bar_menus() {
        const {
            assert!(HIDDEN_AUXILIARY_LEVEL < FULLSCREEN_AUXILIARY_LEVEL);
        }
        assert_eq!(HIDDEN_AUXILIARY_LEVEL, 3);
    }
}
