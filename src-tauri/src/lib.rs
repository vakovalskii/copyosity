#![allow(unexpected_cfgs)]

#[cfg(target_os = "macos")]
mod activation_macos; // docs/architecture/macos-tray-menu.md §9
mod agent;
mod app_exclusion;
mod clipboard_macos;
mod clipboard_monitor;
mod clipboard_write;
mod commands;
mod db;
mod hub;
mod image_format;
mod macos_app;
mod macos_window;
mod mactools;
mod ocr;
mod ollama;
mod overlay_dismiss;
mod palette_window;
mod quick_menu;
mod quick_menu_position;
mod screen;
mod tagging;
mod transcription;
#[cfg(target_os = "macos")]
mod tray_macos; // docs/architecture/macos-tray-menu.md §4–§5
mod whisper;

use db::Database;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
#[cfg(target_os = "macos")]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[cfg(target_os = "macos")]
use tauri_nspanel::{ManagerExt, WebviewWindowExt};

#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel!(panel!(CopyosityPanel {
    config: {
        can_become_key_window: true,
        is_floating_panel: true
    }
}));

static LAST_SHOW_MS: AtomicU64 = AtomicU64::new(0);
pub(crate) static PANEL_HIDE_SCHEDULED: AtomicBool = AtomicBool::new(false);
pub(crate) static PENDING_PASTE_AFTER_HIDE: AtomicBool = AtomicBool::new(false);

/// Main overlay WebviewWindow exists at launch (Handy); NSPanel conversion is deferred.
/// Agent guardrail: docs/architecture/macos-tray-menu.md §3, §7 — do not convert to NSPanel here.
///
/// INVARIANT: `ensure_main_overlay_panel` is **main-thread-only**. All callers reach it through
/// Tauri's main-thread event loop, so no two callers can be concurrent — the load/store pair is
/// safe. Do NOT call from a spawned task or background thread: AppKit panel ops are not thread-safe
/// and `to_panel` would likely panic or corrupt state on re-entry.
#[cfg(target_os = "macos")]
static MAIN_OVERLAY_PANEL: AtomicBool = AtomicBool::new(false);

/// PID of the app that was frontmost when the voice hotkey was pressed.
/// Used to deliver the synthesized Cmd+V directly to that app instead of
/// whatever is frontmost at paste time (which may be Copyosity itself).
static VOICE_TARGET_PID: AtomicI32 = AtomicI32::new(0);

/// Base64 PNG screenshot of the target window captured at hotkey-press time,
/// used as visual context when polishing the transcription.
static VOICE_SCREENSHOT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Detected kind of the target app ("email"/"chat"/"code"/"document"/"general").
static VOICE_APP_KIND: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Text selected in the target app at hotkey-press time (selected-text command
/// mode): the spoken transcription becomes an instruction applied to this text.
static VOICE_SELECTION: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// PID of the app that was frontmost when the command palette was opened, so the
/// agent answer can be inserted back into it.
static PALETTE_TARGET_PID: AtomicI32 = AtomicI32::new(0);

/// PID of the app that was frontmost when the native quick menu was opened, so
/// the selected history entry / snippet pastes back into it.
static QUICK_MENU_TARGET_PID: AtomicI32 = AtomicI32::new(0);

/// Default hotkey for the native quick menu (Clipy-style).
pub(crate) const DEFAULT_QUICK_MENU_SHORTCUT: &str = "cmd+shift+c";

/// Default hotkey for the command / agent palette.
#[cfg(target_os = "macos")]
pub(crate) const DEFAULT_PALETTE_SHORTCUT: &str = "cmd+shift+space";
#[cfg(not(target_os = "macos"))]
pub(crate) const DEFAULT_PALETTE_SHORTCUT: &str = "ctrl+shift+space";

static RECORDING: std::sync::OnceLock<std::sync::Mutex<Option<whisper::RecordingSession>>> =
    std::sync::OnceLock::new();

fn recording_mutex() -> &'static std::sync::Mutex<Option<whisper::RecordingSession>> {
    RECORDING.get_or_init(|| std::sync::Mutex::new(None))
}

/// Separate recording slot for the command-palette mic (independent of the
/// global voice-paste hotkey).
static PALETTE_RECORDING: std::sync::OnceLock<std::sync::Mutex<Option<whisper::RecordingSession>>> =
    std::sync::OnceLock::new();

fn palette_recording_mutex() -> &'static std::sync::Mutex<Option<whisper::RecordingSession>> {
    PALETTE_RECORDING.get_or_init(|| std::sync::Mutex::new(None))
}

/// Transcribe finished audio using hub or standalone Whisper, per settings.
fn transcribe_with_settings(
    settings: &db::AppSettings,
    samples: Vec<f32>,
    sample_rate: u32,
) -> Result<String, String> {
    let (url, tok) = transcription::transcription_endpoint(settings)?;
    whisper::transcribe_audio(
        samples,
        sample_rate,
        &url,
        &tok,
        &settings.whisper_server_model,
    )
}

static CURRENT_VOICE_SHORTCUT: std::sync::OnceLock<std::sync::Mutex<Option<Shortcut>>> =
    std::sync::OnceLock::new();

fn voice_shortcut_mutex() -> &'static std::sync::Mutex<Option<Shortcut>> {
    CURRENT_VOICE_SHORTCUT.get_or_init(|| std::sync::Mutex::new(None))
}

static CURRENT_PALETTE_SHORTCUT: std::sync::OnceLock<std::sync::Mutex<Option<Shortcut>>> =
    std::sync::OnceLock::new();

fn palette_shortcut_mutex() -> &'static std::sync::Mutex<Option<Shortcut>> {
    CURRENT_PALETTE_SHORTCUT.get_or_init(|| std::sync::Mutex::new(None))
}

/// The configured palette shortcut string (DB `palette_shortcut`, or the default).
fn palette_shortcut_str(app: &tauri::AppHandle) -> String {
    app.state::<std::sync::Arc<db::Database>>()
        .get_setting("palette_shortcut")
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_PALETTE_SHORTCUT.to_string())
}

fn palette_shortcut(app: &tauri::AppHandle) -> Shortcut {
    parse_shortcut(&palette_shortcut_str(app)).unwrap_or_else(|| {
        parse_shortcut(DEFAULT_PALETTE_SHORTCUT).expect("valid default palette shortcut")
    })
}

/// Register (or unregister) the hub agent-search shortcut for an explicit hub state.
pub fn sync_palette_shortcut_for_hub(
    app: &tauri::AppHandle,
    hub_enabled: bool,
) -> Result<(), String> {
    if !hub_enabled {
        if let Some(old) = palette_shortcut_mutex().lock().unwrap().take() {
            let _ = app.global_shortcut().unregister(old);
        }
        return Ok(());
    }

    let new_shortcut = palette_shortcut(app);

    {
        let mut current = palette_shortcut_mutex().lock().unwrap();
        if let Some(old) = current.take() {
            let _ = app.global_shortcut().unregister(old);
        }
    }

    let palette_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(new_shortcut, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                toggle_command_palette(&palette_handle);
            }
        })
        .map_err(|e| format!("Failed to register palette shortcut: {}", e))?;

    *palette_shortcut_mutex().lock().unwrap() = Some(new_shortcut);
    Ok(())
}

/// Register (or unregister) the hub agent-search shortcut from current DB settings.
pub fn register_palette_shortcut(app: &tauri::AppHandle) -> Result<(), String> {
    let db = app.state::<std::sync::Arc<db::Database>>();
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    sync_palette_shortcut_for_hub(app, settings.hub_enabled)
}

static CURRENT_QUICK_MENU_SHORTCUT: std::sync::OnceLock<std::sync::Mutex<Option<Shortcut>>> =
    std::sync::OnceLock::new();

fn quick_menu_shortcut_mutex() -> &'static std::sync::Mutex<Option<Shortcut>> {
    CURRENT_QUICK_MENU_SHORTCUT.get_or_init(|| std::sync::Mutex::new(None))
}

/// Register (or re-register) the native quick-menu hotkey from DB settings.
/// Returns the shortcut string on success.
pub fn register_quick_menu_shortcut(app: &tauri::AppHandle) -> Result<String, String> {
    let db = app.state::<std::sync::Arc<db::Database>>();
    let shortcut_str = db
        .get_setting("quick_menu_shortcut")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| DEFAULT_QUICK_MENU_SHORTCUT.to_string());

    let new_shortcut = parse_shortcut(&shortcut_str)
        .ok_or_else(|| format!("Invalid shortcut: {}", shortcut_str))?;

    {
        let mut current = quick_menu_shortcut_mutex().lock().unwrap();
        if let Some(old) = current.take() {
            let _ = app.global_shortcut().unregister(old);
        }
    }

    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(new_shortcut, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                quick_menu::show(&handle);
            }
        })
        .map_err(|e| format!("Failed to register quick-menu shortcut: {}", e))?;

    *quick_menu_shortcut_mutex().lock().unwrap() = Some(new_shortcut);
    Ok(shortcut_str)
}

pub fn open_snippets_editor(app: &tauri::AppHandle) {
    let _ = commands::open_settings_window(app.clone(), Some("quickmenu".to_string()));
}

pub fn quick_menu_shortcut_string(db: &db::Database) -> Result<String, String> {
    Ok(db
        .get_setting("quick_menu_shortcut")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| DEFAULT_QUICK_MENU_SHORTCUT.to_string()))
}

/// Dispatch a selection from the native quick menu. Ignores non-`qm:` ids so it
/// can safely share the app-level menu-event channel with other menus.
fn handle_quick_menu_event(app: &tauri::AppHandle, id: &str) {
    let Some(action) = id.strip_prefix("qm:") else {
        return;
    };

    match action {
        "clear" => {
            let db = app.state::<std::sync::Arc<db::Database>>();
            let _ = db.clear_history();
            let _ = app.emit("history-changed", ());
            return;
        }
        "settings" => {
            let _ = commands::open_settings_window(app.clone(), None);
            return;
        }
        "edit-snippets" => {
            let _ = commands::open_settings_window(app.clone(), Some("quickmenu".to_string()));
            return;
        }
        "quit" => {
            let _ = commands::quit_app(app.clone());
            return;
        }
        _ => {}
    }

    #[cfg(target_os = "macos")]
    {
        let target = QUICK_MENU_TARGET_PID.swap(0, Ordering::Relaxed);
        if let Some(entry_id) = action
            .strip_prefix("paste:")
            .and_then(|s| s.parse::<i64>().ok())
        {
            let db = app.state::<std::sync::Arc<db::Database>>();
            if let Err(e) = commands::quick_menu_paste_entry(&db, entry_id, target) {
                eprintln!("[quick_menu] paste entry failed: {}", e);
            }
        } else if let Some(snip_id) = action
            .strip_prefix("snip:")
            .and_then(|s| s.parse::<i64>().ok())
        {
            let db = app.state::<std::sync::Arc<db::Database>>();
            if let Ok(Some(snip)) = db.get_snippet_by_id(snip_id) {
                if let Err(e) = commands::quick_menu_paste_text(snip.content, target) {
                    eprintln!("[quick_menu] paste snippet failed: {}", e);
                }
            }
        }
    }
}

/// Parse a string like "option+space", "cmd+space", "ctrl+alt+space" into a Shortcut.
fn parse_shortcut(s: &str) -> Option<Shortcut> {
    let lower = s.to_lowercase();
    let parts: Vec<&str> = lower.split('+').map(|p| p.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut mods = Modifiers::empty();
    let mut key_code = None;

    for part in &parts {
        match *part {
            "cmd" | "super" | "command" => mods |= Modifiers::SUPER,
            "option" | "alt" => mods |= Modifiers::ALT,
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "shift" => mods |= Modifiers::SHIFT,
            "space" => key_code = Some(Code::Space),
            "tab" => key_code = Some(Code::Tab),
            "enter" | "return" => key_code = Some(Code::Enter),
            k if k.len() == 1 => {
                let c = k.chars().next().unwrap();
                key_code = match c {
                    'a' => Some(Code::KeyA),
                    'b' => Some(Code::KeyB),
                    'c' => Some(Code::KeyC),
                    'd' => Some(Code::KeyD),
                    'e' => Some(Code::KeyE),
                    'f' => Some(Code::KeyF),
                    'g' => Some(Code::KeyG),
                    'h' => Some(Code::KeyH),
                    'i' => Some(Code::KeyI),
                    'j' => Some(Code::KeyJ),
                    'k' => Some(Code::KeyK),
                    'l' => Some(Code::KeyL),
                    'm' => Some(Code::KeyM),
                    'n' => Some(Code::KeyN),
                    'o' => Some(Code::KeyO),
                    'p' => Some(Code::KeyP),
                    'q' => Some(Code::KeyQ),
                    'r' => Some(Code::KeyR),
                    's' => Some(Code::KeyS),
                    't' => Some(Code::KeyT),
                    'u' => Some(Code::KeyU),
                    'v' => Some(Code::KeyV),
                    'w' => Some(Code::KeyW),
                    'x' => Some(Code::KeyX),
                    'y' => Some(Code::KeyY),
                    'z' => Some(Code::KeyZ),
                    _ => None,
                };
            }
            _ => {}
        }
    }

    let key = key_code?;
    let mods_opt = if mods.is_empty() { None } else { Some(mods) };
    Some(Shortcut::new(mods_opt, key))
}

fn format_shortcut_for_menu(shortcut: &str) -> String {
    let lower = shortcut.to_lowercase();
    let parts: Vec<&str> = lower.split('+').map(|p| p.trim()).collect();
    let key_part = parts.last().copied().unwrap_or("");

    #[cfg(target_os = "macos")]
    {
        let mut display = String::new();
        if parts
            .iter()
            .any(|p| matches!(*p, "cmd" | "super" | "command"))
        {
            display.push('⌘');
        }
        if parts.iter().any(|p| matches!(*p, "ctrl" | "control")) {
            display.push('⌃');
        }
        if parts.iter().any(|p| matches!(*p, "option" | "alt")) {
            display.push('⌥');
        }
        if parts.contains(&"shift") {
            display.push('⇧');
        }
        match key_part {
            "space" => display.push_str("Space"),
            "tab" => display.push_str("Tab"),
            "enter" | "return" => display.push_str("Return"),
            k if k.len() == 1 => display.push(k.chars().next().unwrap().to_ascii_uppercase()),
            _ => {}
        }
        display
    }

    #[cfg(not(target_os = "macos"))]
    {
        let mut parts_display: Vec<String> = Vec::new();
        for part in &parts[..parts.len().saturating_sub(1)] {
            match *part {
                "cmd" | "super" | "command" => parts_display.push("Cmd".to_string()),
                "ctrl" | "control" => parts_display.push("Ctrl".to_string()),
                "option" | "alt" => parts_display.push("Alt".to_string()),
                "shift" => parts_display.push("Shift".to_string()),
                _ => {}
            }
        }
        let key = match key_part {
            "space" => "Space".to_string(),
            "tab" => "Tab".to_string(),
            "enter" | "return" => "Return".to_string(),
            k if k.len() == 1 => k.to_ascii_uppercase(),
            other => other.to_string(),
        };
        if parts_display.is_empty() {
            key
        } else {
            format!("{}+{}", parts_display.join("+"), key)
        }
    }
}

/// Register (or re-register) the voice shortcut from current DB settings.
/// Returns the shortcut string on success.
pub fn register_voice_shortcut(app: &tauri::AppHandle) -> Result<String, String> {
    let db = app.state::<std::sync::Arc<db::Database>>();
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;

    if !settings.voice_transcription_enabled {
        if let Some(old) = voice_shortcut_mutex().lock().unwrap().take() {
            let _ = app.global_shortcut().unregister(old);
        }
        return Ok(settings.voice_shortcut);
    }

    eprintln!(
        "[voice] registering shortcut: \"{}\"",
        settings.voice_shortcut
    );

    let new_shortcut = parse_shortcut(&settings.voice_shortcut)
        .ok_or_else(|| format!("Invalid shortcut: {}", settings.voice_shortcut))?;

    // Unregister old shortcut if any
    {
        let mut current = voice_shortcut_mutex().lock().unwrap();
        if let Some(old) = current.take() {
            let _ = app.global_shortcut().unregister(old);
        }
    }

    // Register new one
    let voice_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(new_shortcut, move |_app, _shortcut, event| {
            handle_voice_event(&voice_handle, event.state);
        })
        .map_err(|e| format!("Failed to register shortcut: {}", e))?;

    // Store it
    *voice_shortcut_mutex().lock().unwrap() = Some(new_shortcut);

    Ok(settings.voice_shortcut)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = {
        let b = tauri::Builder::default()
            .plugin(tauri_plugin_opener::init())
            .plugin(tauri_plugin_process::init())
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_notification::init())
            .plugin(tauri_plugin_autostart::Builder::new().build())
            .plugin(tauri_plugin_global_shortcut::Builder::new().build());
        #[cfg(target_os = "macos")]
        {
            b.plugin(tauri_nspanel::init())
        }
        #[cfg(not(target_os = "macos"))]
        {
            b
        }
    };

    let builder = {
        #[cfg(target_os = "macos")]
        {
            // Agent guardrail: docs/architecture/macos-tray-menu.md §1
            // Tauri default menubar (Apple/File/Edit) fights NSStatusItem — tray blinks if enabled.
            builder.enable_macos_default_menu(false)
        }
        #[cfg(not(target_os = "macos"))]
        {
            builder
        }
    };

    builder
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");
            let db = Arc::new(Database::new(app_dir).expect("Failed to initialize database"));
            app.manage(db.clone());

            // --- TRAY STARTUP ---
            // Agent guardrail: docs/architecture/macos-tray-menu.md — read before editing.
            // Verified 5× in tauri dev. Combined scheme only; partial fixes break 1st OR 2nd click.
            // Do not reorder, remove tray_macos defer, or switch to show_menu_on_left_click(true).
            // Gate: make verify-tray
            #[cfg(target_os = "macos")]
            {
                // §2 Accessory before tray
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            // §3 Hidden main at level 3 before tray (sync on main thread — async defer breaks make dev)
            ensure_main_overlay_window(app.handle())?;

            let tray_menu = build_tray_menu(app.handle())?;
            let tray = {
                let tray_builder = TrayIconBuilder::new()
                    .icon(app.default_window_icon().unwrap().clone())
                    .tooltip("Copyosity")
                    .menu(&tray_menu)
                    .on_menu_event(|app, event| {
                        handle_tray_menu_event(app.app_handle(), event.id().as_ref())
                    });
                #[cfg(target_os = "macos")]
                {
                    // §4 Deferred popup — show_menu_on_left_click(true) regresses 2nd/3rd click
                    tray_builder
                        .show_menu_on_left_click(false)
                        .on_tray_icon_event(|tray, event| {
                            if let TrayIconEvent::Click {
                                button: MouseButton::Left,
                                button_state: MouseButtonState::Down,
                                ..
                            } = event
                            {
                                #[cfg(target_os = "macos")]
                                clipboard_macos::remember_paste_target();
                                tray_macos::set_tray_highlight(tray, true);
                                tray_macos::schedule_tray_menu_popup(tray.clone());
                            }
                        })
                        .build(app)?
                }
                #[cfg(not(target_os = "macos"))]
                {
                    tray_builder.show_menu_on_left_click(true).build(app)?
                }
            };
            app.manage(tray);

            #[cfg(target_os = "macos")]
            {
                // §5 Warmup — do not replace activateIgnoringOtherApps(true) with activate()
                tray_macos::warmup_app_for_status_item_menu();
                overlay_dismiss::install_overlay_dismiss_guards();
                overlay_dismiss::install_cmd_up_dismiss(app.handle().clone());
                overlay_dismiss::install_app_switch_dismiss(app.handle().clone());
                clipboard_macos::install_last_frontmost_observer();
                eprintln!("[tray] startup: hidden main + deferred tray popup ready");
            }
            // --- end TRAY STARTUP ---

            let shortcut = {
                #[cfg(target_os = "macos")]
                {
                    Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyV)
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV)
                }
            };

            let handle = app.handle().clone();
            app.global_shortcut()
                .on_shortcut(shortcut, move |_app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        toggle_window(&handle);
                    }
                })?;

            // Command palette (hub agent search): Cmd+Shift+Space when hub is enabled.
            if let Err(e) = register_palette_shortcut(app.handle()) {
                eprintln!("Palette shortcut registration failed: {}", e);
            }

            // Register voice transcription shortcut from settings
            if let Err(e) = register_voice_shortcut(app.handle()) {
                eprintln!("Voice shortcut registration failed: {}", e);
            }

            // Native quick menu (Clipy-style) hotkey.
            if let Err(e) = register_quick_menu_shortcut(app.handle()) {
                eprintln!("Quick-menu shortcut registration failed: {}", e);
            }

            // Handle selections from the native quick menu (popup menu events are
            // delivered to the app-level handler, not the tray's).
            app.on_menu_event(|app, event| handle_quick_menu_event(app, event.id().as_ref()));

            eprintln!(
                "copyosity: global shortcut registered = {}",
                app.global_shortcut().is_registered(shortcut)
            );

            // §8 Agent guardrail: docs/architecture/macos-tray-menu.md — lazy create only.
            // voice_overlay / command_palette: lazy-created on first hotkey (see show_voice_overlay,
            // toggle_command_palette). Do NOT pre-create here — regresses tray first-click blink.

            let settings = db.get_app_settings().expect("Failed to load app settings");
            ollama::set_active_model(&settings.ollama_model);
            let _ = db.cleanup_old_entries(settings.retention_days);

            {
                let db_backfill = db.clone();
                std::thread::spawn(move || {
                    loop {
                        match db_backfill.backfill_missing_image_formats(100) {
                            Ok(0) => break,
                            Ok(_) => {}
                            Err(e) => {
                                eprintln!("image format backfill error: {e}");
                                std::thread::sleep(std::time::Duration::from_secs(5));
                            }
                        }
                    }
                    loop {
                        match db_backfill.backfill_missing_image_meta(100) {
                            Ok(0) => break,
                            Ok(_) => {}
                            Err(e) => {
                                eprintln!("image meta backfill error: {e}");
                                std::thread::sleep(std::time::Duration::from_secs(5));
                            }
                        }
                    }
                });
            }

            if settings.ai_tagging_enabled {
                ollama::ensure_runtime();
                ollama::backfill_existing_tags(app.handle().clone(), db);
            }
            clipboard_write::sweep_stale_gif_temp_files();
            clipboard_monitor::start_clipboard_monitor(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_entries,
            commands::get_entry,
            commands::get_overlay_tag_counts,
            commands::delete_entry,
            commands::pin_entry,
            commands::set_entry_collection,
            commands::get_collections,
            commands::create_collection,
            commands::delete_collection,
            commands::clear_history,
            commands::clear_all_history,
            commands::get_history_counts,
            commands::hide_main_window,
            commands::resize_main_window,
            commands::reset_overlay_board_sizes,
            commands::open_settings_window,
            commands::quit_app,
            commands::get_app_settings,
            commands::get_model_catalog,
            commands::get_excluded_apps,
            commands::add_excluded_app,
            commands::remove_excluded_app,
            commands::get_excludable_app_candidate,
            commands::add_excludable_app_candidate,
            commands::pick_app_to_exclude,
            commands::update_app_settings,
            commands::retag_entry,
            commands::is_tagging_ready,
            commands::copy_entry,
            commands::copy_text,
            commands::activate_entry,
            commands::paste_entry,
            commands::check_accessibility,
            commands::open_accessibility_settings,
            commands::check_ollama_status,
            commands::start_ollama_server,
            commands::pull_ollama_model,
            commands::unload_ollama_model,
            commands::test_ollama_tagging,
            commands::rebind_voice_shortcut,
            commands::rebind_palette_shortcut,
            commands::get_quick_menu_shortcut,
            commands::set_quick_menu_shortcut,
            commands::get_snippet_folders,
            commands::get_snippets,
            commands::create_snippet_folder,
            commands::rename_snippet_folder,
            commands::delete_snippet_folder,
            commands::create_snippet,
            commands::update_snippet,
            commands::delete_snippet,
            commands::paste_snippet,
            commands::list_microphones,
            commands::hub_test_connection,
            commands::hub_list_models,
            commands::get_platform,
            palette_search,
            palette_hide,
            palette_insert,
            open_command_palette,
            palette_agent,
            palette_voice_start,
            palette_voice_stop,
            palette_set_dot_mode,
            palette_is_dot_mode,
            palette_snap_to_edges,
            install_update,
            agent_capture_active_window,
            commands::agent_web_search,
            commands::agent_create_note,
            commands::agent_create_reminder,
            commands::agent_list_reminders,
            commands::agent_read_calendar,
            commands::get_palette_shortcut,
            commands::set_palette_shortcut,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            tauri::RunEvent::ExitRequested { code, api, .. } => {
                // Menu-bar app: closing the last window must NOT quit — keep
                // running in the tray. That implicit exit has `code == None`.
                //
                // BUT programmatic exits carry a code: `app.restart()` (used by
                // the updater's `relaunch()`) triggers ExitRequested with
                // `RESTART_EXIT_CODE`, and `app.exit(n)` carries `Some(n)`.
                // Unconditionally preventing exit here swallowed the updater
                // relaunch, so a downloaded update never applied. Only prevent
                // the user-driven (code-less) exit; let programmatic ones through.
                if code.is_none() {
                    api.prevent_exit();
                }
            }
            #[cfg(target_os = "macos")]
            tauri::RunEvent::TrayIconEvent(ref tray_event) => {
                if std::env::var("COPYOSITY_TRAY_DEBUG").is_ok() {
                    eprintln!("[tray] TrayIconEvent: {tray_event:?}");
                }
            }
            tauri::RunEvent::WindowEvent { label, event, .. } => match (label.as_str(), &event) {
                ("main", tauri::WindowEvent::CloseRequested { api, .. }) => {
                    api.prevent_close();
                    animated_hide_panel(app);
                }
                ("main", tauri::WindowEvent::Focused(false)) => {
                    overlay_dismiss::handle_focus_lost(app);
                }
                ("main", tauri::WindowEvent::Resized(_)) => {
                    persist_main_window_user_resize(app);
                }
                ("settings", tauri::WindowEvent::Destroyed) => {
                    #[cfg(target_os = "macos")]
                    {
                        // docs/architecture/macos-tray-menu.md §9
                        activation_macos::maybe_restore_accessory(app);
                    }
                }
                _ => {}
            },
            _ => {}
        });
}

pub(crate) fn present_settings_window(window: &tauri::WebviewWindow) {
    // Settings is a real, focusable window. As an Accessory (menu-bar) app it would
    // have no Dock icon and could vanish behind other apps on focus change, so switch
    // to Regular while it is open. Reverted to Accessory on the window's Destroyed event.
    // `MoveToActiveSpace` (not `CanJoinAllSpaces` — that's for the always-floating overlay/palette
    // panels, see macos_window.rs) — without it, focusing Settings while another app (e.g. a
    // fullscreen IDE) owns the active Space forces macOS to switch the user off that Space to
    // reveal the window. This relocates Settings to wherever the user currently is *once*, then
    // leaves it a normal single-Space window again, so it does not keep following the user to
    // other screens/Spaces afterward.
    #[cfg(target_os = "macos")]
    {
        activation_macos::promote_to_regular(window.app_handle());
        macos_window::apply_move_to_active_space_behavior(window);
    }
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

/// Clipboard overlay WebviewWindow — created hidden at startup (macOS devUrl path); idempotent.
/// Agent guardrail: docs/architecture/macos-tray-menu.md §3 — must run in setup before tray,
/// synchronously on main thread; level 3 via apply_hidden_auxiliary_webview.
pub(crate) fn ensure_main_overlay_window(
    app: &tauri::AppHandle,
) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window("main") {
        return Ok(window);
    }

    let window = tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("/".into()))
        .title("")
        .inner_size(OVERLAY_WIDTH_MIN, OVERLAY_HEIGHT_COMPACT)
        .min_inner_size(OVERLAY_WIDTH_MIN, OVERLAY_HEIGHT_MIN)
        .resizable(true)
        .decorations(false)
        .transparent(true)
        .visible(false)
        .skip_taskbar(true)
        .build()
        .map_err(|e| format!("Failed to create main overlay window: {e}"))?;
    let _ = window.set_always_on_top(false);
    #[cfg(target_os = "macos")]
    macos_window::apply_hidden_auxiliary_webview(&window);
    eprintln!("[tray] main overlay window ready (hidden auxiliary level)");
    Ok(window)
}

/// Convert the lazy main WebviewWindow into a floating NSPanel on first show.
/// Agent guardrail: docs/architecture/macos-tray-menu.md §7 — not at startup.
/// Must be called on the main thread only (see INVARIANT on `MAIN_OVERLAY_PANEL`).
///
/// **Idempotency:** the `AtomicBool` early-return makes repeated calls a no-op after the first
/// successful conversion. A unit test would require a full Tauri runtime and is therefore
/// impractical; the correctness guarantee lives in the `debug_assert` (thread guard) and the
/// `load → early-return → store` ordering that is safe because all callers are main-thread-only.
#[cfg(target_os = "macos")]
fn ensure_main_overlay_panel(app: &tauri::AppHandle) -> Result<(), String> {
    debug_assert!(
        objc2::MainThreadMarker::new().is_some(),
        "ensure_main_overlay_panel must run on the main thread"
    );
    if MAIN_OVERLAY_PANEL.load(Ordering::Acquire) {
        return Ok(());
    }

    use tauri_nspanel::panel::NSWindowStyleMask;

    let window = ensure_main_overlay_window(app)?;
    let panel = window
        .to_panel::<CopyosityPanel>()
        .map_err(|e| format!("Failed to convert window to panel: {e}"))?;
    panel.set_style_mask(
        NSWindowStyleMask::Borderless
            | NSWindowStyleMask::NonactivatingPanel
            | NSWindowStyleMask::Resizable,
    );
    macos_window::configure_hidden_auxiliary_panel(&*panel);
    let _ = window.set_always_on_top(false);
    MAIN_OVERLAY_PANEL.store(true, Ordering::Release);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn ensure_main_overlay_panel(app: &tauri::AppHandle) -> Result<(), String> {
    ensure_main_overlay_window(app).map(|_| ())
}

fn toggle_window(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        if ensure_main_overlay_panel(app).is_ok() {
            if let Ok(panel) = app.get_webview_panel("main") {
                if panel.is_visible() {
                    animated_hide_panel(app);
                } else {
                    hide_command_palette(app);
                    if let Some(window) = app.get_webview_window("main") {
                        position_window_bottom(&window, remembered_overlay_height());
                    }
                    clipboard_macos::remember_paste_target();
                    PANEL_HIDE_SCHEDULED.store(false, Ordering::Release);
                    LAST_SHOW_MS.store(now_ms(), Ordering::Relaxed);
                    macos_window::configure_fullscreen_auxiliary_panel(&*panel);
                    panel.show_and_make_key();
                    overlay_dismiss::set_outside_click_dismiss(app, true);
                    let _ = app.emit("window-show", ());
                }
                return;
            }
        }
    }

    // Fallback for non-macOS (or before macOS panel conversion)
    if let Ok(()) = ensure_main_overlay_panel(app) {
        if let Some(window) = app.get_webview_window("main") {
            if window.is_visible().unwrap_or(false) {
                animated_hide_panel(app);
            } else {
                hide_command_palette(app);
                LAST_SHOW_MS.store(now_ms(), Ordering::Relaxed);
                position_window_bottom(&window, remembered_overlay_height());
                let _ = window.show();
                let _ = window.set_focus();
                let _ = app.emit("window-show", ());
            }
        }
    }
}

pub(crate) fn animated_hide_panel(app: &tauri::AppHandle) {
    if PANEL_HIDE_SCHEDULED.swap(true, Ordering::AcqRel) {
        return;
    }

    let _ = app.emit("window-hide-request", ());
}

fn hide_panel(app: &tauri::AppHandle) {
    let mut hid = false;

    #[cfg(target_os = "macos")]
    {
        overlay_dismiss::set_outside_click_dismiss(app, false);
        if ensure_main_overlay_panel(app).is_ok() {
            if let Ok(panel) = app.get_webview_panel("main") {
                if panel.is_visible() {
                    panel.hide();
                    macos_window::configure_hidden_auxiliary_panel(&*panel);
                    let _ = app.emit("window-hide", ());
                    hid = true;
                }
                if hid {
                    // §9 Agent guardrail: docs/architecture/macos-tray-menu.md
                    activation_macos::maybe_restore_accessory(app);
                }
                return;
            }
        }
    }

    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
            let _ = app.emit("window-hide", ());
            #[cfg(target_os = "macos")]
            activation_macos::maybe_restore_accessory(app);
        }
    }
}

/// Hide the native panel and run any deferred paste (same path as `hide_main_window`).
pub(crate) fn finalize_panel_hide(app: &tauri::AppHandle) {
    PANEL_HIDE_SCHEDULED.store(true, Ordering::Release);
    hide_panel(app);
    PANEL_HIDE_SCHEDULED.store(false, Ordering::Release);

    if PENDING_PASTE_AFTER_HIDE.swap(false, Ordering::AcqRel) {
        #[cfg(target_os = "macos")]
        crate::clipboard_macos::spawn_automated_paste(true);
    }
}

fn main_panel_visible(app: &tauri::AppHandle) -> bool {
    #[cfg(target_os = "macos")]
    {
        if let Ok(panel) = app.get_webview_panel("main") {
            return panel.is_visible();
        }
    }

    app.get_webview_window("main")
        .map(|window| window.is_visible().unwrap_or(false))
        .unwrap_or(false)
}

/// Dismiss the clipboard overlay synchronously when another floating UI takes over.
pub(crate) fn dismiss_main_panel_if_visible(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    overlay_dismiss::set_outside_click_dismiss(app, false);
    if main_panel_visible(app) {
        PANEL_HIDE_SCHEDULED.store(false, Ordering::Release);
        finalize_panel_hide(app);
    }
}

/// Dismiss floating auxiliary UIs before the native quick menu opens its modal loop.
/// Tray clicks and open overlay/agent panels race with `popup_menu` and make snippets blink.
#[cfg(target_os = "macos")]
pub(crate) fn prepare_for_quick_menu(app: &tauri::AppHandle) {
    dismiss_main_panel_if_visible(app);
    hide_command_palette(app);
}

pub(crate) fn hide_command_palette(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        if let Ok(panel) = app.get_webview_panel("command_palette") {
            panel.hide();
            macos_window::configure_hidden_auxiliary_panel(&*panel);
            return;
        }
    }
    if let Some(win) = app.get_webview_window("command_palette") {
        let _ = win.hide();
    }
}

fn build_tray_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let version = &app.package_info().version;
    let version_label = format!("Copyosity v{}", version);

    let hub_enabled = app
        .try_state::<std::sync::Arc<db::Database>>()
        .and_then(|db| db.get_app_settings().ok())
        .map(|s| s.hub_enabled)
        .unwrap_or(false);

    let search = MenuItem::with_id(
        app,
        "search",
        "Agent Search  ⌘⇧Space",
        hub_enabled,
        None::<&str>,
    )?;
    #[cfg(target_os = "macos")]
    let overlay_label = "Open Clipboard  ⌘⇧V";
    #[cfg(not(target_os = "macos"))]
    let overlay_label = "Open Clipboard  Ctrl+Shift+V";
    let overlay = MenuItem::with_id(app, "overlay", overlay_label, true, None::<&str>)?;
    let snippets_label = app
        .try_state::<std::sync::Arc<db::Database>>()
        .and_then(|db| quick_menu_shortcut_string(db.as_ref()).ok())
        .map(|shortcut| format!("Open Snippets  {}", format_shortcut_for_menu(&shortcut)))
        .unwrap_or_else(|| {
            #[cfg(target_os = "macos")]
            {
                "Open Snippets  ⌘⇧C".to_string()
            }
            #[cfg(not(target_os = "macos"))]
            {
                "Open Snippets  Ctrl+Shift+C".to_string()
            }
        });
    let snippets = MenuItem::with_id(app, "snippets", &snippets_label, true, None::<&str>)?;
    let ver = MenuItem::with_id(app, "version", &version_label, false, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    Menu::with_items(
        app,
        &[
            &search, &overlay, &snippets, &ver, &sep, &settings, &sep2, &quit,
        ],
    )
}

pub fn refresh_tray_menu(app: &tauri::AppHandle) -> Result<(), String> {
    let menu = build_tray_menu(app).map_err(|e| e.to_string())?;
    let tray = app.state::<tauri::tray::TrayIcon<tauri::Wry>>();
    tray.set_menu(Some(menu))
        .map_err(|e| format!("Failed to refresh tray menu: {e}"))
}

fn handle_tray_menu_event(app: &tauri::AppHandle, id: &str) {
    match id {
        "search" => toggle_command_palette(app),
        "overlay" => toggle_window(app),
        "snippets" => quick_menu::show(app),
        "settings" => {
            let _ = commands::open_settings_window(app.clone(), None);
        }
        "quit" => {
            let _ = commands::quit_app(app.clone());
        }
        _ => {
            // "version" and any future info-only items are intentionally no-ops.
            #[cfg(debug_assertions)]
            eprintln!("[tray] unhandled menu event id: {id}");
        }
    }
}

/// Run the transcription through the hub polish step when enabled, falling back
/// to the raw transcription on any error. Consumes the screenshot/app-kind
/// context captured at press time.
fn maybe_polish(settings: &db::AppSettings, raw: &str) -> String {
    let screenshot = VOICE_SCREENSHOT.lock().unwrap().take();
    let app_kind = std::mem::take(&mut *VOICE_APP_KIND.lock().unwrap());
    let selection = VOICE_SELECTION.lock().unwrap().take();

    if !settings.hub_enabled
        || !settings.voice_polish_enabled
        || settings.hub_token.trim().is_empty()
        || settings.hub_url.trim().is_empty()
        || settings.voice_polish_model.trim().is_empty()
    {
        return raw.to_string();
    }

    let dictionary: Vec<String> = settings
        .voice_dictionary
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let kind = if app_kind.is_empty() {
        "general"
    } else {
        app_kind.as_str()
    };
    let selected = if settings.voice_selected_text {
        selection.as_deref()
    } else {
        None
    };
    if selected.is_some() {
        eprintln!("[voice] selected-text command mode");
    }
    eprintln!(
        "[voice] polishing: model={} kind={} screenshot={} dict={}",
        settings.voice_polish_model,
        kind,
        screenshot.is_some(),
        dictionary.len()
    );

    match hub::polish_text(
        &settings.hub_url,
        &settings.hub_token,
        &settings.voice_polish_model,
        raw,
        kind,
        screenshot.as_deref(),
        &dictionary,
        &settings.voice_polish_prompt,
        &settings.voice_translate_lang,
        selected,
    ) {
        Ok(polished) => {
            eprintln!("[voice] polished ({}): \"{}\"", kind, polished);
            polished
        }
        Err(e) => {
            eprintln!("[voice] polish failed ({}), using raw transcription", e);
            raw.to_string()
        }
    }
}

fn handle_voice_event(app: &tauri::AppHandle, state: ShortcutState) {
    eprintln!(
        "[voice] event: {:?}",
        match state {
            ShortcutState::Pressed => "PRESSED",
            ShortcutState::Released => "RELEASED",
        }
    );
    match state {
        ShortcutState::Pressed => {
            // Capture the app that is frontmost right now, before we touch any
            // of our own windows — that's where the transcript must be pasted.
            // Reset any leftover context from a previous run.
            *VOICE_SCREENSHOT.lock().unwrap() = None;
            *VOICE_APP_KIND.lock().unwrap() = String::new();

            #[cfg(target_os = "macos")]
            if let Some(pid) = frontmost_app_pid() {
                if pid != std::process::id() as i32 {
                    VOICE_TARGET_PID.store(pid, Ordering::Relaxed);
                    clipboard_macos::remember_paste_target_for_pid(pid);
                    eprintln!("[voice] captured target pid={}", pid);

                    // Context-aware polishing: classify the target app and grab a
                    // screenshot now, while the target window is still frontmost.
                    let polish = app
                        .try_state::<std::sync::Arc<db::Database>>()
                        .and_then(|db| db.get_app_settings().ok());
                    if let Some(s) = polish {
                        eprintln!(
                            "[voice] polish_enabled={} screenshot={} selected_text={}",
                            s.voice_polish_enabled,
                            s.voice_polish_screenshot,
                            s.voice_selected_text
                        );
                        if s.voice_polish_enabled {
                            if let Some(bundle) = app_bundle_id(pid) {
                                let kind = classify_app_kind(&bundle);
                                eprintln!("[voice] target app: {} -> {}", bundle, kind);
                                *VOICE_APP_KIND.lock().unwrap() = kind.to_string();
                            }
                            if s.voice_polish_screenshot {
                                eprintln!("[voice] capturing target-window screenshot…");
                                std::thread::spawn(|| {
                                    if let Some(png) = screen::capture_context_png() {
                                        let b64 = base64::Engine::encode(
                                            &base64::engine::general_purpose::STANDARD,
                                            &png,
                                        );
                                        eprintln!(
                                            "[voice] screenshot ready ({} b64 chars)",
                                            b64.len()
                                        );
                                        *VOICE_SCREENSHOT.lock().unwrap() = Some(b64);
                                    } else {
                                        eprintln!("[voice] screenshot capture returned nothing (Screen Recording permission?)");
                                    }
                                });
                            }
                            // Selected-text command mode: copy the current selection
                            // now (clipboard saved & restored) so the transcription
                            // can be applied to it as an instruction.
                            if s.voice_selected_text {
                                std::thread::spawn(move || {
                                    *VOICE_SELECTION.lock().unwrap() = capture_selection(pid);
                                });
                            }
                        }
                    }
                }
            }
            let mut rec = recording_mutex().lock().unwrap();
            if rec.is_none() {
                // Read selected microphone from settings
                let mic_name = app
                    .try_state::<std::sync::Arc<db::Database>>()
                    .and_then(|db| db.get_app_settings().ok())
                    .map(|s| s.selected_microphone)
                    .filter(|s| !s.is_empty());
                eprintln!("[voice] starting recording, mic={:?}", mic_name);
                match whisper::RecordingSession::start(mic_name.as_deref()) {
                    Ok(session) => {
                        eprintln!(
                            "[voice] recording started, sample_rate={}",
                            session.sample_rate
                        );
                        show_voice_overlay(app);
                        let level_arc = session.level.clone();
                        let emit_handle = app.clone();
                        std::thread::spawn(move || loop {
                            let still_recording = recording_mutex().lock().unwrap().is_some();
                            if !still_recording {
                                break;
                            }
                            let lvl = level_arc.load(std::sync::atomic::Ordering::Relaxed);
                            if let Some(win) = emit_handle.get_webview_window("voice_overlay") {
                                let _ = win.emit("audio-level", lvl);
                            } else {
                                let _ = emit_handle.emit("audio-level", lvl);
                            }
                            std::thread::sleep(std::time::Duration::from_millis(60));
                        });
                        *rec = Some(session);
                    }
                    Err(e) => eprintln!("[voice] FAILED to start recording: {}", e),
                }
            } else {
                eprintln!("[voice] already recording, ignoring press");
            }
        }
        ShortcutState::Released => {
            let session = recording_mutex().lock().unwrap().take();
            if let Some(session) = session {
                let app = app.clone();
                // Keep the capsule visible in a "transcribing" state (spinner)
                // until the transcript is delivered, instead of hiding immediately.
                if let Some(win) = app.get_webview_window("voice_overlay") {
                    let _ = win.emit("voice-transcribing", ());
                }
                std::thread::spawn(move || {
                    let (samples, sample_rate) = session.finish();
                    eprintln!(
                        "[voice] stopped, {} samples at {}Hz ({:.1}s)",
                        samples.len(),
                        sample_rate,
                        samples.len() as f64 / sample_rate as f64
                    );
                    let db = app.state::<std::sync::Arc<db::Database>>();
                    let settings = match db.get_app_settings() {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("[voice] failed to load settings: {}", e);
                            return;
                        }
                    };
                    let (whisper_url, whisper_token) =
                        match transcription::transcription_endpoint(&settings) {
                            Ok(pair) => pair,
                            Err(e) => {
                                eprintln!("[voice] ERROR: {e}");
                                return;
                            }
                        };
                    eprintln!("[voice] sending to {}", whisper_url);
                    match whisper::transcribe_audio(
                        samples,
                        sample_rate,
                        &whisper_url,
                        &whisper_token,
                        &settings.whisper_server_model,
                    ) {
                        Ok(text) if !text.is_empty() => {
                            eprintln!("[voice] transcription: \"{}\"", text);
                            let final_text = maybe_polish(&settings, &text);

                            // Always record the transcript in history so it is
                            // retrievable even if the paste into the target app
                            // fails (own clipboard writes are skipped by the monitor).
                            clipboard_monitor::record_text_entry(
                                &app,
                                &db,
                                &final_text,
                                Some("Voice".to_owned()),
                            );

                            // Put it on the system clipboard for pasting, retrying
                            // so the transcript reliably reaches the pasteboard.
                            let mut delivered = false;
                            for attempt in 1..=3 {
                                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                    if clipboard_write::write_text(
                                        &mut clipboard,
                                        final_text.clone(),
                                        clipboard_write::ClipboardWriteMode::Paste,
                                    )
                                    .is_ok()
                                    {
                                        delivered = true;
                                        break;
                                    }
                                }
                                eprintln!(
                                    "[voice] clipboard write attempt {attempt}/3 failed, retrying"
                                );
                                std::thread::sleep(std::time::Duration::from_millis(80));
                            }
                            if !delivered {
                                eprintln!(
                                    "[voice] WARNING: transcript not written to clipboard after retries (still in history)"
                                );
                            }
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            #[cfg(target_os = "macos")]
                            {
                                let voice_pid = VOICE_TARGET_PID.swap(0, Ordering::Relaxed);
                                if voice_pid > 0 {
                                    clipboard_macos::remember_paste_target_for_pid(voice_pid);
                                }
                                clipboard_macos::spawn_automated_paste(false);
                            }
                            #[cfg(not(target_os = "macos"))]
                            {
                                let target_pid = VOICE_TARGET_PID.swap(0, Ordering::Relaxed);
                                simulate_cmd_v(target_pid);
                            }
                        }
                        Ok(_) => eprintln!("[voice] transcription returned empty text"),
                        Err(e) => {
                            eprintln!("[voice] transcription ERROR: {}", e);
                            // Surface to the user (e.g. a hub 429 raise-tariff hint).
                            let _ = app.emit("voice-error", e);
                        }
                    }
                    // hide_voice_overlay touches AppKit (NSPanel); this closure runs on a
                    // worker thread, so hop to the main thread — calling NSPanel.hide() off
                    // the main thread crashes the app after every transcription.
                    let hide_app = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        hide_voice_overlay(&hide_app);
                    });
                });
            }
        }
    }
}

fn ensure_voice_overlay(app: &tauri::AppHandle) {
    #[cfg(not(target_os = "macos"))]
    let _ = app;
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        // Already created
        if app.get_webview_panel("voice_overlay").is_ok() {
            return;
        }

        let builder = tauri::WebviewWindowBuilder::new(
            app,
            "voice_overlay",
            tauri::WebviewUrl::App("/overlay".into()),
        )
        .title("")
        .inner_size(260.0, 54.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .skip_taskbar(true)
        .visible(false)
        .center();

        if let Ok(win) = builder.build() {
            use tauri_nspanel::panel::NSWindowStyleMask;
            use tauri_nspanel::WebviewWindowExt;

            if let Ok(panel) = win.to_panel::<CopyosityPanel>() {
                panel.set_style_mask(
                    NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
                );
                panel.set_becomes_key_only_if_needed(true);
                macos_window::configure_hidden_auxiliary_panel(&*panel);
            }
        }
    }
}

fn show_voice_overlay(app: &tauri::AppHandle) {
    ensure_voice_overlay(app);

    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        if let Ok(panel) = app.get_webview_panel("voice_overlay") {
            macos_window::configure_fullscreen_auxiliary_panel(&*panel);
            panel.order_front_regardless();
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(win) = app.get_webview_window("voice_overlay") {
            let _ = win.show();
        }
    }
}

fn hide_voice_overlay(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        if let Ok(panel) = app.get_webview_panel("voice_overlay") {
            panel.hide();
            macos_window::configure_hidden_auxiliary_panel(&*panel);
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(win) = app.get_webview_window("voice_overlay") {
            let _ = win.close();
        }
    }
}

/// PID of the application that is frontmost right now, via NSWorkspace.
#[cfg(target_os = "macos")]
fn frontmost_app_pid() -> Option<i32> {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    unsafe {
        let workspace_cls = objc::runtime::Class::get("NSWorkspace")?;
        let workspace: *mut Object = msg_send![workspace_cls, sharedWorkspace];
        if workspace.is_null() {
            return None;
        }
        let app: *mut Object = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return None;
        }
        let pid: i32 = msg_send![app, processIdentifier];
        if pid > 0 {
            Some(pid)
        } else {
            None
        }
    }
}

/// Bundle identifier of the running application with the given pid, via
/// NSRunningApplication.
#[cfg(target_os = "macos")]
fn app_bundle_id(pid: i32) -> Option<String> {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    use std::ffi::CStr;
    unsafe {
        let cls = objc::runtime::Class::get("NSRunningApplication")?;
        let app: *mut Object = msg_send![cls, runningApplicationWithProcessIdentifier: pid];
        if app.is_null() {
            return None;
        }
        let bundle: *mut Object = msg_send![app, bundleIdentifier];
        if bundle.is_null() {
            return None;
        }
        let utf8: *const std::os::raw::c_char = msg_send![bundle, UTF8String];
        if utf8.is_null() {
            return None;
        }
        Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }
}

/// Classify the target app into a context bucket for polishing.
#[cfg(target_os = "macos")]
fn classify_app_kind(bundle_id: &str) -> &'static str {
    let b = bundle_id.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| b.contains(n));
    if has(&["mail", "outlook", "spark", "airmail", "sparrow"]) {
        "email"
    } else if has(&[
        "telegram",
        "slack",
        "discord",
        "whatsapp",
        "messages",
        "mobilesms",
        "signal",
        "viber",
        "messenger",
        "rocket.chat",
    ]) {
        "chat"
    } else if has(&[
        "vscode",
        "xcode",
        "jetbrains",
        "intellij",
        "pycharm",
        "goland",
        "webstorm",
        "iterm",
        "terminal",
        "sublime",
        "zed",
        "nova",
        "cursor",
        "warp",
    ]) {
        "code"
    } else if has(&[
        "pages",
        "word",
        "notion",
        "obsidian",
        "bear",
        "ulysses",
        "docs",
        "scrivener",
    ]) {
        "document"
    } else {
        "general"
    }
}

// ---- Hub agent command palette ----

/// Toggle the command palette. Captures the frontmost app first so the answer
/// can be inserted there, then shows the palette and gives it keyboard focus.
fn toggle_command_palette(app: &tauri::AppHandle) {
    let db = app.state::<std::sync::Arc<db::Database>>();
    let settings = match db.get_app_settings() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[palette] failed to load settings: {}", e);
            return;
        }
    };
    if !settings.hub_enabled {
        return;
    }

    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        ensure_command_palette(app);
        if let Ok(panel) = app.get_webview_panel("command_palette") {
            if panel.is_visible() {
                panel.hide();
                macos_window::configure_hidden_auxiliary_panel(&*panel);
                return;
            }
            dismiss_main_panel_if_visible(app);
            if let Some(pid) = frontmost_app_pid() {
                if pid != std::process::id() as i32 {
                    PALETTE_TARGET_PID.store(pid, Ordering::Relaxed);
                    clipboard_macos::remember_paste_target_for_pid(pid);
                }
            }
            // Don't re-center on show — keep the window where the user last
            // moved/resized it (it was centered once at creation).
            macos_window::present_fullscreen_auxiliary_panel(&*panel);
            let _ = app.emit("palette-show", ());
        } else if let Some(window) = app.get_webview_window("command_palette") {
            if window.is_visible().unwrap_or(false) {
                let _ = window.hide();
                return;
            }
            dismiss_main_panel_if_visible(app);
            macos_window::present_fullscreen_auxiliary_webview(&window);
            let _ = app.emit("palette-show", ());
        }
    }
}

#[cfg(target_os = "macos")]
fn ensure_command_palette(app: &tauri::AppHandle) {
    use tauri_nspanel::ManagerExt;
    if app.get_webview_panel("command_palette").is_ok() {
        return;
    }

    let builder = tauri::WebviewWindowBuilder::new(
        app,
        "command_palette",
        tauri::WebviewUrl::App("/palette".into()),
    )
    .title("")
    .inner_size(640.0, 460.0)
    .min_inner_size(
        palette_window::PALETTE_MIN_WIDTH,
        palette_window::PALETTE_MIN_HEIGHT,
    )
    .resizable(true)
    .decorations(false)
    .transparent(true)
    .skip_taskbar(true)
    .accept_first_mouse(true)
    .visible(false)
    .center();

    if let Ok(win) = builder.build() {
        use tauri_nspanel::panel::NSWindowStyleMask;
        use tauri_nspanel::WebviewWindowExt;
        if let Ok(panel) = win.to_panel::<CopyosityPanel>() {
            panel.set_style_mask(
                NSWindowStyleMask::Borderless
                    | NSWindowStyleMask::NonactivatingPanel
                    | NSWindowStyleMask::Resizable,
            );
            macos_window::configure_hidden_auxiliary_panel(&*panel);
        } else {
            eprintln!("[palette] failed to convert command_palette to NSPanel");
        }
    }
}

/// Run a web search against the hub Search API and return formatted results.
#[tauri::command]
fn palette_search(app: tauri::AppHandle, query: String) -> Result<String, String> {
    let db = app.state::<std::sync::Arc<db::Database>>();
    let s = db.get_app_settings().map_err(|e| e.to_string())?;
    if !s.hub_enabled {
        return Err("NeuralDeep hub is disabled in Settings".to_string());
    }
    hub::web_search(&s.hub_url, &s.hub_token, &query, 5)
}

/// Open the command palette from an explicit UI action (tray / button).
#[tauri::command]
fn open_command_palette(app: tauri::AppHandle) {
    toggle_command_palette(&app);
}

/// Run the research agent loop for `query` in the background, streaming
/// progress to the palette via agent-progress / agent-final / agent-error.
#[tauri::command]
fn palette_agent(
    app: tauri::AppHandle,
    query: String,
    model: Option<String>,
    screenshot: Option<bool>,
) -> Result<(), String> {
    let db = app.state::<std::sync::Arc<db::Database>>();
    let s = db.get_app_settings().map_err(|e| e.to_string())?;
    if !s.hub_enabled {
        return Err("NeuralDeep hub is disabled in Settings".to_string());
    }
    let model = model
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| s.hub_chat_model.clone());

    // Optionally attach a screenshot of the app that was frontmost when the
    // palette opened, so the agent can reason about the current screen.
    let screenshot_b64 = if screenshot.unwrap_or(false) {
        #[cfg(target_os = "macos")]
        {
            let pid = PALETTE_TARGET_PID.load(Ordering::Relaxed);
            let png = if pid > 0 {
                screen::capture_window_png(pid)
            } else {
                screen::capture_context_png()
            };
            png.map(|b| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &b))
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    } else {
        None
    };

    std::thread::spawn(move || {
        agent::run(
            &app,
            &s.hub_url,
            &s.hub_token,
            &query,
            &model,
            screenshot_b64,
        );
    });
    Ok(())
}

/// Capture a PNG screenshot of the app that was frontmost when the palette
/// opened, base64-encoded, for the frontend agent to attach as image context.
/// Returns None when unavailable (non-macOS, no target, capture failed).
#[tauri::command]
fn agent_capture_active_window() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let pid = PALETTE_TARGET_PID.load(Ordering::Relaxed);
        let png = if pid > 0 {
            screen::capture_window_png(pid)
        } else {
            screen::capture_context_png()
        };
        png.map(|b| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &b))
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Install the pending update, then relaunch.
///
/// The updater plugin extracts the new bundle into `$TMPDIR` (usually
/// `/var/folders/...`) and `rename()`s it over the running app. When the app and
/// `$TMPDIR` live on different volumes that rename fails with EXDEV ("Cross-device
/// link", os error 18) — the download succeeds but the install never applies.
/// Pointing `TMPDIR` at the app bundle's own directory keeps every rename
/// intra-volume. Also refuses to run from an App Translocation sandbox (a
/// read-only quarantine mount) or a read-only volume (a mounted `.dmg`, which
/// lives under `/Volumes/…`) where self-update is impossible — in either case
/// the user must drag Copyosity into /Applications first.
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;

    #[cfg(target_os = "macos")]
    {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let p = exe.to_string_lossy().to_string();
        if p.contains("/AppTranslocation/") {
            return Err(
                "Copyosity is running from a quarantine sandbox (App Translocation). \
                 Move Copyosity to /Applications, reopen it, then update."
                    .to_string(),
            );
        }
        // exe = <bundle>/Contents/MacOS/copyosity → nth(3) = <bundle>, parent = its folder.
        let parent = exe
            .ancestors()
            .nth(3)
            .and_then(|b| b.parent())
            .ok_or_else(|| "Could not locate the Copyosity app bundle.".to_string())?;

        // The updater extracts the new bundle next to the app and renames it into
        // place. If that folder isn't writable — the classic case being the app
        // still running from the read-only mounted disk image (/Volumes/…) — the
        // install dies with a cryptic "Read-only file system (os error 30)".
        // Probe for writability up front and return an actionable message instead.
        let probe = parent.join(".copyosity-update-probe");
        match std::fs::create_dir(&probe) {
            Ok(()) => {
                let _ = std::fs::remove_dir(&probe);
            }
            Err(e) => {
                return Err(format!(
                    "Copyosity can't update itself from “{}” ({}). It looks like you're \
                     running Copyosity straight from the disk image (a read-only location). \
                     Quit Copyosity, drag it onto the Applications folder, then open it from \
                     Applications and update again.",
                    parent.display(),
                    e
                ));
            }
        }
        std::env::set_var("TMPDIR", parent);
    }

    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No update available".to_string())?;

    let progress_app = app.clone();
    let mut downloaded: u64 = 0;
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let pct = total
                    .filter(|t| *t > 0)
                    .map(|t| (downloaded * 100 / t) as u32);
                let _ = progress_app.emit("update-progress", pct);
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;

    let _ = app.emit("update-progress", Some(100u32));
    app.restart();
}

/// Snap the palette window to the nearest screen work-area edge when the user
/// dragged it close to one. Called from the frontend after a drag ends.
#[tauri::command]
fn palette_snap_to_edges(app: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        if let Some(window) = app.get_webview_window("command_palette") {
            macos_window::snap_window_to_edges(&window, 24.0);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
    }
}

/// Start recording from the palette mic.
#[tauri::command]
fn palette_voice_start(app: tauri::AppHandle) -> Result<(), String> {
    let mut rec = palette_recording_mutex().lock().unwrap();
    if rec.is_some() {
        return Ok(());
    }
    let db = app
        .try_state::<std::sync::Arc<db::Database>>()
        .ok_or_else(|| "database not ready".to_string())?;
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    transcription::transcription_endpoint(&settings)?;
    let mic = if settings.selected_microphone.is_empty() {
        None
    } else {
        Some(settings.selected_microphone.as_str())
    };
    let session = whisper::RecordingSession::start(mic)?;
    *rec = Some(session);
    Ok(())
}

/// Stop palette recording, transcribe, and return the text.
#[tauri::command]
fn palette_voice_stop(app: tauri::AppHandle) -> Result<String, String> {
    let session = palette_recording_mutex().lock().unwrap().take();
    let Some(session) = session else {
        return Ok(String::new());
    };
    let (samples, sample_rate) = session.finish();
    let db = app.state::<std::sync::Arc<db::Database>>();
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;
    transcribe_with_settings(&settings, samples, sample_rate)
}

#[cfg(not(target_os = "macos"))]
fn palette_webview_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    app.get_webview_window("command_palette")
        .ok_or_else(|| "command palette window not found".to_string())
}

/// Shrink/restore the palette window for min-dot mode (NSPanel + min_inner_size).
#[tauri::command]
fn palette_set_dot_mode(
    app: tauri::AppHandle,
    minimized: bool,
    restore_width: f64,
    restore_height: f64,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        macos_window::palette_set_dot_mode(&app, minimized, restore_width, restore_height)
    }
    #[cfg(not(target_os = "macos"))]
    {
        use palette_window::{PALETTE_DOT_SIZE, PALETTE_MIN_HEIGHT, PALETTE_MIN_WIDTH};
        use tauri::LogicalSize;
        let win = palette_webview_window(&app)?;
        if minimized {
            let dot = LogicalSize::new(PALETTE_DOT_SIZE, PALETTE_DOT_SIZE);
            win.set_min_size(Some(dot)).map_err(|e| e.to_string())?;
            win.set_size(dot).map_err(|e| e.to_string())?;
        } else {
            win.set_min_size(Some(LogicalSize::new(
                PALETTE_MIN_WIDTH,
                PALETTE_MIN_HEIGHT,
            )))
            .map_err(|e| e.to_string())?;
            win.set_size(LogicalSize::new(restore_width, restore_height))
                .map_err(|e| e.to_string())?;
            win.center().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

/// Returns whether the palette window is currently in min-dot mode.
#[tauri::command]
fn palette_is_dot_mode(app: tauri::AppHandle) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        macos_window::palette_is_dot_mode(&app)
    }
    #[cfg(not(target_os = "macos"))]
    {
        use palette_window::is_dot_logical_size;
        let win = palette_webview_window(&app)?;
        let size = win.inner_size().map_err(|e| e.to_string())?;
        let scale = win.scale_factor().map_err(|e| e.to_string())?;
        let logical_width = f64::from(size.width) / scale;
        let logical_height = f64::from(size.height) / scale;
        Ok(is_dot_logical_size(logical_width, logical_height))
    }
}

/// Hide the command palette.
#[tauri::command]
fn palette_hide(app: tauri::AppHandle) {
    hide_command_palette(&app);
}

/// Hide the palette and paste `text` into the app that was frontmost when it opened.
#[tauri::command]
fn palette_insert(app: tauri::AppHandle, text: String) {
    let target_pid = PALETTE_TARGET_PID.swap(0, Ordering::Relaxed);
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard_write::write_text(
            &mut clipboard,
            text,
            clipboard_write::ClipboardWriteMode::Paste,
        );
    }
    palette_hide(app);
    #[cfg(target_os = "macos")]
    {
        if target_pid > 0 {
            clipboard_macos::remember_paste_target_for_pid(target_pid);
        }
        clipboard_macos::spawn_automated_paste(false);
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::thread::sleep(std::time::Duration::from_millis(120));
        simulate_cmd_v(target_pid);
    }
}

#[cfg(not(target_os = "macos"))]
fn simulate_cmd_v(target_pid: i32) {
    // Non-macOS: post synthetic Cmd+V via platform APIs when available.
    let _ = target_pid;
}

/// Synthesize Cmd+C, delivered to `target_pid` when valid. Used to read the
/// current text selection of the target app (selected-text command mode).
#[cfg(target_os = "macos")]
fn simulate_cmd_c(target_pid: i32) {
    unsafe {
        type CGEventSourceRef = *mut std::ffi::c_void;
        type CGEventRef = *mut std::ffi::c_void;
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventCreateKeyboardEvent(
                source: CGEventSourceRef,
                keycode: u16,
                key_down: bool,
            ) -> CGEventRef;
            fn CGEventSetFlags(event: CGEventRef, flags: u64);
            fn CGEventPost(tap: u32, event: CGEventRef);
            fn CGEventPostToPid(pid: i32, event: CGEventRef);
            fn CFRelease(cf: *mut std::ffi::c_void);
        }
        let down = CGEventCreateKeyboardEvent(std::ptr::null_mut(), 8, true);
        let up = CGEventCreateKeyboardEvent(std::ptr::null_mut(), 8, false);
        if !down.is_null() && !up.is_null() {
            CGEventSetFlags(down, 0x00100000);
            CGEventSetFlags(up, 0x00100000);
            if target_pid > 0 {
                CGEventPostToPid(target_pid, down);
                CGEventPostToPid(target_pid, up);
            } else {
                CGEventPost(0, down);
                CGEventPost(0, up);
            }
        }
        if !down.is_null() {
            CFRelease(down);
        }
        if !up.is_null() {
            CFRelease(up);
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
fn simulate_cmd_c(_target_pid: i32) {}

/// Read the target app's current text selection via Cmd+C, preserving the user's
/// clipboard. Returns None when nothing is selected.
#[cfg(target_os = "macos")]
fn capture_selection(pid: i32) -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let original = clipboard.get_text().ok();
    // Sentinel lets us distinguish "copy did nothing" from "copied real text".
    let sentinel = "\u{0}copyosity-sel\u{0}";
    let _ = clipboard.set_text(sentinel);
    drop(clipboard);

    simulate_cmd_c(pid);
    std::thread::sleep(std::time::Duration::from_millis(160));

    let mut clipboard = arboard::Clipboard::new().ok()?;
    let after = clipboard.get_text().ok();
    if let Some(t) = &original {
        let _ = clipboard.set_text(t);
    }

    match after {
        Some(t) if t != sentinel && !t.trim().is_empty() => Some(t),
        _ => None,
    }
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
fn capture_selection(_pid: i32) -> Option<String> {
    None
}

/// The monitor the user is currently working on — the one containing the mouse
/// cursor. Falls back to the window's current monitor, then the primary monitor.
fn active_monitor(window: &tauri::WebviewWindow) -> Option<tauri::Monitor> {
    if let Ok(pos) = window.app_handle().cursor_position() {
        if let Ok(monitors) = window.available_monitors() {
            for m in monitors {
                let mp = m.position();
                let ms = m.size();
                let left = mp.x as f64;
                let top = mp.y as f64;
                let right = left + ms.width as f64;
                let bottom = top + ms.height as f64;
                if pos.x >= left && pos.x < right && pos.y >= top && pos.y < bottom {
                    return Some(m);
                }
            }
        }
    }
    window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
}

/// Position + size the main board on the active screen. Horizontal = a wide bar
/// docked to the bottom; vertical = a tall mini-clipboard docked to the right edge.
/// Default overlay height until the frontend applies layout (base + hints on).
pub const OVERLAY_HEIGHT_COMPACT: f64 = 450.0;

pub const OVERLAY_HEIGHT_MIN: f64 = 360.0;
pub const OVERLAY_HEIGHT_MAX: f64 = 560.0;

/// Horizontal board: minimum width (logical px); user resize cannot go below this.
pub(crate) const OVERLAY_WIDTH_MIN: f64 = 900.0;
pub(crate) const OVERLAY_WIDTH_PREFERRED: f64 = 1200.0;

/// Vertical board: minimum width (logical px); user resize cannot go below this.
pub(crate) const OVERLAY_WIDTH_MIN_VERTICAL: f64 = 360.0;
/// Vertical board: default width used until the user resizes it.
pub(crate) const OVERLAY_WIDTH_PREFERRED_VERTICAL: f64 = OVERLAY_WIDTH_MIN_VERTICAL;
/// Vertical board: maximum width (logical px).
pub(crate) const OVERLAY_WIDTH_MAX_VERTICAL: f64 = 720.0;
const OVERLAY_HEIGHT_PREFERRED_VERTICAL: f64 = 820.0;
const OVERLAY_HEIGHT_MIN_VERTICAL: f64 = 520.0;

static LAST_OVERLAY_HEIGHT_BITS: AtomicU64 = AtomicU64::new(0);
/// Physical size `position_window_bottom` last applied to the main window, used by
/// `persist_main_window_user_resize` to tell an echoed `Resized` event (our own
/// `set_size` call) apart from an actual user drag on the resize grip.
static LAST_PROGRAMMATIC_WIDTH: AtomicU32 = AtomicU32::new(0);
static LAST_PROGRAMMATIC_HEIGHT: AtomicU32 = AtomicU32::new(0);

pub(crate) fn remember_overlay_height(height: f64) {
    let clamped = height.clamp(OVERLAY_HEIGHT_MIN, OVERLAY_HEIGHT_MAX);
    LAST_OVERLAY_HEIGHT_BITS.store(clamped.to_bits(), Ordering::Relaxed);
}

pub(crate) fn remembered_overlay_height() -> f64 {
    // Hint for pre-show placement only; frontend resize_main_window is authoritative before reveal.
    let bits = LAST_OVERLAY_HEIGHT_BITS.load(Ordering::Relaxed);
    if bits == 0 {
        OVERLAY_HEIGHT_COMPACT
    } else {
        f64::from_bits(bits).clamp(OVERLAY_HEIGHT_MIN, OVERLAY_HEIGHT_MAX)
    }
}

pub(crate) fn reset_remembered_overlay_height() {
    LAST_OVERLAY_HEIGHT_BITS.store(0, Ordering::Relaxed);
}

/// Horizontal board: clamp a logical width to the allowed range (no upper bound
/// other than the active monitor's work area, applied separately).
fn clamp_horizontal_width(width: f64) -> f64 {
    width.max(OVERLAY_WIDTH_MIN)
}

/// Vertical board: clamp a logical width to the allowed range.
fn clamp_vertical_width(width: f64) -> f64 {
    width.clamp(OVERLAY_WIDTH_MIN_VERTICAL, OVERLAY_WIDTH_MAX_VERTICAL)
}

/// Resolve the physical width to apply for the horizontal board, given the
/// preferred logical width and the active monitor's physical work-area width.
fn resolve_horizontal_win_width(preferred_logical: f64, scale: f64, work_area_width: u32) -> u32 {
    let min_width = (OVERLAY_WIDTH_MIN * scale) as u32;
    let preferred_width = (clamp_horizontal_width(preferred_logical) * scale) as u32;
    preferred_width.min(work_area_width).max(min_width)
}

/// Resolve the physical width to apply for the vertical board, given the
/// preferred logical width and the active monitor's physical work-area width.
fn resolve_vertical_win_width(preferred_logical: f64, scale: f64, work_area_width: u32) -> u32 {
    let min_w = (OVERLAY_WIDTH_MIN_VERTICAL * scale) as u32;
    let max_w = (OVERLAY_WIDTH_MAX_VERTICAL * scale) as u32;
    let w = (clamp_vertical_width(preferred_logical) * scale) as u32;
    w.min(work_area_width).max(min_w).min(max_w)
}

fn overlay_horizontal_width_from_db(window: &tauri::WebviewWindow) -> f64 {
    window
        .app_handle()
        .try_state::<Arc<db::Database>>()
        .and_then(|db| db.overlay_horizontal_width().ok().flatten())
        .map(clamp_horizontal_width)
        .unwrap_or(OVERLAY_WIDTH_PREFERRED)
}

fn overlay_vertical_width_from_db(window: &tauri::WebviewWindow) -> f64 {
    window
        .app_handle()
        .try_state::<Arc<db::Database>>()
        .and_then(|db| db.overlay_vertical_width().ok().flatten())
        .map(clamp_vertical_width)
        .unwrap_or(OVERLAY_WIDTH_PREFERRED_VERTICAL)
}

fn apply_overlay_size_limits(
    window: &tauri::WebviewWindow,
    vertical: bool,
    horizontal_height: f64,
) {
    use tauri::LogicalSize;

    if vertical {
        let _ = window.set_min_size(Some(LogicalSize::new(
            OVERLAY_WIDTH_MIN_VERTICAL,
            OVERLAY_HEIGHT_MIN_VERTICAL,
        )));
        let _ = window.set_max_size(Some(LogicalSize::new(OVERLAY_WIDTH_MAX_VERTICAL, 10_000.0)));
    } else {
        // Cards are fixed-height, so vertical drag-resize is locked entirely (min == max);
        // only the width edges remain draggable. See docs/architecture — horizontal board.
        // Vertical-board *height* resize (see the `vertical` branch above) is intentionally
        // never persisted — only width is saved to disk — so it resets to the preferred
        // height on next reveal.
        let _ = window.set_min_size(Some(LogicalSize::new(OVERLAY_WIDTH_MIN, horizontal_height)));
        let _ = window.set_max_size(Some(LogicalSize::new(100_000.0, horizontal_height)));
    }
}

/// User dragged the native resize edge/grip; persist the resulting logical width so the next
/// `position_window_bottom` restores it. Ignores resizes while the panel is hidden (creation,
/// pre-show `position_window_bottom` calls), and ignores resizes that merely echo the size
/// `position_window_bottom` itself just applied (e.g. `resize_main_window` height animations),
/// since neither is a user-driven width change.
fn persist_main_window_user_resize(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    let Ok(size) = window.inner_size() else {
        return;
    };
    if size.width == LAST_PROGRAMMATIC_WIDTH.load(Ordering::Relaxed)
        && size.height == LAST_PROGRAMMATIC_HEIGHT.load(Ordering::Relaxed)
    {
        return;
    }
    let Ok(scale) = window.scale_factor() else {
        return;
    };
    let logical_width = size.width as f64 / scale;
    let Some(db) = app.try_state::<Arc<db::Database>>() else {
        return;
    };
    let vertical = db
        .get_app_settings()
        .map(|s| s.board_vertical)
        .unwrap_or(false);
    if vertical {
        let _ = db.set_overlay_vertical_width(clamp_vertical_width(logical_width));
    } else {
        let _ = db.set_overlay_horizontal_width(clamp_horizontal_width(logical_width));
    }
}

pub(crate) fn position_window_bottom(window: &tauri::WebviewWindow, height_px: f64) {
    use tauri::{PhysicalPosition, PhysicalSize};

    let vertical = window
        .app_handle()
        .try_state::<Arc<db::Database>>()
        .and_then(|db| db.get_app_settings().ok())
        .map(|s| s.board_vertical)
        .unwrap_or(false);

    let Some(monitor) = active_monitor(window) else {
        return;
    };
    let work_area = monitor.work_area();
    let scale = monitor.scale_factor();
    let pad = (28.0 * scale) as i32;

    let (win_width, win_height, x, y) = if vertical {
        // Tall narrow panel docked to the right edge of the active screen.
        let min_h = (OVERLAY_HEIGHT_MIN_VERTICAL * scale) as u32;
        let preferred_h = (OVERLAY_HEIGHT_PREFERRED_VERTICAL * scale) as u32;
        let win_width = resolve_vertical_win_width(
            overlay_vertical_width_from_db(window),
            scale,
            work_area.size.width,
        );
        let win_height = preferred_h
            .min(work_area.size.height.saturating_sub(pad as u32 * 2))
            .max(min_h.min(work_area.size.height));
        let x = work_area.position.x + work_area.size.width as i32 - win_width as i32 - pad;
        let y = work_area.position.y + ((work_area.size.height as i32 - win_height as i32) / 2);
        (win_width, win_height, x, y)
    } else {
        // Wide bar docked to the bottom-centre of the active screen.
        let win_width = resolve_horizontal_win_width(
            overlay_horizontal_width_from_db(window),
            scale,
            work_area.size.width,
        );
        let win_height = (height_px * scale) as u32;
        let x = work_area.position.x + ((work_area.size.width as i32 - win_width as i32) / 2);
        let y = work_area.position.y + work_area.size.height as i32 - win_height as i32 - pad;
        (win_width, win_height, x, y)
    };

    // Record the size we're about to apply *before* calling `set_size`: on macOS,
    // `setFrame` invokes the `windowDidResize` delegate (our `Resized` handler)
    // synchronously, before `set_size` returns. Storing these first means that
    // reentrant handler sees a matching size immediately and skips straight past
    // the DB lookup, instead of doing a synchronous SQLite round-trip on every
    // frame of a height-animation loop (`animateOverlayResize`), which caused
    // visible animation jank.
    LAST_PROGRAMMATIC_WIDTH.store(win_width, Ordering::Relaxed);
    LAST_PROGRAMMATIC_HEIGHT.store(win_height, Ordering::Relaxed);
    // Apply the target min/max constraints before resizing so the OS never clamps
    // `set_size` against a stale limit left over from the previous call.
    apply_overlay_size_limits(window, vertical, height_px);
    let _ = window.set_size(PhysicalSize::new(win_width, win_height));
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

#[cfg(test)]
mod overlay_height_tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn remembered_height_defaults_to_compact() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_remembered_overlay_height();
        assert_eq!(remembered_overlay_height(), OVERLAY_HEIGHT_COMPACT);
    }

    #[test]
    fn remember_overlay_height_round_trips() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_remembered_overlay_height();
        remember_overlay_height(508.0);
        assert_eq!(remembered_overlay_height(), 508.0);
        reset_remembered_overlay_height();
    }

    #[test]
    fn remember_overlay_height_clamps_out_of_range_values() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_remembered_overlay_height();
        remember_overlay_height(999.0);
        assert_eq!(remembered_overlay_height(), OVERLAY_HEIGHT_MAX);
        remember_overlay_height(100.0);
        assert_eq!(remembered_overlay_height(), OVERLAY_HEIGHT_MIN);
        reset_remembered_overlay_height();
    }
}

#[cfg(test)]
mod overlay_width_tests {
    use super::*;

    #[test]
    fn clamp_horizontal_width_enforces_min_only() {
        assert_eq!(clamp_horizontal_width(400.0), OVERLAY_WIDTH_MIN);
        assert_eq!(clamp_horizontal_width(OVERLAY_WIDTH_MIN), OVERLAY_WIDTH_MIN);
        assert_eq!(clamp_horizontal_width(2000.0), 2000.0);
    }

    #[test]
    fn clamp_vertical_width_enforces_min_and_max() {
        assert_eq!(clamp_vertical_width(100.0), OVERLAY_WIDTH_MIN_VERTICAL);
        assert_eq!(clamp_vertical_width(5000.0), OVERLAY_WIDTH_MAX_VERTICAL);
        assert_eq!(clamp_vertical_width(500.0), 500.0);
    }

    #[test]
    fn resolve_horizontal_win_width_clamps_to_work_area() {
        // Preferred width fits comfortably within a large work area.
        assert_eq!(resolve_horizontal_win_width(1200.0, 1.0, 2560), 1200);
        // Work area narrower than preferred width caps the result, but never below min.
        assert_eq!(resolve_horizontal_win_width(1200.0, 1.0, 1000), 1000);
        // Below-minimum preferred widths are raised to the minimum.
        assert_eq!(resolve_horizontal_win_width(100.0, 1.0, 2560), 900);
    }

    #[test]
    fn resolve_vertical_win_width_clamps_to_range_and_work_area() {
        assert_eq!(resolve_vertical_win_width(360.0, 1.0, 2560), 360);
        // Above-maximum preferred widths are capped.
        assert_eq!(resolve_vertical_win_width(5000.0, 1.0, 2560), 720);
        // A narrow work area wins over the preferred width but not below the minimum.
        assert_eq!(resolve_vertical_win_width(600.0, 1.0, 200), 360);
    }
}
