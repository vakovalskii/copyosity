//! Target-screen capture for context-aware voice polishing.
//!
//! Grabs the current screen via the macOS `screencapture` CLI (requires the
//! Screen Recording permission — degrades to `None` when not granted) and
//! downscales it to a small PNG so it can be cheaply sent to a multimodal model
//! as visual context for the polishing step.

#[cfg(target_os = "macos")]
pub fn capture_context_png() -> Option<Vec<u8>> {
    use std::process::Command;

    let path = std::env::temp_dir().join(format!("copyosity_ctx_{}.png", std::process::id()));

    // -x: silent (no shutter sound). -t png. Non-interactive capture of the
    // main display. The voice overlay is tiny and we capture before it covers
    // anything meaningful.
    let status = Command::new("/usr/sbin/screencapture")
        .arg("-x")
        .arg("-t")
        .arg("png")
        .arg(&path)
        .status();

    match &status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            eprintln!("[screen] screencapture exited with {:?}", s.code());
            let _ = std::fs::remove_file(&path);
            return None;
        }
        Err(e) => {
            eprintln!("[screen] failed to run screencapture: {}", e);
            return None;
        }
    }

    let bytes = std::fs::read(&path).ok();
    let _ = std::fs::remove_file(&path);
    match &bytes {
        Some(b) => eprintln!("[screen] captured {} bytes (pre-downscale)", b.len()),
        None => eprintln!("[screen] screencapture produced no readable file"),
    }
    let out = downscale_png(&bytes?);
    match &out {
        Some(o) => eprintln!("[screen] downscaled to {} bytes PNG", o.len()),
        None => eprintln!("[screen] downscale failed"),
    }
    out
}

/// Capture just the frontmost on-screen window belonging to `pid` (the app that
/// was frontmost when the palette opened), downscaled to a small PNG. Falls back
/// to a full-screen capture when the window can't be resolved. Returns `None`
/// when Screen Recording permission is missing.
#[cfg(target_os = "macos")]
pub fn capture_window_png(pid: i32) -> Option<Vec<u8>> {
    match frontmost_window_id_for_pid(pid) {
        Some(window_id) => capture_by_window_id(window_id).or_else(capture_context_png),
        None => capture_context_png(),
    }
}

#[cfg(target_os = "macos")]
fn capture_by_window_id(window_id: u32) -> Option<Vec<u8>> {
    use std::process::Command;

    let path = std::env::temp_dir().join(format!("copyosity_win_{}.png", std::process::id()));
    // -x: silent. -o: omit the window's drop shadow. -l<id>: capture that window.
    let status = Command::new("/usr/sbin/screencapture")
        .arg("-x")
        .arg("-o")
        .arg(format!("-l{}", window_id))
        .arg("-t")
        .arg("png")
        .arg(&path)
        .status();

    match &status {
        Ok(s) if s.success() => {}
        _ => {
            let _ = std::fs::remove_file(&path);
            return None;
        }
    }
    let bytes = std::fs::read(&path).ok();
    let _ = std::fs::remove_file(&path);
    downscale_png(&bytes?)
}

/// Frontmost normal (layer 0) on-screen window number owned by `pid`, via
/// CoreGraphics. The window list is ordered front-to-back, so the first match is
/// the frontmost window.
#[cfg(target_os = "macos")]
fn frontmost_window_id_for_pid(pid: i32) -> Option<u32> {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        // Returns a CFArrayRef (toll-free bridged to NSArray) of CFDictionary.
        fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> *mut Object;
    }
    const OPTION_ON_SCREEN_ONLY: u32 = 1 << 0;
    const OPTION_EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;
    const NULL_WINDOW_ID: u32 = 0;

    // Build an autoreleased NSString from a &str via the objc runtime (no cocoa dep).
    unsafe fn ns_string(s: &str) -> *mut Object {
        let cls = match objc::runtime::Class::get("NSString") {
            Some(c) => c,
            None => return std::ptr::null_mut(),
        };
        let c = std::ffi::CString::new(s).unwrap_or_default();
        msg_send![cls, stringWithUTF8String: c.as_ptr()]
    }

    unsafe {
        let arr: *mut Object = CGWindowListCopyWindowInfo(
            OPTION_ON_SCREEN_ONLY | OPTION_EXCLUDE_DESKTOP_ELEMENTS,
            NULL_WINDOW_ID,
        );
        if arr.is_null() {
            return None;
        }
        let owner_key = ns_string("kCGWindowOwnerPID");
        let layer_key = ns_string("kCGWindowLayer");
        let number_key = ns_string("kCGWindowNumber");

        let count: usize = msg_send![arr, count];
        let mut found: Option<u32> = None;
        for i in 0..count {
            let dict: *mut Object = msg_send![arr, objectAtIndex: i];
            if dict.is_null() {
                continue;
            }
            let pid_num: *mut Object = msg_send![dict, objectForKey: owner_key];
            if pid_num.is_null() {
                continue;
            }
            let owner_pid: i32 = msg_send![pid_num, intValue];
            if owner_pid != pid {
                continue;
            }
            let layer_num: *mut Object = msg_send![dict, objectForKey: layer_key];
            let layer: i32 = if layer_num.is_null() {
                0
            } else {
                msg_send![layer_num, intValue]
            };
            if layer != 0 {
                continue;
            }
            let num: *mut Object = msg_send![dict, objectForKey: number_key];
            if num.is_null() {
                continue;
            }
            let wid: u32 = msg_send![num, unsignedIntValue];
            found = Some(wid);
            break;
        }
        let _: () = msg_send![arr, release];
        found
    }
}

#[cfg(target_os = "macos")]
fn downscale_png(bytes: &[u8]) -> Option<Vec<u8>> {
    use image::{GenericImageView, ImageFormat};

    let img = image::load_from_memory(bytes).ok()?;
    let (w, h) = img.dimensions();
    const MAX: u32 = 1280;
    let scaled = if w.max(h) > MAX {
        img.resize(MAX, MAX, image::imageops::FilterType::Triangle)
    } else {
        img
    };

    let mut out = Vec::new();
    scaled
        .write_to(&mut std::io::Cursor::new(&mut out), ImageFormat::Png)
        .ok()?;
    Some(out)
}
