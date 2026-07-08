use crate::app_exclusion::{self, ExcludableAppSource};
use crate::db::{
    AppSettings, ClipboardEntry, Collection, Database, EntryTaggedPayload, ExcludedApp,
    HistoryCounts, ModelCatalog, OverlayTagCounts,
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
    tag: Option<String>,
    tag_variants: Option<Vec<String>>,
    content_kind: Option<String>,
) -> Result<Vec<ClipboardEntry>, String> {
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let offset = offset.unwrap_or(0).max(0);

    db.get_entries(
        limit,
        offset,
        collection_id,
        pinned_only.unwrap_or(false),
        search.as_deref(),
        tag.as_deref(),
        tag_variants.as_deref(),
        content_kind.as_deref(),
    )
    .map_err(|e| e.to_string())
}

/// Full entry payload (including full-resolution `image_data`) for Quick Look.
/// `get_entries` deliberately omits `image_data` for list-fetch cost; this is the
/// on-demand fetch used only when a single entry's Quick Look preview is opened.
#[tauri::command]
pub fn get_entry(db: State<'_, Arc<Database>>, id: i64) -> Result<Option<ClipboardEntry>, String> {
    db.get_entry_by_id(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_overlay_tag_counts(
    db: State<'_, Arc<Database>>,
    collection_id: Option<i64>,
    pinned_only: Option<bool>,
    search: Option<String>,
) -> Result<OverlayTagCounts, String> {
    db.get_overlay_tag_counts(
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
pub fn resize_main_window(
    window: tauri::WebviewWindow,
    height: f64,
    remember_height: Option<bool>,
) -> Result<(), String> {
    let clamped = height.clamp(crate::OVERLAY_HEIGHT_MIN, crate::OVERLAY_HEIGHT_MAX);
    if remember_height.unwrap_or(true) {
        crate::remember_overlay_height(clamped);
    }
    crate::position_window_bottom(&window, clamped);
    Ok(())
}

#[tauri::command]
pub fn reset_overlay_board_sizes(
    app: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.clear_overlay_board_sizes().map_err(|e| e.to_string())?;
    crate::reset_remembered_overlay_height();
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            crate::position_window_bottom(&window, crate::OVERLAY_HEIGHT_COMPACT);
        }
    }
    let _ = app.emit("overlay-board-sizes-reset", ());
    Ok(())
}

#[tauri::command]
pub fn hide_main_window(app: tauri::AppHandle) -> Result<(), String> {
    // Frontend played close motion; hide native panel on the main thread.
    crate::finalize_panel_hide(&app);
    Ok(())
}

#[cfg(target_os = "macos")]
fn activate_for_settings_window(app: &tauri::AppHandle) {
    // docs/architecture/macos-tray-menu.md §9 — Regular only while Settings is open
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    crate::activation_macos::promote_to_regular(app);
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        app.activate();
    }
}

/// Sidebar (184px) + content max-width (540px) + horizontal padding.
const SETTINGS_WINDOW_WIDTH: f64 = 760.0;
const SETTINGS_WINDOW_HEIGHT: f64 = 720.0;
const SETTINGS_WINDOW_MIN_WIDTH: f64 = 680.0;
const SETTINGS_WINDOW_MIN_HEIGHT: f64 = 560.0;

fn ensure_settings_window_size(window: &tauri::WebviewWindow) {
    use tauri::LogicalSize;

    let scale = window.scale_factor().unwrap_or(1.0);
    let Ok(size) = window.inner_size() else {
        return;
    };
    let width = size.width as f64 / scale;
    let height = size.height as f64 / scale;
    if width >= SETTINGS_WINDOW_MIN_WIDTH && height >= SETTINGS_WINDOW_MIN_HEIGHT {
        return;
    }
    let _ = window.set_size(LogicalSize::new(
        SETTINGS_WINDOW_WIDTH,
        SETTINGS_WINDOW_HEIGHT,
    ));
}

const SETTINGS_PANES: &[&str] = &[
    "hub",
    "voice",
    "quickmenu",
    "ai",
    "history",
    "permissions",
    "updates",
];

fn is_settings_pane(pane: &str) -> bool {
    !pane.is_empty() && SETTINGS_PANES.contains(&pane)
}

fn settings_app_path(initial_pane: Option<&str>) -> String {
    match initial_pane.filter(|pane| is_settings_pane(pane)) {
        Some(pane) => format!("/settings?pane={}", percent_encode_query_component(pane)),
        None => "/settings".to_string(),
    }
}

fn percent_encode_query_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

fn settings_app_url(initial_pane: Option<&str>) -> tauri::WebviewUrl {
    tauri::WebviewUrl::App(settings_app_path(initial_pane).into())
}

fn emit_settings_pane(window: &tauri::WebviewWindow, pane: &str) {
    let _ = window.emit("navigate-settings-pane", pane);
}

#[tauri::command]
pub fn open_settings_window(
    app: tauri::AppHandle,
    initial_pane: Option<String>,
) -> Result<(), String> {
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
        ensure_settings_window_size(&window);
        crate::present_settings_window(&window);
        #[cfg(target_os = "macos")]
        activate_for_settings_window(&app);
        let _ = window.emit("settings-shown", ());
        if let Some(pane) = initial_pane
            .as_deref()
            .filter(|pane| is_settings_pane(pane))
        {
            emit_settings_pane(&window, pane);
        }
        return Ok(());
    }

    // Create a new settings window
    let builder = tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        settings_app_url(initial_pane.as_deref()),
    )
    .title("Copyosity Settings")
    .inner_size(SETTINGS_WINDOW_WIDTH, SETTINGS_WINDOW_HEIGHT)
    .min_inner_size(SETTINGS_WINDOW_MIN_WIDTH, SETTINGS_WINDOW_MIN_HEIGHT)
    .resizable(true)
    .center();

    let window = builder.build().map_err(|e| e.to_string())?;
    crate::present_settings_window(&window);
    #[cfg(target_os = "macos")]
    activate_for_settings_window(&app);
    let _ = window.emit("settings-shown", ());

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
#[allow(clippy::too_many_arguments)]
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
    overlay_shortcut_hints_enabled: Option<bool>,
    hub_enabled: Option<bool>,
    hub_url: Option<String>,
    hub_token: Option<String>,
    hub_chat_model: Option<String>,
    hub_tagging_enabled: Option<bool>,
    hub_transcribe_enabled: Option<bool>,
    voice_polish_enabled: Option<bool>,
    voice_polish_model: Option<String>,
    voice_polish_screenshot: Option<bool>,
    voice_polish_prompt: Option<String>,
    voice_translate_lang: Option<String>,
    voice_dictionary: Option<String>,
    voice_selected_text: Option<bool>,
    board_vertical: Option<bool>,
) -> Result<AppSettings, String> {
    if let Some(model) = ollama_model.as_deref() {
        ollama::validate_model_name(model)?;
    }

    let was_tagging_enabled = db.is_ai_tagging_enabled();
    let prior_hub_enabled = db
        .get_app_settings()
        .map_err(|e| e.to_string())?
        .hub_enabled;
    let next_hub_enabled = hub_enabled.unwrap_or(prior_hub_enabled);

    if hub_enabled.is_some() {
        crate::sync_palette_shortcut_for_hub(&app, next_hub_enabled)?;
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
            voice_transcription_enabled,
            ai_tagging_enabled,
            overlay_shortcut_hints_enabled,
            hub_enabled,
            hub_url.as_deref(),
            hub_token.as_deref(),
            hub_chat_model.as_deref(),
            hub_tagging_enabled,
            hub_transcribe_enabled,
            voice_polish_enabled,
            voice_polish_model.as_deref(),
            voice_polish_screenshot,
            voice_polish_prompt.as_deref(),
            voice_translate_lang.as_deref(),
            voice_dictionary.as_deref(),
            voice_selected_text,
            board_vertical,
        )
        .map_err(|e| {
            if hub_enabled.is_some() {
                let _ = crate::sync_palette_shortcut_for_hub(&app, prior_hub_enabled);
            }
            e.to_string()
        })?;

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

    if hub_enabled.is_some() {
        if let Err(e) = crate::refresh_tray_menu(&app) {
            eprintln!("Tray menu refresh failed: {e}");
        }
    }

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
) -> Result<Vec<String>, String> {
    if !crate::tagging::is_retag_ready(db.inner()) {
        return Ok(Vec::new());
    }

    let Some(entry) = db.get_entry_by_id(entry_id).map_err(|e| e.to_string())? else {
        return Ok(Vec::new());
    };

    let tagged = match entry.content_type.as_str() {
        "text" => entry
            .text_content
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .and_then(|text| crate::tagging::tag(db.inner(), text)),
        "image" => crate::tagging::tag_image_entry(db.inner(), &entry),
        _ => None,
    };

    let tags = match tagged {
        Some(tags) => {
            db.set_entry_tags(entry_id, &tags)
                .map_err(|e| e.to_string())?;
            tags
        }
        None => {
            db.set_entry_tag_state(entry_id, "skipped")
                .map_err(|e| e.to_string())?;
            Vec::new()
        }
    };

    let _ = app.emit(
        "entry-tagged",
        EntryTaggedPayload {
            entry_id,
            tags: tags.clone(),
        },
    );
    Ok(tags)
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

/// Copy arbitrary text (e.g. the OCR-recognised text of an image) to the clipboard.
#[tauri::command]
pub fn copy_text(text: String) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard_write::set_text(&mut clipboard, text)?;
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

// ---- Quick menu (native Clipy-style menu) paste helpers ----
//
// Unlike the overlay actions, the quick menu has no panel to hide, so these
// paste directly into a captured target pid (mirroring `palette_insert`).

/// Paste a history entry (by id) into `target_pid` from the native quick menu.
#[cfg(target_os = "macos")]
pub fn quick_menu_paste_entry(
    db: &Arc<Database>,
    entry_id: i64,
    target_pid: i32,
) -> Result<(), String> {
    let Some(entry) = db.get_entry_by_id(entry_id).map_err(|e| e.to_string())? else {
        return Ok(());
    };
    {
        let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
        write_entry_for_paste(&mut clipboard, &entry)?;
    }
    if target_pid > 0 {
        crate::clipboard_macos::remember_paste_target_for_pid(target_pid);
    }
    crate::clipboard_macos::spawn_automated_paste(false);
    Ok(())
}

/// Paste arbitrary text (a snippet) into `target_pid` from the native quick menu.
#[cfg(target_os = "macos")]
pub fn quick_menu_paste_text(text: String, target_pid: i32) -> Result<(), String> {
    {
        let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
        clipboard_write::write_text(&mut clipboard, text, ClipboardWriteMode::Paste)?;
    }
    if target_pid > 0 {
        crate::clipboard_macos::remember_paste_target_for_pid(target_pid);
    }
    crate::clipboard_macos::spawn_automated_paste(false);
    Ok(())
}

// ---- Quick menu shortcut (stored under its own settings key) ----

#[tauri::command]
pub fn get_quick_menu_shortcut(db: State<'_, Arc<Database>>) -> Result<String, String> {
    Ok(db
        .get_setting("quick_menu_shortcut")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| crate::DEFAULT_QUICK_MENU_SHORTCUT.to_string()))
}

#[tauri::command]
pub fn set_quick_menu_shortcut(
    app: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    shortcut: String,
) -> Result<String, String> {
    let trimmed = shortcut.trim();
    db.set_setting("quick_menu_shortcut", trimmed)
        .map_err(|e| e.to_string())?;
    let stored = crate::register_quick_menu_shortcut(&app)?;
    if let Err(e) = crate::refresh_tray_menu(&app) {
        eprintln!("Tray menu refresh failed: {e}");
    }
    Ok(stored)
}

// ---- Snippet commands ----

#[tauri::command]
pub fn get_snippet_folders(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<crate::db::SnippetFolder>, String> {
    db.get_snippet_folders().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_snippets(db: State<'_, Arc<Database>>) -> Result<Vec<crate::db::Snippet>, String> {
    db.get_snippets().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_snippet_folder(db: State<'_, Arc<Database>>, name: String) -> Result<i64, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Folder name cannot be empty".to_string());
    }
    db.create_snippet_folder(trimmed).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_snippet_folder(
    db: State<'_, Arc<Database>>,
    id: i64,
    name: String,
) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Folder name cannot be empty".to_string());
    }
    db.rename_snippet_folder(id, trimmed)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_snippet_folder(db: State<'_, Arc<Database>>, id: i64) -> Result<(), String> {
    db.delete_snippet_folder(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_snippet(
    db: State<'_, Arc<Database>>,
    folder_id: i64,
    title: String,
    content: String,
) -> Result<i64, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Snippet title cannot be empty".to_string());
    }
    db.create_snippet(folder_id, title, &content)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_snippet(
    db: State<'_, Arc<Database>>,
    id: i64,
    title: String,
    content: String,
) -> Result<(), String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Snippet title cannot be empty".to_string());
    }
    db.update_snippet(id, title, &content)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_snippet(db: State<'_, Arc<Database>>, id: i64) -> Result<(), String> {
    db.delete_snippet(id).map_err(|e| e.to_string())
}

/// Paste a snippet (by id) into the app that was frontmost when the quick menu
/// opened. Also usable from the settings UI for a quick test.
#[tauri::command]
pub fn paste_snippet(
    app: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    id: i64,
) -> Result<(), String> {
    let Some(snippet) = db.get_snippet_by_id(id).map_err(|e| e.to_string())? else {
        return Ok(());
    };
    paste_text_into_target(&app, snippet.content)
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
    Ok(crate::tagging::is_retag_ready(db.inner()))
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
pub fn rebind_palette_shortcut(app: tauri::AppHandle) -> Result<(), String> {
    crate::register_palette_shortcut(&app)
}

#[tauri::command]
pub fn list_microphones() -> Result<Vec<crate::whisper::AudioInputDevice>, String> {
    Ok(crate::whisper::list_input_devices())
}

/// Test connectivity to the NeuralDeep hub. Uses provided url/token when given,
/// otherwise falls back to the saved settings. Returns the number of models.
#[tauri::command]
pub fn hub_test_connection(
    db: State<'_, Arc<Database>>,
    url: Option<String>,
    token: Option<String>,
) -> Result<usize, String> {
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    let url = url
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(settings.hub_url);
    let token = token
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(settings.hub_token);
    crate::hub::test_connection(&url, &token)
}

/// List available hub model ids (uses saved url/token unless overridden).
#[tauri::command]
pub fn hub_list_models(
    db: State<'_, Arc<Database>>,
    url: Option<String>,
    token: Option<String>,
) -> Result<Vec<String>, String> {
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    let url = url
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(settings.hub_url);
    let token = token
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(settings.hub_token);
    crate::hub::list_models(&url, &token)
}

#[tauri::command]
pub fn get_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_accessibility_settings_command_returns_ok() {
        assert!(open_accessibility_settings().is_ok());
    }

    #[test]
    fn settings_app_path_defaults_to_settings_root() {
        assert_eq!(settings_app_path(None), "/settings");
        assert_eq!(settings_app_path(Some("")), "/settings");
    }

    #[test]
    fn settings_app_path_includes_pane_query() {
        assert_eq!(
            settings_app_path(Some("quickmenu")),
            "/settings?pane=quickmenu"
        );
        assert_eq!(settings_app_path(Some("hub")), "/settings?pane=hub");
    }

    #[test]
    fn settings_app_path_rejects_unknown_pane() {
        assert_eq!(settings_app_path(Some("not-a-pane")), "/settings");
        assert_eq!(settings_app_path(Some("QuickMenu")), "/settings");
    }

    #[test]
    fn settings_app_path_rejects_unsafe_pane_values() {
        assert_eq!(settings_app_path(Some("hub&x=1")), "/settings");
    }
}
