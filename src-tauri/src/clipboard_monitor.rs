use arboard::Clipboard;
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use sha2::{Sha256, Digest};
use std::io::Cursor;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};


use crate::db::{ClipboardEntry, Database};
use crate::ollama;

fn encode_image_from_rgba(bytes: &[u8], width: usize, height: usize) -> Option<(String, String)> {
    let rgba = ImageBuffer::<Rgba<u8>, _>::from_raw(width as u32, height as u32, bytes.to_vec())?;
    let full = DynamicImage::ImageRgba8(rgba);

    // Encode full-size image as PNG
    let mut full_buf = Cursor::new(Vec::new());
    full.write_to(&mut full_buf, ImageFormat::Png).ok()?;
    let full_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        full_buf.into_inner(),
    );

    // Encode thumbnail
    let thumb = full.thumbnail(240, 160);
    let mut thumb_buf = Cursor::new(Vec::new());
    thumb.write_to(&mut thumb_buf, ImageFormat::Png).ok()?;
    let thumb_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        thumb_buf.into_inner(),
    );

    Some((full_b64, thumb_b64))
}

pub fn start_clipboard_monitor(app: AppHandle) {
    let db = app.state::<Arc<Database>>().inner().clone();

    // Run clipboard polling in a dedicated thread (not async — arboard is sync)
    std::thread::spawn(move || {
        let mut clipboard = Clipboard::new().expect("Failed to access clipboard");
        let mut last_hash = String::new();

        loop {
            std::thread::sleep(std::time::Duration::from_millis(300));

            // Try text
            if let Ok(text) = clipboard.get_text() {
                if text.is_empty() {
                    continue;
                }

                let hash = fast_hash(text.as_bytes());
                if hash == last_hash {
                    continue;
                }
                last_hash = hash.clone();

                let source_app = get_frontmost_app();

                // Skip changes from our own app (user copied from Copyosity)
                if let Some(app_name) = &source_app {
                    if app_name == "Copyosity" {
                        continue;
                    }
                    if db.is_app_excluded(app_name).unwrap_or(false) {
                        continue;
                    }
                }

                let entry = ClipboardEntry {
                    id: 0,
                    content_type: "text".to_string(),
                    text_content: Some(text.clone()),
                    image_data: None,
                    image_thumb: None,
                    source_app,
                    source_app_icon: None,
                    content_hash: hash,
                    char_count: Some(text.len() as i64),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    is_pinned: false,
                    collection_id: None,
                    tags: Vec::new(),
                };

                if let Ok((id, is_new)) = db.insert_entry(&entry) {
                    let mut saved = entry.clone();
                    saved.id = id;
                    let _ = app.emit("clipboard-changed", &saved);

                    // Only tag genuinely new entries
                    if is_new {
                        let db = db.clone();
                        let app = app.clone();
                        std::thread::spawn(move || {
                            if let Some(tags) = ollama::tag_text(&text) {
                                if db.set_entry_tags(id, &tags).is_ok() {
                                    let _ = app.emit("entry-tagged", id);
                                }
                            } else {
                                let _ = db.set_entry_tag_state(id, "skipped");
                            }
                        });
                    }
                }
                continue;
            }

            // Try image only if text failed
            if let Ok(img) = clipboard.get_image() {
                if img.bytes.is_empty() {
                    continue;
                }
                let hash = fast_hash(&img.bytes);
                if hash == last_hash {
                    continue;
                }
                last_hash = hash.clone();

                let source_app = get_frontmost_app();
                if let Some(app_name) = &source_app {
                    if app_name == "Copyosity" {
                        continue;
                    }
                    if db.is_app_excluded(app_name).unwrap_or(false) {
                        continue;
                    }
                }
                let Some((image_full_b64, image_thumb_b64)) =
                    encode_image_from_rgba(&img.bytes, img.width, img.height)
                else {
                    continue;
                };

                let entry = ClipboardEntry {
                    id: 0,
                    content_type: "image".to_string(),
                    text_content: None,
                    image_data: Some(image_full_b64),
                    image_thumb: Some(image_thumb_b64),
                    source_app,
                    source_app_icon: None,
                    content_hash: hash,
                    char_count: None,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    is_pinned: false,
                    collection_id: None,
                    tags: Vec::new(),
                };

                if let Ok((id, _is_new)) = db.insert_entry(&entry) {
                    let mut saved = entry.clone();
                    saved.id = id;
                    saved.image_data = None;
                    let _ = app.emit("clipboard-changed", &saved);
                }
            }
        }
    });
}

/// Fast hash — only hash first 4KB + length for speed on large content
fn fast_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    let prefix = if data.len() > 4096 { &data[..4096] } else { data };
    hasher.update(prefix);
    hasher.update(data.len().to_le_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(target_os = "macos")]
pub fn get_frontmost_app() -> Option<String> {
    use std::process::Command;
    // Use much faster NSWorkspace via swift instead of osascript
    let output = Command::new("lsappinfo")
        .arg("info")
        .arg("-only")
        .arg("name")
        .arg("-app")
        .arg("front")
        .output()
        .ok()?;
    let out = String::from_utf8_lossy(&output.stdout);
    // Parse: "name"="AppName"
    out.split('"')
        .nth(3)
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(target_os = "windows")]
fn get_frontmost_app() -> Option<String> {
    None
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn get_frontmost_app() -> Option<String> {
    None
}
