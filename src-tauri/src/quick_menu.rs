//! Native Clipy-style quick menu.
//!
//! A global hotkey pops up a native macOS menu (built with Tauri's cross-platform
//! menu API, which renders as an `NSMenu` on macOS) listing recent clipboard
//! history and saved snippets. Selecting an item pastes it straight into the app
//! that was frontmost when the menu opened — two clicks, no overlay browsing.

/// How many recent history entries the quick menu surfaces at most.
pub const QUICK_MENU_HISTORY_LIMIT: usize = 100;

/// Recent entries shown inline before the rest spill into range submenus.
#[cfg(target_os = "macos")]
const INLINE_COUNT: usize = 9;

/// Number of entries grouped into each "N–M" range submenu.
#[cfg(target_os = "macos")]
const RANGE_SIZE: usize = 20;

/// Max characters shown for a text preview label before it is ellipsised.
#[cfg(target_os = "macos")]
const LABEL_MAX_CHARS: usize = 52;

/// Build and pop up the quick menu at the mouse cursor. No-op off macOS.
#[cfg(target_os = "macos")]
pub fn show(app: &tauri::AppHandle) {
    use std::sync::atomic::Ordering;
    use tauri::Manager;

    // Capture the currently-frontmost app *before* we activate ourselves, so the
    // eventual paste lands back in it.
    let target = crate::frontmost_app_pid().unwrap_or(0);
    if target > 0 && target != std::process::id() as i32 {
        crate::QUICK_MENU_TARGET_PID.store(target, Ordering::Relaxed);
        crate::clipboard_macos::remember_paste_target_for_pid(target);
    } else {
        crate::QUICK_MENU_TARGET_PID.store(0, Ordering::Relaxed);
    }

    let db = app.state::<std::sync::Arc<crate::db::Database>>();
    let entries = db
        .get_entries(
            QUICK_MENU_HISTORY_LIMIT as i64,
            0,
            None,
            false,
            None,
            None,
            None,
            None,
        )
        .unwrap_or_default();
    let folders = db.get_snippet_folders().unwrap_or_default();
    let snippets = db.get_snippets().unwrap_or_default();

    let app = app.clone();
    // Build and present on the main thread; the popup runs a modal loop there.
    let _ = app.clone().run_on_main_thread(move || {
        // popup_menu blocks the main thread; open overlay/agent panels race it and blink closed.
        crate::prepare_for_quick_menu(&app);

        activate_app();
        let menu = match build_menu(&app, &entries, &folders, &snippets) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[quick_menu] failed to build menu: {}", e);
                return;
            }
        };
        if let Ok(win) = crate::ensure_main_overlay_window(&app) {
            let row_count = menu_row_count(&menu);
            if let Err(e) = popup_menu_smart(&win, &menu, row_count) {
                eprintln!("[quick_menu] popup failed: {}", e);
            }
        } else {
            eprintln!("[quick_menu] failed to create main window for menu anchor");
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub fn show(_app: &tauri::AppHandle) {}

#[cfg(target_os = "macos")]
fn build_menu(
    app: &tauri::AppHandle,
    entries: &[crate::db::ClipboardEntry],
    folders: &[crate::db::SnippetFolder],
    snippets: &[crate::db::Snippet],
) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};

    let menu = Menu::new(app)?;

    if entries.is_empty() {
        let empty = MenuItem::with_id(
            app,
            "qm:noop",
            "No clipboard history yet",
            false,
            None::<&str>,
        )?;
        menu.append(&empty)?;
    } else {
        let inline = entries.len().min(INLINE_COUNT);
        for (i, entry) in entries.iter().take(inline).enumerate() {
            // Number key-equivalents 1–9 for the first entries (Clipy-style).
            let accel = (i < 9).then(|| (i + 1).to_string());
            let item = MenuItem::with_id(
                app,
                format!("qm:paste:{}", entry.id),
                entry_label(i + 1, entry),
                true,
                accel.as_deref(),
            )?;
            menu.append(&item)?;
        }

        // Remaining entries spill into "N–M" range submenus.
        let mut start = inline;
        while start < entries.len() {
            let end = (start + RANGE_SIZE).min(entries.len());
            let sub = Submenu::with_id(
                app,
                format!("qm:range:{}", start),
                format!("{}–{}", start + 1, end),
                true,
            )?;
            for (offset, entry) in entries[start..end].iter().enumerate() {
                let item = MenuItem::with_id(
                    app,
                    format!("qm:paste:{}", entry.id),
                    entry_label(start + offset + 1, entry),
                    true,
                    None::<&str>,
                )?;
                sub.append(&item)?;
            }
            menu.append(&sub)?;
            start = end;
        }
    }

    // Snippets section: one submenu per folder.
    if !folders.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        let heading =
            MenuItem::with_id(app, "qm:snippets-heading", "Snippets", false, None::<&str>)?;
        menu.append(&heading)?;
        for folder in folders {
            let sub = Submenu::with_id(
                app,
                format!("qm:folder:{}", folder.id),
                folder.name.clone(),
                true,
            )?;
            let mut any = false;
            for snip in snippets.iter().filter(|s| s.folder_id == folder.id) {
                any = true;
                let item = MenuItem::with_id(
                    app,
                    format!("qm:snip:{}", snip.id),
                    truncate(&collapse_ws(&snip.title)),
                    true,
                    None::<&str>,
                )?;
                sub.append(&item)?;
            }
            if !any {
                let empty = MenuItem::with_id(
                    app,
                    format!("qm:folder-empty:{}", folder.id),
                    "(empty)",
                    false,
                    None::<&str>,
                )?;
                sub.append(&empty)?;
            }
            menu.append(&sub)?;
        }
    }

    // Bottom actions.
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "qm:clear",
        "Clear History",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        "qm:edit-snippets",
        "Edit Snippets…",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        "qm:settings",
        "Settings…",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        "qm:quit",
        "Quit Copyosity",
        true,
        None::<&str>,
    )?)?;

    Ok(menu)
}

/// A "1. preview text" style label for a history entry.
#[cfg(target_os = "macos")]
fn entry_label(index: usize, entry: &crate::db::ClipboardEntry) -> String {
    let body = if entry.content_type == "image" {
        let fmt = entry.image_format.as_deref().unwrap_or("Image");
        match (entry.image_width, entry.image_height) {
            (Some(w), Some(h)) => format!("🖼 {} {}×{}", fmt, w, h),
            _ => format!("🖼 {}", fmt),
        }
    } else {
        let raw = entry.text_content.as_deref().unwrap_or("");
        truncate(&collapse_ws(raw))
    };
    format!("{}. {}", index, body)
}

/// Collapse all runs of whitespace (including newlines) into single spaces.
#[cfg(target_os = "macos")]
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Truncate on a char boundary, appending an ellipsis when shortened.
#[cfg(target_os = "macos")]
fn truncate(s: &str) -> String {
    if s.chars().count() <= LABEL_MAX_CHARS {
        return s.to_string();
    }
    let truncated: String = s.chars().take(LABEL_MAX_CHARS).collect();
    format!("{}…", truncated.trim_end())
}

/// Bring Copyosity to the foreground so the native menu can track input.
#[cfg(target_os = "macos")]
fn popup_menu_smart(
    window: &tauri::WebviewWindow,
    menu: &tauri::menu::Menu<tauri::Wry>,
    row_count: usize,
) -> tauri::Result<()> {
    use objc2_app_kit::NSEvent;
    use tauri::LogicalPosition;

    use crate::quick_menu_position::{estimated_menu_height, popup_top_y, should_flip_menu_up};

    let mouse = NSEvent::mouseLocation();
    let menu_height = estimated_menu_height(row_count);
    let visible = visible_frame_at_point(mouse.x, mouse.y);
    let visible_bottom = visible.origin.y;
    let visible_top = visible.origin.y + visible.size.height;

    if !should_flip_menu_up(mouse.y, menu_height, visible_bottom) {
        return window.popup_menu(menu);
    }

    let top_y = popup_top_y(mouse.y, menu_height, visible_bottom, visible_top);
    let Some(position) = screen_point_to_menu_position(window, mouse.x, top_y) else {
        return window.popup_menu(menu);
    };
    window.popup_menu_at(menu, LogicalPosition::new(position.x, position.y))
}

#[cfg(target_os = "macos")]
fn menu_row_count(menu: &tauri::menu::Menu<tauri::Wry>) -> usize {
    menu.items().map(|items| items.len()).unwrap_or(0)
}

#[cfg(target_os = "macos")]
fn visible_frame_at_point(x: f64, y: f64) -> objc2_foundation::NSRect {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;
    use objc2_foundation::NSPoint;

    let mtm = MainThreadMarker::new().expect("quick menu popup requires main thread");
    let screens = NSScreen::screens(mtm);
    for screen in screens.iter() {
        let frame = screen.visibleFrame();
        if crate::overlay_dismiss::point_in_screen_rect(
            x,
            y,
            frame.origin.x,
            frame.origin.y,
            frame.size.width,
            frame.size.height,
        ) {
            return frame;
        }
    }

    NSScreen::mainScreen(mtm)
        .map(|screen| screen.visibleFrame())
        .unwrap_or_else(|| objc2_foundation::NSRect {
            origin: NSPoint::new(0.0, 0.0),
            size: objc2_foundation::NSSize::new(0.0, 0.0),
        })
}

#[cfg(target_os = "macos")]
fn screen_point_to_menu_position(
    window: &tauri::WebviewWindow,
    screen_x: f64,
    screen_y: f64,
) -> Option<objc2_foundation::NSPoint> {
    use objc2_app_kit::NSWindow;
    use objc2_foundation::NSPoint;

    let raw = window.ns_window().ok()?;
    unsafe {
        let ns_window = &*raw.cast::<NSWindow>();
        let content_view = ns_window.contentView()?;
        let view_frame = content_view.frame();
        let window_point = ns_window.convertPointFromScreen(NSPoint::new(screen_x, screen_y));
        let view_point = content_view.convertPoint_fromView(window_point, None);
        Some(NSPoint::new(
            view_point.x,
            view_frame.size.height - view_point.y,
        ))
    }
}

#[cfg(target_os = "macos")]
fn activate_app() {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    unsafe {
        let Some(cls) = objc::runtime::Class::get("NSApplication") else {
            return;
        };
        let app: *mut Object = msg_send![cls, sharedApplication];
        if !app.is_null() {
            let _: () = msg_send![app, activateIgnoringOtherApps: true];
        }
    }
}
