//! macOS window/panel presentation — float above fullscreen apps on all Spaces.

use crate::palette_window::{
    center_in_work_area, is_dot_logical_size, window_center, PALETTE_DOT_SIZE, PALETTE_MIN_HEIGHT,
    PALETTE_MIN_WIDTH,
};

/// Same level as the main overlay panel (`NSMainMenuWindowLevel`).
#[cfg(target_os = "macos")]
pub const FULLSCREEN_AUXILIARY_LEVEL: i64 = 24;

#[cfg(target_os = "macos")]
pub fn fullscreen_auxiliary_collection_behavior() -> objc2_app_kit::NSWindowCollectionBehavior {
    use tauri_nspanel::CollectionBehavior;

    CollectionBehavior::new()
        .can_join_all_spaces()
        .full_screen_auxiliary()
        .into()
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

/// Pure edge-snap geometry: given a window rect (x,y,w,h) and a work-area rect
/// (ax,ay,aw,ah) in the same coordinate space, return the position snapped to any
/// work-area edge the window is within `threshold` of. All values in physical px.
#[cfg(any(target_os = "macos", test))]
pub fn snapped_position(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    ax: i32,
    ay: i32,
    aw: i32,
    ah: i32,
    threshold: i32,
) -> (i32, i32) {
    let right = ax + aw;
    let bottom = ay + ah;
    let mut nx = x;
    let mut ny = y;
    if (x - ax).abs() <= threshold {
        nx = ax;
    } else if ((x + w) - right).abs() <= threshold {
        nx = right - w;
    }
    if (y - ay).abs() <= threshold {
        ny = ay;
    } else if ((y + h) - bottom).abs() <= threshold {
        ny = bottom - h;
    }
    (nx, ny)
}

/// Snap `win` to the nearest work-area edge when it was dragged within
/// `threshold_logical` px of one. No-op when already away from every edge.
#[cfg(target_os = "macos")]
pub fn snap_window_to_edges(win: &tauri::WebviewWindow, threshold_logical: f64) {
    use tauri::PhysicalPosition;

    let Some(monitor) = monitor_for_window(win) else {
        return;
    };
    let Ok(pos) = win.outer_position() else {
        return;
    };
    let Ok(size) = win.outer_size() else {
        return;
    };
    let scale = monitor.scale_factor();
    let threshold = (threshold_logical * scale).round() as i32;
    let wa = monitor.work_area();
    let (nx, ny) = snapped_position(
        pos.x,
        pos.y,
        size.width as i32,
        size.height as i32,
        wa.position.x,
        wa.position.y,
        wa.size.width as i32,
        wa.size.height as i32,
        threshold,
    );
    if nx != pos.x || ny != pos.y {
        let _ = win.set_position(PhysicalPosition::new(nx, ny));
    }
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
    fn snap_pulls_to_left_and_top_edges_within_threshold() {
        // window near the top-left of a 1440x900 work area at (0,0)
        assert_eq!(
            snapped_position(10, 8, 400, 300, 0, 0, 1440, 900, 24),
            (0, 0)
        );
    }

    #[test]
    fn snap_pulls_to_right_and_bottom_edges() {
        // right edge: x+w = 1420+400 = 1820, work right = 1440 → too far, no snap on x
        // put it 10px shy of the right/bottom edges instead
        assert_eq!(
            snapped_position(1030, 590, 400, 300, 0, 0, 1440, 900, 24),
            (1040, 600)
        );
    }

    #[test]
    fn snap_leaves_centered_window_untouched() {
        assert_eq!(
            snapped_position(500, 300, 400, 300, 0, 0, 1440, 900, 24),
            (500, 300)
        );
    }

    #[test]
    fn snap_respects_non_zero_work_area_origin() {
        // work area starts at (0,25) (menu bar); a window 5px below snaps to y=25
        assert_eq!(
            snapped_position(500, 29, 400, 300, 0, 25, 1440, 875, 24),
            (500, 25)
        );
    }
}
