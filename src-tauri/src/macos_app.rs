use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const COPYOSITY_BUNDLE_ID: &str = "com.vkovalskii.copyosity";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIdentity {
    pub bundle_id: String,
    pub display_name: String,
}

pub fn is_copyosity_bundle(bundle_id: &str) -> bool {
    bundle_id == COPYOSITY_BUNDLE_ID
}

#[cfg(target_os = "macos")]
pub fn frontmost_app_identity() -> Option<AppIdentity> {
    use objc2_app_kit::NSWorkspace;

    let workspace = NSWorkspace::sharedWorkspace();
    let app = workspace.frontmostApplication()?;
    identity_from_running_app(&app)
}

#[cfg(not(target_os = "macos"))]
pub fn frontmost_app_identity() -> Option<AppIdentity> {
    None
}

/// Whether Copyosity is the frontmost application (clipboard capture must skip).
#[cfg(target_os = "macos")]
pub fn is_copyosity_frontmost() -> bool {
    use objc2_app_kit::NSWorkspace;

    let workspace = NSWorkspace::sharedWorkspace();
    workspace
        .frontmostApplication()
        .and_then(|app| app.bundleIdentifier().map(|s| s.to_string()))
        .is_some_and(|id| is_copyosity_bundle(&id))
}

#[cfg(not(target_os = "macos"))]
pub fn is_copyosity_frontmost() -> bool {
    false
}

#[cfg(target_os = "macos")]
pub fn app_identity_for_pid(pid: i32) -> Option<AppIdentity> {
    use objc2_app_kit::NSRunningApplication;

    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
    identity_from_running_app(&app)
}

#[cfg(not(target_os = "macos"))]
pub fn app_identity_for_pid(_pid: i32) -> Option<AppIdentity> {
    None
}

#[cfg(target_os = "macos")]
pub fn app_identity_from_app_bundle_path(path: &Path) -> Option<AppIdentity> {
    use objc2_foundation::{NSBundle, NSString};

    let path_str = path.to_str()?;
    let ns_path = NSString::from_str(path_str);
    let bundle = NSBundle::bundleWithPath(&ns_path)?;
    let bundle_id = bundle.bundleIdentifier()?.to_string();
    if bundle_id.is_empty() || is_copyosity_bundle(&bundle_id) {
        return None;
    }

    let display_name = display_name_from_running_bundle_id(&bundle_id)
        .or_else(|| display_name_from_bundle_plist(&bundle))
        .or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| humanize_bundle_id(&bundle_id));

    Some(AppIdentity {
        bundle_id,
        display_name,
    })
}

#[cfg(not(target_os = "macos"))]
pub fn app_identity_from_app_bundle_path(_path: &Path) -> Option<AppIdentity> {
    None
}

pub fn display_name_for_bundle_id(bundle_id: &str) -> String {
    let trimmed = bundle_id.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if !looks_like_bundle_id(trimmed) {
        return title_case_first_char(trimmed);
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(name) = display_name_from_running_bundle_id(trimmed) {
            return name;
        }
        if let Some(identity) = find_installed_app_by_bundle_id(trimmed) {
            return identity.display_name;
        }
    }

    humanize_bundle_id(trimmed)
}

/// Resolve display names for many bundle IDs with at most one filesystem scan (list views).
pub fn display_names_for_bundle_ids(bundle_ids: &[&str]) -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        let running = running_app_display_names();
        let installed = installed_app_display_names();
        bundle_ids
            .iter()
            .map(|bundle_id| {
                list_display_name_for_bundle_id(bundle_id, Some(&running), Some(&installed))
            })
            .collect()
    }
    #[cfg(not(target_os = "macos"))]
    {
        bundle_ids
            .iter()
            .map(|bundle_id| list_display_name_for_bundle_id(bundle_id, None, None))
            .collect()
    }
}

fn list_display_name_for_bundle_id(
    bundle_id: &str,
    running: Option<&HashMap<String, String>>,
    installed: Option<&HashMap<String, String>>,
) -> String {
    let trimmed = bundle_id.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if !looks_like_bundle_id(trimmed) {
        return title_case_first_char(trimmed);
    }

    if let Some(running) = running {
        if let Some(name) = running.get(trimmed) {
            return name.clone();
        }
    }
    if let Some(installed) = installed {
        if let Some(name) = installed.get(trimmed) {
            return name.clone();
        }
    }

    humanize_bundle_id(trimmed)
}

/// Resolve a user-entered app name or bundle ID to a stable bundle identifier.
pub fn resolve_app_identity_from_input(input: &str) -> Option<AppIdentity> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("copyosity") {
        return None;
    }

    if looks_like_bundle_id(trimmed) {
        if is_copyosity_bundle(trimmed) {
            return None;
        }
        return Some(AppIdentity {
            bundle_id: trimmed.to_owned(),
            display_name: display_name_for_bundle_id(trimmed),
        });
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(identity) = find_running_app_by_display_name(trimmed) {
            return Some(identity);
        }
        if let Some(identity) = find_installed_app_by_display_name(trimmed) {
            return Some(identity);
        }
    }

    None
}

fn is_unique_constraint_violation(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(code, _)
            if code.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    )
}

/// Update a legacy row to `bundle_id`, or delete it when that bundle ID is already excluded.
pub(crate) fn migrate_legacy_excluded_row(
    conn: &rusqlite::Connection,
    row_id: i64,
    bundle_id: &str,
) -> Result<(), rusqlite::Error> {
    match conn.execute(
        "UPDATE excluded_apps SET bundle_id = ?1 WHERE id = ?2",
        rusqlite::params![bundle_id, row_id],
    ) {
        Ok(_) => Ok(()),
        Err(err) if is_unique_constraint_violation(&err) => {
            // Target bundle_id already exists — drop the legacy duplicate row.
            conn.execute(
                "DELETE FROM excluded_apps WHERE id = ?1",
                rusqlite::params![row_id],
            )?;
            Ok(())
        }
        Err(err) => Err(err),
    }
}

/// Rows in `excluded_apps` that still store a display name instead of a bundle ID.
pub(crate) fn legacy_excluded_app_rows(
    conn: &rusqlite::Connection,
) -> Result<Vec<(i64, String)>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT id, bundle_id FROM excluded_apps")?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter(|(_, bundle_id)| !looks_like_bundle_id(bundle_id))
        .collect())
}

pub(crate) fn migrate_legacy_excluded_app_rows(
    conn: &rusqlite::Connection,
    legacy_rows: &[(i64, String)],
    mut resolve_bundle_id: impl FnMut(&str) -> Option<String>,
) -> Result<(), rusqlite::Error> {
    for (id, legacy_name) in legacy_rows {
        let Some(bundle_id) = resolve_bundle_id(legacy_name) else {
            eprintln!(
                "[migration] could not resolve legacy excluded app {legacy_name:?} (id {id})"
            );
            continue;
        };
        migrate_legacy_excluded_row(conn, *id, &bundle_id)?;
    }
    Ok(())
}

/// Upgrade legacy excluded-app rows that stored display names instead of bundle IDs.
#[cfg(target_os = "macos")]
pub fn migrate_legacy_excluded_app_names(
    conn: &rusqlite::Connection,
) -> Result<(), rusqlite::Error> {
    let legacy_rows = legacy_excluded_app_rows(conn)?;
    migrate_legacy_excluded_app_rows(conn, &legacy_rows, |legacy_name| {
        resolve_app_identity_from_input(legacy_name).map(|identity| identity.bundle_id)
    })
}

#[cfg(not(target_os = "macos"))]
pub fn migrate_legacy_excluded_app_names(
    _conn: &rusqlite::Connection,
) -> Result<(), rusqlite::Error> {
    Ok(())
}

fn looks_like_bundle_id(value: &str) -> bool {
    value.contains('.') && !value.contains('/')
}

/// Resolve a display-name lookup: one match succeeds, zero or many return `None`.
fn pick_unique_app_identity(mut matches: Vec<AppIdentity>) -> Option<AppIdentity> {
    match matches.len() {
        0 => None,
        1 => Some(matches.remove(0)),
        _ => None,
    }
}

const GENERIC_BUNDLE_SEGMENTS: &[&str] = &[
    "agent",
    "android",
    "app",
    "application",
    "client",
    "desktop",
    "extension",
    "helper",
    "ios",
    "mac",
    "macos",
    "osx",
    "plugin",
    "service",
];

fn is_generic_bundle_segment(segment: &str) -> bool {
    let lower = segment.to_ascii_lowercase();
    GENERIC_BUNDLE_SEGMENTS
        .iter()
        .any(|generic| lower == *generic)
}

fn meaningful_bundle_segment(bundle_id: &str) -> &str {
    let segments: Vec<&str> = bundle_id.split('.').collect();
    segments
        .iter()
        .rev()
        .find(|segment| !is_generic_bundle_segment(segment))
        .copied()
        .or_else(|| segments.last().copied())
        .unwrap_or(bundle_id)
}

fn humanize_bundle_id(bundle_id: &str) -> String {
    let segment = meaningful_bundle_segment(bundle_id);
    title_case_words(&segment.replace(['-', '_'], " "))
}

fn title_case_first_char(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn title_case_words(value: &str) -> String {
    value
        .split_whitespace()
        .map(title_case_first_char)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(target_os = "macos")]
fn identity_from_running_app(app: &objc2_app_kit::NSRunningApplication) -> Option<AppIdentity> {
    let bundle_id = app.bundleIdentifier()?.to_string();
    if bundle_id.is_empty() || is_copyosity_bundle(&bundle_id) {
        return None;
    }
    let display_name = app
        .localizedName()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| humanize_bundle_id(&bundle_id));
    Some(AppIdentity {
        bundle_id,
        display_name,
    })
}

#[cfg(target_os = "macos")]
fn display_name_from_bundle_plist(bundle: &objc2_foundation::NSBundle) -> Option<String> {
    display_name_from_plist_key(bundle, "CFBundleDisplayName")
        .or_else(|| display_name_from_plist_key(bundle, "CFBundleName"))
}

#[cfg(target_os = "macos")]
fn display_name_from_plist_key(bundle: &objc2_foundation::NSBundle, key: &str) -> Option<String> {
    use objc2_foundation::NSString;

    let key = NSString::from_str(key);
    let value = bundle.objectForInfoDictionaryKey(&key)?;
    value.downcast_ref::<NSString>().map(|s| s.to_string())
}

#[cfg(target_os = "macos")]
fn running_app_display_names() -> HashMap<String, String> {
    use objc2_app_kit::NSWorkspace;

    let workspace = NSWorkspace::sharedWorkspace();
    let mut names = HashMap::new();
    for app in workspace.runningApplications().iter() {
        let Some(identity) = identity_from_running_app(&app) else {
            continue;
        };
        names
            .entry(identity.bundle_id)
            .or_insert(identity.display_name);
    }
    names
}

#[cfg(target_os = "macos")]
const MAX_APP_SCAN_DEPTH: u8 = 4;

#[cfg(target_os = "macos")]
fn collect_app_bundle_paths(dir: &Path, depth: u8, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("app") {
            out.push(path);
        } else if path.is_dir() {
            collect_app_bundle_paths(&path, depth - 1, out);
        }
    }
}

#[cfg(target_os = "macos")]
fn installed_app_bundle_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for dir in application_directories() {
        collect_app_bundle_paths(&dir, MAX_APP_SCAN_DEPTH, &mut paths);
    }
    paths
}

#[cfg(target_os = "macos")]
fn installed_app_display_names() -> HashMap<String, String> {
    let mut names = HashMap::new();
    for path in installed_app_bundle_paths() {
        let Some(identity) = app_identity_from_app_bundle_path(&path) else {
            continue;
        };
        names
            .entry(identity.bundle_id)
            .or_insert(identity.display_name);
    }
    names
}

#[cfg(target_os = "macos")]
fn display_name_from_running_bundle_id(bundle_id: &str) -> Option<String> {
    use objc2_app_kit::NSWorkspace;

    let workspace = NSWorkspace::sharedWorkspace();
    for app in workspace.runningApplications().iter() {
        if app.bundleIdentifier().map(|s| s.to_string()).as_deref() == Some(bundle_id) {
            return app
                .localizedName()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn find_running_app_by_display_name(name: &str) -> Option<AppIdentity> {
    use objc2_app_kit::NSWorkspace;

    let workspace = NSWorkspace::sharedWorkspace();
    for app in workspace.runningApplications().iter() {
        let Some(identity) = identity_from_running_app(&app) else {
            continue;
        };
        if identity.display_name.eq_ignore_ascii_case(name) {
            return Some(identity);
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn find_installed_app_by_bundle_id(bundle_id: &str) -> Option<AppIdentity> {
    for path in installed_app_bundle_paths() {
        let Some(identity) = app_identity_from_app_bundle_path(&path) else {
            continue;
        };
        if identity.bundle_id == bundle_id {
            return Some(identity);
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn find_installed_app_by_display_name(name: &str) -> Option<AppIdentity> {
    let mut matches = Vec::new();
    for path in installed_app_bundle_paths() {
        let Some(identity) = app_identity_from_app_bundle_path(&path) else {
            continue;
        };
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if identity.display_name.eq_ignore_ascii_case(name) || stem.eq_ignore_ascii_case(name) {
            matches.push(identity);
        }
    }
    pick_unique_app_identity(matches)
}

#[cfg(target_os = "macos")]
fn application_directories() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/Applications")];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join("Applications"));
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_unique_app_identity_rejects_ambiguous_display_name() {
        let notes = |bundle_id: &str| AppIdentity {
            bundle_id: bundle_id.into(),
            display_name: "Notes".into(),
        };

        assert!(pick_unique_app_identity(vec![]).is_none());
        assert_eq!(
            pick_unique_app_identity(vec![notes("com.apple.Notes")])
                .expect("single match")
                .bundle_id,
            "com.apple.Notes"
        );
        assert!(pick_unique_app_identity(vec![
            notes("com.example.notes-a"),
            notes("com.example.notes-b")
        ])
        .is_none());
    }

    #[test]
    fn recognizes_bundle_id_shape() {
        assert!(looks_like_bundle_id("com.apple.Safari"));
        assert!(!looks_like_bundle_id("Safari"));
        assert!(!looks_like_bundle_id("/Applications/Safari.app"));
    }

    #[test]
    fn resolves_bundle_id_input_to_humanized_name_when_not_installed() {
        let bundle_id = "com.copyosity.tests.uninstalled-app";
        let identity = resolve_app_identity_from_input(bundle_id).expect("bundle id");
        assert_eq!(identity.bundle_id, bundle_id);
        assert_eq!(identity.display_name, "Uninstalled App");
    }

    #[test]
    fn ignores_copyosity_bundle_id_input() {
        assert!(resolve_app_identity_from_input(COPYOSITY_BUNDLE_ID).is_none());
        assert!(resolve_app_identity_from_input("Copyosity").is_none());
    }

    #[test]
    fn humanizes_bundle_id_meaningful_segment() {
        assert_eq!(humanize_bundle_id("com.electron.asana"), "Asana");
        assert_eq!(humanize_bundle_id("com.apple.Safari"), "Safari");
        assert_eq!(humanize_bundle_id("org.telegram.desktop"), "Telegram");
        assert_eq!(humanize_bundle_id("com.example.myapp.client"), "Myapp");
    }

    #[test]
    fn display_name_for_plain_name_title_cases() {
        assert_eq!(display_name_for_bundle_id("cursor"), "Cursor");
    }

    #[test]
    fn batch_display_names_humanize_without_filesystem_lookup() {
        let names = display_names_for_bundle_ids(&["com.apple.Safari", "Safari", ""]);
        assert_eq!(names, vec!["Safari", "Safari", ""]);
    }

    #[test]
    fn recognizes_unique_constraint_violation() {
        let unique = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE),
            Some("UNIQUE constraint failed".into()),
        );
        assert!(is_unique_constraint_violation(&unique));

        let locked = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
            Some("database is locked".into()),
        );
        assert!(!is_unique_constraint_violation(&locked));
    }

    fn excluded_apps_fixture() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE excluded_apps (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bundle_id TEXT NOT NULL UNIQUE
            )",
        )
        .unwrap();
        conn
    }

    #[test]
    fn legacy_migration_skips_bundle_shaped_rows() {
        let conn = excluded_apps_fixture();
        conn.execute(
            "INSERT INTO excluded_apps (bundle_id) VALUES ('com.apple.Safari')",
            [],
        )
        .unwrap();

        assert!(legacy_excluded_app_rows(&conn).unwrap().is_empty());
    }

    #[test]
    fn migrate_legacy_keeps_unresolvable_rows() {
        let conn = excluded_apps_fixture();
        conn.execute(
            "INSERT INTO excluded_apps (bundle_id) VALUES ('Unknown Legacy App')",
            [],
        )
        .unwrap();
        let legacy_rows = legacy_excluded_app_rows(&conn).unwrap();
        assert_eq!(legacy_rows.len(), 1);

        migrate_legacy_excluded_app_rows(&conn, &legacy_rows, |_| None).unwrap();

        let bundle_id: String = conn
            .query_row("SELECT bundle_id FROM excluded_apps LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(bundle_id, "Unknown Legacy App");
    }

    #[test]
    fn collect_app_bundle_paths_finds_nested_apps() {
        let root =
            std::env::temp_dir().join(format!("copyosity-nested-apps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let nested = root.join("Utilities");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir(nested.join("Nested.app")).unwrap();

        let mut paths = Vec::new();
        collect_app_bundle_paths(&root, MAX_APP_SCAN_DEPTH, &mut paths);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("Nested.app"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn migrate_legacy_excluded_app_names_drops_duplicate_row() {
        let conn = excluded_apps_fixture();
        conn.execute(
            "INSERT INTO excluded_apps (bundle_id) VALUES ('org.telegram.desktop')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO excluded_apps (bundle_id) VALUES ('Telegram')",
            [],
        )
        .unwrap();
        let legacy_rows = legacy_excluded_app_rows(&conn).unwrap();
        assert_eq!(legacy_rows.len(), 1);
        assert_eq!(legacy_rows[0].1, "Telegram");

        migrate_legacy_excluded_app_rows(&conn, &legacy_rows, |legacy_name| {
            (legacy_name == "Telegram").then_some("org.telegram.desktop".into())
        })
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM excluded_apps", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let bundle_id: String = conn
            .query_row("SELECT bundle_id FROM excluded_apps LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(bundle_id, "org.telegram.desktop");
    }
}
