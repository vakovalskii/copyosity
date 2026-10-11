//! Windows app identity: which program owns the foreground window.
//!
//! macOS identifies apps by bundle id (`com.google.Chrome`). On Windows the stable id is
//! the executable file name, lower-cased (`chrome.exe`): it is what the user sees in Task
//! Manager and it does not change between installs. The display name comes from the
//! exe's version resource (`FileDescription`, e.g. "Google Chrome"), falling back to the
//! file stem.
//!
//! Known gap: Store (UWP) apps run inside `ApplicationFrameHost.exe`, so they all share
//! one id until we resolve the hosted app from the window's child.

#![cfg(target_os = "windows")]

use std::ffi::c_void;
use std::os::windows::ffi::OsStringExt;

use windows_sys::Win32::Foundation::{CloseHandle, HWND};
use windows_sys::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use crate::macos_app::AppIdentity;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn foreground_pid() -> Option<u32> {
    // SAFETY: plain Win32 queries; a null HWND is handled below.
    let hwnd: HWND = unsafe { GetForegroundWindow() };
    if hwnd == 0 {
        return None;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    (pid != 0).then_some(pid)
}

fn exe_path(pid: u32) -> Option<String> {
    // SAFETY: the handle is checked and closed; the buffer length is passed in/out.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle == 0 {
            return None;
        }
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        Some(
            std::ffi::OsString::from_wide(&buf[..len as usize])
                .to_string_lossy()
                .into_owned(),
        )
    }
}

/// `FileDescription` from the exe's version resource, in the first listed language.
fn file_description(path: &str) -> Option<String> {
    let wpath = wide(path);
    // SAFETY: buffers are sized by GetFileVersionInfoSizeW; VerQueryValueW returns pointers
    // into `data`, which outlives every read below.
    unsafe {
        let size = GetFileVersionInfoSizeW(wpath.as_ptr(), std::ptr::null_mut());
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(wpath.as_ptr(), 0, size, data.as_mut_ptr() as *mut c_void) == 0 {
            return None;
        }
        let mut ptr: *mut c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let tr = wide("\\VarFileInfo\\Translation");
        if VerQueryValueW(
            data.as_ptr() as *const c_void,
            tr.as_ptr(),
            &mut ptr,
            &mut len,
        ) == 0
            || len < 4
        {
            return None;
        }
        let lang = *(ptr as *const u16);
        let cp = *((ptr as *const u16).add(1));
        let key = wide(&format!(
            "\\StringFileInfo\\{lang:04x}{cp:04x}\\FileDescription"
        ));
        if VerQueryValueW(
            data.as_ptr() as *const c_void,
            key.as_ptr(),
            &mut ptr,
            &mut len,
        ) == 0
            || len == 0
        {
            return None;
        }
        let chars = std::slice::from_raw_parts(ptr as *const u16, len as usize);
        let end = chars.iter().position(|c| *c == 0).unwrap_or(chars.len());
        let s = String::from_utf16_lossy(&chars[..end]).trim().to_owned();
        (!s.is_empty()).then_some(s)
    }
}

/// `C:\...\Chrome.exe` → `chrome.exe`.
pub fn exe_id(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .to_lowercase()
}

/// `chrome.exe` → `Chrome` (used when the exe has no version resource).
pub fn display_name_for_exe(id: &str) -> String {
    let stem = id.strip_suffix(".exe").unwrap_or(id);
    let mut chars = stem.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => id.to_owned(),
    }
}

pub fn identity_for_pid(pid: u32) -> Option<AppIdentity> {
    // Our own windows report the macOS bundle id so the shared "is it Copyosity?" checks hold.
    // SAFETY: no arguments.
    if pid == unsafe { GetCurrentProcessId() } {
        return Some(AppIdentity {
            bundle_id: crate::macos_app::COPYOSITY_BUNDLE_ID.to_owned(),
            display_name: "Copyosity".to_owned(),
        });
    }
    let path = exe_path(pid)?;
    let id = exe_id(&path);
    let display_name = file_description(&path).unwrap_or_else(|| display_name_for_exe(&id));
    Some(AppIdentity {
        bundle_id: id,
        display_name,
    })
}

/// App that owned the foreground before our overlay took focus (tracked by win_platform).
pub fn last_external_app_identity() -> Option<AppIdentity> {
    let hwnd = crate::win_platform::last_foreground_hwnd();
    if hwnd == 0 {
        return None;
    }
    let mut pid = 0u32;
    // SAFETY: a stale HWND just yields pid 0.
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    identity_for_pid(pid)
}

pub fn frontmost_app_identity() -> Option<AppIdentity> {
    identity_for_pid(foreground_pid()?)
}

pub fn is_copyosity_frontmost() -> bool {
    // SAFETY: no arguments.
    foreground_pid() == Some(unsafe { GetCurrentProcessId() })
}

/// What the user typed in "Excluded apps": `chrome`, `Chrome.exe` or a full path all map
/// to `chrome.exe`.
pub fn identity_from_input(input: &str) -> Option<AppIdentity> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut id = exe_id(trimmed);
    if !id.ends_with(".exe") {
        id.push_str(".exe");
    }
    if id == "copyosity.exe" {
        return None;
    }
    let display_name = display_name_for_exe(&id);
    Some(AppIdentity {
        bundle_id: id,
        display_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exe_ids_are_lowercase_file_names() {
        assert_eq!(
            exe_id(r"C:\Program Files\Google\Chrome\Application\Chrome.exe"),
            "chrome.exe"
        );
        assert_eq!(exe_id("Code.exe"), "code.exe");
    }

    #[test]
    fn display_names_capitalize_the_stem() {
        assert_eq!(display_name_for_exe("chrome.exe"), "Chrome");
        assert_eq!(display_name_for_exe("telegram.exe"), "Telegram");
    }

    #[test]
    fn typed_names_resolve_to_exe_ids() {
        assert_eq!(
            identity_from_input("Chrome").unwrap().bundle_id,
            "chrome.exe"
        );
        assert_eq!(
            identity_from_input(" KeePassXC.exe ").unwrap().bundle_id,
            "keepassxc.exe"
        );
        assert!(identity_from_input("copyosity").is_none());
        assert!(identity_from_input("  ").is_none());
    }
}
