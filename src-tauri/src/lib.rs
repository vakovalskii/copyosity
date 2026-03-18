mod clipboard_monitor;
mod commands;
mod db;
mod ollama;

use db::Database;
use std::sync::atomic::{AtomicU64, Ordering};
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
            commands::test_ollama_tagging,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                api.prevent_exit();
            }
            tauri::RunEvent::WindowEvent { label, event, .. } => {
                if label == "main" {
                    match event {
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            api.prevent_close();
                            hide_panel(app);
                        }
                        tauri::WindowEvent::Focused(false) => {
                            let elapsed = now_ms() - LAST_SHOW_MS.load(Ordering::Relaxed);
                            if elapsed > 500 {
                                hide_panel(app);
                            }
                        }
                        _ => {}
                    }
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
    let ver = MenuItem::with_id(app, "version", &version_label, false, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    Menu::with_items(app, &[&status, &ver, &sep, &settings, &sep2, &quit])
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
