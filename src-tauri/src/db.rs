use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub ollama_model: String,
    pub retention_days: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelOption {
    pub value: String,
    pub label: String,
    pub memory_gb: f64,
    pub fits: bool,
    pub installed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelCatalog {
    pub total_memory_gb: f64,
    pub recommended_memory_gb: f64,
    pub options: Vec<ModelOption>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClipboardEntry {
    pub id: i64,
    pub content_type: String, // "text", "image", "file"
    pub text_content: Option<String>,
    pub image_data: Option<String>, // base64-encoded
    pub image_thumb: Option<String>, // base64-encoded thumbnail
    pub source_app: Option<String>,
    pub source_app_icon: Option<String>, // base64-encoded
    pub content_hash: String,
    pub char_count: Option<i64>,
    pub created_at: String,
    pub is_pinned: bool,
    pub collection_id: Option<i64>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExcludedApp {
    pub id: i64,
    pub bundle_id: String,
}

pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    pub fn new(app_dir: PathBuf) -> Result<Self, rusqlite::Error> {
        std::fs::create_dir_all(&app_dir).ok();
        let db_path = app_dir.join("copyosity.db");
        let conn = Connection::open(db_path)?;

        conn.execute_batch("
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;
        ")?;

        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS collections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                color TEXT,
                sort_order INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS clipboard_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL DEFAULT 'text',
                text_content TEXT,
                image_data BLOB,
                image_thumb BLOB,
                source_app TEXT,
                source_app_icon BLOB,
                content_hash TEXT NOT NULL,
                char_count INTEGER,
                created_at TEXT NOT NULL,
                is_pinned INTEGER DEFAULT 0,
                collection_id INTEGER REFERENCES collections(id) ON DELETE SET NULL
            );

            CREATE INDEX IF NOT EXISTS idx_entries_created_at ON clipboard_entries(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_entries_content_hash ON clipboard_entries(content_hash);
            CREATE INDEX IF NOT EXISTS idx_entries_collection ON clipboard_entries(collection_id);

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS clipboard_tags (
                entry_id INTEGER NOT NULL REFERENCES clipboard_entries(id) ON DELETE CASCADE,
                tag TEXT NOT NULL,
                PRIMARY KEY (entry_id, tag)
            );

            CREATE TABLE IF NOT EXISTS clipboard_tag_state (
                entry_id INTEGER PRIMARY KEY REFERENCES clipboard_entries(id) ON DELETE CASCADE,
                status TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS excluded_apps (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bundle_id TEXT NOT NULL UNIQUE
            );

            CREATE INDEX IF NOT EXISTS idx_clipboard_tags_entry ON clipboard_tags(entry_id);
            CREATE INDEX IF NOT EXISTS idx_clipboard_tags_tag ON clipboard_tags(tag);
            CREATE INDEX IF NOT EXISTS idx_clipboard_tag_state_status ON clipboard_tag_state(status);
        ")?;

        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .map(Some)
        .or_else(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            _ => Err(err),
        })
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_app_settings(&self) -> Result<AppSettings, rusqlite::Error> {
        let ollama_model = self
            .get_setting("ollama_model")?
            .unwrap_or_else(|| "qwen3:4b-instruct-2507-q4_K_M".to_string());
        let retention_days = self
            .get_setting("retention_days")?
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|days| matches!(*days, 1 | 7 | 30 | 180))
            .unwrap_or(30);

        Ok(AppSettings {
            ollama_model,
            retention_days,
        })
    }

    pub fn update_app_settings(
        &self,
        ollama_model: Option<&str>,
        retention_days: Option<i64>,
    ) -> Result<AppSettings, rusqlite::Error> {
        if let Some(model) = ollama_model {
            self.set_setting("ollama_model", model.trim())?;
        }

        if let Some(days) = retention_days {
            self.set_setting("retention_days", &days.to_string())?;
        }

        self.get_app_settings()
    }

    /// Returns (id, is_new). When is_new is false, the entry already existed (duplicate hash).
    pub fn insert_entry(&self, entry: &ClipboardEntry) -> Result<(i64, bool), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        // Check for duplicate by hash
        let existing: Option<i64> = conn.query_row(
            "SELECT id FROM clipboard_entries WHERE content_hash = ?1 ORDER BY created_at DESC LIMIT 1",
            params![entry.content_hash],
            |row| row.get(0),
        ).ok();

        if let Some(id) = existing {
            return Ok((id, false));
        }

        conn.execute(
            "INSERT INTO clipboard_entries (content_type, text_content, image_data, image_thumb, source_app, source_app_icon, content_hash, char_count, created_at, is_pinned, collection_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                entry.content_type,
                entry.text_content,
                entry.image_data,
                entry.image_thumb,
                entry.source_app,
                entry.source_app_icon,
                entry.content_hash,
                entry.char_count,
                entry.created_at,
                entry.is_pinned as i32,
                entry.collection_id,
            ],
        )?;

        Ok((conn.last_insert_rowid(), true))
    }

    pub fn get_entries(&self, limit: i64, offset: i64, collection_id: Option<i64>, pinned_only: bool, search: Option<&str>) -> Result<Vec<ClipboardEntry>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT id, content_type, text_content, NULL as image_data, COALESCE(image_thumb, image_data) as image_thumb, source_app, NULL as source_app_icon, content_hash, char_count, created_at, is_pinned, collection_id,
             COALESCE((SELECT GROUP_CONCAT(tag, '|') FROM clipboard_tags WHERE entry_id = clipboard_entries.id), '') as tags
             FROM clipboard_entries WHERE 1=1"
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(cid) = collection_id {
            sql.push_str(" AND collection_id = ?");
            param_values.push(Box::new(cid));
        }

        if pinned_only {
            sql.push_str(" AND is_pinned = 1");
        }

        if let Some(q) = search {
            sql.push_str(" AND text_content LIKE ?");
            param_values.push(Box::new(format!("%{}%", q)));
        }

        sql.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");
        param_values.push(Box::new(limit));
        param_values.push(Box::new(offset));

        let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let entries = stmt.query_map(params_ref.as_slice(), |row| {
            Ok(ClipboardEntry {
                id: row.get(0)?,
                content_type: row.get(1)?,
                text_content: row.get(2)?,
                image_data: row.get(3)?,
                image_thumb: row.get(4)?,
                source_app: row.get(5)?,
                source_app_icon: row.get(6)?,
                content_hash: row.get(7)?,
                char_count: row.get(8)?,
                created_at: row.get(9)?,
                is_pinned: row.get::<_, i32>(10)? != 0,
                collection_id: row.get(11)?,
                tags: row
                    .get::<_, String>(12)?
                    .split('|')
                    .filter(|tag| !tag.is_empty())
                    .map(|tag| tag.to_string())
                    .collect(),
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    pub fn delete_entry(&self, id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM clipboard_entries WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn pin_entry(&self, id: i64, pinned: bool) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE clipboard_entries SET is_pinned = ?1 WHERE id = ?2",
            params![pinned as i32, id],
        )?;
        Ok(())
    }

    pub fn set_collection(&self, entry_id: i64, collection_id: Option<i64>) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE clipboard_entries SET collection_id = ?1 WHERE id = ?2",
            params![collection_id, entry_id],
        )?;
        Ok(())
    }

    // Collections CRUD
    pub fn get_collections(&self) -> Result<Vec<Collection>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, color, sort_order FROM collections ORDER BY sort_order")?;
        let cols = stmt.query_map([], |row| {
            Ok(Collection {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                sort_order: row.get(3)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(cols)
    }

    pub fn create_collection(&self, name: &str, color: Option<&str>) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO collections (name, color) VALUES (?1, ?2)",
            params![name, color],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn delete_collection(&self, id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM collections WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear_history(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM clipboard_entries WHERE is_pinned = 0", [])?;
        Ok(())
    }

    pub fn get_excluded_apps(&self) -> Result<Vec<ExcludedApp>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, bundle_id FROM excluded_apps ORDER BY bundle_id COLLATE NOCASE")?;
        let apps = stmt
            .query_map([], |row| {
                Ok(ExcludedApp {
                    id: row.get(0)?,
                    bundle_id: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(apps)
    }

    pub fn add_excluded_app(&self, bundle_id: &str) -> Result<(), rusqlite::Error> {
        let normalized = bundle_id.trim();
        if normalized.is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO excluded_apps (bundle_id) VALUES (?1)",
            params![normalized],
        )?;
        Ok(())
    }

    pub fn remove_excluded_app(&self, id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM excluded_apps WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn is_app_excluded(&self, bundle_id: &str) -> Result<bool, rusqlite::Error> {
        let normalized = bundle_id.trim();
        if normalized.is_empty() {
            return Ok(false);
        }

        let conn = self.conn.lock().unwrap();
        let exists: Option<i64> = conn
            .query_row(
                "SELECT id FROM excluded_apps WHERE bundle_id = ?1 LIMIT 1",
                params![normalized],
                |row| row.get(0),
            )
            .ok();
        Ok(exists.is_some())
    }

    pub fn set_entry_tags(&self, entry_id: i64, tags: &[String]) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM clipboard_tags WHERE entry_id = ?1", params![entry_id])?;

        for tag in tags {
            tx.execute(
                "INSERT OR IGNORE INTO clipboard_tags (entry_id, tag) VALUES (?1, ?2)",
                params![entry_id, tag],
            )?;
        }

        tx.execute(
            "INSERT INTO clipboard_tag_state (entry_id, status) VALUES (?1, 'done')
             ON CONFLICT(entry_id) DO UPDATE SET status = excluded.status",
            params![entry_id],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn set_entry_tag_state(&self, entry_id: i64, status: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO clipboard_tag_state (entry_id, status) VALUES (?1, ?2)
             ON CONFLICT(entry_id) DO UPDATE SET status = excluded.status",
            params![entry_id, status],
        )?;
        Ok(())
    }

    pub fn get_untagged_text_entries(
        &self,
        limit: i64,
    ) -> Result<Vec<(i64, String)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT clipboard_entries.id, clipboard_entries.text_content
             FROM clipboard_entries
             LEFT JOIN clipboard_tags ON clipboard_tags.entry_id = clipboard_entries.id
             LEFT JOIN clipboard_tag_state ON clipboard_tag_state.entry_id = clipboard_entries.id
             WHERE clipboard_entries.content_type = 'text'
               AND clipboard_entries.text_content IS NOT NULL
               AND TRIM(clipboard_entries.text_content) != ''
               AND clipboard_tags.entry_id IS NULL
               AND clipboard_tag_state.entry_id IS NULL
             ORDER BY clipboard_entries.created_at DESC
             LIMIT ?1",
        )?;

        let entries = stmt
            .query_map(params![limit], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    pub fn get_text_entries_for_retag(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<(i64, String, Vec<String>)>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT clipboard_entries.id,
                    clipboard_entries.text_content,
                    COALESCE((SELECT GROUP_CONCAT(tag, '|')
                              FROM clipboard_tags
                              WHERE entry_id = clipboard_entries.id), '') AS tags
             FROM clipboard_entries
             WHERE clipboard_entries.content_type = 'text'
               AND clipboard_entries.text_content IS NOT NULL
               AND TRIM(clipboard_entries.text_content) != ''
             ORDER BY clipboard_entries.created_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;

        let entries = stmt
            .query_map(params![limit, offset], |row| {
                let tags = row.get::<_, String>(2)?;
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    tags.split('|')
                        .filter(|tag| !tag.is_empty())
                        .map(|tag| tag.to_string())
                        .collect(),
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    pub fn get_entry_text(&self, entry_id: i64) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT text_content
             FROM clipboard_entries
             WHERE id = ?1
               AND content_type = 'text'
               AND text_content IS NOT NULL",
            params![entry_id],
            |row| row.get(0),
        )
        .map(Some)
        .or_else(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            _ => Err(err),
        })
    }

    pub fn get_entry_by_id(&self, entry_id: i64) -> Result<Option<ClipboardEntry>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, content_type, text_content, image_data, image_thumb, source_app, source_app_icon,
                    content_hash, char_count, created_at, is_pinned, collection_id,
                    COALESCE((SELECT GROUP_CONCAT(tag, '|') FROM clipboard_tags WHERE entry_id = clipboard_entries.id), '') as tags
             FROM clipboard_entries
             WHERE id = ?1",
            params![entry_id],
            |row| {
                Ok(ClipboardEntry {
                    id: row.get(0)?,
                    content_type: row.get(1)?,
                    text_content: row.get(2)?,
                    image_data: row.get(3)?,
                    image_thumb: row.get(4)?,
                    source_app: row.get(5)?,
                    source_app_icon: row.get(6)?,
                    content_hash: row.get(7)?,
                    char_count: row.get(8)?,
                    created_at: row.get(9)?,
                    is_pinned: row.get::<_, i32>(10)? != 0,
                    collection_id: row.get(11)?,
                    tags: row
                        .get::<_, String>(12)?
                        .split('|')
                        .filter(|tag| !tag.is_empty())
                        .map(|tag| tag.to_string())
                        .collect(),
                })
            },
        )
        .map(Some)
        .or_else(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            _ => Err(err),
        })
    }

    #[allow(dead_code)]
    pub fn cleanup_old_entries(&self, max_age_days: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM clipboard_entries WHERE is_pinned = 0 AND created_at < datetime('now', ?1)",
            params![format!("-{} days", max_age_days)],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        // In-memory database for tests
        let db = Database {
            conn: Mutex::new(Connection::open_in_memory().unwrap()),
        };
        db.conn.lock().unwrap().execute_batch("
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS collections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                color TEXT,
                sort_order INTEGER DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS clipboard_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL DEFAULT 'text',
                text_content TEXT,
                image_data BLOB,
                image_thumb BLOB,
                source_app TEXT,
                source_app_icon BLOB,
                content_hash TEXT NOT NULL,
                char_count INTEGER,
                created_at TEXT NOT NULL,
                is_pinned INTEGER DEFAULT 0,
                collection_id INTEGER REFERENCES collections(id) ON DELETE SET NULL
            );
            CREATE INDEX IF NOT EXISTS idx_entries_content_hash ON clipboard_entries(content_hash);
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS clipboard_tags (
                entry_id INTEGER NOT NULL REFERENCES clipboard_entries(id) ON DELETE CASCADE,
                tag TEXT NOT NULL,
                PRIMARY KEY (entry_id, tag)
            );
            CREATE TABLE IF NOT EXISTS clipboard_tag_state (
                entry_id INTEGER PRIMARY KEY REFERENCES clipboard_entries(id) ON DELETE CASCADE,
                status TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS excluded_apps (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bundle_id TEXT NOT NULL UNIQUE
            );
        ").unwrap();
        db
    }

    fn make_entry(text: &str, hash: &str) -> ClipboardEntry {
        ClipboardEntry {
            id: 0,
            content_type: "text".to_string(),
            text_content: Some(text.to_string()),
            image_data: None,
            image_thumb: None,
            source_app: Some("TestApp".to_string()),
            source_app_icon: None,
            content_hash: hash.to_string(),
            char_count: Some(text.len() as i64),
            created_at: chrono::Utc::now().to_rfc3339(),
            is_pinned: false,
            collection_id: None,
            tags: Vec::new(),
        }
    }

    // --- Insert & Dedup ---

    #[test]
    fn insert_entry_returns_new() {
        let db = test_db();
        let entry = make_entry("hello", "hash1");
        let (id, is_new) = db.insert_entry(&entry).unwrap();
        assert!(id > 0);
        assert!(is_new);
    }

    #[test]
    fn insert_duplicate_hash_returns_existing() {
        let db = test_db();
        let e1 = make_entry("hello", "hash_dup");
        let (id1, new1) = db.insert_entry(&e1).unwrap();
        assert!(new1);

        let e2 = make_entry("hello again", "hash_dup");
        let (id2, new2) = db.insert_entry(&e2).unwrap();
        assert!(!new2);
        assert_eq!(id1, id2);
    }

    #[test]
    fn insert_different_hashes_creates_separate() {
        let db = test_db();
        let (id1, _) = db.insert_entry(&make_entry("a", "h1")).unwrap();
        let (id2, _) = db.insert_entry(&make_entry("b", "h2")).unwrap();
        assert_ne!(id1, id2);
    }

    // --- Get entries ---

    #[test]
    fn get_entries_respects_limit() {
        let db = test_db();
        for i in 0..10 {
            db.insert_entry(&make_entry(&format!("text {}", i), &format!("h{}", i))).unwrap();
        }
        let entries = db.get_entries(3, 0, None, false, None).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn get_entries_with_search() {
        let db = test_db();
        db.insert_entry(&make_entry("rust programming", "h1")).unwrap();
        db.insert_entry(&make_entry("python script", "h2")).unwrap();
        db.insert_entry(&make_entry("rust cargo", "h3")).unwrap();

        let results = db.get_entries(50, 0, None, false, Some("rust")).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn get_entries_pinned_only() {
        let db = test_db();
        let (id1, _) = db.insert_entry(&make_entry("pinned", "h1")).unwrap();
        db.insert_entry(&make_entry("not pinned", "h2")).unwrap();
        db.pin_entry(id1, true).unwrap();

        let pinned = db.get_entries(50, 0, None, true, None).unwrap();
        assert_eq!(pinned.len(), 1);
        assert_eq!(pinned[0].text_content.as_deref(), Some("pinned"));
    }

    // --- Pin / Delete ---

    #[test]
    fn pin_and_unpin_entry() {
        let db = test_db();
        let (id, _) = db.insert_entry(&make_entry("test", "h1")).unwrap();

        db.pin_entry(id, true).unwrap();
        let e = db.get_entry_by_id(id).unwrap().unwrap();
        assert!(e.is_pinned);

        db.pin_entry(id, false).unwrap();
        let e = db.get_entry_by_id(id).unwrap().unwrap();
        assert!(!e.is_pinned);
    }

    #[test]
    fn delete_entry_removes_it() {
        let db = test_db();
        let (id, _) = db.insert_entry(&make_entry("to delete", "h1")).unwrap();
        db.delete_entry(id).unwrap();
        assert!(db.get_entry_by_id(id).unwrap().is_none());
    }

    #[test]
    fn clear_history_keeps_pinned() {
        let db = test_db();
        let (id1, _) = db.insert_entry(&make_entry("pinned", "h1")).unwrap();
        db.insert_entry(&make_entry("not pinned", "h2")).unwrap();
        db.pin_entry(id1, true).unwrap();

        db.clear_history().unwrap();
        let all = db.get_entries(50, 0, None, false, None).unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].is_pinned);
    }

    // --- Tags ---

    #[test]
    fn set_and_get_tags() {
        let db = test_db();
        let (id, _) = db.insert_entry(&make_entry("tagged text", "h1")).unwrap();
        db.set_entry_tags(id, &["rust".to_string(), "code".to_string()]).unwrap();

        let entry = db.get_entry_by_id(id).unwrap().unwrap();
        assert!(entry.tags.contains(&"rust".to_string()));
        assert!(entry.tags.contains(&"code".to_string()));
    }

    #[test]
    fn overwrite_tags() {
        let db = test_db();
        let (id, _) = db.insert_entry(&make_entry("text", "h1")).unwrap();
        db.set_entry_tags(id, &["old".to_string()]).unwrap();
        db.set_entry_tags(id, &["new".to_string()]).unwrap();

        let entry = db.get_entry_by_id(id).unwrap().unwrap();
        assert_eq!(entry.tags, vec!["new".to_string()]);
    }

    #[test]
    fn untagged_entries_returned() {
        let db = test_db();
        db.insert_entry(&make_entry("no tags", "h1")).unwrap();
        let (id2, _) = db.insert_entry(&make_entry("has tags", "h2")).unwrap();
        db.set_entry_tags(id2, &["tagged".to_string()]).unwrap();

        let untagged = db.get_untagged_text_entries(50).unwrap();
        assert_eq!(untagged.len(), 1);
        assert_eq!(untagged[0].1, "no tags");
    }

    // --- Settings ---

    #[test]
    fn default_settings() {
        let db = test_db();
        let s = db.get_app_settings().unwrap();
        assert_eq!(s.ollama_model, "qwen3:4b-instruct-2507-q4_K_M");
        assert_eq!(s.retention_days, 30);
    }

    #[test]
    fn update_settings() {
        let db = test_db();
        db.update_app_settings(Some("custom-model"), Some(7)).unwrap();
        let s = db.get_app_settings().unwrap();
        assert_eq!(s.ollama_model, "custom-model");
        assert_eq!(s.retention_days, 7);
    }

    #[test]
    fn invalid_retention_falls_back() {
        let db = test_db();
        db.set_setting("retention_days", "999").unwrap();
        let s = db.get_app_settings().unwrap();
        assert_eq!(s.retention_days, 30); // fallback
    }

    // --- Collections ---

    #[test]
    fn create_and_get_collections() {
        let db = test_db();
        let _id = db.create_collection("Work", Some("#ff0000")).unwrap();
        let cols = db.get_collections().unwrap();
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].name, "Work");
        assert_eq!(cols[0].color, Some("#ff0000".to_string()));
    }

    #[test]
    fn delete_collection_nullifies_entries() {
        let db = test_db();
        let col_id = db.create_collection("Temp", None).unwrap();
        let (entry_id, _) = db.insert_entry(&make_entry("in collection", "h1")).unwrap();
        db.set_collection(entry_id, Some(col_id)).unwrap();

        db.delete_collection(col_id).unwrap();
        let entry = db.get_entry_by_id(entry_id).unwrap().unwrap();
        assert!(entry.collection_id.is_none());
    }

    // --- Excluded apps ---

    #[test]
    fn exclude_and_check_app() {
        let db = test_db();
        assert!(!db.is_app_excluded("Telegram").unwrap());

        db.add_excluded_app("Telegram").unwrap();
        assert!(db.is_app_excluded("Telegram").unwrap());

        let apps = db.get_excluded_apps().unwrap();
        assert_eq!(apps.len(), 1);

        db.remove_excluded_app(apps[0].id).unwrap();
        assert!(!db.is_app_excluded("Telegram").unwrap());
    }

    #[test]
    fn exclude_empty_app_is_noop() {
        let db = test_db();
        db.add_excluded_app("  ").unwrap();
        assert_eq!(db.get_excluded_apps().unwrap().len(), 0);
    }

    #[test]
    fn exclude_duplicate_app_is_noop() {
        let db = test_db();
        db.add_excluded_app("Safari").unwrap();
        db.add_excluded_app("Safari").unwrap();
        assert_eq!(db.get_excluded_apps().unwrap().len(), 1);
    }
}
