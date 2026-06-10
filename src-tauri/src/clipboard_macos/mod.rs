//! macOS pasteboard: change detection, paste target, skipping app-owned copies.

mod accessibility;
mod paste;

pub use accessibility::{accessibility_trusted, open_accessibility_settings};
#[allow(unused_imports)]
pub use paste::{paste_into_target, simulate_cmd_v, spawn_automated_paste};

pub(crate) use accessibility::{
    capture_focus_for_pid, has_paste_focus, refresh_paste_focus_if_needed,
    restore_focused_ui_element, store_focused_ui_element, try_ax_paste,
};
pub(crate) use paste::{activate_pid, capture_mouse_location};


use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU64, Ordering};
#[cfg(target_os = "macos")]
use std::sync::Mutex;

#[cfg(target_os = "macos")]
use objc2_app_kit::{NSPasteboard, NSWorkspace};
#[cfg(target_os = "macos")]
use objc2_foundation::{ns_string, NSData};

#[cfg(target_os = "macos")]
pub(crate) struct FocusRef(*mut std::ffi::c_void);

#[cfg(target_os = "macos")]
unsafe impl Send for FocusRef {}

#[cfg(target_os = "macos")]
unsafe impl Sync for FocusRef {}

static IGNORE_CAPTURE_AT: AtomicI64 = AtomicI64::new(-1);
pub(crate) static PASTE_TARGET_PID: AtomicI32 = AtomicI32::new(0);

#[cfg(target_os = "macos")]
pub(crate) static PASTE_TARGET_FOCUS: Mutex<Option<FocusRef>> = Mutex::new(None);

#[cfg(target_os = "macos")]
pub(crate) static PASTE_MOUSE_X: AtomicU64 = AtomicU64::new(0);
#[cfg(target_os = "macos")]
pub(crate) static PASTE_MOUSE_Y: AtomicU64 = AtomicU64::new(0);

pub(crate) fn paste_debug_enabled() -> bool {
    std::env::var("COPYOSITY_DEBUG_PASTE")
        .map(|v| {
            let value = v.trim().to_ascii_lowercase();
            value == "1" || value == "true" || value == "yes" || value == "on"
        })
        .unwrap_or(false)
}

pub(crate) fn paste_log(message: impl AsRef<str>) {
    if paste_debug_enabled() {
        eprintln!("[paste] {}", message.as_ref());
    }
}
#[cfg(target_os = "macos")]
pub(crate) static PASTE_MOUSE_VALID: AtomicBool = AtomicBool::new(false);

pub fn change_count() -> i64 {
    #[cfg(target_os = "macos")]
    {
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.changeCount() as i64
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}

pub fn mark_own_clipboard_write(change_count: i64) {
    IGNORE_CAPTURE_AT.store(change_count, Ordering::SeqCst);
}

pub fn should_ignore_capture(change_count: i64) -> bool {
    change_count == IGNORE_CAPTURE_AT.load(Ordering::SeqCst)
}

/// Raw animated GIF bytes from the general pasteboard, if present.
#[cfg(target_os = "macos")]
pub fn pasteboard_gif_bytes() -> Option<Vec<u8>> {
    const MAX_GIF_BYTES: usize = 20 * 1024 * 1024;

    let pasteboard = NSPasteboard::generalPasteboard();
    for ty in [ns_string!("com.compuserve.gif"), ns_string!("public.gif")] {
        let Some(data) = pasteboard.dataForType(ty) else {
            continue;
        };
        let bytes = unsafe { data.as_bytes_unchecked() };
        if crate::clipboard_monitor::is_gif_bytes(bytes) && bytes.len() <= MAX_GIF_BYTES {
            return Some(bytes.to_vec());
        }
    }
    None
}

#[cfg(not(target_os = "macos"))]
pub fn pasteboard_gif_bytes() -> Option<Vec<u8>> {
    None
}

/// Write animated GIF bytes to the general pasteboard.
#[cfg(target_os = "macos")]
pub fn write_gif_to_pasteboard(bytes: &[u8], exclude_from_history: bool) -> Result<(), String> {
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();

    let ns_data = NSData::with_bytes(bytes);
    let compuserve = ns_string!("com.compuserve.gif");
    let public_gif = ns_string!("public.gif");

    let ok_compuserve = pasteboard.setData_forType(Some(&ns_data), compuserve);
    let ok_public = pasteboard.setData_forType(Some(&ns_data), public_gif);
    let ok = ok_compuserve || ok_public;

    if !ok {
        return Err("Failed to write GIF to pasteboard".to_string());
    }

    if exclude_from_history {
        let concealed = ns_string!("org.nspasteboard.ConcealedType");
        pasteboard.setString_forType(ns_string!(""), concealed);
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn write_gif_to_pasteboard(_bytes: &[u8], _exclude_from_history: bool) -> Result<(), String> {
    Err("GIF pasteboard write is only supported on macOS".to_string())
}

pub fn is_concealed() -> bool {
    #[cfg(target_os = "macos")]
    {
        let pasteboard = NSPasteboard::generalPasteboard();
        let ty = ns_string!("org.nspasteboard.ConcealedType");
        pasteboard.dataForType(ty).is_some()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Remember frontmost app before the panel becomes key (call before `show_and_make_key`).
pub fn remember_paste_target() {
    if let Some(pid) = frontmost_pid_excluding_self() {
        PASTE_TARGET_PID.store(pid, Ordering::SeqCst);
        #[cfg(target_os = "macos")]
        {
            crate::app_exclusion::remember_from_pid(pid);
            let focus = capture_focus_for_pid(pid);
            paste_log(format!(
                "remember pid={pid} focus={}",
                if focus.is_some() { "yes" } else { "no" }
            ));
            store_focused_ui_element(focus);
            capture_mouse_location();
        }
    }
}

/// Reactivate the app that had focus before Copyosity (call after `hide_panel`, before Cmd+V).
pub fn restore_paste_target() {
    let pid = PASTE_TARGET_PID.load(Ordering::SeqCst);
    if pid <= 0 {
        return;
    }
    if activate_pid(pid) {
        std::thread::sleep(std::time::Duration::from_millis(90));
    }
    if frontmost_pid() != Some(pid) && activate_pid(pid) {
        std::thread::sleep(std::time::Duration::from_millis(120));
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(FocusRef(element)) = *PASTE_TARGET_FOCUS.lock().unwrap() {
            restore_focused_ui_element(pid, element);
            std::thread::sleep(std::time::Duration::from_millis(80));
        }
    }
}
#[cfg(target_os = "macos")]
fn frontmost_pid_excluding_self() -> Option<i32> {
    let workspace = NSWorkspace::sharedWorkspace();
    let app = workspace.frontmostApplication()?;
    let pid = app.processIdentifier();
    if pid == std::process::id() as i32 {
        return None;
    }
    Some(pid)
}

#[cfg(not(target_os = "macos"))]
fn frontmost_pid_excluding_self() -> Option<i32> {
    None
}

/// Last non-Copyosity app remembered before the panel took focus.
pub fn paste_target_pid() -> Option<i32> {
    let pid = PASTE_TARGET_PID.load(Ordering::SeqCst);
    (pid > 0).then_some(pid)
}

#[cfg(target_os = "macos")]
pub(crate) fn frontmost_pid() -> Option<i32> {
    let workspace = NSWorkspace::sharedWorkspace();
    let app = workspace.frontmostApplication()?;
    Some(app.processIdentifier())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn frontmost_pid() -> Option<i32> {
    None
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_clipboard_write_is_ignored_once() {
        mark_own_clipboard_write(42);
        assert!(should_ignore_capture(42));
        assert!(!should_ignore_capture(43));

        mark_own_clipboard_write(99);
        assert!(should_ignore_capture(99));
        assert!(!should_ignore_capture(42));
    }

    #[test]
    fn paste_into_target_is_send_for_background_spawn() {
        let _job: Box<dyn Send + 'static> = Box::new(paste_into_target);
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn write_gif_to_pasteboard_rejects_non_macos() {
        let minimal = b"GIF89a\x01\x00\x01\x00\x00\x00\x00!";
        let err = write_gif_to_pasteboard(minimal, true).unwrap_err();
        assert!(err.contains("macOS"));
    }
}
