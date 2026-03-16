mod clipboard_monitor;
mod commands;
mod db;
mod ollama;

use db::Database;
use std::sync::Arc;
use tauri::{
    Emitter, Manager,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[tauri::command]
fn frontend_ready(app: tauri::AppHandle) {
    // Frontend loaded — now hide the off-screen window, it's warmed up
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let app_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            let db = Arc::new(Database::new(app_dir).expect("Failed to initialize database"));
            app.manage(db.clone());

            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Copyosity")
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
            std::env::set_var("COPYOSITY_OLLAMA_MODEL", &settings.ollama_model);
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
            commands::get_app_settings,
            commands::get_model_catalog,
            commands::update_app_settings,
            commands::retag_entry,
            commands::paste_entry,
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
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.hide();
                            }
                        }
                        tauri::WindowEvent::Focused(false) => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.hide();
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
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            position_window_bottom(&window);
            let _ = window.show();
            let _ = window.set_focus();
            let _ = app.emit("window-show", ());
        }
    }
}

fn position_window_bottom(window: &tauri::WebviewWindow) {
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
