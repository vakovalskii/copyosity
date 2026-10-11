//! Tauri commands for the experimental LAN sync. Always compiled so the handler list in
//! lib.rs stays one list; without the `lan-sync` feature every command answers
//! `lan_sync_unavailable` and the settings UI hides the section.

#[cfg(feature = "lan-sync")]
use std::sync::Arc;

#[cfg(feature = "lan-sync")]
use tauri::Manager;

#[cfg(feature = "lan-sync")]
fn sync(app: &tauri::AppHandle) -> Result<Arc<crate::lan_sync::LanSync>, String> {
    app.try_state::<Arc<crate::lan_sync::LanSync>>()
        .map(|s| s.inner().clone())
        .ok_or_else(|| "lan_sync_not_ready".to_owned())
}

#[cfg(not(feature = "lan-sync"))]
fn unavailable<T>() -> Result<T, String> {
    Err("lan_sync_unavailable".to_owned())
}

#[tauri::command]
pub fn lan_sync_status(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    #[cfg(feature = "lan-sync")]
    return Ok(sync(&app)?.status());
    #[cfg(not(feature = "lan-sync"))]
    {
        let _ = app;
        unavailable()
    }
}

#[tauri::command]
pub fn lan_sync_set_enabled(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    #[cfg(feature = "lan-sync")]
    return sync(&app)?.set_enabled(enabled);
    #[cfg(not(feature = "lan-sync"))]
    {
        let _ = (app, enabled);
        unavailable()
    }
}

#[tauri::command]
pub fn lan_sync_start_pairing(app: tauri::AppHandle) -> Result<String, String> {
    #[cfg(feature = "lan-sync")]
    return Ok(sync(&app)?.start_pairing());
    #[cfg(not(feature = "lan-sync"))]
    {
        let _ = app;
        unavailable()
    }
}

#[tauri::command]
pub fn lan_sync_cancel_pairing(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(feature = "lan-sync")]
    {
        sync(&app)?.cancel_pairing();
        Ok(())
    }
    #[cfg(not(feature = "lan-sync"))]
    {
        let _ = app;
        unavailable()
    }
}

#[tauri::command]
pub async fn lan_sync_pair(
    app: tauri::AppHandle,
    device_id: String,
    code: String,
) -> Result<(), String> {
    #[cfg(feature = "lan-sync")]
    {
        let s = sync(&app)?;
        tauri::async_runtime::spawn_blocking(move || s.pair_with(&device_id, &code))
            .await
            .map_err(|e| e.to_string())?
    }
    #[cfg(not(feature = "lan-sync"))]
    {
        let _ = (app, device_id, code);
        unavailable()
    }
}

#[tauri::command]
pub fn lan_sync_unpair(app: tauri::AppHandle, device_id: String) -> Result<(), String> {
    #[cfg(feature = "lan-sync")]
    return sync(&app)?.unpair(&device_id);
    #[cfg(not(feature = "lan-sync"))]
    {
        let _ = (app, device_id);
        unavailable()
    }
}
