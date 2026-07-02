//! Synthetic paste: Cmd+V, osascript, mouse click fallback.

#[cfg(target_os = "macos")]
use std::sync::atomic::Ordering;

#[cfg(target_os = "macos")]
use objc2_app_kit::NSRunningApplication;

#[cfg(target_os = "macos")]
use super::{
    accessibility_trusted, frontmost_pid, has_paste_focus, paste_log, prefers_keyboard_paste,
    refresh_paste_focus_if_needed, restore_paste_target, try_ax_paste_for_pid, PASTE_MOUSE_VALID,
    PASTE_MOUSE_X, PASTE_MOUSE_Y, PASTE_TARGET_PID,
};

/// Whether synthetic Cmd+V should use the session event tap (frontmost app) vs `CGEventPostToPid`.
#[cfg(target_os = "macos")]
pub(crate) fn cmd_v_uses_session_tap(pid: i32, frontmost: Option<i32>) -> bool {
    pid <= 0 || frontmost == Some(pid)
}

/// Spawn automated paste on a background thread after optional accessibility prompt.
#[cfg(target_os = "macos")]
pub fn spawn_automated_paste(prompt_if_needed: bool) {
    if !accessibility_trusted(false) {
        if prompt_if_needed {
            accessibility_trusted(true);
        }
        if !accessibility_trusted(false) {
            paste_log("skipped: accessibility not granted");
            return;
        }
    }
    std::thread::spawn(paste_into_target);
}

/// Restore the previous target and paste. Call from a background thread after the panel hides.
#[cfg(target_os = "macos")]
pub fn paste_into_target() {
    struct GifTempCleanup;
    impl Drop for GifTempCleanup {
        fn drop(&mut self) {
            #[cfg(target_os = "macos")]
            crate::clipboard_write::cleanup_pending_gif_temp();
        }
    }
    let _gif_cleanup = GifTempCleanup;

    let pid = PASTE_TARGET_PID.load(Ordering::SeqCst);
    let remembered_focus = has_paste_focus();
    paste_log(format!(
        "start pid={pid} remembered_focus={}",
        if remembered_focus { "yes" } else { "no" }
    ));

    // Let the main run loop finish hiding Copyosity and transferring focus.
    std::thread::sleep(std::time::Duration::from_millis(180));
    restore_paste_target();
    let activated = wait_for_frontmost(pid);
    paste_log(format!(
        "frontmost={:?} target={pid} activated={activated}",
        frontmost_pid()
    ));
    refresh_paste_focus_if_needed();

    let keyboard_paste = prefers_keyboard_paste(pid);
    if keyboard_paste {
        paste_log("target prefers keyboard paste");
    }

    if try_ax_paste_for_pid(pid) {
        paste_log("succeeded via AXPaste");
        return;
    }

    if !has_paste_focus() {
        if click_saved_mouse() {
            paste_log("clicked saved mouse position");
            std::thread::sleep(std::time::Duration::from_millis(150));
            refresh_paste_focus_if_needed();
            if try_ax_paste_for_pid(pid) {
                paste_log("succeeded via AXPaste after click");
                return;
            }
        }

        // Prefer CGEvent (Accessibility) over osascript — System Events often misses Electron webviews.
        std::thread::sleep(std::time::Duration::from_millis(100));
        if simulate_cmd_v() {
            paste_log(format!("sent Cmd+V via CGEvent (no ax focus, pid={pid})"));
            return;
        }
    }

    std::thread::sleep(std::time::Duration::from_millis(120));
    if simulate_cmd_v() {
        paste_log(format!("sent Cmd+V (pid={pid})"));
        return;
    }

    if simulate_cmd_v_osascript() {
        paste_log("sent Cmd+V via osascript (fallback)");
        return;
    }

    paste_log("all paste methods failed");
}
/// Post synthetic Cmd+V to the target app (requires Accessibility).
#[cfg(target_os = "macos")]
pub fn simulate_cmd_v() -> bool {
    unsafe {
        type CGEventRef = *mut std::ffi::c_void;

        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventSourceCreate(state_id: i32) -> *mut std::ffi::c_void;
            fn CGEventCreateKeyboardEvent(
                source: *mut std::ffi::c_void,
                keycode: u16,
                key_down: bool,
            ) -> CGEventRef;
            fn CGEventSetFlags(event: CGEventRef, flags: u64);
            fn CGEventPost(tap: u32, event: CGEventRef);
            fn CGEventPostToPid(pid: i32, event: CGEventRef);
            fn CFRelease(cf: *mut std::ffi::c_void);
        }

        const K_CG_EVENT_FLAG_COMMAND: u64 = 0x00100000;
        const K_CG_EVENT_SOURCE_STATE_HID_SYSTEM_STATE: i32 = 1;
        const K_CG_SESSION_EVENT_TAP: u32 = 1;
        const K_V_KEYCODE: u16 = 9;

        let source = CGEventSourceCreate(K_CG_EVENT_SOURCE_STATE_HID_SYSTEM_STATE);
        let event_down = CGEventCreateKeyboardEvent(source, K_V_KEYCODE, true);
        let event_up = CGEventCreateKeyboardEvent(source, K_V_KEYCODE, false);

        if event_down.is_null() || event_up.is_null() {
            if !source.is_null() {
                CFRelease(source);
            }
            return false;
        }

        CGEventSetFlags(event_down, K_CG_EVENT_FLAG_COMMAND);
        CGEventSetFlags(event_up, K_CG_EVENT_FLAG_COMMAND);

        let pid = PASTE_TARGET_PID.load(Ordering::SeqCst);
        if cmd_v_uses_session_tap(pid, frontmost_pid()) {
            // Session tap reaches the frontmost app — required for Messages and similar
            // native apps that ignore CGEventPostToPid. Post to one tap only; session + HID
            // together would deliver two paste events.
            CGEventPost(K_CG_SESSION_EVENT_TAP, event_down);
            CGEventPost(K_CG_SESSION_EVENT_TAP, event_up);
        } else {
            // Target not frontmost yet — deliver directly to the process.
            CGEventPostToPid(pid, event_down);
            CGEventPostToPid(pid, event_up);
        }
        CFRelease(event_down);
        CFRelease(event_up);
        if !source.is_null() {
            CFRelease(source);
        }
        true
    }
}

#[cfg(target_os = "macos")]
fn simulate_cmd_v_osascript() -> bool {
    let pid = PASTE_TARGET_PID.load(Ordering::SeqCst);
    if pid > 0 {
        if let Some(name) = localized_app_name_for_pid(pid) {
            let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
            let by_process = format!(
                r#"tell application "System Events"
  tell process "{escaped}"
    set frontmost to true
    delay 0.25
    key code 9 using command down
  end tell
end tell"#
            );
            if run_osascript(&by_process) {
                return true;
            }
        }

        let by_pid = format!(
            r#"tell application "System Events"
  tell (first process whose unix id is {pid})
    set frontmost to true
    delay 0.25
    key code 9 using command down
  end tell
end tell"#
        );
        if run_osascript(&by_pid) {
            return true;
        }
    }

    run_osascript(r#"tell application "System Events" to key code 9 using command down"#)
}

#[cfg(target_os = "macos")]
fn run_osascript(script: &str) -> bool {
    let output = std::process::Command::new("osascript")
        .args(["-e", script])
        .output();

    match output {
        Ok(out) if out.status.success() => true,
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if !stderr.trim().is_empty() {
                paste_log(format!("osascript error: {stderr}"));
            }
            false
        }
        Err(err) => {
            paste_log(format!("osascript spawn failed: {err}"));
            false
        }
    }
}

#[cfg(target_os = "macos")]
fn wait_for_frontmost(target_pid: i32) -> bool {
    if target_pid <= 0 {
        return false;
    }
    for attempt in 0..25 {
        if frontmost_pid() == Some(target_pid) {
            return true;
        }
        activate_pid(target_pid);
        std::thread::sleep(std::time::Duration::from_millis(50 + attempt * 10));
    }
    false
}

#[cfg(target_os = "macos")]
pub(crate) fn capture_mouse_location() {
    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    unsafe {
        type CGEventRef = *mut std::ffi::c_void;

        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventCreate(source: *mut std::ffi::c_void) -> CGEventRef;
            fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
            fn CFRelease(cf: *mut std::ffi::c_void);
        }

        let event = CGEventCreate(std::ptr::null_mut());
        if event.is_null() {
            return;
        }
        let point = CGEventGetLocation(event);
        CFRelease(event);
        PASTE_MOUSE_X.store(point.x.to_bits(), Ordering::SeqCst);
        PASTE_MOUSE_Y.store(point.y.to_bits(), Ordering::SeqCst);
        PASTE_MOUSE_VALID.store(true, Ordering::SeqCst);
    }
}

#[cfg(target_os = "macos")]
fn click_saved_mouse() -> bool {
    if !PASTE_MOUSE_VALID.load(Ordering::SeqCst) {
        return false;
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    let point = CGPoint {
        x: f64::from_bits(PASTE_MOUSE_X.load(Ordering::SeqCst)),
        y: f64::from_bits(PASTE_MOUSE_Y.load(Ordering::SeqCst)),
    };

    unsafe {
        type CGEventRef = *mut std::ffi::c_void;

        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventCreateMouseEvent(
                source: *mut std::ffi::c_void,
                mouse_type: u32,
                mouse_cursor_position: CGPoint,
                mouse_button: u32,
            ) -> CGEventRef;
            fn CGEventPost(tap: u32, event: CGEventRef);
            fn CFRelease(cf: *mut std::ffi::c_void);
        }

        const K_CG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
        const K_CG_EVENT_LEFT_MOUSE_UP: u32 = 2;
        const K_CG_MOUSE_BUTTON_LEFT: u32 = 0;
        const K_CG_HID_EVENT_TAP: u32 = 0;

        let down = CGEventCreateMouseEvent(
            std::ptr::null_mut(),
            K_CG_EVENT_LEFT_MOUSE_DOWN,
            point,
            K_CG_MOUSE_BUTTON_LEFT,
        );
        let up = CGEventCreateMouseEvent(
            std::ptr::null_mut(),
            K_CG_EVENT_LEFT_MOUSE_UP,
            point,
            K_CG_MOUSE_BUTTON_LEFT,
        );
        if down.is_null() || up.is_null() {
            if !down.is_null() {
                CFRelease(down);
            }
            if !up.is_null() {
                CFRelease(up);
            }
            return false;
        }

        CGEventPost(K_CG_HID_EVENT_TAP, down);
        CGEventPost(K_CG_HID_EVENT_TAP, up);
        CFRelease(down);
        CFRelease(up);
        true
    }
}

#[cfg(target_os = "macos")]
fn localized_app_name_for_pid(pid: i32) -> Option<String> {
    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
    app.localizedName().map(|s| s.to_string())
}

#[cfg(target_os = "macos")]
pub(crate) fn activate_pid(pid: i32) -> bool {
    use objc2_app_kit::NSApplicationActivationOptions;
    let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
        return false;
    };
    #[allow(deprecated)]
    app.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn cmd_v_uses_session_tap_when_target_is_frontmost() {
        assert!(cmd_v_uses_session_tap(42, Some(42)));
    }

    #[test]
    fn cmd_v_uses_session_tap_when_pid_unknown() {
        assert!(cmd_v_uses_session_tap(0, None));
        assert!(cmd_v_uses_session_tap(-1, Some(99)));
    }

    #[test]
    fn cmd_v_uses_post_to_pid_when_target_not_frontmost() {
        assert!(!cmd_v_uses_session_tap(42, Some(99)));
        assert!(!cmd_v_uses_session_tap(42, None));
    }
}
