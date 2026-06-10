use crate::macos_app::{self, AppIdentity};
use serde::Serialize;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExcludableAppSource {
    Frontmost,
    Remembered,
}

static REMEMBERED_BUNDLE_ID: Mutex<Option<String>> = Mutex::new(None);

pub fn remember_app_identity(identity: &AppIdentity) {
    if macos_app::is_copyosity_bundle(&identity.bundle_id) {
        return;
    }
    *REMEMBERED_BUNDLE_ID.lock().unwrap() = Some(identity.bundle_id.clone());
}

pub fn remembered_bundle_id() -> Option<String> {
    REMEMBERED_BUNDLE_ID.lock().unwrap().clone()
}

pub fn remember_from_pid(pid: i32) {
    if let Some(identity) = macos_app::app_identity_for_pid(pid) {
        remember_app_identity(&identity);
    }
}

/// Live frontmost when it is not Copyosity; otherwise paste-target or remembered app.
pub fn resolve_excludable_app_identity() -> Option<(AppIdentity, ExcludableAppSource)> {
    let frontmost = macos_app::frontmost_app_identity();
    if let Some(resolved) = resolve_excludable_from(frontmost, remembered_bundle_id()) {
        return Some(resolved);
    }
    #[cfg(target_os = "macos")]
    if macos_app::is_copyosity_frontmost() {
        if let Some(pid) = crate::clipboard_macos::paste_target_pid() {
            if let Some(identity) = macos_app::app_identity_for_pid(pid) {
                return Some((identity, ExcludableAppSource::Remembered));
            }
        }
    }
    None
}

fn resolve_excludable_from(
    frontmost: Option<AppIdentity>,
    remembered_bundle_id: Option<String>,
) -> Option<(AppIdentity, ExcludableAppSource)> {
    if let Some(identity) = frontmost {
        if !macos_app::is_copyosity_bundle(&identity.bundle_id) {
            return Some((identity, ExcludableAppSource::Frontmost));
        }
    }
    remembered_bundle_id.map(|bundle_id| {
        let display_name = macos_app::display_name_for_bundle_id(&bundle_id);
        (
            AppIdentity {
                bundle_id,
                display_name,
            },
            ExcludableAppSource::Remembered,
        )
    })
}

#[cfg(target_os = "macos")]
pub fn pick_application_identity_on_main_thread() -> Result<Option<AppIdentity>, &'static str> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};
    use objc2_foundation::{NSURL, NSString};

    let mtm = MainThreadMarker::new().ok_or("main_thread_required")?;
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setTitle(Some(&NSString::from_str(
        "Choose an application to exclude",
    )));
    panel.setDirectoryURL(Some(&NSURL::fileURLWithPath_isDirectory(
        &NSString::from_str("/Applications"),
        true,
    )));
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(false);
    panel.setAllowsMultipleSelection(false);
    panel.setResolvesAliases(true);

    if panel.runModal() != NSModalResponseOK {
        return Ok(None);
    }

    let url = panel.URLs().firstObject().ok_or("picker_failed")?;
    let path = std::path::PathBuf::from(url.path().ok_or("picker_failed")?.to_string());
    Ok(macos_app::app_identity_from_app_bundle_path(&path))
}

#[cfg(not(target_os = "macos"))]
pub fn pick_application_identity_on_main_thread() -> Result<Option<AppIdentity>, &'static str> {
    Ok(None)
}

#[cfg(test)]
fn clear_remembered_for_test() {
    *REMEMBERED_BUNDLE_ID.lock().unwrap() = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(bundle_id: &str, display_name: &str) -> AppIdentity {
        AppIdentity {
            bundle_id: bundle_id.into(),
            display_name: display_name.into(),
        }
    }

    #[test]
    fn remembered_used_when_frontmost_is_copyosity() {
        clear_remembered_for_test();
        let resolved = resolve_excludable_from(
            Some(identity(macos_app::COPYOSITY_BUNDLE_ID, "Copyosity")),
            Some("com.apple.Safari".into()),
        );
        let (app, source) = resolved.expect("candidate");
        assert_eq!(app.bundle_id, "com.apple.Safari");
        assert_eq!(source, ExcludableAppSource::Remembered);
    }

    #[test]
    fn frontmost_takes_priority_over_remembered() {
        clear_remembered_for_test();
        let resolved = resolve_excludable_from(
            Some(identity("org.telegram.desktop", "Telegram")),
            Some("com.apple.Notes".into()),
        );
        let (app, source) = resolved.expect("candidate");
        assert_eq!(app.bundle_id, "org.telegram.desktop");
        assert_eq!(app.display_name, "Telegram");
        assert_eq!(source, ExcludableAppSource::Frontmost);
    }

    #[test]
    fn ignores_copyosity_bundle_id() {
        clear_remembered_for_test();
        remember_app_identity(&identity(macos_app::COPYOSITY_BUNDLE_ID, "Copyosity"));
        assert!(remembered_bundle_id().is_none());
    }

    #[test]
    fn no_candidate_when_frontmost_is_copyosity_and_nothing_remembered() {
        clear_remembered_for_test();
        assert!(resolve_excludable_from(
            Some(identity(macos_app::COPYOSITY_BUNDLE_ID, "Copyosity")),
            None,
        )
        .is_none());
    }

    #[test]
    fn remembered_used_when_frontmost_unavailable() {
        clear_remembered_for_test();
        let resolved = resolve_excludable_from(None, Some("com.apple.Notes".into()));
        let (app, source) = resolved.expect("candidate");
        assert_eq!(app.bundle_id, "com.apple.Notes");
        assert_eq!(source, ExcludableAppSource::Remembered);
    }

    #[test]
    fn copyosity_frontmost_with_remembered_still_resolves() {
        clear_remembered_for_test();
        let resolved = resolve_excludable_from(
            Some(identity(macos_app::COPYOSITY_BUNDLE_ID, "Copyosity")),
            Some("com.apple.Safari".into()),
        );
        let (app, source) = resolved.expect("candidate");
        assert_eq!(app.bundle_id, "com.apple.Safari");
        assert_eq!(source, ExcludableAppSource::Remembered);
    }
}
