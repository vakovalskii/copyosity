use crate::db::{AppSettings, ClipboardEntry, Collection, Database, ExcludedApp, ModelCatalog};

#[cfg(target_os = "macos")]
fn simulate_paste() {
    std::thread::sleep(std::time::Duration::from_millis(150));

    unsafe {
        // CoreGraphics FFI — CGEvent for Cmd+V
        type CGEventSourceRef = *mut std::ffi::c_void;
        type CGEventRef = *mut std::ffi::c_void;

        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventCreateKeyboardEvent(source: CGEventSourceRef, keycode: u16, key_down: bool) -> CGEventRef;
            fn CGEventSetFlags(event: CGEventRef, flags: u64);
            fn CGEventPost(tap: u32, event: CGEventRef);
            fn CFRelease(cf: *mut std::ffi::c_void);
        }

        const K_CG_EVENT_FLAG_COMMAND: u64 = 0x00100000;
        const K_CG_HID_EVENT_TAP: u32 = 0;
        const K_V_KEYCODE: u16 = 9;

        let event_down = CGEventCreateKeyboardEvent(std::ptr::null_mut(), K_V_KEYCODE, true);
        let event_up = CGEventCreateKeyboardEvent(std::ptr::null_mut(), K_V_KEYCODE, false);

        if !event_down.is_null() && !event_up.is_null() {
            CGEventSetFlags(event_down, K_CG_EVENT_FLAG_COMMAND);
            CGEventSetFlags(event_up, K_CG_EVENT_FLAG_COMMAND);
            CGEventPost(K_CG_HID_EVENT_TAP, event_down);
            CGEventPost(K_CG_HID_EVENT_TAP, event_up);
            CFRelease(event_down);
            CFRelease(event_up);
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn simulate_paste() {}
use crate::ollama;
use arboard::{Clipboard, ImageData};
use base64::Engine;
use image::GenericImageView;
use std::borrow::Cow;
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

#[tauri::command]
pub fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    // If settings window already exists, just focus it
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
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
    .resizable(true)
    .center();

    #[cfg(target_os = "macos")]
    let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay);

    let _window = builder.build().map_err(|e| e.to_string())?;

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
) -> Result<AppSettings, String> {
    let settings = db
        .update_app_settings(ollama_model.as_deref(), retention_days)
        .map_err(|e| e.to_string())?;

    // Keep the active process aligned with the saved model so new tagging uses it immediately.
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
                clipboard.set_text(text).map_err(|e| e.to_string())?;
            }
        }
        "image" => {
            let encoded = entry
                .image_data
                .or(entry.image_thumb)
                .ok_or_else(|| "Image data is missing".to_string())?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| e.to_string())?;
            let image = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
            let rgba = image.to_rgba8();
            let (width, height) = image.dimensions();
            clipboard
                .set_image(ImageData {
                    width: width as usize,
                    height: height as usize,
                    bytes: Cow::Owned(rgba.into_raw()),
                })
                .map_err(|e| e.to_string())?;
        }
        _ => {}
    }

    Ok(())
}

#[tauri::command]
pub fn activate_entry(app: tauri::AppHandle, db: State<'_, Arc<Database>>, entry_id: i64) -> Result<(), String> {
    let Some(entry) = db.get_entry_by_id(entry_id).map_err(|e| e.to_string())? else {
        return Ok(());
    };

    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;

    match entry.content_type.as_str() {
        "text" => {
            if let Some(text) = entry.text_content {
                clipboard.set_text(text).map_err(|e| e.to_string())?;
            }
        }
        "image" => {
            let encoded = entry
                .image_data
                .or(entry.image_thumb)
                .ok_or_else(|| "Image data is missing".to_string())?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| e.to_string())?;
            let image = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
            let rgba = image.to_rgba8();
            let (width, height) = image.dimensions();
            clipboard
                .set_image(ImageData {
                    width: width as usize,
                    height: height as usize,
                    bytes: Cow::Owned(rgba.into_raw()),
                })
                .map_err(|e| e.to_string())?;
        }
        _ => return Ok(()),
    }

    crate::hide_panel(&app);
    simulate_paste();

    Ok(())
}

#[tauri::command]
pub fn paste_entry(app: tauri::AppHandle, text: String) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(&text).map_err(|e| e.to_string())?;

    crate::hide_panel(&app);
    simulate_paste();

    Ok(())
}

#[tauri::command]
pub fn check_accessibility() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        // AXIsProcessTrustedWithOptions with prompt: true shows the system dialog
        unsafe {
            #[link(name = "ApplicationServices", kind = "framework")]
            extern "C" {
                fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
            }

            use objc::{msg_send, sel, sel_impl};
            use objc::runtime::Object;

            let key: *mut Object = msg_send![
                objc::runtime::Class::get("NSString").unwrap(),
                stringWithUTF8String: b"AXTrustedCheckOptionPrompt\0".as_ptr()
            ];
            let yes: *mut Object = msg_send![
                objc::runtime::Class::get("NSNumber").unwrap(),
                numberWithBool: true
            ];
            let dict: *mut Object = msg_send![
                objc::runtime::Class::get("NSDictionary").unwrap(),
                dictionaryWithObject: yes forKey: key
            ];

            let trusted = AXIsProcessTrustedWithOptions(dict as *const _);
            return Ok(trusted);
        }
    }

    #[cfg(not(target_os = "macos"))]
    Ok(true)
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
