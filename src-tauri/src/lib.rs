#![allow(unexpected_cfgs)]

mod app_exclusion;
mod macos_app;
mod clipboard_macos;
mod clipboard_monitor;
mod clipboard_write;
mod commands;
mod db;
mod image_format;
mod ollama;
mod whisper;

use db::Database;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{
    Emitter, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[cfg(target_os = "macos")]
use tauri_nspanel::{ManagerExt, WebviewWindowExt};

#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel!(
    panel!(CopyosityPanel {
        config: {
            can_become_key_window: true,
            is_floating_panel: true
        }
    })
);

static LAST_SHOW_MS: AtomicU64 = AtomicU64::new(0);

/// Main panel level while hidden — below status-bar menu popups.
#[cfg(target_os = "macos")]
const PANEL_LEVEL_IDLE: i64 = 3;
/// Main panel level while shown — above fullscreen apps.
#[cfg(target_os = "macos")]
const PANEL_LEVEL_ACTIVE: i64 = 24;

static RECORDING: std::sync::OnceLock<std::sync::Mutex<Option<whisper::RecordingSession>>> =
    std::sync::OnceLock::new();

fn recording_mutex() -> &'static std::sync::Mutex<Option<whisper::RecordingSession>> {
    RECORDING.get_or_init(|| std::sync::Mutex::new(None))
}

static CURRENT_VOICE_SHORTCUT: std::sync::OnceLock<std::sync::Mutex<Option<Shortcut>>> =
    std::sync::OnceLock::new();

fn voice_shortcut_mutex() -> &'static std::sync::Mutex<Option<Shortcut>> {
    CURRENT_VOICE_SHORTCUT.get_or_init(|| std::sync::Mutex::new(None))
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
                    'a' => Some(Code::KeyA), 'b' => Some(Code::KeyB), 'c' => Some(Code::KeyC),
                    'd' => Some(Code::KeyD), 'e' => Some(Code::KeyE), 'f' => Some(Code::KeyF),
                    'g' => Some(Code::KeyG), 'h' => Some(Code::KeyH), 'i' => Some(Code::KeyI),
                    'j' => Some(Code::KeyJ), 'k' => Some(Code::KeyK), 'l' => Some(Code::KeyL),
                    'm' => Some(Code::KeyM), 'n' => Some(Code::KeyN), 'o' => Some(Code::KeyO),
                    'p' => Some(Code::KeyP), 'q' => Some(Code::KeyQ), 'r' => Some(Code::KeyR),
                    's' => Some(Code::KeyS), 't' => Some(Code::KeyT), 'u' => Some(Code::KeyU),
                    'v' => Some(Code::KeyV), 'w' => Some(Code::KeyW), 'x' => Some(Code::KeyX),
                    'y' => Some(Code::KeyY), 'z' => Some(Code::KeyZ),
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

fn unregister_voice_shortcut(app: &tauri::AppHandle) {
    let mut current = voice_shortcut_mutex().lock().unwrap();
    if let Some(old) = current.take() {
        let _ = app.global_shortcut().unregister(old);
    }
}

/// Register (or re-register) the voice shortcut from current DB settings.
/// Returns the shortcut string on success.
pub fn register_voice_shortcut(app: &tauri::AppHandle) -> Result<String, String> {
    let db = app.state::<std::sync::Arc<db::Database>>();
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;

    unregister_voice_shortcut(app);

    if !settings.voice_transcription_enabled {
        eprintln!("[voice] transcription disabled, shortcut not registered");
        return Ok(settings.voice_shortcut);
    }

    eprintln!("[voice] registering shortcut: \"{}\"", settings.voice_shortcut);

    let new_shortcut = parse_shortcut(&settings.voice_shortcut)
        .ok_or_else(|| format!("Invalid shortcut: {}", settings.voice_shortcut))?;

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

#[tauri::command]
fn frontend_ready(_app: tauri::AppHandle) {
    // Main window starts hidden in tauri.conf.json; nothing to hide here.
    // (Previously called hide() and raced with the first tray-menu click.)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build());

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .setup(|app| {
            // Menu-bar app: no Dock icon, no Cmd+Tab entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let app_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            let db = Arc::new(Database::new(app_dir).expect("Failed to initialize database"));
            app.manage(db.clone());

            // Convert main window to NSPanel (non-activating, floating)
            #[cfg(target_os = "macos")]
            {
                use tauri_nspanel::panel::NSWindowStyleMask;
                use tauri_nspanel::CollectionBehavior;

                let window = app.get_webview_window("main").unwrap();
                let panel = window.to_panel::<CopyosityPanel>().expect("Failed to convert window to panel");

                // Keep hidden panel below status-bar menus (level 24 fights first tray click).
                panel.set_level(PANEL_LEVEL_IDLE);
                panel.set_style_mask(
                    NSWindowStyleMask::Borderless
                        | NSWindowStyleMask::NonactivatingPanel
                        | NSWindowStyleMask::Resizable,
                );
                // Show on all spaces including over fullscreen apps
                panel.set_collection_behavior(
                    CollectionBehavior::new()
                        .can_join_all_spaces()
                        .full_screen_auxiliary()
                        .into(),
                );
                panel.set_hides_on_deactivate(false);
            }

            let tray_menu = build_tray_menu(app.handle())?;

            let tray_builder = TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Copyosity")
                .menu(&tray_menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => toggle_window(app.app_handle()),
                    "settings" => {
                        let _ = commands::open_settings_window(app.app_handle().clone());
                    }
                    "quit" => {
                        let _ = commands::quit_app(app.app_handle().clone());
                    }
                    _ => {}
                });

            let tray = tray_builder.build(app)?;
            app.manage(tray);

            let shortcut = {
                #[cfg(target_os = "macos")]
                { Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyV) }
                #[cfg(not(target_os = "macos"))]
                { Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV) }
            };

            let handle = app.handle().clone();
            app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    toggle_window(&handle);
                }
            })?;

            // Register voice transcription shortcut from settings
            if let Err(e) = register_voice_shortcut(app.handle()) {
                eprintln!("Voice shortcut registration failed: {}", e);
            }
            eprintln!(
                "copyosity: global shortcut registered = {}",
                app.global_shortcut().is_registered(shortcut)
            );

            let settings = db.get_app_settings().expect("Failed to load app settings");
            ollama::set_active_model(&settings.ollama_model);
            let _ = db.cleanup_old_entries(settings.retention_days);

            if settings.ai_tagging_enabled {
                ollama::ensure_runtime();
                ollama::backfill_existing_tags(app.handle().clone(), db.clone());
            }
            clipboard_write::sweep_stale_gif_temp_files();
            {
                let db_backfill = db.clone();
                std::thread::spawn(move || {
                    loop {
                        match db_backfill.backfill_missing_image_formats(100) {
                            Ok(0) | Err(_) => break,
                            Ok(_) => {}
                        }
                    }
                });
            }
            clipboard_monitor::start_clipboard_monitor(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            frontend_ready,
            commands::get_entries,
            commands::delete_entry,
            commands::pin_entry,
            commands::set_entry_collection,
            commands::get_collections,
            commands::create_collection,
            commands::delete_collection,
            commands::clear_history,
            commands::hide_main_window,
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
            commands::list_microphones,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                api.prevent_exit();
            }
            tauri::RunEvent::WindowEvent { label, event, .. } => {
                match (label.as_str(), &event) {
                    ("main", tauri::WindowEvent::CloseRequested { api, .. }) => {
                        api.prevent_close();
                        hide_panel(app);
                    }
                    ("main", tauri::WindowEvent::Focused(false)) => {
                        let last_show = LAST_SHOW_MS.load(Ordering::Relaxed);
                        if last_show == 0 {
                            return;
                        }
                        let elapsed = now_ms() - last_show;
                        if elapsed > 500 && main_panel_visible(app) {
                            hide_panel(app);
                        }
                    }
                    ("settings", tauri::WindowEvent::Destroyed) => {}
                    _ => {}
                }
            }
            _ => {}
        });
}

fn toggle_window(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        if let Ok(panel) = app.get_webview_panel("main") {
            if panel.is_visible() {
                panel.hide();
                panel.set_level(PANEL_LEVEL_IDLE);
            } else {
                if let Some(window) = app.get_webview_window("main") {
                    position_window_bottom(&window);
                }
                clipboard_macos::remember_paste_target();
                LAST_SHOW_MS.store(now_ms(), Ordering::Relaxed);
                panel.set_level(PANEL_LEVEL_ACTIVE);
                panel.show_and_make_key();
                let _ = app.emit("window-show", ());
            }
            return;
        }
    }

    // Fallback for non-macOS
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            LAST_SHOW_MS.store(now_ms(), Ordering::Relaxed);
            position_window_bottom(&window);
            let _ = window.show();
            let _ = window.set_focus();
            let _ = app.emit("window-show", ());
        }
    }
}

fn hide_panel(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        if let Ok(panel) = app.get_webview_panel("main") {
            if panel.is_visible() {
                panel.hide();
                panel.set_level(PANEL_LEVEL_IDLE);
            }
            return;
        }
    }

    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        }
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

fn build_tray_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let version = &app.package_info().version;
    let version_label = format!("Copyosity v{}", version);

    let open = MenuItem::with_id(app, "open", "Open Copyosity", true, None::<&str>)?;
    let ver = MenuItem::with_id(app, "version", &version_label, false, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    Menu::with_items(app, &[&open, &ver, &sep1, &settings, &sep2, &quit])
}

fn handle_voice_event(app: &tauri::AppHandle, state: ShortcutState) {
    eprintln!("[voice] event: {:?}", match state {
        ShortcutState::Pressed => "PRESSED",
        ShortcutState::Released => "RELEASED",
    });
    match state {
        ShortcutState::Pressed => {
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
                        eprintln!("[voice] recording started, sample_rate={}", session.sample_rate);
                        show_voice_overlay(app);
                        let level_arc = session.level.clone();
                        let emit_handle = app.clone();
                        std::thread::spawn(move || {
                            loop {
                                let still_recording =
                                    recording_mutex().lock().unwrap().is_some();
                                if !still_recording {
                                    break;
                                }
                                let lvl =
                                    level_arc.load(std::sync::atomic::Ordering::Relaxed);
                                if let Some(win) = emit_handle.get_webview_window("voice_overlay") {
                                    let _ = win.emit("audio-level", lvl);
                                } else {
                                    let _ = emit_handle.emit("audio-level", lvl);
                                }
                                std::thread::sleep(std::time::Duration::from_millis(60));
                            }
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
                hide_voice_overlay(&app);
                std::thread::spawn(move || {
                    let (samples, sample_rate) = session.finish();
                    eprintln!("[voice] stopped, {} samples at {}Hz ({:.1}s)",
                        samples.len(), sample_rate,
                        samples.len() as f64 / sample_rate as f64);
                    let db = app.state::<std::sync::Arc<db::Database>>();
                    let settings = match db.get_app_settings() {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("[voice] failed to load settings: {}", e);
                            return;
                        }
                    };
                    if settings.whisper_server_url.is_empty() {
                        eprintln!("[voice] ERROR: Whisper server URL is not configured");
                        return;
                    }
                    eprintln!("[voice] sending to {}", settings.whisper_server_url);
                    match whisper::transcribe_audio(
                        samples,
                        sample_rate,
                        &settings.whisper_server_url,
                        &settings.whisper_server_token,
                        &settings.whisper_server_model,
                    ) {
                        Ok(text) if !text.is_empty() => {
                            eprintln!("[voice] transcription: \"{}\"", text);
                            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                let _ = clipboard_write::write_text(
                                    &mut clipboard,
                                    text,
                                    clipboard_write::ClipboardWriteMode::Paste,
                                );
                            }
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            #[cfg(target_os = "macos")]
                            {
                                clipboard_macos::spawn_automated_paste(false);
                            }
                        }
                        Ok(_) => eprintln!("[voice] transcription returned empty text"),
                        Err(e) => eprintln!("[voice] transcription ERROR: {}", e),
                    }
                });
            }
        }
    }
}

fn ensure_voice_overlay(app: &tauri::AppHandle) {
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
        .inner_size(96.0, 44.0)
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
                panel.set_level(24);
                panel.set_style_mask(
                    NSWindowStyleMask::Borderless
                        | NSWindowStyleMask::NonactivatingPanel,
                );
                panel.set_becomes_key_only_if_needed(true);
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
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(win) = app.get_webview_window("voice_overlay") {
            let _ = win.close();
        }
    }
}

pub(crate) fn position_window_bottom(window: &tauri::WebviewWindow) {
    use tauri::PhysicalPosition;

    if let Ok(Some(monitor)) = window.current_monitor() {
        let work_area = monitor.work_area();
        let scale = monitor.scale_factor();
        let bottom_padding = (28.0 * scale) as i32;
        let min_width = (900.0 * scale) as u32;
        let preferred_width = (1180.0 * scale) as u32;
        let win_height = (410.0 * scale) as u32;
        let win_width = preferred_width.min(work_area.size.width).max(min_width);

        let x = work_area.position.x + ((work_area.size.width as i32 - win_width as i32) / 2);
        let y = work_area.position.y + work_area.size.height as i32 - win_height as i32 - bottom_padding;

        let _ = window.set_size(tauri::PhysicalSize::new(win_width, win_height));
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}
