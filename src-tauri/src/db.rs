use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i64,
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

            CREATE TABLE IF NOT EXISTS excluded_apps (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bundle_id TEXT NOT NULL UNIQUE
            );
        ")?;

        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn insert_entry(&self, entry: &ClipboardEntry) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        // Check for duplicate by hash
        let existing: Option<i64> = conn.query_row(
            "SELECT id FROM clipboard_entries WHERE content_hash = ?1 ORDER BY created_at DESC LIMIT 1",
            params![entry.content_hash],
            |row| row.get(0),
        ).ok();

        if let Some(id) = existing {
            // Move existing entry to top by updating timestamp
            conn.execute(
                "UPDATE clipboard_entries SET created_at = ?1 WHERE id = ?2",
                params![entry.created_at, id],
            )?;
            return Ok(id);
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

        Ok(conn.last_insert_rowid())
    }

    pub fn get_entries(&self, limit: i64, offset: i64, collection_id: Option<i64>, pinned_only: bool, search: Option<&str>) -> Result<Vec<ClipboardEntry>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT id, content_type, text_content, NULL as image_data, COALESCE(image_thumb, image_data) as image_thumb, source_app, NULL as source_app_icon, content_hash, char_count, created_at, is_pinned, collection_id
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
