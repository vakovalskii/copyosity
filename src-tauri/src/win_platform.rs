//! Windows native layer: foreground-window tracking + synthetic paste (Ctrl+V).
//!
//! This is the Windows counterpart to the macOS `clipboard_macos::paste` /
//! `install_last_frontmost_observer` pipeline. The overlay paste flow is:
//! `paste_entry` → writes text to the clipboard → `finish_paste` hides the panel
//! → `finalize_panel_hide` calls [`spawn_automated_paste`], which restores the
//! previously-focused window and injects Ctrl+V via `SendInput`.
//!
//! We track the last non-Copyosity foreground window in a background poll (a
//! window handle, unlike macOS which tracks a PID) so paste has a target even
//! though our overlay briefly took focus. Text + image capture already works on
//! Windows through `arboard`; this module adds the paste-back half.
//!
//! NOTE: `SetForegroundWindow` is subject to Windows' foreground lock — a
//! background process may be denied the right to steal focus. In practice, when
//! our overlay window hides, Windows restores focus to the previous window on its
//! own, and `SendInput` targets whatever is foreground, so paste still lands. The
//! explicit `SetForegroundWindow` is a best-effort nudge.

#![cfg(target_os = "windows")]

use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::Duration;

use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
};

/// Handle (`HWND`, stored as `isize`) of the last foreground window that was not
/// Copyosity itself — the app we paste into.
static LAST_FOREGROUND_HWND: AtomicIsize = AtomicIsize::new(0);

fn paste_log(msg: impl AsRef<str>) {
    if std::env::var_os("COPYOSITY_PASTE_DEBUG").is_some() {
        eprintln!("[paste/win] {}", msg.as_ref());
    }
}

/// Start a background poll that remembers the most recent foreground window that
/// belongs to another process. Call once at startup.
pub fn install_last_frontmost_observer() {
    std::thread::spawn(|| {
        let me = unsafe { GetCurrentProcessId() };
        loop {
            std::thread::sleep(Duration::from_millis(250));
            let hwnd = unsafe { GetForegroundWindow() };
            if hwnd == 0 {
                continue;
            }
            let mut pid: u32 = 0;
            unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
            if pid != 0 && pid != me {
                LAST_FOREGROUND_HWND.store(hwnd, Ordering::SeqCst);
            }
        }
    });
}

/// Spawn synthetic paste on a background thread (signature mirrors the macOS
/// `spawn_automated_paste`; the prompt flag is macOS-only and ignored here).
pub fn spawn_automated_paste(_prompt_if_needed: bool) {
    std::thread::spawn(paste_into_target);
}

fn paste_into_target() {
    // Let the overlay finish hiding and focus return to the previous window.
    std::thread::sleep(Duration::from_millis(180));

    let target = LAST_FOREGROUND_HWND.load(Ordering::SeqCst);
    if target != 0 {
        // Best-effort: nudge the previous window back to foreground before typing.
        let ok = unsafe { SetForegroundWindow(target) };
        paste_log(format!("SetForegroundWindow(hwnd={target}) -> {ok}"));
        std::thread::sleep(Duration::from_millis(60));
    } else {
        paste_log("no remembered target window; pasting into current foreground");
    }

    if send_ctrl_v() {
        paste_log("sent Ctrl+V via SendInput");
    } else {
        paste_log("SendInput reported 0 events injected");
    }
}

/// Build a single keyboard `INPUT` event for a virtual-key code.
fn key_input(vk: u16, key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if key_up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Inject a Ctrl+V key chord. Returns whether all four key events were accepted.
fn send_ctrl_v() -> bool {
    let inputs = [
        key_input(VK_CONTROL, false),
        key_input(VK_V, false),
        key_input(VK_V, true),
        key_input(VK_CONTROL, true),
    ];
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    sent as usize == inputs.len()
}
