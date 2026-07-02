use arboard::{Clipboard, ImageData};
use std::borrow::Cow;

#[cfg(target_os = "macos")]
use arboard::SetExtApple;
#[cfg(target_os = "macos")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(target_os = "macos")]
static PENDING_GIF_TEMP_FILES: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

#[cfg(target_os = "macos")]
const GIF_TEMP_CLEANUP_DELAY: Duration = Duration::from_secs(60);

#[cfg(target_os = "macos")]
const GIF_TEMP_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[cfg(target_os = "macos")]
pub fn gif_temp_dir() -> PathBuf {
    std::env::temp_dir().join("copyosity-gif-paste")
}

/// How a clipboard write should be treated for history and pasteboard semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardWriteMode {
    /// Copy to clipboard without recording in history (`exclude_from_history` on macOS).
    Copy,
    /// Prepare pasteboard for pasting into another app (standard write + mark own).
    Paste,
}

/// Write text to the system clipboard.
pub fn write_text<'a>(
    clipboard: &mut Clipboard,
    text: impl Into<Cow<'a, str>>,
    mode: ClipboardWriteMode,
) -> Result<(), String> {
    let text = text.into();
    #[cfg(target_os = "macos")]
    {
        match mode {
            ClipboardWriteMode::Copy => {
                clipboard
                    .set()
                    .exclude_from_history()
                    .text(text)
                    .map_err(|e| e.to_string())?;
            }
            ClipboardWriteMode::Paste => {
                clipboard.set_text(text).map_err(|e| e.to_string())?;
            }
        }
        let count = crate::clipboard_macos::change_count();
        crate::clipboard_macos::mark_own_clipboard_write(count);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = mode;
        clipboard.set_text(text).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Write image pixels to the system clipboard.
pub fn write_image(
    clipboard: &mut Clipboard,
    image: ImageData<'static>,
    mode: ClipboardWriteMode,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        match mode {
            ClipboardWriteMode::Copy => {
                clipboard
                    .set()
                    .exclude_from_history()
                    .image(image)
                    .map_err(|e| e.to_string())?;
            }
            ClipboardWriteMode::Paste => {
                clipboard.set_image(image).map_err(|e| e.to_string())?;
            }
        }
        let count = crate::clipboard_macos::change_count();
        crate::clipboard_macos::mark_own_clipboard_write(count);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = mode;
        clipboard.set_image(image).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Write text to the system clipboard without recording it in Copyosity history.
pub fn set_text<'a>(
    clipboard: &mut Clipboard,
    text: impl Into<Cow<'a, str>>,
) -> Result<(), String> {
    write_text(clipboard, text, ClipboardWriteMode::Copy)
}

/// Write image pixels to the system clipboard without recording it in Copyosity history.
pub fn set_image(clipboard: &mut Clipboard, image: ImageData<'static>) -> Result<(), String> {
    write_image(clipboard, image, ClipboardWriteMode::Copy)
}

/// Write animated GIF bytes to the system clipboard.
pub fn write_gif(
    clipboard: &mut Clipboard,
    bytes: &[u8],
    mode: ClipboardWriteMode,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // Telegram/Finder path is more reliable when pasteboard contains file URLs.
        if mode == ClipboardWriteMode::Paste {
            if let Ok(path) = write_temp_gif_file(bytes) {
                if clipboard.set().file_list(&[path]).is_ok() {
                    let count = crate::clipboard_macos::change_count();
                    crate::clipboard_macos::mark_own_clipboard_write(count);
                    return Ok(());
                }
            }
        }
    }

    let exclude_from_history = mode == ClipboardWriteMode::Copy;
    let _ = clipboard;
    crate::clipboard_macos::write_gif_to_pasteboard(bytes, exclude_from_history)?;
    #[cfg(target_os = "macos")]
    {
        let count = crate::clipboard_macos::change_count();
        crate::clipboard_macos::mark_own_clipboard_write(count);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn write_temp_gif_file(bytes: &[u8]) -> Result<PathBuf, String> {
    let base_dir = gif_temp_dir();
    std::fs::create_dir_all(&base_dir).map_err(|e| e.to_string())?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let path = base_dir.join(format!("gif-{ts}.gif"));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    register_pending_gif_temp(path.clone());
    Ok(path)
}

#[cfg(target_os = "macos")]
fn register_pending_gif_temp(path: PathBuf) {
    if let Ok(mut pending) = PENDING_GIF_TEMP_FILES.lock() {
        pending.push(path);
    }
}

/// Schedule removal of temp GIF files after paste completes (target app may read them asynchronously).
#[cfg(target_os = "macos")]
pub fn cleanup_pending_gif_temp() {
    let paths: Vec<PathBuf> = PENDING_GIF_TEMP_FILES
        .lock()
        .map(|mut pending| std::mem::take(&mut *pending))
        .unwrap_or_default();
    if paths.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(GIF_TEMP_CLEANUP_DELAY);
        for path in paths {
            let _ = std::fs::remove_file(path);
        }
    });
}

/// Remove stale GIF temp files left from prior sessions.
#[cfg(target_os = "macos")]
pub fn sweep_stale_gif_temp_files() {
    remove_stale_files_in_dir(&gif_temp_dir(), GIF_TEMP_MAX_AGE);
}

#[cfg(target_os = "macos")]
fn remove_stale_files_in_dir(base_dir: &Path, max_age: Duration) {
    let Ok(entries) = std::fs::read_dir(base_dir) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        if now.duration_since(modified).unwrap_or(Duration::ZERO) > max_age {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn sweep_stale_gif_temp_files() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn sweep_stale_gif_temp_files_removes_old_only() {
        use std::fs::OpenOptions;

        let base_dir =
            std::env::temp_dir().join(format!("copyosity-gif-paste-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base_dir);
        std::fs::create_dir_all(&base_dir).unwrap();

        let old_path = base_dir.join("gif-old.gif");
        let new_path = base_dir.join("gif-new.gif");
        std::fs::write(&old_path, b"GIF89a").unwrap();
        std::fs::write(&new_path, b"GIF89a").unwrap();

        let now = SystemTime::now();
        let stale_mtime = now
            .checked_sub(Duration::from_secs(60))
            .expect("system clock before unix epoch");
        OpenOptions::new()
            .write(true)
            .open(&old_path)
            .unwrap()
            .set_modified(stale_mtime)
            .unwrap();

        remove_stale_files_in_dir(&base_dir, Duration::from_secs(30));

        assert!(!old_path.exists());
        assert!(new_path.exists());

        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn write_temp_gif_file_round_trips_bytes() {
        let bytes = b"GIF89a\x01\x00\x01\x00\x00\x00\x00!";
        let path = write_temp_gif_file(bytes).expect("temp gif file");
        assert!(path.extension().is_some_and(|e| e == "gif"));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn write_gif_returns_error_off_macos() {
        let mut clipboard = Clipboard::new().expect("clipboard");
        let bytes = b"GIF89a\x01\x00\x01\x00\x00\x00\x00!";
        let err = write_gif(&mut clipboard, bytes, ClipboardWriteMode::Copy).unwrap_err();
        assert!(err.contains("macOS"));
    }

    #[cfg(target_os = "macos")]
    fn remove_stale_files_in_dir(base_dir: &Path, max_age: Duration) {
        let Ok(entries) = std::fs::read_dir(base_dir) else {
            return;
        };
        let now = SystemTime::now();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let Ok(modified) = meta.modified() else {
                continue;
            };
            if now.duration_since(modified).unwrap_or(Duration::ZERO) > max_age {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}
