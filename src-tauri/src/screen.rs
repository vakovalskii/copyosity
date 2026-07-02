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
