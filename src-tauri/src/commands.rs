use crate::db::{AppSettings, ClipboardEntry, Collection, Database, ExcludedApp, ModelCatalog};

#[cfg(not(target_os = "macos"))]
fn simulate_paste() {}

use crate::ollama;
use arboard::{Clipboard, ImageData};
use base64::Engine;
use image::GenericImageView;
use std::borrow::Cow;

use crate::clipboard_write::{self, ClipboardWriteMode};
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

#[tauri::command]
pub fn get_entries(
    db: State<'_, Arc<Database>>,
    limit: Option<i64>,
    offset: Option<i64>,
    collection_id: Option<i64>,
    pinned_only: Option<bool>,
    search: Option<String>,
) -> Result<Vec<ClipboardEntry>, String> {
    db.get_entries(
        limit.unwrap_or(50),
        offset.unwrap_or(0),
        collection_id,
        pinned_only.unwrap_or(false),
        search.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_entry(db: State<'_, Arc<Database>>, id: i64) -> Result<(), String> {
    db.delete_entry(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pin_entry(db: State<'_, Arc<Database>>, id: i64, pinned: bool) -> Result<(), String> {
    db.pin_entry(id, pinned).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_entry_collection(
    db: State<'_, Arc<Database>>,
    entry_id: i64,
    collection_id: Option<i64>,
) -> Result<(), String> {
    db.set_collection(entry_id, collection_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_collections(db: State<'_, Arc<Database>>) -> Result<Vec<Collection>, String> {
    db.get_collections().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_collection(
    db: State<'_, Arc<Database>>,
    name: String,
    color: Option<String>,
) -> Result<i64, String> {
    db.create_collection(&name, color.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_collection(db: State<'_, Arc<Database>>, id: i64) -> Result<(), String> {
    db.delete_collection(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_history(db: State<'_, Arc<Database>>) -> Result<(), String> {
    db.clear_history().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn hide_main_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::hide_panel(&app);
    Ok(())
}

#[cfg(target_os = "macos")]
fn activate_for_settings_window() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        app.activate();
    }
}

#[tauri::command]
pub fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    // If settings window already exists, just focus it
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
        #[cfg(target_os = "macos")]
        activate_for_settings_window();
        let _ = window.emit("settings-shown", ());
        return Ok(());
    }

    // Create a new settings window
    let builder = tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("/settings".into()),
    )
    .title("Copyosity Settings")
    .inner_size(580.0, 680.0)
    .resizable(true);

    let window = builder.build().map_err(|e| e.to_string())?;
    let _ = window.show();
    let _ = window.set_focus();
    #[cfg(target_os = "macos")]
    activate_for_settings_window();

    Ok(())
}

#[tauri::command]
pub fn quit_app(_app: tauri::AppHandle) -> Result<(), String> {
    std::process::exit(0);
}

#[tauri::command]
pub fn get_app_settings(db: State<'_, Arc<Database>>) -> Result<AppSettings, String> {
    db.get_app_settings().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_app_settings(
    db: State<'_, Arc<Database>>,
    ollama_model: Option<String>,
    retention_days: Option<i64>,
    whisper_server_url: Option<String>,
    whisper_server_token: Option<String>,
    whisper_server_model: Option<String>,
    voice_shortcut: Option<String>,
    selected_microphone: Option<String>,
) -> Result<AppSettings, String> {
    if let Some(model) = ollama_model.as_deref() {
        ollama::validate_model_name(model)?;
    }

    let settings = db
        .update_app_settings(
            ollama_model.as_deref(),
            retention_days,
            whisper_server_url.as_deref(),
            whisper_server_token.as_deref(),
            whisper_server_model.as_deref(),
            voice_shortcut.as_deref(),
            selected_microphone.as_deref(),
        )
        .map_err(|e| e.to_string())?;

    ollama::set_active_model(&settings.ollama_model);
    ollama::ensure_runtime();

    db.cleanup_old_entries(settings.retention_days)
        .map_err(|e| e.to_string())?;

    Ok(settings)
}

#[tauri::command]
pub fn get_model_catalog() -> Result<ModelCatalog, String> {
    Ok(ollama::model_catalog())
}

#[tauri::command]
pub fn get_excluded_apps(db: State<'_, Arc<Database>>) -> Result<Vec<ExcludedApp>, String> {
    db.get_excluded_apps().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_excluded_app(db: State<'_, Arc<Database>>, bundle_id: String) -> Result<(), String> {
    db.add_excluded_app(&bundle_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_excluded_app(db: State<'_, Arc<Database>>, id: i64) -> Result<(), String> {
    db.remove_excluded_app(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_frontmost_app_to_excluded(
    db: State<'_, Arc<Database>>,
) -> Result<Option<String>, String> {
    let app_name = crate::clipboard_monitor::get_frontmost_app();
    if let Some(app_name) = &app_name {
        db.add_excluded_app(app_name).map_err(|e| e.to_string())?;
    }
    Ok(app_name)
}

#[tauri::command]
pub fn retag_entry(
    app: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    entry_id: i64,
) -> Result<(), String> {
    let Some(text) = db.get_entry_text(entry_id).map_err(|e| e.to_string())? else {
        return Ok(());
    };

    match ollama::tag_text(&text) {
        Some(tags) => db.set_entry_tags(entry_id, &tags).map_err(|e| e.to_string())?,
        None => db
            .set_entry_tag_state(entry_id, "skipped")
            .map_err(|e| e.to_string())?,
    }

    let _ = app.emit("entry-tagged", entry_id);
    Ok(())
}

#[tauri::command]
pub fn copy_entry(db: State<'_, Arc<Database>>, entry_id: i64) -> Result<(), String> {
    let Some(entry) = db.get_entry_by_id(entry_id).map_err(|e| e.to_string())? else {
        return Ok(());
    };

    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;

    match entry.content_type.as_str() {
        "text" => {
            if let Some(text) = entry.text_content {
                clipboard_write::set_text(&mut clipboard, text)?;
            }
        }
        "image" => {
            write_image_entry(&mut clipboard, &entry, ClipboardWriteMode::Copy)?;
        }
        _ => {}
    }

    Ok(())
}

fn write_entry_for_paste(clipboard: &mut Clipboard, entry: &ClipboardEntry) -> Result<(), String> {
    match entry.content_type.as_str() {
        "text" => {
            if let Some(text) = &entry.text_content {
                clipboard_write::write_text(clipboard, text.clone(), ClipboardWriteMode::Paste)?;
            }
        }
        "image" => {
            write_image_entry(clipboard, entry, ClipboardWriteMode::Paste)?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn finish_paste(_app: &tauri::AppHandle) {
    crate::hide_panel(_app);

    // Paste must run off the main thread: blocking here prevents the run loop from
    // completing panel hide and returning focus to the target app (Cursor/Electron).
    crate::clipboard_macos::spawn_automated_paste(true);
}

#[cfg(not(target_os = "macos"))]
fn finish_paste(app: &tauri::AppHandle) {
    crate::hide_panel(app);
    simulate_paste();
}

fn image_bytes_from_entry(entry: &ClipboardEntry) -> Result<Vec<u8>, String> {
    let encoded = entry
        .image_data
        .as_ref()
        .or(entry.image_thumb.as_ref())
        .ok_or_else(|| "Image data is missing".to_string())?;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| e.to_string())
}

fn raster_image_from_bytes(bytes: &[u8]) -> Result<ImageData<'static>, String> {
    let image = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let rgba = image.to_rgba8();
    let (width, height) = image.dimensions();
    Ok(ImageData {
        width: width as usize,
        height: height as usize,
        bytes: Cow::Owned(rgba.into_raw()),
    })
}

fn write_image_entry(
    clipboard: &mut Clipboard,
    entry: &ClipboardEntry,
    mode: ClipboardWriteMode,
) -> Result<(), String> {
    let bytes = image_bytes_from_entry(entry)?;
    if crate::clipboard_monitor::is_gif_bytes(&bytes) {
        return clipboard_write::write_gif(clipboard, &bytes, mode);
    }

    let image = raster_image_from_bytes(&bytes)?;
    match mode {
        ClipboardWriteMode::Copy => clipboard_write::set_image(clipboard, image),
        ClipboardWriteMode::Paste => clipboard_write::write_image(clipboard, image, mode),
    }
}

fn paste_text_into_target(app: &tauri::AppHandle, text: String) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard_write::write_text(&mut clipboard, text, ClipboardWriteMode::Paste)?;
    finish_paste(app);
    Ok(())
}

#[tauri::command]
pub fn activate_entry(app: tauri::AppHandle, db: State<'_, Arc<Database>>, entry_id: i64) -> Result<(), String> {
    let Some(entry) = db.get_entry_by_id(entry_id).map_err(|e| e.to_string())? else {
        return Ok(());
    };

    if entry.content_type == "text" {
        let Some(text) = entry.text_content else {
            return Ok(());
        };
        return paste_text_into_target(&app, text);
    }

    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    write_entry_for_paste(&mut clipboard, &entry)?;
    finish_paste(&app);

    Ok(())
}

#[tauri::command]
pub fn paste_entry(app: tauri::AppHandle, text: String) -> Result<(), String> {
    paste_text_into_target(&app, text)
}

#[tauri::command]
pub fn check_accessibility(prompt: bool) -> Result<bool, String> {
    Ok(crate::clipboard_macos::accessibility_trusted(prompt))
}

#[tauri::command]
pub fn open_accessibility_settings() -> Result<(), String> {
    crate::clipboard_macos::open_accessibility_settings();
    Ok(())
}

#[tauri::command]
pub fn check_ollama_status() -> Result<ollama::OllamaStatus, String> {
    Ok(ollama::check_status())
}

#[tauri::command]
pub fn start_ollama_server() -> Result<bool, String> {
    Ok(ollama::try_start_server())
}

#[tauri::command]
pub fn pull_ollama_model(app: tauri::AppHandle) -> Result<(), String> {
    std::thread::spawn(move || {
        let result = ollama::try_pull_model(Some(&app));
        let _ = app.emit("ollama-pull-done", result);
    });
    Ok(())
}

#[tauri::command]
pub fn unload_ollama_model() -> Result<bool, String> {
    Ok(ollama::unload_model())
}

#[tauri::command]
pub fn test_ollama_tagging() -> Result<Option<Vec<String>>, String> {
    Ok(ollama::test_tagging())
}

#[tauri::command]
pub fn rebind_voice_shortcut(app: tauri::AppHandle) -> Result<String, String> {
    crate::register_voice_shortcut(&app)
}

#[tauri::command]
pub fn list_microphones() -> Result<Vec<crate::whisper::AudioInputDevice>, String> {
    Ok(crate::whisper::list_input_devices())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_accessibility_settings_command_returns_ok() {
        assert!(open_accessibility_settings().is_ok());
    }
}
