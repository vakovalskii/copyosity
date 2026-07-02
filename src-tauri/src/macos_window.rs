//! macOS window/panel presentation — float above fullscreen apps on all Spaces.

/// Same level as the main overlay panel (`NSMainMenuWindowLevel`).
#[cfg(target_os = "macos")]
pub const FULLSCREEN_AUXILIARY_LEVEL: i64 = 24;

#[cfg(target_os = "macos")]
pub fn fullscreen_auxiliary_collection_behavior() -> objc2_app_kit::NSWindowCollectionBehavior {
    use tauri_nspanel::CollectionBehavior;

    CollectionBehavior::new()
        .can_join_all_spaces()
        .full_screen_auxiliary()
        .into()
}

#[cfg(target_os = "macos")]
pub fn configure_fullscreen_auxiliary_panel(panel: &dyn tauri_nspanel::Panel) {
    panel.set_level(FULLSCREEN_AUXILIARY_LEVEL);
    panel.set_collection_behavior(fullscreen_auxiliary_collection_behavior());
    panel.set_hides_on_deactivate(false);
    panel.set_floating_panel(true);
}

#[cfg(target_os = "macos")]
pub fn present_fullscreen_auxiliary_panel(panel: &dyn tauri_nspanel::Panel) {
    configure_fullscreen_auxiliary_panel(panel);
    panel.show_and_make_key();
}

#[cfg(target_os = "macos")]
pub fn apply_fullscreen_auxiliary_webview(window: &tauri::WebviewWindow) {
    use objc2_app_kit::NSWindow;

    let Ok(raw) = window.ns_window() else {
        return;
    };
    unsafe {
        let ns_window = &*raw.cast::<NSWindow>();
        ns_window.setCollectionBehavior(fullscreen_auxiliary_collection_behavior());
        ns_window.setLevel(FULLSCREEN_AUXILIARY_LEVEL as isize);
        ns_window.setHidesOnDeactivate(false);
    }
}

#[cfg(target_os = "macos")]
pub fn present_fullscreen_auxiliary_webview(window: &tauri::WebviewWindow) {
    use objc2_app_kit::NSWindow;

    apply_fullscreen_auxiliary_webview(window);
    let Ok(raw) = window.ns_window() else {
        return;
    };
    unsafe {
        let ns_window = &*raw.cast::<NSWindow>();
        ns_window.orderFrontRegardless();
        ns_window.makeKeyAndOrderFront(None);
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use objc2_app_kit::NSWindowCollectionBehavior;

    #[test]
    fn fullscreen_auxiliary_behavior_includes_required_flags() {
        let behavior = fullscreen_auxiliary_collection_behavior();
        assert!(behavior.contains(NSWindowCollectionBehavior::CanJoinAllSpaces));
        assert!(behavior.contains(NSWindowCollectionBehavior::FullScreenAuxiliary));
    }

    #[test]
    fn fullscreen_auxiliary_level_matches_main_menu() {
        assert_eq!(FULLSCREEN_AUXILIARY_LEVEL, 24);
    }
}
