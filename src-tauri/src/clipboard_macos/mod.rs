//! macOS pasteboard: change detection, paste target, skipping app-owned copies.
//!
//! Paste-into-target flow: see `docs/architecture/macos-paste-pipeline.md`.

mod accessibility;
mod paste;

pub use accessibility::{accessibility_trusted, open_accessibility_settings};
#[cfg(target_os = "macos")]
pub use paste::spawn_automated_paste;

#[cfg(target_os = "macos")]
pub(crate) use accessibility::{
    capture_focus_for_pid, has_paste_focus, prefers_keyboard_paste, refresh_paste_focus_if_needed,
    restore_focused_ui_element, store_focused_ui_element, try_ax_paste_for_pid,
};
#[cfg(target_os = "macos")]
pub(crate) use paste::activate_pid;
#[cfg(target_os = "macos")]
pub(crate) use paste::capture_mouse_location;

#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU64, Ordering};
#[cfg(target_os = "macos")]
use std::sync::Mutex;

#[cfg(target_os = "macos")]
use objc2_app_kit::{NSPasteboard, NSURLNSPasteboardSupport, NSWorkspace};
#[cfg(target_os = "macos")]
use objc2_foundation::{ns_string, NSArray, NSData, NSString, NSURL};
#[cfg(target_os = "macos")]
use std::path::PathBuf;

#[cfg(target_os = "macos")]
pub(crate) struct FocusRef(*mut std::ffi::c_void);

#[cfg(target_os = "macos")]
unsafe impl Send for FocusRef {}

#[cfg(target_os = "macos")]
unsafe impl Sync for FocusRef {}

#[cfg(target_os = "macos")]
static IGNORE_CAPTURE_AT: AtomicI64 = AtomicI64::new(-1);
#[cfg(target_os = "macos")]
pub(crate) static PASTE_TARGET_PID: AtomicI32 = AtomicI32::new(0);

/// Last non-Copyosity app remembered for paste (overlay, voice, tray click, etc.).
#[cfg(target_os = "macos")]
static LAST_NON_SELF_FRONTMOST_PID: AtomicI32 = AtomicI32::new(0);

#[cfg(target_os = "macos")]
pub(crate) static PASTE_TARGET_FOCUS: Mutex<Option<FocusRef>> = Mutex::new(None);

#[cfg(target_os = "macos")]
pub(crate) static PASTE_MOUSE_X: AtomicU64 = AtomicU64::new(0);
#[cfg(target_os = "macos")]
pub(crate) static PASTE_MOUSE_Y: AtomicU64 = AtomicU64::new(0);

#[cfg(target_os = "macos")]
pub(crate) fn paste_debug_enabled() -> bool {
    std::env::var("COPYOSITY_DEBUG_PASTE")
        .map(|v| {
            let value = v.trim().to_ascii_lowercase();
            value == "1" || value == "true" || value == "yes" || value == "on"
        })
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
pub(crate) fn paste_log(message: impl AsRef<str>) {
    if paste_debug_enabled() {
        eprintln!("[paste] {}", message.as_ref());
    }
}
#[cfg(target_os = "macos")]
pub(crate) static PASTE_MOUSE_VALID: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
pub fn change_count() -> i64 {
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.changeCount() as i64
}

#[cfg(target_os = "macos")]
pub fn mark_own_clipboard_write(change_count: i64) {
    IGNORE_CAPTURE_AT.store(change_count, Ordering::SeqCst);
}

#[cfg(target_os = "macos")]
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

/// Absolute file paths from Finder-style copy (`public.file-url` / legacy `NSFilenamesPboardType`).
#[cfg(target_os = "macos")]
pub fn pasteboard_file_paths() -> Vec<PathBuf> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let from_urls = paths_from_file_url_pasteboard(&pasteboard);
    if !from_urls.is_empty() {
        return from_urls;
    }
    paths_from_legacy_filenames_pasteboard(&pasteboard)
}

#[cfg(target_os = "macos")]
fn path_from_ns_url(url: &NSURL) -> Option<PathBuf> {
    url.path().map(|p| PathBuf::from(p.to_string()))
}

#[cfg(target_os = "macos")]
fn path_from_file_url_string(s: &NSString) -> Option<PathBuf> {
    NSURL::URLWithString(s).and_then(|url| path_from_ns_url(&url))
}

#[cfg(target_os = "macos")]
fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        if !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn paths_from_file_url_pasteboard(pasteboard: &NSPasteboard) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let file_url = ns_string!("public.file-url");

    if let Some(items) = pasteboard.pasteboardItems() {
        for item in items.iter() {
            let Some(url_string) = item.stringForType(file_url) else {
                continue;
            };
            if let Some(path) = path_from_file_url_string(&url_string) {
                paths.push(path);
            }
        }
    }

    if paths.is_empty() {
        if let Some(url) = NSURL::URLFromPasteboard(pasteboard) {
            if let Some(path) = path_from_ns_url(&url) {
                paths.push(path);
            }
        }
    }

    dedupe_paths(paths)
}

#[cfg(target_os = "macos")]
fn paths_from_legacy_filenames_pasteboard(pasteboard: &NSPasteboard) -> Vec<PathBuf> {
    let ty = ns_string!("NSFilenamesPboardType");
    let Some(plist) = pasteboard.propertyListForType(ty) else {
        return Vec::new();
    };
    let Ok(array) = plist.downcast::<NSArray>() else {
        return Vec::new();
    };

    let mut paths = Vec::new();
    let count = array.count();
    for i in 0..count {
        let obj = array.objectAtIndex(i);
        if let Some(s) = obj.downcast_ref::<NSString>() {
            paths.push(PathBuf::from(s.to_string()));
        }
    }
    paths
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
        return Err("Failed to write GIF to pasteboard".to_owned());
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

#[cfg(target_os = "macos")]
pub fn is_concealed() -> bool {
    let pasteboard = NSPasteboard::generalPasteboard();
    let ty = ns_string!("org.nspasteboard.ConcealedType");
    pasteboard.dataForType(ty).is_some()
}

/// Remember a specific app as the paste target (voice hotkey, palette open, etc.).
#[cfg(target_os = "macos")]
pub fn remember_paste_target_for_pid(pid: i32) {
    if pid <= 0 || pid == std::process::id() as i32 {
        return;
    }
    PASTE_TARGET_PID.store(pid, Ordering::SeqCst);
    LAST_NON_SELF_FRONTMOST_PID.store(pid, Ordering::SeqCst);
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

/// Remember frontmost app before the panel becomes key (call before `show_and_make_key`).
#[cfg(target_os = "macos")]
pub fn remember_paste_target() {
    if let Some(pid) = frontmost_pid_excluding_self() {
        remember_paste_target_for_pid(pid);
    }
}

/// Track the last non-Copyosity app that became frontmost (lightweight PID only).
#[cfg(target_os = "macos")]
pub(crate) fn note_last_non_self_frontmost(pid: i32) {
    if pid > 0 && pid != std::process::id() as i32 {
        LAST_NON_SELF_FRONTMOST_PID.store(pid, Ordering::SeqCst);
    }
}

/// Last remembered non-Copyosity paste target that is still running.
#[cfg(target_os = "macos")]
pub(crate) fn last_remembered_paste_target_pid() -> Option<i32> {
    let pid = LAST_NON_SELF_FRONTMOST_PID.load(Ordering::SeqCst);
    if pid > 0 && pid != std::process::id() as i32 && crate::macos_app::is_pid_running(pid) {
        Some(pid)
    } else {
        None
    }
}

/// Keep `LAST_NON_SELF_FRONTMOST_PID` fresh when the user switches apps (tray quick-menu fallback).
#[cfg(target_os = "macos")]
pub fn install_last_frontmost_observer() {
    use std::ptr::NonNull;
    use std::sync::atomic::AtomicBool;

    use block2::RcBlock;
    use objc2_app_kit::NSWorkspaceDidActivateApplicationNotification;
    use objc2_foundation::{NSNotification, NSNotificationCenter};

    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.swap(true, Ordering::AcqRel) {
        return;
    }

    let block = RcBlock::new(|_notification: NonNull<NSNotification>| {
        if let Some(pid) = frontmost_pid_excluding_self() {
            note_last_non_self_frontmost(pid);
        }
    });
    let center = NSNotificationCenter::defaultCenter();
    let observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidActivateApplicationNotification),
            None,
            None,
            &block,
        )
    };
    // Intentionally leaked for app lifetime — observer must outlive all activations.
    std::mem::forget(observer);
    std::mem::forget(block);
}

/// Reactivate the app that had focus before Copyosity (call after `hide_panel`, before Cmd+V).
#[cfg(target_os = "macos")]
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

/// Last non-Copyosity app remembered before the panel took focus.
#[cfg(target_os = "macos")]
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
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static CLIPBOARD_TARGET_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    #[cfg(target_os = "macos")]
    fn own_clipboard_write_is_ignored_once() {
        mark_own_clipboard_write(42);
        assert!(should_ignore_capture(42));
        assert!(!should_ignore_capture(43));

        mark_own_clipboard_write(99);
        assert!(should_ignore_capture(99));
        assert!(!should_ignore_capture(42));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn remember_paste_target_for_pid_ignores_invalid_pids() {
        let _guard = CLIPBOARD_TARGET_TEST_LOCK.lock().unwrap();
        let previous_pid = PASTE_TARGET_PID.load(Ordering::SeqCst);
        let previous_last = LAST_NON_SELF_FRONTMOST_PID.load(Ordering::SeqCst);
        remember_paste_target_for_pid(0);
        remember_paste_target_for_pid(-1);
        remember_paste_target_for_pid(std::process::id() as i32);
        assert_eq!(PASTE_TARGET_PID.load(Ordering::SeqCst), previous_pid);

        remember_paste_target_for_pid(42_001);
        assert_eq!(PASTE_TARGET_PID.load(Ordering::SeqCst), 42_001);
        assert_eq!(LAST_NON_SELF_FRONTMOST_PID.load(Ordering::SeqCst), 42_001);
        PASTE_TARGET_PID.store(previous_pid, Ordering::SeqCst);
        LAST_NON_SELF_FRONTMOST_PID.store(previous_last, Ordering::SeqCst);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn note_last_non_self_frontmost_ignores_self_and_invalid() {
        let _guard = CLIPBOARD_TARGET_TEST_LOCK.lock().unwrap();
        let previous = LAST_NON_SELF_FRONTMOST_PID.load(Ordering::SeqCst);
        let self_pid = std::process::id() as i32;

        note_last_non_self_frontmost(0);
        note_last_non_self_frontmost(-1);
        note_last_non_self_frontmost(self_pid);
        assert_eq!(LAST_NON_SELF_FRONTMOST_PID.load(Ordering::SeqCst), previous);

        note_last_non_self_frontmost(999_999);
        assert_eq!(LAST_NON_SELF_FRONTMOST_PID.load(Ordering::SeqCst), 999_999);
        assert_eq!(last_remembered_paste_target_pid(), None);

        let workspace = NSWorkspace::sharedWorkspace();
        let Some(running_pid) = workspace
            .runningApplications()
            .iter()
            .map(|app| app.processIdentifier())
            .find(|&pid| pid > 0 && pid != self_pid)
        else {
            LAST_NON_SELF_FRONTMOST_PID.store(previous, Ordering::SeqCst);
            return;
        };

        note_last_non_self_frontmost(running_pid);
        assert_eq!(last_remembered_paste_target_pid(), Some(running_pid));

        LAST_NON_SELF_FRONTMOST_PID.store(previous, Ordering::SeqCst);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn remember_paste_target_for_pid_is_send_for_background_spawn() {
        let _job: Box<dyn Send + 'static> = Box::new(|| remember_paste_target_for_pid(1));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn paste_into_target_is_send_for_background_spawn() {
        use paste::paste_into_target;
        let _job: Box<dyn Send + 'static> = Box::new(paste_into_target);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn dedupe_paths_preserves_first_occurrence() {
        use std::path::PathBuf;
        let a = PathBuf::from("/tmp/a.jpg");
        let b = PathBuf::from("/tmp/b.jpg");
        let out = dedupe_paths(vec![a.clone(), b.clone(), a.clone()]);
        assert_eq!(out, vec![a, b]);
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn write_gif_to_pasteboard_rejects_non_macos() {
        let minimal = b"GIF89a\x01\x00\x01\x00\x00\x00\x00!";
        let err = write_gif_to_pasteboard(minimal, true).unwrap_err();
        assert!(err.contains("macOS"));
    }
}
