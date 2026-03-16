use crate::db::{AppSettings, ClipboardEntry, Collection, Database, ModelCatalog};
use crate::ollama;
use arboard::Clipboard;
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
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
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
) -> Result<AppSettings, String> {
    let settings = db
        .update_app_settings(ollama_model.as_deref(), retention_days)
        .map_err(|e| e.to_string())?;

    // Keep the active process aligned with the saved model so new tagging uses it immediately.
    std::env::set_var("COPYOSITY_OLLAMA_MODEL", &settings.ollama_model);
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
pub fn paste_entry(app: tauri::AppHandle, text: String) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(&text).map_err(|e| e.to_string())?;

    // Hide window first so paste goes to the right app
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    // Small delay then simulate Cmd+V / Ctrl+V
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        std::thread::sleep(std::time::Duration::from_millis(100));
        Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to keystroke \"v\" using command down")
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}
