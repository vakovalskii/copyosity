use arboard::Clipboard;
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

use crate::db::{ClipboardEntry, Database};
use crate::image_format;
use crate::ollama;

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif"];

/// Skip decoding very large image files from disk (~20 MB).
const MAX_IMAGE_FILE_BYTES: u64 = 20 * 1024 * 1024;

pub fn is_gif_bytes(data: &[u8]) -> bool {
    data.len() >= 6 && (data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a"))
}

fn encode_png_thumb(image: &DynamicImage) -> Option<String> {
    let thumb = image.thumbnail(240, 160);
    let mut thumb_buf = Cursor::new(Vec::new());
    thumb.write_to(&mut thumb_buf, ImageFormat::Png).ok()?;
    Some(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        thumb_buf.into_inner(),
    ))
}

fn encode_stored_gif(raw: &[u8]) -> Option<(String, String)> {
    if !is_gif_bytes(raw) || raw.len() as u64 > MAX_IMAGE_FILE_BYTES {
        return None;
    }
    let full_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        raw,
    );
    // Keep the GIF even when the first-frame decode fails; only the thumb is optional.
    let thumb_b64 = image::load_from_memory(raw)
        .ok()
        .and_then(|image| encode_png_thumb(&image))
        .unwrap_or_else(|| full_b64.clone());
    Some((full_b64, thumb_b64))
}

fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn encode_image_from_rgba(bytes: &[u8], width: usize, height: usize) -> Option<(String, String)> {
    let rgba = ImageBuffer::<Rgba<u8>, _>::from_raw(width as u32, height as u32, bytes.to_vec())?;
    encode_image_from_dynamic(&DynamicImage::ImageRgba8(rgba))
}

fn encode_image_from_dynamic(image: &DynamicImage) -> Option<(String, String)> {
    let mut full_buf = Cursor::new(Vec::new());
    image.write_to(&mut full_buf, ImageFormat::Png).ok()?;
    let full_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        full_buf.into_inner(),
    );
    let thumb_b64 = encode_png_thumb(image)?;
    Some((full_b64, thumb_b64))
}

fn encode_gif_file(path: &Path) -> Option<(String, String)> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > MAX_IMAGE_FILE_BYTES {
        return None;
    }
    let raw = std::fs::read(path).ok()?;
    encode_stored_gif(&raw)
}

fn encode_image_file(path: &Path) -> Option<(String, String)> {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("gif"))
    {
        return encode_gif_file(path);
    }

    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > MAX_IMAGE_FILE_BYTES {
        return None;
    }
    let image = image::open(path).ok()?;
    encode_image_from_dynamic(&image)
}

fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    let prefix = if data.len() > 4096 {
        &data[..4096]
    } else {
        data
    };
    hasher.update(prefix);
    hasher.update(data.len().to_le_bytes());
    hex::encode(hasher.finalize())
}

fn hash_file_image(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > MAX_IMAGE_FILE_BYTES {
        return None;
    }
    let data = std::fs::read(path).ok()?;
    Some(hash_bytes(&data))
}

fn hash_raster_image(bytes: &[u8], width: usize, height: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.update((width as u64).to_le_bytes());
    hasher.update((height as u64).to_le_bytes());
    hex::encode(hasher.finalize())
}

fn entry_content_hash(base: &str) -> String {
    base.to_string()
}

/// Content fingerprint for dedup when pasteboard changeCount bumps but payload is unchanged.
pub fn probe_clipboard_hash(clipboard: &mut Clipboard) -> Option<String> {
    // Keep the same priority order as `try_capture_from_clipboard`.
    if let Some(gif_bytes) = crate::clipboard_macos::pasteboard_gif_bytes() {
        return Some(hash_bytes(&gif_bytes));
    }

    if let Ok(paths) = clipboard.get().file_list() {
        let image_paths: Vec<PathBuf> = paths.into_iter().filter(|p| is_image_path(p)).collect();
        for path in image_paths {
            if let Some(hash) = hash_file_image(&path) {
                return Some(hash);
            }
        }
    }

    if let Ok(img) = clipboard.get_image() {
        if !img.bytes.is_empty() {
            return Some(hash_raster_image(&img.bytes, img.width, img.height));
        }
    }

    if let Ok(text) = clipboard.get_text() {
        if !text.is_empty() {
            return Some(hash_bytes(text.as_bytes()));
        }
    }

    None
}

fn should_skip_source(db: &Database, bundle_id: &Option<String>) -> bool {
    match bundle_id {
        Some(id) if crate::macos_app::is_copyosity_bundle(id) => true,
        Some(id) => match db.is_app_excluded(id) {
            Ok(excluded) => excluded,
            Err(err) => {
                eprintln!("[clipboard] exclusion check failed: {err}");
                true
            }
        },
        None => crate::macos_app::is_copyosity_frontmost(),
    }
}

struct CaptureContext {
    app: AppHandle,
    db: Arc<Database>,
}

impl CaptureContext {
    fn try_image(
        &self,
        image_full_b64: String,
        image_thumb_b64: String,
        base_hash: String,
        source_bundle_id: Option<String>,
        source_app: Option<String>,
        format_hint: Option<&str>,
    ) -> bool {
        if should_skip_source(&self.db, &source_bundle_id) {
            return false;
        }

        let content_hash = entry_content_hash(&base_hash);
        let image_format = format_hint
            .map(|hint| image_format::normalize(hint).to_string())
            .unwrap_or_else(|| image_format::detect_from_b64(&image_full_b64).to_string());
        let format_tag = image_format::tag_from_format(&image_format);

        let entry = ClipboardEntry {
            id: 0,
            content_type: "image".to_string(),
            text_content: None,
            image_data: Some(image_full_b64),
            image_thumb: Some(image_thumb_b64),
            source_app,
            source_app_icon: None,
            content_hash,
            char_count: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            is_pinned: false,
            collection_id: None,
            tags: vec![format_tag.clone()],
            image_format: Some(image_format),
        };

        match self.db.insert_entry(&entry) {
            Ok((id, is_new)) => {
                if is_new {
                    let _ = self.db.set_entry_tags(id, &[format_tag]);
                    let mut saved = entry.clone();
                    saved.id = id;
                    saved.image_data = None;
                    let _ = self.app.emit("clipboard-changed", &saved);
                }
                true
            }
            Err(_) => false,
        }
    }

    fn try_text(
        &self,
        text: String,
        base_hash: String,
        source_bundle_id: Option<String>,
        source_app: Option<String>,
    ) -> bool {
        if should_skip_source(&self.db, &source_bundle_id) {
            return false;
        }

        let content_hash = entry_content_hash(&base_hash);

        let entry = ClipboardEntry {
            id: 0,
            content_type: "text".to_string(),
            text_content: Some(text.clone()),
            image_data: None,
            image_thumb: None,
            source_app,
            source_app_icon: None,
            content_hash,
            char_count: Some(text.len() as i64),
            created_at: chrono::Utc::now().to_rfc3339(),
            is_pinned: false,
            collection_id: None,
            tags: Vec::new(),
            image_format: None,
        };

        match self.db.insert_entry(&entry) {
            Ok((id, is_new)) => {
                if is_new {
                    let mut saved = entry.clone();
                    saved.id = id;
                    let _ = self.app.emit("clipboard-changed", &saved);

                    let db = self.db.clone();
                    let app = self.app.clone();
                    std::thread::spawn(move || {
                        if !db.is_ai_tagging_enabled() {
                            return;
                        }
                        if let Some(tags) = ollama::tag_text(&text) {
                            if db.set_entry_tags(id, &tags).is_ok() {
                                let _ = app.emit("entry-tagged", id);
                            }
                        } else {
                            let _ = db.set_entry_tag_state(id, "skipped");
                        }
                    });
                }
                true
            }
            Err(_) => false,
        }
    }
}

fn try_capture_from_clipboard(clipboard: &mut Clipboard, ctx: &CaptureContext) -> bool {
    let source = crate::macos_app::frontmost_app_identity();
    let source_bundle_id = source.as_ref().map(|app| app.bundle_id.clone());
    let source_app = source.map(|app| app.display_name);

    // 1. Animated GIF from pasteboard (Telegram/browsers) — keep raw bytes.
    if let Some(gif_bytes) = crate::clipboard_macos::pasteboard_gif_bytes() {
        let base_hash = hash_bytes(&gif_bytes);
        if let Some((full_b64, thumb_b64)) = encode_stored_gif(&gif_bytes) {
            return ctx.try_image(
                full_b64,
                thumb_b64,
                base_hash,
                source_bundle_id.clone(),
                source_app.clone(),
                Some("GIF"),
            );
        }
    }

    // 2. Copied files (Finder, Desktop) — read real pixels from disk.
    //    Must run before get_image(): macOS also puts a generic file-icon TIFF on the pasteboard.
    if let Ok(paths) = clipboard.get().file_list() {
        let image_paths: Vec<PathBuf> = paths.into_iter().filter(|p| is_image_path(p)).collect();
        if !image_paths.is_empty() {
            let mut captured = false;
            for path in image_paths {
                let Some(base_hash) = hash_file_image(&path) else {
                    continue;
                };
                let Some((full_b64, thumb_b64)) = encode_image_file(&path) else {
                    continue;
                };
                let format = image_format::detect_from_path(&path);
                captured |= ctx.try_image(
                    full_b64,
                    thumb_b64,
                    base_hash,
                    source_bundle_id.clone(),
                    source_app.clone(),
                    Some(format),
                );
            }
            if captured {
                return true;
            }
        }
    }

    // 3. Raster image (screenshot to clipboard, Copy Image, etc.)
    if let Ok(img) = clipboard.get_image() {
        if !img.bytes.is_empty() {
            let base_hash = hash_raster_image(&img.bytes, img.width, img.height);
            if let Some((full_b64, thumb_b64)) =
                encode_image_from_rgba(&img.bytes, img.width, img.height)
            {
                return ctx.try_image(
                    full_b64,
                    thumb_b64,
                    base_hash,
                    source_bundle_id,
                    source_app,
                    None,
                );
            }
        }
    }

    // 4. Plain text
    if let Ok(text) = clipboard.get_text() {
        if text.is_empty() {
            return false;
        }
        let base_hash = hash_bytes(text.as_bytes());
        return ctx.try_text(text, base_hash, source_bundle_id, source_app);
    }

    false
}

/// Capture-retry state for the clipboard monitor loop.
struct CaptureRetryState {
    last_content_hash: String,
    capture_pending: bool,
}

impl CaptureRetryState {
    fn new() -> Self {
        Self {
            last_content_hash: String::new(),
            capture_pending: false,
        }
    }

    /// One capture attempt after pasteboard change (when `capture_pending` is true).
    /// On failure (e.g. GIF encode error), keeps `capture_pending` so the next tick retries
    /// without advancing `last_content_hash` (hash poisoning fix).
    fn attempt_capture<F>(&mut self, probe_hash: Option<String>, try_capture: F)
    where
        F: FnOnce() -> bool,
    {
        if !self.capture_pending {
            return;
        }

        let Some(probe_hash) = probe_hash else {
            self.capture_pending = false;
            return;
        };
        if probe_hash == self.last_content_hash {
            self.capture_pending = false;
            return;
        }

        if try_capture() {
            self.last_content_hash = probe_hash;
            self.capture_pending = false;
        }
    }
}

pub fn start_clipboard_monitor(app: AppHandle) {
    let db = app.state::<Arc<Database>>().inner().clone();

    std::thread::spawn(move || {
        let mut clipboard = Clipboard::new().expect("Failed to access clipboard");
        #[cfg(target_os = "macos")]
        let mut last_change_count = crate::clipboard_macos::change_count();
        let mut state = CaptureRetryState::new();

        loop {
            std::thread::sleep(std::time::Duration::from_millis(300));

            #[cfg(target_os = "macos")]
            {
                let change_count = crate::clipboard_macos::change_count();
                if change_count != last_change_count {
                    last_change_count = change_count;
                    state.capture_pending = true;

                    if crate::clipboard_macos::should_ignore_capture(change_count)
                        || crate::clipboard_macos::is_concealed()
                    {
                        state.capture_pending = false;
                    }
                }
            }

            #[cfg(not(target_os = "macos"))]
            {
                state.capture_pending = true;
            }

            let ctx = CaptureContext {
                app: app.clone(),
                db: db.clone(),
            };
            state.attempt_capture(
                probe_clipboard_hash(&mut clipboard),
                || try_capture_from_clipboard(&mut clipboard, &ctx),
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use rusqlite::Connection;
    use std::path::Path;
    use std::sync::Mutex;

    fn test_db() -> Database {
        let db = Database {
            conn: Mutex::new(Connection::open_in_memory().unwrap()),
        };
        db.conn.lock().unwrap().execute_batch("
            CREATE TABLE IF NOT EXISTS excluded_apps (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bundle_id TEXT NOT NULL UNIQUE
            );
        ").unwrap();
        db
    }

    #[test]
    fn entry_content_hash_uses_base_only() {
        assert_eq!(entry_content_hash("abc123"), "abc123");
        assert_eq!(entry_content_hash("abc123"), entry_content_hash("abc123"));
    }

    #[test]
    fn hash_bytes_is_deterministic() {
        let h1 = hash_bytes(b"hello");
        let h2 = hash_bytes(b"hello");
        assert_eq!(h1, h2);
        assert_ne!(h1, hash_bytes(b"world"));
    }

    #[test]
    fn hash_bytes_distinguishes_same_prefix_different_length() {
        let short = hash_bytes(b"abcdefghijklmnop");
        let long = hash_bytes(b"abcdefghijklmnop-extra");
        assert_ne!(short, long);
    }

    #[test]
    fn hash_raster_image_includes_dimensions() {
        let bytes = vec![0u8; 12];
        let small = hash_raster_image(&bytes, 2, 2);
        let large = hash_raster_image(&bytes, 4, 3);
        assert_ne!(small, large);
    }

    #[test]
    fn is_image_path_checks_extensions() {
        assert!(is_image_path(Path::new("/tmp/photo.PNG")));
        assert!(is_image_path(Path::new("/tmp/photo.jpg")));
        assert!(!is_image_path(Path::new("/tmp/readme.txt")));
        assert!(!is_image_path(Path::new("/tmp/noext")));
    }

    #[test]
    fn is_gif_bytes_checks_magic() {
        assert!(is_gif_bytes(b"GIF89a\x00\x00\x00"));
        assert!(is_gif_bytes(b"GIF87a\x00\x00\x00"));
        assert!(!is_gif_bytes(b"\x89PNG\r\n"));
        assert!(!is_gif_bytes(b"GIF89"));
    }

    #[test]
    fn encode_stored_gif_keeps_bytes_when_thumb_decode_fails() {
        let minimal = b"GIF89a\x01\x00\x01\x00\x00\x00\x00!";
        let (full, thumb) = encode_stored_gif(minimal).expect("should store minimal gif");
        assert!(!full.is_empty());
        assert!(!thumb.is_empty());
    }

    #[test]
    fn should_skip_source_copyosity_and_excluded_apps() {
        let db = test_db();
        assert!(should_skip_source(
            &db,
            &Some(crate::macos_app::COPYOSITY_BUNDLE_ID.to_string()),
        ));
        assert_eq!(
            should_skip_source(&db, &None),
            crate::macos_app::is_copyosity_frontmost()
        );
        assert!(!should_skip_source(
            &db,
            &Some("com.apple.Safari".to_string()),
        ));

        db.add_excluded_app("com.apple.Safari").unwrap();
        assert!(should_skip_source(
            &db,
            &Some("com.apple.Safari".to_string()),
        ));
    }

    #[test]
    fn failed_capture_keeps_pending_without_hash_update() {
        let mut state = CaptureRetryState::new();
        state.capture_pending = true;
        let probe = "gif-encode-failed-hash";

        // Simulates try_capture_from_clipboard returning false (e.g. failed GIF encode).
        state.attempt_capture(Some(probe.to_string()), || false);

        assert!(
            state.capture_pending,
            "capture_pending must stay true so the monitor retries"
        );
        assert!(
            state.last_content_hash.is_empty(),
            "hash must not advance on failed capture"
        );
    }

    #[test]
    fn successful_capture_clears_pending_and_updates_hash() {
        let mut state = CaptureRetryState::new();
        state.capture_pending = true;
        let probe = "captured-content-hash";

        state.attempt_capture(Some(probe.to_string()), || true);

        assert!(!state.capture_pending);
        assert_eq!(state.last_content_hash, probe);
    }

    #[test]
    fn retry_after_failed_capture_succeeds_on_next_tick() {
        let mut state = CaptureRetryState::new();
        state.capture_pending = true;
        let probe = "retry-gif-hash";
        let mut attempts = 0;

        state.attempt_capture(Some(probe.to_string()), || {
            attempts += 1;
            false
        });
        assert!(state.capture_pending);
        assert_eq!(attempts, 1);

        state.attempt_capture(Some(probe.to_string()), || {
            attempts += 1;
            true
        });
        assert!(!state.capture_pending);
        assert_eq!(state.last_content_hash, probe);
        assert_eq!(attempts, 2);
    }
}
