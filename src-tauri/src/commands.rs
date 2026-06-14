use crate::app_exclusion::{self, ExcludableAppSource};
use crate::db::{
    AppSettings, ClipboardEntry, Collection, Database, ExcludedApp, HistoryCounts, ModelCatalog,
};
use crate::macos_app;
use serde::Serialize;

use crate::ollama;
use arboard::{Clipboard, ImageData};
use base64::Engine;
use image::GenericImageView;
use std::borrow::Cow;

use crate::clipboard_write::{self, ClipboardWriteMode};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

fn emit_history_changed(app: &AppHandle) {
    let _ = app.emit("history-changed", ());
}

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
pub fn delete_entry(app: AppHandle, db: State<'_, Arc<Database>>, id: i64) -> Result<(), String> {
    let emptied = db.delete_entry(id).map_err(|e| e.to_string())?;
    if emptied {
        crate::clipboard_monitor::notify_history_cleared();
    }
    emit_history_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn pin_entry(
    app: AppHandle,
    db: State<'_, Arc<Database>>,
    id: i64,
    pinned: bool,
) -> Result<(), String> {
    db.pin_entry(id, pinned).map_err(|e| e.to_string())?;
    emit_history_changed(&app);
    Ok(())
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
pub fn clear_history(app: AppHandle, db: State<'_, Arc<Database>>) -> Result<(), String> {
    db.clear_history().map_err(|e| e.to_string())?;
    crate::clipboard_monitor::notify_history_cleared();
    emit_history_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn clear_all_history(app: AppHandle, db: State<'_, Arc<Database>>) -> Result<(), String> {
    db.clear_all_history().map_err(|e| e.to_string())?;
    crate::clipboard_monitor::notify_history_cleared();
    emit_history_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn get_history_counts(db: State<'_, Arc<Database>>) -> Result<HistoryCounts, String> {
    db.get_history_counts().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resize_main_window(window: tauri::WebviewWindow, height: f64) -> Result<(), String> {
    const MIN_HEIGHT: f64 = 360.0;
    const MAX_HEIGHT: f64 = 560.0;
    let clamped = height.clamp(MIN_HEIGHT, MAX_HEIGHT);
    crate::position_window_bottom(&window, clamped);
    Ok(())
}

#[tauri::command]
pub fn hide_main_window(app: tauri::AppHandle) -> Result<(), String> {
    // Frontend played close motion; hide native panel on the main thread.
    crate::finalize_panel_hide(&app);
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
    #[cfg(target_os = "macos")]
    crate::clipboard_macos::remember_paste_target();

    // The clipboard panel is always-on-top; if it stays visible, macOS delivers mouse
    // events to the panel instead of settings — no hover, pointer cursor, or focus rings.
    if crate::main_panel_visible(&app) {
        crate::PANEL_HIDE_SCHEDULED.store(false, Ordering::Release);
        crate::finalize_panel_hide(&app);
    }

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
    app: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    ollama_model: Option<String>,
    retention_days: Option<i64>,
    whisper_server_url: Option<String>,
    whisper_server_token: Option<String>,
    whisper_server_model: Option<String>,
    voice_shortcut: Option<String>,
    selected_microphone: Option<String>,
    voice_transcription_enabled: Option<bool>,
    ai_tagging_enabled: Option<bool>,
) -> Result<AppSettings, String> {
    if let Some(model) = ollama_model.as_deref() {
        ollama::validate_model_name(model)?;
    }

    let was_tagging_enabled = db.is_ai_tagging_enabled();

    let settings = db
        .update_app_settings(
            ollama_model.as_deref(),
            retention_days,
            whisper_server_url.as_deref(),
            whisper_server_token.as_deref(),
            whisper_server_model.as_deref(),
            voice_shortcut.as_deref(),
            selected_microphone.as_deref(),
            voice_transcription_enabled,
            ai_tagging_enabled,
        )
        .map_err(|e| e.to_string())?;

    ollama::set_active_model(&settings.ollama_model);

    if settings.ai_tagging_enabled {
        ollama::ensure_runtime();
        if !was_tagging_enabled {
            ollama::backfill_existing_tags(app.clone(), db.inner().clone());
        }
    }

    db.cleanup_old_entries(settings.retention_days)
        .map_err(|e| e.to_string())?;
    emit_history_changed(&app);

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcludeAppResult {
    pub display_name: String,
    pub already_excluded: bool,
}

fn exclude_app_result(
    db: &Database,
    identity: &macos_app::AppIdentity,
) -> Result<ExcludeAppResult, String> {
    let is_new = db
        .add_excluded_app(&identity.bundle_id)
        .map_err(|e| e.to_string())?;
    Ok(ExcludeAppResult {
        display_name: identity.display_name.clone(),
        already_excluded: !is_new,
    })
}

#[tauri::command]
pub fn add_excluded_app(
    db: State<'_, Arc<Database>>,
    app_name_or_bundle_id: String,
) -> Result<ExcludeAppResult, String> {
    let input = app_name_or_bundle_id;
    let identity = macos_app::resolve_app_identity_from_input(&input)
        .ok_or_else(|| format!("app_not_found:{}", input.trim()))?;
    exclude_app_result(db.inner(), &identity)
}

#[tauri::command]
pub fn remove_excluded_app(db: State<'_, Arc<Database>>, id: i64) -> Result<(), String> {
    db.remove_excluded_app(id).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcludableAppCandidate {
    pub bundle_id: String,
    pub display_name: String,
    pub already_excluded: bool,
    pub source: ExcludableAppSource,
}

#[tauri::command]
pub fn get_excludable_app_candidate(
    db: State<'_, Arc<Database>>,
) -> Result<Option<ExcludableAppCandidate>, String> {
    let Some((identity, source)) = app_exclusion::resolve_excludable_app_identity() else {
        return Ok(None);
    };
    let already_excluded = db
        .is_app_excluded(&identity.bundle_id)
        .map_err(|e| e.to_string())?;
    Ok(Some(ExcludableAppCandidate {
        bundle_id: identity.bundle_id,
        display_name: identity.display_name,
        already_excluded,
        source,
    }))
}

#[tauri::command]
pub fn add_excludable_app_candidate(
    db: State<'_, Arc<Database>>,
) -> Result<Option<ExcludeAppResult>, String> {
    let Some((identity, _)) = app_exclusion::resolve_excludable_app_identity() else {
        return Ok(None);
    };
    Ok(Some(exclude_app_result(db.inner(), &identity)?))
}

#[tauri::command]
pub fn pick_app_to_exclude(
    app: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
) -> Result<Option<ExcludeAppResult>, String> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = tx.send(app_exclusion::pick_application_identity_on_main_thread());
    })
    .map_err(|e| format!("main_thread_required:{}", e))?;
    let identity = rx
        .recv()
        .map_err(|_| "main_thread_required:channel".to_owned())?
        .map_err(|e| e.to_owned())?;
    let Some(identity) = identity else {
        return Ok(None);
    };
    Ok(Some(exclude_app_result(db.inner(), &identity)?))
}

#[tauri::command]
pub fn retag_entry(
    app: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    entry_id: i64,
) -> Result<(), String> {
    if !ollama::is_tagging_ready(db.inner()) {
        return Ok(());
    }

    let Some(text) = db.get_entry_text(entry_id).map_err(|e| e.to_string())? else {
        return Ok(());
    };

    match ollama::tag_text(&text) {
        Some(tags) => db
            .set_entry_tags(entry_id, &tags)
            .map_err(|e| e.to_string())?,
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

fn finish_paste(app: &tauri::AppHandle) {
    crate::PENDING_PASTE_AFTER_HIDE.store(true, Ordering::Release);
    if crate::PANEL_HIDE_SCHEDULED.swap(true, Ordering::AcqRel) {
        return;
    }
    let _ = app.emit("window-hide-request", ());
}

fn image_bytes_from_entry(entry: &ClipboardEntry) -> Result<Vec<u8>, String> {
    let encoded = entry
        .image_data
        .as_ref()
        .or(entry.image_thumb.as_ref())
        .ok_or_else(|| "Image data is missing".to_owned())?;
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
pub fn activate_entry(
    app: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    entry_id: i64,
) -> Result<(), String> {
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
pub fn is_tagging_ready(db: State<'_, Arc<Database>>) -> Result<bool, String> {
    Ok(ollama::is_tagging_ready(db.inner()))
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
pub async fn unload_ollama_model() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(ollama::unload_model)
        .await
        .map_err(|err| format!("Unload failed: {err}"))
}

#[tauri::command]
pub async fn test_ollama_tagging() -> Result<Option<Vec<String>>, String> {
    tauri::async_runtime::spawn_blocking(ollama::test_tagging)
        .await
        .map_err(|err| format!("Tagging test failed: {err}"))
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
