mod agent;
mod clipboard_monitor;
mod commands;
mod db;
mod hub;
mod mactools;
mod ocr;
mod ollama;
mod screen;
mod tagging;
mod whisper;

use db::Database;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{
    Emitter, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
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
    let use_hub = settings.hub_transcribe_enabled
        && !settings.hub_token.is_empty()
        && !settings.hub_url.trim().is_empty();
    let (url, tok) = if use_hub {
        (
            format!("{}/v1/audio/transcriptions", settings.hub_url.trim_end_matches('/')),
            settings.hub_token.clone(),
        )
    } else {
        (settings.whisper_server_url.clone(), settings.whisper_server_token.clone())
    };
    if url.is_empty() {
        return Err("Transcription endpoint not configured".to_string());
    }
    whisper::transcribe_audio(samples, sample_rate, &url, &tok, &settings.whisper_server_model)
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

/// Register (or re-register) the voice shortcut from current DB settings.
/// Returns the shortcut string on success.
pub fn register_voice_shortcut(app: &tauri::AppHandle) -> Result<String, String> {
    let db = app.state::<std::sync::Arc<db::Database>>();
    let settings = db.get_app_settings().map_err(|e| e.to_string())?;

    eprintln!("[voice] registering shortcut: \"{}\"", settings.voice_shortcut);

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

#[tauri::command]
fn frontend_ready(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
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

                // Floating above other windows like Spotlight
                panel.set_level(24); // NSPopUpMenuWindowLevel
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
            }

            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Copyosity")
                .menu(&build_tray_menu(app)?)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => toggle_window(app.app_handle()),
                    "search" => toggle_command_palette(app.app_handle()),
                    "settings" => {
                        let _ = commands::open_settings_window(app.app_handle().clone());
                    }
                    "quit" => {
                        let _ = commands::quit_app(app.app_handle().clone());
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        toggle_window(app);
                    }
                })
                .build(app)?;
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

            // Command palette (hub agent search): Cmd+Shift+Space.
            let palette_shortcut =
                Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Space);
            let palette_handle = app.handle().clone();
            app.global_shortcut()
                .on_shortcut(palette_shortcut, move |_app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        toggle_command_palette(&palette_handle);
                    }
                })?;

            // Pre-create voice overlay panel so it's ready without stealing focus later
            ensure_voice_overlay(app.handle());
            #[cfg(target_os = "macos")]
            ensure_command_palette(app.handle());

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

            ollama::ensure_runtime();
            ollama::backfill_existing_tags(app.handle().clone(), db.clone());
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
            commands::add_frontmost_app_to_excluded,
            commands::update_app_settings,
            commands::retag_entry,
            commands::copy_entry,
            commands::activate_entry,
            commands::paste_entry,
            commands::check_accessibility,
            commands::check_ollama_status,
            commands::start_ollama_server,
            commands::pull_ollama_model,
            commands::unload_ollama_model,
            commands::test_ollama_tagging,
            commands::rebind_voice_shortcut,
            commands::list_microphones,
            commands::hub_test_connection,
            commands::hub_list_models,
            palette_search,
            palette_hide,
            palette_insert,
            open_command_palette,
            palette_agent,
            palette_voice_start,
            palette_voice_stop,
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
                        let elapsed = now_ms() - LAST_SHOW_MS.load(Ordering::Relaxed);
                        if elapsed > 500 {
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
            } else {
                if let Some(window) = app.get_webview_window("main") {
                    position_window_bottom(&window);
                }
                LAST_SHOW_MS.store(now_ms(), Ordering::Relaxed);
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
            panel.hide();
            return;
        }
    }

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

fn build_tray_menu(app: &tauri::App) -> tauri::Result<Menu<tauri::Wry>> {
    let version = &app.package_info().version;
    let version_label = format!("Copyosity v{}", version);

    let status = MenuItem::with_id(app, "open", "Open Copyosity", true, None::<&str>)?;
    let search = MenuItem::with_id(app, "search", "Agent Search  ⌘⇧Space", true, None::<&str>)?;
    let ver = MenuItem::with_id(app, "version", &version_label, false, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    Menu::with_items(app, &[&status, &search, &ver, &sep, &settings, &sep2, &quit])
}

/// Run the transcription through the hub polish step when enabled, falling back
/// to the raw transcription on any error. Consumes the screenshot/app-kind
/// context captured at press time.
fn maybe_polish(settings: &db::AppSettings, raw: &str) -> String {
    let screenshot = VOICE_SCREENSHOT.lock().unwrap().take();
    let app_kind = std::mem::take(&mut *VOICE_APP_KIND.lock().unwrap());
    let selection = VOICE_SELECTION.lock().unwrap().take();

    if !settings.voice_polish_enabled
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
    let kind = if app_kind.is_empty() { "general" } else { app_kind.as_str() };
    let selected = if settings.voice_selected_text { selection.as_deref() } else { None };
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
    eprintln!("[voice] event: {:?}", match state {
        ShortcutState::Pressed => "PRESSED",
        ShortcutState::Released => "RELEASED",
    });
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
                    eprintln!("[voice] captured target pid={}", pid);

                    // Context-aware polishing: classify the target app and grab a
                    // screenshot now, while the target window is still frontmost.
                    let polish = app
                        .try_state::<std::sync::Arc<db::Database>>()
                        .and_then(|db| db.get_app_settings().ok());
                    if let Some(s) = polish {
                        eprintln!(
                            "[voice] polish_enabled={} screenshot={} selected_text={}",
                            s.voice_polish_enabled, s.voice_polish_screenshot, s.voice_selected_text
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
                                        eprintln!("[voice] screenshot ready ({} b64 chars)", b64.len());
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
                    // Route to the hub's transcription endpoint when enabled,
                    // otherwise use the standalone Whisper server config.
                    let use_hub = settings.hub_transcribe_enabled
                        && !settings.hub_token.is_empty()
                        && !settings.hub_url.trim().is_empty();
                    let (whisper_url, whisper_token) = if use_hub {
                        (
                            format!(
                                "{}/v1/audio/transcriptions",
                                settings.hub_url.trim_end_matches('/')
                            ),
                            settings.hub_token.clone(),
                        )
                    } else {
                        (
                            settings.whisper_server_url.clone(),
                            settings.whisper_server_token.clone(),
                        )
                    };
                    if whisper_url.is_empty() {
                        eprintln!("[voice] ERROR: transcription endpoint is not configured");
                        return;
                    }
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
                            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                let _ = clipboard.set_text(&final_text);
                            }
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            let target_pid = VOICE_TARGET_PID.swap(0, Ordering::Relaxed);
                            simulate_cmd_v(target_pid);
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
fn classify_app_kind(bundle_id: &str) -> &'static str {
    let b = bundle_id.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| b.contains(n));
    if has(&["mail", "outlook", "spark", "airmail", "sparrow"]) {
        "email"
    } else if has(&[
        "telegram", "slack", "discord", "whatsapp", "messages", "mobilesms", "signal",
        "viber", "messenger", "rocket.chat",
    ]) {
        "chat"
    } else if has(&[
        "vscode", "xcode", "jetbrains", "intellij", "pycharm", "goland", "webstorm",
        "iterm", "terminal", "sublime", "zed", "nova", "cursor", "warp",
    ]) {
        "code"
    } else if has(&["pages", "word", "notion", "obsidian", "bear", "ulysses", "docs", "scrivener"]) {
        "document"
    } else {
        "general"
    }
}

/// Synthesize Cmd+V. When `target_pid` is a valid pid, the event is delivered
/// straight to that process so it works regardless of which app is frontmost
/// (the recording overlay can briefly make Copyosity itself frontmost).
// ---- Hub agent command palette ----

/// Toggle the command palette. Captures the frontmost app first so the answer
/// can be inserted there, then shows the palette and gives it keyboard focus.
fn toggle_command_palette(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        ensure_command_palette(app);
        if let Ok(panel) = app.get_webview_panel("command_palette") {
            if panel.is_visible() {
                panel.hide();
                return;
            }
            if let Some(pid) = frontmost_app_pid() {
                if pid != std::process::id() as i32 {
                    PALETTE_TARGET_PID.store(pid, Ordering::Relaxed);
                }
            }
            // Don't re-center on show — keep the window where the user last
            // moved/resized it (it was centered once at creation).
            panel.show_and_make_key();
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
    .min_inner_size(380.0, 160.0)
    .resizable(true)
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
            panel.set_style_mask(NSWindowStyleMask::Borderless | NSWindowStyleMask::Resizable);
        }
    }
}

/// Run a web search against the hub Search API and return formatted results.
#[tauri::command]
fn palette_search(app: tauri::AppHandle, query: String) -> Result<String, String> {
    let db = app.state::<std::sync::Arc<db::Database>>();
    let s = db.get_app_settings().map_err(|e| e.to_string())?;
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
fn palette_agent(app: tauri::AppHandle, query: String) -> Result<(), String> {
    let db = app.state::<std::sync::Arc<db::Database>>();
    let s = db.get_app_settings().map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        agent::run(&app, &s.hub_url, &s.hub_token, &query);
    });
    Ok(())
}

/// Start recording from the palette mic.
#[tauri::command]
fn palette_voice_start(app: tauri::AppHandle) -> Result<(), String> {
    let mut rec = palette_recording_mutex().lock().unwrap();
    if rec.is_some() {
        return Ok(());
    }
    let mic = app
        .try_state::<std::sync::Arc<db::Database>>()
        .and_then(|db| db.get_app_settings().ok())
        .map(|s| s.selected_microphone)
        .filter(|s| !s.is_empty());
    let session = whisper::RecordingSession::start(mic.as_deref())?;
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

/// Hide the command palette.
#[tauri::command]
fn palette_hide(app: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        if let Ok(panel) = app.get_webview_panel("command_palette") {
            panel.hide();
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(win) = app.get_webview_window("command_palette") {
            let _ = win.hide();
        }
    }
}

/// Hide the palette and paste `text` into the app that was frontmost when it opened.
#[tauri::command]
fn palette_insert(app: tauri::AppHandle, text: String) {
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(&text);
    }
    palette_hide(app);
    std::thread::sleep(std::time::Duration::from_millis(120));
    let target_pid = PALETTE_TARGET_PID.swap(0, Ordering::Relaxed);
    simulate_cmd_v(target_pid);
}

#[cfg(target_os = "macos")]
fn simulate_cmd_v(target_pid: i32) {
    unsafe {
        type CGEventSourceRef = *mut std::ffi::c_void;
        type CGEventRef = *mut std::ffi::c_void;
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventCreateKeyboardEvent(source: CGEventSourceRef, keycode: u16, key_down: bool) -> CGEventRef;
            fn CGEventSetFlags(event: CGEventRef, flags: u64);
            fn CGEventPost(tap: u32, event: CGEventRef);
            fn CGEventPostToPid(pid: i32, event: CGEventRef);
            fn CFRelease(cf: *mut std::ffi::c_void);
        }
        let down = CGEventCreateKeyboardEvent(std::ptr::null_mut(), 9, true);
        let up = CGEventCreateKeyboardEvent(std::ptr::null_mut(), 9, false);
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
        // Free each event independently to avoid leaking on partial allocation.
        if !down.is_null() {
            CFRelease(down);
        }
        if !up.is_null() {
            CFRelease(up);
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn simulate_cmd_v(_target_pid: i32) {}

/// Synthesize Cmd+C, delivered to `target_pid` when valid. Used to read the
/// current text selection of the target app (selected-text command mode).
#[cfg(target_os = "macos")]
fn simulate_cmd_c(target_pid: i32) {
    unsafe {
        type CGEventSourceRef = *mut std::ffi::c_void;
        type CGEventRef = *mut std::ffi::c_void;
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGEventCreateKeyboardEvent(source: CGEventSourceRef, keycode: u16, key_down: bool) -> CGEventRef;
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
