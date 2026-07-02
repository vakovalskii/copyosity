use arboard::Clipboard;
use image::{DynamicImage, GenericImageView, ImageBuffer, ImageFormat, Rgba};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

/// Bumped when history is cleared so the monitor can recapture clipboard content
/// whose hash is still in `last_content_hash` but no longer in the database.
static HISTORY_CLEAR_EPOCH: AtomicU64 = AtomicU64::new(0);

pub fn notify_history_cleared() {
    HISTORY_CLEAR_EPOCH.fetch_add(1, Ordering::Release);
}

use crate::db::{ClipboardEntry, Database, EntryOcrPayload, EntryTaggedPayload};

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif"];

/// Skip decoding very large image files from disk (~20 MB).
const MAX_IMAGE_FILE_BYTES: u64 = 20 * 1024 * 1024;

pub fn is_gif_bytes(data: &[u8]) -> bool {
    data.len() >= 6 && (data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a"))
}

struct EncodedImage {
    full_b64: String,
    thumb_b64: String,
    width: i64,
    height: i64,
    byte_size: i64,
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

fn encode_stored_gif(raw: &[u8]) -> Option<EncodedImage> {
    if !is_gif_bytes(raw) || raw.len() as u64 > MAX_IMAGE_FILE_BYTES {
        return None;
    }
    let full_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, raw);
    let decoded = image::load_from_memory(raw).ok();
    let (width, height) = decoded
        .as_ref()
        .map(|image| image.dimensions())
        .map(|(w, h)| (w as i64, h as i64))
        .unwrap_or((0, 0));
    let thumb_b64 = decoded
        .as_ref()
        .and_then(encode_png_thumb)
        .unwrap_or_else(|| full_b64.clone());
    Some(EncodedImage {
        full_b64,
        thumb_b64,
        width,
        height,
        byte_size: raw.len() as i64,
    })
}

fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Single-line clipboard text that is only an image filename (Finder fallback label).
fn is_probable_image_filename(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains('\n') || trimmed.contains('\r') {
        return false;
    }
    is_image_path(Path::new(trimmed))
}

fn image_file_paths_from_clipboard(clipboard: &mut Clipboard) -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let native: Vec<PathBuf> = crate::clipboard_macos::pasteboard_file_paths()
            .into_iter()
            .filter(|p| is_image_path(p))
            .collect();
        if !native.is_empty() {
            return native;
        }
    }

    clipboard
        .get()
        .file_list()
        .ok()
        .map(|paths| paths.into_iter().filter(|p| is_image_path(p)).collect())
        .unwrap_or_default()
}

fn encode_image_from_rgba(bytes: &[u8], width: usize, height: usize) -> Option<EncodedImage> {
    let rgba = ImageBuffer::<Rgba<u8>, _>::from_raw(width as u32, height as u32, bytes.to_vec())?;
    encode_image_from_dynamic(&DynamicImage::ImageRgba8(rgba))
}

fn encode_image_from_dynamic(image: &DynamicImage) -> Option<EncodedImage> {
    let mut full_buf = Cursor::new(Vec::new());
    image.write_to(&mut full_buf, ImageFormat::Png).ok()?;
    let full_bytes = full_buf.into_inner();
    let full_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &full_bytes);
    let thumb_b64 = encode_png_thumb(image)?;
    let (width, height) = image.dimensions();
    Some(EncodedImage {
        full_b64,
        thumb_b64,
        width: width as i64,
        height: height as i64,
        byte_size: full_bytes.len() as i64,
    })
}

fn encode_gif_file(path: &Path) -> Option<EncodedImage> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > MAX_IMAGE_FILE_BYTES {
        return None;
    }
    let raw = std::fs::read(path).ok()?;
    encode_stored_gif(&raw)
}

fn encode_image_file(path: &Path) -> Option<EncodedImage> {
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
    let mut encoded = encode_image_from_dynamic(&image)?;
    encoded.byte_size = metadata.len() as i64;
    Some(encoded)
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
    base.to_owned()
}

/// Content fingerprint for dedup when pasteboard changeCount bumps but payload is unchanged.
pub fn probe_clipboard_hash(clipboard: &mut Clipboard) -> Option<String> {
    // Keep the same priority order as `try_capture_from_clipboard`.
    if let Some(gif_bytes) = crate::clipboard_macos::pasteboard_gif_bytes() {
        return Some(hash_bytes(&gif_bytes));
    }

    let image_paths = image_file_paths_from_clipboard(clipboard);
    for path in &image_paths {
        if let Some(hash) = hash_file_image(path) {
            return Some(hash);
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
    /// Notify the overlay after a capture. Loads the row from the DB so
    /// re-copies emit the bumped `created_at` and existing tags/OCR text.
    fn emit_clipboard_changed(&self, entry_id: i64, fallback: &ClipboardEntry) {
        let mut saved = self
            .db
            .get_entry_by_id(entry_id)
            .ok()
            .flatten()
            .unwrap_or_else(|| {
                let mut entry = fallback.clone();
                entry.id = entry_id;
                entry
            });
        saved.image_data = None;
        let _ = self.app.emit("clipboard-changed", &saved);
    }

    fn try_image(
        &self,
        encoded: EncodedImage,
        base_hash: String,
        source_bundle_id: Option<String>,
        source_app: Option<String>,
        format_hint: Option<&str>,
    ) -> Option<String> {
        if should_skip_source(&self.db, &source_bundle_id) {
            return None;
        }

        let content_hash = entry_content_hash(&base_hash);
        let image_format = format_hint
            .map(|hint| crate::image_format::normalize(hint).to_owned())
            .unwrap_or_else(|| crate::image_format::detect_from_b64(&encoded.full_b64).to_owned());
        let format_tag = crate::image_format::tag_from_format(&image_format);

        let entry = ClipboardEntry {
            id: 0,
            content_type: "image".to_owned(),
            text_content: None,
            image_data: Some(encoded.full_b64.clone()),
            image_thumb: Some(encoded.thumb_b64.clone()),
            source_app,
            source_app_icon: None,
            content_hash: content_hash.clone(),
            char_count: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            is_pinned: false,
            collection_id: None,
            tags: vec![format_tag.clone()],
            ocr_text: None,
            image_format: Some(image_format),
            image_width: Some(encoded.width),
            image_height: Some(encoded.height),
            image_byte_size: Some(encoded.byte_size),
        };

        match self.db.insert_entry(&entry) {
            Ok((id, is_new)) => {
                if is_new {
                    let _ = self.db.set_entry_tags(id, &[format_tag]);

                    let db = self.db.clone();
                    let app = self.app.clone();
                    let png_b64_for_ocr = encoded.full_b64;
                    let thumb_b64_for_tag = encoded.thumb_b64;
                    std::thread::spawn(move || {
                        let ocr_text = base64::Engine::decode(
                            &base64::engine::general_purpose::STANDARD,
                            &png_b64_for_ocr,
                        )
                        .ok()
                        .and_then(|bytes| crate::ocr::ocr_image_png(&bytes))
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty());

                        if let Some(text) = &ocr_text {
                            if db.set_ocr_text(id, text).is_ok() {
                                let _ = app.emit(
                                    "entry-ocr",
                                    EntryOcrPayload {
                                        entry_id: id,
                                        ocr_text: text.clone(),
                                    },
                                );
                            }
                        }

                        let settings = db.get_app_settings().ok();
                        let use_hub = settings
                            .as_ref()
                            .map(crate::tagging::hub_tagging_configured)
                            .unwrap_or(false);

                        let tags = if use_hub {
                            let s = settings.as_ref().unwrap();
                            crate::hub::tag_image(
                                &s.hub_url,
                                &s.hub_token,
                                crate::tagging::HUB_IMAGE_TAG_MODEL,
                                &thumb_b64_for_tag,
                            )
                            .or_else(|| ocr_text.as_ref().and_then(|t| crate::tagging::tag(&db, t)))
                        } else if crate::tagging::should_auto_tag_text_on_capture(&db) {
                            ocr_text.as_ref().and_then(|t| crate::tagging::tag(&db, t))
                        } else {
                            None
                        };

                        if let Some(tags) = tags {
                            if db.set_entry_tags(id, &tags).is_ok() {
                                let _ = app.emit(
                                    "entry-tagged",
                                    EntryTaggedPayload { entry_id: id, tags },
                                );
                            }
                        }
                    });
                }
                self.emit_clipboard_changed(id, &entry);
                Some(content_hash)
            }
            Err(_) => None,
        }
    }

    fn try_text(
        &self,
        text: String,
        base_hash: String,
        source_bundle_id: Option<String>,
        source_app: Option<String>,
    ) -> Option<String> {
        if should_skip_source(&self.db, &source_bundle_id) {
            return None;
        }

        let content_hash = entry_content_hash(&base_hash);

        let entry = ClipboardEntry {
            id: 0,
            content_type: "text".to_owned(),
            text_content: Some(text.clone()),
            image_data: None,
            image_thumb: None,
            source_app,
            source_app_icon: None,
            content_hash: content_hash.clone(),
            char_count: Some(text.len() as i64),
            created_at: chrono::Utc::now().to_rfc3339(),
            is_pinned: false,
            collection_id: None,
            tags: Vec::new(),
            ocr_text: None,
            image_format: None,
            image_width: None,
            image_height: None,
            image_byte_size: None,
        };

        match self.db.insert_entry(&entry) {
            Ok((id, is_new)) => {
                if is_new {
                    let db = self.db.clone();
                    let app = self.app.clone();
                    std::thread::spawn(move || {
                        if !crate::tagging::should_auto_tag_text_on_capture(&db) {
                            return;
                        }
                        if let Some(tags) = crate::tagging::tag(&db, &text) {
                            if db.set_entry_tags(id, &tags).is_ok() {
                                let _ = app.emit(
                                    "entry-tagged",
                                    EntryTaggedPayload { entry_id: id, tags },
                                );
                            }
                        } else {
                            let _ = db.set_entry_tag_state(id, "skipped");
                        }
                    });
                }
                self.emit_clipboard_changed(id, &entry);
                Some(content_hash)
            }
            Err(_) => None,
        }
    }
}

fn try_capture_from_clipboard(clipboard: &mut Clipboard, ctx: &CaptureContext) -> Option<String> {
    let source = crate::macos_app::frontmost_app_identity();
    let source_bundle_id = source.as_ref().map(|app| app.bundle_id.clone());
    let source_app = source.map(|app| app.display_name);

    // 1. Animated GIF from pasteboard (Telegram/browsers) — keep raw bytes.
    if let Some(gif_bytes) = crate::clipboard_macos::pasteboard_gif_bytes() {
        let base_hash = hash_bytes(&gif_bytes);
        if let Some(encoded) = encode_stored_gif(&gif_bytes) {
            return ctx.try_image(
                encoded,
                base_hash,
                source_bundle_id,
                source_app,
                Some("GIF"),
            );
        }
    }

    // 2. Copied files (Finder, Desktop) — read real pixels from disk.
    //    Must run before get_image(): macOS also puts a generic file-icon TIFF on the pasteboard.
    let image_paths = image_file_paths_from_clipboard(clipboard);
    if !image_paths.is_empty() {
        let mut stored_hash = None;
        for path in image_paths {
            let Some(base_hash) = hash_file_image(&path) else {
                continue;
            };
            let Some(encoded) = encode_image_file(&path) else {
                continue;
            };
            let format = crate::image_format::detect_from_path(&path);
            if let Some(hash) = ctx.try_image(
                encoded,
                base_hash,
                source_bundle_id.clone(),
                source_app.clone(),
                Some(format),
            ) {
                stored_hash = Some(hash);
            }
        }
        // Never fall through to text when the pasteboard advertises image files.
        return stored_hash;
    }

    // 3. Raster image (screenshot to clipboard, Copy Image, etc.)
    if let Ok(img) = clipboard.get_image() {
        if !img.bytes.is_empty() {
            let base_hash = hash_raster_image(&img.bytes, img.width, img.height);
            if let Some(encoded) = encode_image_from_rgba(&img.bytes, img.width, img.height) {
                return ctx.try_image(encoded, base_hash, source_bundle_id, source_app, None);
            }
        }
    }

    // 4. Plain text — skip Finder filename stubs when image paths were unavailable.
    if let Ok(text) = clipboard.get_text() {
        if text.is_empty() {
            return None;
        }
        if is_probable_image_filename(&text) {
            return None;
        }
        let base_hash = hash_bytes(text.as_bytes());
        return ctx.try_text(text, base_hash, source_bundle_id, source_app);
    }

    None
}

/// Capture-retry state for the clipboard monitor loop.
struct CaptureRetryState {
    last_content_hash: String,
    capture_pending: bool,
    history_epoch: u64,
}

impl CaptureRetryState {
    fn new() -> Self {
        Self {
            last_content_hash: String::new(),
            capture_pending: false,
            history_epoch: HISTORY_CLEAR_EPOCH.load(Ordering::Acquire),
        }
    }

    /// After history clear or deleting the last unpinned entry: snapshot the current
    /// pasteboard hash so stale clipboard content is not re-inserted until the user
    /// copies again (pasteboard changeCount bump).
    fn sync_history_clear(&mut self, current_probe: Option<&String>) {
        let epoch = HISTORY_CLEAR_EPOCH.load(Ordering::Acquire);
        if epoch == self.history_epoch {
            return;
        }
        self.history_epoch = epoch;
        self.last_content_hash = current_probe.cloned().unwrap_or_default();
        self.capture_pending = false;
    }

    /// One capture attempt after pasteboard change (when `capture_pending` is true).
    /// On failure (e.g. GIF encode error), keeps `capture_pending` so the next tick retries
    /// without advancing `last_content_hash` (hash poisoning fix).
    fn attempt_capture<F>(&mut self, probe_hash: Option<String>, hash_in_db: bool, try_capture: F)
    where
        F: FnOnce() -> Option<String>,
    {
        if !self.capture_pending {
            return;
        }

        let Some(probe_hash) = probe_hash else {
            self.capture_pending = false;
            return;
        };
        if probe_hash == self.last_content_hash && hash_in_db {
            self.capture_pending = false;
            return;
        }

        if let Some(captured_hash) = try_capture() {
            self.last_content_hash = captured_hash;
            self.capture_pending = false;
        }
    }
}

pub fn start_clipboard_monitor(app: AppHandle) {
    let db = app.state::<Arc<Database>>().inner().clone();

    std::thread::spawn(move || {
        let mut clipboard = loop {
            match Clipboard::new() {
                Ok(cb) => break cb,
                Err(e) => {
                    eprintln!("[clipboard] init failed, retrying in 1s: {}", e);
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        };
        #[cfg(target_os = "macos")]
        let mut last_change_count = crate::clipboard_macos::change_count();
        let mut state = CaptureRetryState::new();

        loop {
            std::thread::sleep(std::time::Duration::from_millis(300));

            let probe_hash = probe_clipboard_hash(&mut clipboard);
            state.sync_history_clear(probe_hash.as_ref());

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

            let hash_in_db = match (&probe_hash, state.capture_pending) {
                (Some(h), true) if h == &state.last_content_hash => {
                    db.has_entry_with_content_hash(h).unwrap_or(false)
                }
                _ => false,
            };

            let ctx = CaptureContext {
                app: app.clone(),
                db: db.clone(),
            };
            state.attempt_capture(probe_hash, hash_in_db, || {
                try_capture_from_clipboard(&mut clipboard, &ctx)
            });
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
        db.conn
            .lock()
            .unwrap()
            .execute_batch(
                "
            CREATE TABLE IF NOT EXISTS excluded_apps (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bundle_id TEXT NOT NULL UNIQUE
            );
        ",
            )
            .unwrap();
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
        let encoded = encode_stored_gif(minimal).expect("should store minimal gif");
        assert!(!encoded.full_b64.is_empty());
        assert!(!encoded.thumb_b64.is_empty());
    }

    #[test]
    fn should_skip_source_copyosity_and_excluded_apps() {
        let db = test_db();
        assert!(should_skip_source(
            &db,
            &Some(crate::macos_app::COPYOSITY_BUNDLE_ID.to_owned()),
        ));
        assert_eq!(
            should_skip_source(&db, &None),
            crate::macos_app::is_copyosity_frontmost()
        );
        assert!(!should_skip_source(
            &db,
            &Some("com.apple.Safari".to_owned()),
        ));

        db.add_excluded_app("com.apple.Safari").unwrap();
        assert!(should_skip_source(
            &db,
            &Some("com.apple.Safari".to_owned()),
        ));
    }

    #[test]
    fn failed_capture_keeps_pending_without_hash_update() {
        let mut state = CaptureRetryState::new();
        state.capture_pending = true;
        let probe = "gif-encode-failed-hash";

        // Simulates try_capture_from_clipboard returning None (e.g. failed GIF encode).
        state.attempt_capture(Some(probe.to_owned()), false, || None);

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
    fn history_clear_snapshots_clipboard_without_auto_capture() {
        let mut state = CaptureRetryState::new();
        let probe = "finder-jpg-hash";

        state.capture_pending = true;
        state.attempt_capture(Some(probe.to_owned()), false, || Some(probe.to_owned()));
        assert_eq!(state.last_content_hash, probe);

        notify_history_cleared();
        state.sync_history_clear(Some(&probe.to_owned()));
        assert!(
            !state.capture_pending,
            "must not auto-recapture stale clipboard"
        );
        assert_eq!(state.last_content_hash, probe);

        let mut captured = false;
        state.attempt_capture(Some(probe.to_owned()), false, || {
            captured = true;
            Some(probe.to_owned())
        });
        assert!(!captured, "no pasteboard change yet");
    }

    #[test]
    fn history_clear_allows_recapture_after_new_copy() {
        let mut state = CaptureRetryState::new();
        let probe = "finder-gif-hash";

        state.capture_pending = true;
        state.attempt_capture(Some(probe.to_owned()), false, || Some(probe.to_owned()));
        assert_eq!(state.last_content_hash, probe);

        notify_history_cleared();
        state.sync_history_clear(Some(&probe.to_owned()));

        // User copies the same file again (pasteboard changeCount bumped).
        state.capture_pending = true;
        let mut captured = false;
        state.attempt_capture(Some(probe.to_owned()), false, || {
            captured = true;
            Some(probe.to_owned())
        });
        assert!(captured);
        assert_eq!(state.last_content_hash, probe);
    }

    #[test]
    fn successful_capture_clears_pending_and_updates_hash() {
        let mut state = CaptureRetryState::new();
        state.capture_pending = true;
        let probe = "captured-content-hash";

        state.attempt_capture(Some(probe.to_owned()), false, || Some(probe.to_owned()));

        assert!(!state.capture_pending);
        assert_eq!(state.last_content_hash, probe);
    }

    #[test]
    fn unchanged_clipboard_skipped_when_hash_already_in_db() {
        let mut state = CaptureRetryState::new();
        let probe = "existing-entry-hash";
        state.last_content_hash = probe.to_owned();
        state.capture_pending = true;

        let mut captured = false;
        state.attempt_capture(Some(probe.to_owned()), true, || {
            captured = true;
            Some(probe.to_owned())
        });

        assert!(!captured);
        assert!(!state.capture_pending);
    }

    #[test]
    fn is_probable_image_filename_detects_basename_only() {
        assert!(is_probable_image_filename("3.jpg"));
        assert!(is_probable_image_filename("images 2.jpeg"));
        assert!(!is_probable_image_filename("readme.txt"));
        assert!(!is_probable_image_filename("line one\nline two"));
    }

    #[test]
    fn successful_capture_uses_captured_hash_not_probe() {
        let mut state = CaptureRetryState::new();
        state.capture_pending = true;
        let probe = "file-probe-hash";
        let captured = "stored-content-hash";

        state.attempt_capture(Some(probe.to_owned()), false, || Some(captured.to_owned()));

        assert_eq!(state.last_content_hash, captured);
        assert_ne!(state.last_content_hash, probe);
    }

    #[test]
    fn retry_after_failed_capture_succeeds_on_next_tick() {
        let mut state = CaptureRetryState::new();
        state.capture_pending = true;
        let probe = "retry-gif-hash";
        let mut attempts = 0;

        state.attempt_capture(Some(probe.to_owned()), false, || {
            attempts += 1;
            None
        });
        assert!(state.capture_pending);
        assert_eq!(attempts, 1);

        state.attempt_capture(Some(probe.to_owned()), false, || {
            attempts += 1;
            Some(probe.to_owned())
        });
        assert!(!state.capture_pending);
        assert_eq!(state.last_content_hash, probe);
        assert_eq!(attempts, 2);
    }
}
