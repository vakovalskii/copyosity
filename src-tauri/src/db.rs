use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub ollama_model: String,
    pub retention_days: i64,
    pub whisper_server_url: String,
    pub whisper_server_token: String,
    pub whisper_server_model: String,
    /// e.g. "option+space", "cmd+space", "ctrl+alt+space"
    pub voice_shortcut: String,
    /// Selected microphone device name (empty = default)
    pub selected_microphone: String,
    /// When false, voice shortcut is not registered (default off).
    pub voice_transcription_enabled: bool,
    /// When false, clipboard entries are not auto-tagged (default off).
    pub ai_tagging_enabled: bool,
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
    pub image_data: Option<String>,  // base64-encoded
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
    /// Display format for image entries: GIF, PNG, JPG.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_width: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_height: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_byte_size: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExcludedApp {
    pub id: i64,
    pub bundle_id: String,
    pub display_name: String,
}

pub struct Database {
    pub conn: Mutex<Connection>,
}

fn lowercase_search_text(text: &str) -> String {
    text.to_lowercase()
}

fn backfill_text_content_search(conn: &Connection) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, text_content
         FROM clipboard_entries
         WHERE text_content_search IS NULL
           AND text_content IS NOT NULL",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if rows.is_empty() {
        return Ok(());
    }

    let tx = conn.unchecked_transaction()?;
    for (id, text) in rows {
        tx.execute(
            "UPDATE clipboard_entries SET text_content_search = ?1 WHERE id = ?2",
            params![lowercase_search_text(&text), id],
        )?;
    }
    tx.commit()
}

fn resolve_image_format(entry: &mut ClipboardEntry) {
    if entry.content_type != "image" {
        return;
    }

    if let Some(ref fmt) = entry.image_format {
        let normalized = crate::image_format::normalize(fmt).to_owned();
        if normalized != *fmt {
            entry.image_format = Some(normalized);
        }
        return;
    }

    let b64 = entry.image_data.as_deref().or(entry.image_thumb.as_deref());
    if let Some(b64) = b64 {
        entry.image_format = Some(crate::image_format::detect_from_b64(b64).to_owned());
    }
}

fn decode_image_dimensions(b64: &str) -> Option<(i64, i64)> {
    use base64::Engine;
    use image::GenericImageView;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    let image = image::load_from_memory(&bytes).ok()?;
    let (width, height) = image.dimensions();
    Some((width as i64, height as i64))
}

fn resolve_image_meta(entry: &mut ClipboardEntry) {
    if entry.content_type != "image" {
        return;
    }

    if entry.image_width.is_some()
        && entry.image_height.is_some()
        && entry.image_byte_size.is_some()
    {
        return;
    }

    let b64 = entry.image_data.as_deref().or(entry.image_thumb.as_deref());
    let Some(b64) = b64 else {
        return;
    };

    if entry.image_width.is_none() || entry.image_height.is_none() {
        if let Some((width, height)) = decode_image_dimensions(b64) {
            entry.image_width = Some(width);
            entry.image_height = Some(height);
        }
    }

    if entry.image_byte_size.is_none() {
        use base64::Engine;
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
            entry.image_byte_size = Some(bytes.len() as i64);
        }
    }
}

impl Database {
    pub fn new(app_dir: PathBuf) -> Result<Self, rusqlite::Error> {
        std::fs::create_dir_all(&app_dir).ok();
        let db_path = app_dir.join("copyosity.db");
        let conn = Connection::open(db_path)?;

        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;
        ",
        )?;

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

        let _ = conn.execute(
            "ALTER TABLE clipboard_entries ADD COLUMN image_format TEXT",
            [],
        );

        let _ = conn.execute(
            "UPDATE clipboard_entries SET image_format = 'JPG' WHERE image_format = 'JPEG'",
            [],
        );
        let _ = conn.execute(
            "UPDATE clipboard_tags SET tag = 'jpg' WHERE tag = 'jpeg'",
            [],
        );

        let _ = conn.execute(
            "ALTER TABLE clipboard_entries ADD COLUMN text_content_search TEXT",
            [],
        );
        backfill_text_content_search(&conn)?;

        let _ = conn.execute(
            "ALTER TABLE clipboard_entries ADD COLUMN image_width INTEGER",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE clipboard_entries ADD COLUMN image_height INTEGER",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE clipboard_entries ADD COLUMN image_byte_size INTEGER",
            [],
        );

        crate::macos_app::migrate_legacy_excluded_app_names(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
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

    pub fn is_ai_tagging_enabled(&self) -> bool {
        self.get_app_settings()
            .map(|s| s.ai_tagging_enabled)
            .unwrap_or(false)
    }

    pub fn get_app_settings(&self) -> Result<AppSettings, rusqlite::Error> {
        let ollama_model = self
            .get_setting("ollama_model")?
            .unwrap_or_else(|| "qwen3:4b-instruct-2507-q4_K_M".to_owned());
        let retention_days = self
            .get_setting("retention_days")?
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|days| matches!(*days, 1 | 7 | 30 | 180))
            .unwrap_or(30);

        let whisper_server_url = self.get_setting("whisper_server_url")?.unwrap_or_default();
        let whisper_server_token = self
            .get_setting("whisper_server_token")?
            .unwrap_or_default();
        let whisper_server_model = self
            .get_setting("whisper_server_model")?
            .unwrap_or_else(|| "whisper-1".to_owned());

        let voice_shortcut = self
            .get_setting("voice_shortcut")?
            .unwrap_or_else(|| "option+space".to_owned());
        let selected_microphone = self.get_setting("selected_microphone")?.unwrap_or_default();
        let voice_transcription_enabled = self
            .get_setting("voice_transcription_enabled")?
            .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
            .unwrap_or(false);
        let ai_tagging_enabled = self
            .get_setting("ai_tagging_enabled")?
            .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
            .unwrap_or(false);

        Ok(AppSettings {
            ollama_model,
            retention_days,
            whisper_server_url,
            whisper_server_token,
            whisper_server_model,
            voice_shortcut,
            selected_microphone,
            voice_transcription_enabled,
            ai_tagging_enabled,
        })
    }

    pub fn update_app_settings(
        &self,
        ollama_model: Option<&str>,
        retention_days: Option<i64>,
        whisper_server_url: Option<&str>,
        whisper_server_token: Option<&str>,
        whisper_server_model: Option<&str>,
        voice_shortcut: Option<&str>,
        selected_microphone: Option<&str>,
        voice_transcription_enabled: Option<bool>,
        ai_tagging_enabled: Option<bool>,
    ) -> Result<AppSettings, rusqlite::Error> {
        if let Some(model) = ollama_model {
            self.set_setting("ollama_model", model.trim())?;
        }
        if let Some(days) = retention_days {
            self.set_setting("retention_days", &days.to_string())?;
        }
        if let Some(url) = whisper_server_url {
            self.set_setting("whisper_server_url", url.trim())?;
        }
        if let Some(token) = whisper_server_token {
            self.set_setting("whisper_server_token", token.trim())?;
        }
        if let Some(model) = whisper_server_model {
            self.set_setting("whisper_server_model", model.trim())?;
        }
        if let Some(sc) = voice_shortcut {
            self.set_setting("voice_shortcut", sc.trim())?;
        }
        if let Some(mic) = selected_microphone {
            self.set_setting("selected_microphone", mic.trim())?;
        }
        if let Some(enabled) = voice_transcription_enabled {
            self.set_setting(
                "voice_transcription_enabled",
                if enabled { "true" } else { "false" },
            )?;
        }
        if let Some(enabled) = ai_tagging_enabled {
            self.set_setting("ai_tagging_enabled", if enabled { "true" } else { "false" })?;
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
            // Backfill image_data for entries created before full-size storage was added
            if entry.image_data.is_some() {
                conn.execute(
                    "UPDATE clipboard_entries SET image_data = ?1 WHERE id = ?2 AND image_data IS NULL",
                    params![entry.image_data, id],
                )?;
            }
            if entry.content_type == "image" {
                if let Some(ref fmt) = entry.image_format {
                    let normalized = crate::image_format::normalize(fmt).to_owned();
                    conn.execute(
                        "UPDATE clipboard_entries SET image_format = ?1 WHERE id = ?2 AND image_format IS NULL",
                        params![normalized, id],
                    )?;
                    conn.execute(
                        "UPDATE clipboard_entries SET image_format = 'JPG' WHERE id = ?1 AND image_format = 'JPEG'",
                        params![id],
                    )?;
                }
                if entry.image_width.is_some() {
                    conn.execute(
                        "UPDATE clipboard_entries SET image_width = ?1 WHERE id = ?2 AND image_width IS NULL",
                        params![entry.image_width, id],
                    )?;
                }
                if entry.image_height.is_some() {
                    conn.execute(
                        "UPDATE clipboard_entries SET image_height = ?1 WHERE id = ?2 AND image_height IS NULL",
                        params![entry.image_height, id],
                    )?;
                }
                if entry.image_byte_size.is_some() {
                    conn.execute(
                        "UPDATE clipboard_entries SET image_byte_size = ?1 WHERE id = ?2 AND image_byte_size IS NULL",
                        params![entry.image_byte_size, id],
                    )?;
                }
            }
            return Ok((id, false));
        }

        let text_content_search = entry.text_content.as_deref().map(lowercase_search_text);

        conn.execute(
            "INSERT INTO clipboard_entries (content_type, text_content, text_content_search, image_data, image_thumb, source_app, source_app_icon, content_hash, char_count, created_at, is_pinned, collection_id, image_format, image_width, image_height, image_byte_size)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                entry.content_type,
                entry.text_content,
                text_content_search,
                entry.image_data,
                entry.image_thumb,
                entry.source_app,
                entry.source_app_icon,
                entry.content_hash,
                entry.char_count,
                entry.created_at,
                entry.is_pinned as i32,
                entry.collection_id,
                entry.image_format,
                entry.image_width,
                entry.image_height,
                entry.image_byte_size,
            ],
        )?;

        Ok((conn.last_insert_rowid(), true))
    }

    pub fn get_entries(
        &self,
        limit: i64,
        offset: i64,
        collection_id: Option<i64>,
        pinned_only: bool,
        search: Option<&str>,
    ) -> Result<Vec<ClipboardEntry>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT id, content_type, text_content, NULL as image_data, COALESCE(image_thumb, image_data) as image_thumb, source_app, NULL as source_app_icon, content_hash, char_count, created_at, is_pinned, collection_id,
             COALESCE((SELECT GROUP_CONCAT(tag, '|') FROM clipboard_tags WHERE entry_id = clipboard_entries.id), '') as tags,
             image_format, image_width, image_height, image_byte_size
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
            let q_lower = lowercase_search_text(q);
            if !q_lower.is_empty() {
                sql.push_str(" AND text_content_search LIKE ?");
                param_values.push(Box::new(format!("%{}%", q_lower)));
            }
        }

        sql.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");
        param_values.push(Box::new(limit));
        param_values.push(Box::new(offset));

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let entries = stmt
            .query_map(params_ref.as_slice(), |row| {
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
                        .map(|tag| tag.to_owned())
                        .collect(),
                    image_format: row.get(13)?,
                    image_width: row.get(14)?,
                    image_height: row.get(15)?,
                    image_byte_size: row.get(16)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut entries = entries;
        for entry in &mut entries {
            resolve_image_format(entry);
        }
        Ok(entries)
    }

    pub fn has_entry_with_content_hash(&self, hash: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(1) FROM clipboard_entries WHERE content_hash = ?1",
            params![hash],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn delete_entry(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM clipboard_entries WHERE id = ?1", params![id])?;
        let unpinned_remaining: i64 = conn.query_row(
            "SELECT COUNT(*) FROM clipboard_entries WHERE is_pinned = 0",
            [],
            |row| row.get(0),
        )?;
        Ok(unpinned_remaining == 0)
    }

    pub fn pin_entry(&self, id: i64, pinned: bool) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE clipboard_entries SET is_pinned = ?1 WHERE id = ?2",
            params![pinned as i32, id],
        )?;
        Ok(())
    }

    pub fn set_collection(
        &self,
        entry_id: i64,
        collection_id: Option<i64>,
    ) -> Result<(), rusqlite::Error> {
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
        let mut stmt = conn
            .prepare("SELECT id, name, color, sort_order FROM collections ORDER BY sort_order")?;
        let cols = stmt
            .query_map([], |row| {
                Ok(Collection {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    sort_order: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(cols)
    }

    pub fn create_collection(
        &self,
        name: &str,
        color: Option<&str>,
    ) -> Result<i64, rusqlite::Error> {
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
        let rows: Vec<(i64, String)> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, bundle_id FROM excluded_apps ORDER BY bundle_id COLLATE NOCASE",
            )?;
            let rows = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };

        let bundle_ids: Vec<&str> = rows
            .iter()
            .map(|(_, bundle_id)| bundle_id.as_str())
            .collect();
        let display_names = crate::macos_app::display_names_for_bundle_ids(&bundle_ids);

        Ok(rows
            .into_iter()
            .zip(display_names)
            .map(|((id, bundle_id), display_name)| ExcludedApp {
                id,
                bundle_id,
                display_name,
            })
            .collect())
    }

    /// Returns `true` when a new row was inserted, `false` when the app was already excluded.
    pub fn add_excluded_app(&self, bundle_id: &str) -> Result<bool, rusqlite::Error> {
        let normalized = bundle_id.trim();
        if normalized.is_empty() {
            return Ok(false);
        }
        let conn = self.conn.lock().unwrap();
        let changes = conn.execute(
            "INSERT OR IGNORE INTO excluded_apps (bundle_id) VALUES (?1)",
            params![normalized],
        )?;
        Ok(changes > 0)
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
        tx.execute(
            "DELETE FROM clipboard_tags WHERE entry_id = ?1",
            params![entry_id],
        )?;

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
                        .map(|tag| tag.to_owned())
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

    pub fn get_entry_by_id(
        &self,
        entry_id: i64,
    ) -> Result<Option<ClipboardEntry>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, content_type, text_content, image_data, image_thumb, source_app, source_app_icon,
                    content_hash, char_count, created_at, is_pinned, collection_id,
                    COALESCE((SELECT GROUP_CONCAT(tag, '|') FROM clipboard_tags WHERE entry_id = clipboard_entries.id), '') as tags,
                    image_format, image_width, image_height, image_byte_size
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
                        .map(|tag| tag.to_owned())
                        .collect(),
                    image_format: row.get(13)?,
                    image_width: row.get(14)?,
                    image_height: row.get(15)?,
                    image_byte_size: row.get(16)?,
                })
            },
        )
        .map(Some)
        .or_else(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            _ => Err(err),
        })
        .map(|opt| {
            opt.map(|mut entry| {
                resolve_image_format(&mut entry);
                entry
            })
        })
    }

    /// Persist image meta for legacy image rows missing width/height/size. Returns rows updated.
    pub fn backfill_missing_image_meta(&self, batch_size: i64) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, image_data, image_thumb FROM clipboard_entries
             WHERE content_type = 'image'
               AND (image_width IS NULL OR image_height IS NULL OR image_byte_size IS NULL)
             LIMIT ?1",
        )?;
        let rows: Vec<(i64, Option<String>, Option<String>)> = stmt
            .query_map(params![batch_size], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut updated = 0i64;
        for (id, image_data, image_thumb) in rows {
            if image_data.as_deref().or(image_thumb.as_deref()).is_none() {
                continue;
            }
            let mut entry = ClipboardEntry {
                id,
                content_type: "image".to_owned(),
                text_content: None,
                image_data,
                image_thumb,
                source_app: None,
                source_app_icon: None,
                content_hash: String::new(),
                char_count: None,
                created_at: String::new(),
                is_pinned: false,
                collection_id: None,
                tags: Vec::new(),
                image_format: None,
                image_width: None,
                image_height: None,
                image_byte_size: None,
            };
            resolve_image_meta(&mut entry);
            let changed = conn.execute(
                "UPDATE clipboard_entries
                 SET image_width = COALESCE(image_width, ?1),
                     image_height = COALESCE(image_height, ?2),
                     image_byte_size = COALESCE(image_byte_size, ?3)
                 WHERE id = ?4",
                params![
                    entry.image_width,
                    entry.image_height,
                    entry.image_byte_size,
                    id,
                ],
            )?;
            updated += changed as i64;
        }
        Ok(updated)
    }

    /// Persist image_format for legacy image rows missing the column. Returns rows updated.
    pub fn backfill_missing_image_formats(&self, batch_size: i64) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, image_data, image_thumb FROM clipboard_entries
             WHERE content_type = 'image' AND image_format IS NULL
             LIMIT ?1",
        )?;
        let rows: Vec<(i64, Option<String>, Option<String>)> = stmt
            .query_map(params![batch_size], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut updated = 0i64;
        for (id, image_data, image_thumb) in rows {
            let b64 = image_data.as_deref().or(image_thumb.as_deref());
            let Some(b64) = b64 else {
                continue;
            };
            let format = crate::image_format::detect_from_b64(b64).to_owned();
            let changed = conn.execute(
                "UPDATE clipboard_entries SET image_format = ?1 WHERE id = ?2 AND image_format IS NULL",
                params![format, id],
            )?;
            updated += changed as i64;
        }
        Ok(updated)
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
        db.conn
            .lock()
            .unwrap()
            .execute_batch(
                "
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
                collection_id INTEGER REFERENCES collections(id) ON DELETE SET NULL,
                image_format TEXT,
                text_content_search TEXT,
                image_width INTEGER,
                image_height INTEGER,
                image_byte_size INTEGER
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
        ",
            )
            .unwrap();
        db
    }

    fn make_entry(text: &str, hash: &str) -> ClipboardEntry {
        ClipboardEntry {
            id: 0,
            content_type: "text".to_owned(),
            text_content: Some(text.to_owned()),
            image_data: None,
            image_thumb: None,
            source_app: Some("TestApp".to_owned()),
            source_app_icon: None,
            content_hash: hash.to_owned(),
            char_count: Some(text.len() as i64),
            created_at: chrono::Utc::now().to_rfc3339(),
            is_pinned: false,
            collection_id: None,
            tags: Vec::new(),
            image_format: None,
            image_width: None,
            image_height: None,
            image_byte_size: None,
        }
    }

    fn make_image_entry(
        hash: &str,
        thumb_b64: &str,
        image_format: Option<&str>,
        image_width: Option<i64>,
        image_height: Option<i64>,
        image_byte_size: Option<i64>,
    ) -> ClipboardEntry {
        ClipboardEntry {
            id: 0,
            content_type: "image".to_owned(),
            text_content: None,
            image_data: None,
            image_thumb: Some(thumb_b64.to_owned()),
            source_app: Some("TestApp".to_owned()),
            source_app_icon: None,
            content_hash: hash.to_owned(),
            char_count: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            is_pinned: false,
            collection_id: None,
            tags: Vec::new(),
            image_format: image_format.map(str::to_string),
            image_width,
            image_height,
            image_byte_size,
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
    fn has_entry_with_content_hash() {
        let db = test_db();
        assert!(!db.has_entry_with_content_hash("missing").unwrap());
        let entry = make_entry("hello", "hash_lookup");
        db.insert_entry(&entry).unwrap();
        assert!(db.has_entry_with_content_hash("hash_lookup").unwrap());
        assert!(!db.has_entry_with_content_hash("other").unwrap());
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

    #[test]
    fn insert_duplicate_image_backfills_image_format() {
        let db = test_db();
        let thumb = "iVBORw0KGgoAAAANSUhEUg";
        let e1 = make_image_entry("img_dup", thumb, None, None, None, None);
        let (id1, new1) = db.insert_entry(&e1).unwrap();
        assert!(new1);

        let e2 = make_image_entry("img_dup", thumb, Some("PNG"), None, None, None);
        let (id2, new2) = db.insert_entry(&e2).unwrap();
        assert!(!new2);
        assert_eq!(id1, id2);

        let entry = db.get_entry_by_id(id1).unwrap().unwrap();
        assert_eq!(entry.image_format.as_deref(), Some("PNG"));
    }

    #[test]
    fn get_entries_resolves_image_format_from_thumb() {
        let db = test_db();
        let thumb = "/9j/4AAQSkZJRgABAQAAAQ";
        let entry = make_image_entry("img_jpg", thumb, None, None, None, None);
        let (id, _) = db.insert_entry(&entry).unwrap();

        let entries = db.get_entries(10, 0, None, false, None).unwrap();
        let found = entries.iter().find(|e| e.id == id).unwrap();
        assert_eq!(found.image_format.as_deref(), Some("JPG"));
    }

    #[test]
    fn backfill_missing_image_formats_persists_rows() {
        let db = test_db();
        let thumb = "R0lGODlhAQABAIAAAAAAAP";
        let entry = make_image_entry("img_gif", thumb, None, None, None, None);
        db.insert_entry(&entry).unwrap();

        let updated = db.backfill_missing_image_formats(100).unwrap();
        assert_eq!(updated, 1);

        let entries = db.get_entries(10, 0, None, false, None).unwrap();
        assert_eq!(entries[0].image_format.as_deref(), Some("GIF"));
    }

    #[test]
    fn image_format_round_trips_on_insert_and_fetch() {
        let db = test_db();
        let mut entry = make_image_entry(
            "img_png",
            "iVBORw0KGgoAAAANSUhEUg",
            Some("PNG"),
            Some(64),
            Some(48),
            Some(12_345),
        );
        entry.image_data = Some("iVBORw0KGgoAAAANSUhEUg".to_owned());

        let (id, is_new) = db.insert_entry(&entry).unwrap();
        assert!(is_new);

        let fetched = db.get_entry_by_id(id).unwrap().unwrap();
        assert_eq!(fetched.image_format.as_deref(), Some("PNG"));
        assert_eq!(fetched.image_width, Some(64));
        assert_eq!(fetched.image_height, Some(48));
        assert_eq!(fetched.image_byte_size, Some(12_345));

        let listed = db.get_entries(10, 0, None, false, None).unwrap();
        let found = listed.iter().find(|e| e.id == id).unwrap();
        assert_eq!(found.image_format.as_deref(), Some("PNG"));
        assert_eq!(found.image_width, Some(64));
        assert_eq!(found.image_height, Some(48));
        assert_eq!(found.image_byte_size, Some(12_345));
    }

    #[test]
    fn backfill_missing_image_meta_persists_rows() {
        let db = test_db();
        // Valid 1×1 PNG (decodable for width/height/byte_size backfill).
        let thumb = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let entry = make_image_entry("img_meta", thumb, Some("PNG"), None, None, None);
        db.insert_entry(&entry).unwrap();

        let updated = db.backfill_missing_image_meta(100).unwrap();
        assert!(updated >= 1);

        let entries = db.get_entries(10, 0, None, false, None).unwrap();
        assert!(entries[0].image_width.is_some());
        assert!(entries[0].image_height.is_some());
        assert!(entries[0].image_byte_size.is_some());
    }

    #[test]
    fn image_format_migration_adds_column_to_legacy_schema() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE clipboard_entries (
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
                collection_id INTEGER
            );",
        )
        .unwrap();

        let _ = conn.execute(
            "ALTER TABLE clipboard_entries ADD COLUMN image_format TEXT",
            [],
        );

        conn.execute(
            "INSERT INTO clipboard_entries (content_type, image_thumb, content_hash, created_at, image_format)
             VALUES ('image', 'R0lGODlh', 'legacy_gif', '2026-01-01T00:00:00Z', 'GIF')",
            [],
        )
        .unwrap();

        let format: String = conn
            .query_row(
                "SELECT image_format FROM clipboard_entries WHERE content_hash = 'legacy_gif'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(format, "GIF");
    }

    // --- Get entries ---

    #[test]
    fn get_entries_respects_limit() {
        let db = test_db();
        for i in 0..10 {
            db.insert_entry(&make_entry(&format!("text {}", i), &format!("h{}", i)))
                .unwrap();
        }
        let entries = db.get_entries(3, 0, None, false, None).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn get_entries_with_search() {
        let db = test_db();
        db.insert_entry(&make_entry("rust programming", "h1"))
            .unwrap();
        db.insert_entry(&make_entry("python script", "h2")).unwrap();
        db.insert_entry(&make_entry("rust cargo", "h3")).unwrap();

        let results = db.get_entries(50, 0, None, false, Some("rust")).unwrap();
        assert_eq!(results.len(), 2);

        let upper = db.get_entries(50, 0, None, false, Some("RUST")).unwrap();
        assert_eq!(upper.len(), 2);
    }

    #[test]
    fn get_entries_search_is_case_insensitive_for_cyrillic() {
        let db = test_db();
        db.insert_entry(&make_entry("Что учесть при реализации", "h_cyr"))
            .unwrap();
        db.insert_entry(&make_entry("другой текст", "h_other"))
            .unwrap();

        let results = db.get_entries(50, 0, None, false, Some("что уч")).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].text_content.as_deref(),
            Some("Что учесть при реализации")
        );
    }

    #[test]
    fn text_content_search_backfills_legacy_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE clipboard_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL DEFAULT 'text',
                text_content TEXT,
                text_content_search TEXT,
                content_hash TEXT NOT NULL,
                created_at TEXT NOT NULL
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO clipboard_entries (content_type, text_content, content_hash, created_at)
             VALUES ('text', 'Что учесть', 'legacy', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();

        backfill_text_content_search(&conn).unwrap();

        let search_value: String = conn
            .query_row(
                "SELECT text_content_search FROM clipboard_entries WHERE content_hash = 'legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(search_value, "что учесть");
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
        assert!(db.delete_entry(id).unwrap());
        assert!(db.get_entry_by_id(id).unwrap().is_none());
    }

    #[test]
    fn delete_last_unpinned_entry_reports_empty_history() {
        let db = test_db();
        let (id, _) = db.insert_entry(&make_entry("only one", "h1")).unwrap();
        assert!(db.delete_entry(id).unwrap());
    }

    #[test]
    fn delete_last_unpinned_with_pinned_still_reports_empty_unpinned_pool() {
        let db = test_db();
        let (pinned_id, _) = db.insert_entry(&make_entry("pinned", "h1")).unwrap();
        db.pin_entry(pinned_id, true).unwrap();
        let (temp_id, _) = db.insert_entry(&make_entry("temp", "h2")).unwrap();
        assert!(db.delete_entry(temp_id).unwrap());
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
        db.set_entry_tags(id, &["rust".to_owned(), "code".to_owned()])
            .unwrap();

        let entry = db.get_entry_by_id(id).unwrap().unwrap();
        assert!(entry.tags.contains(&"rust".to_owned()));
        assert!(entry.tags.contains(&"code".to_owned()));
    }

    #[test]
    fn overwrite_tags() {
        let db = test_db();
        let (id, _) = db.insert_entry(&make_entry("text", "h1")).unwrap();
        db.set_entry_tags(id, &["old".to_owned()]).unwrap();
        db.set_entry_tags(id, &["new".to_owned()]).unwrap();

        let entry = db.get_entry_by_id(id).unwrap().unwrap();
        assert_eq!(entry.tags, vec!["new".to_owned()]);
    }

    #[test]
    fn untagged_entries_returned() {
        let db = test_db();
        db.insert_entry(&make_entry("no tags", "h1")).unwrap();
        let (id2, _) = db.insert_entry(&make_entry("has tags", "h2")).unwrap();
        db.set_entry_tags(id2, &["tagged".to_owned()]).unwrap();

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
        assert!(!s.voice_transcription_enabled);
        assert!(!s.ai_tagging_enabled);
    }

    fn seed_full_settings(db: &Database) {
        db.update_app_settings(
            Some("custom-model"),
            Some(7),
            Some("https://whisper.example/v1"),
            Some("token-abc"),
            Some("whisper-1"),
            Some("option+space"),
            Some("Built-in Microphone"),
            Some(true),
            Some(true),
        )
        .unwrap();
    }

    #[test]
    fn update_settings() {
        let db = test_db();
        seed_full_settings(&db);
        let s = db.get_app_settings().unwrap();
        assert_eq!(s.ollama_model, "custom-model");
        assert_eq!(s.retention_days, 7);
        assert_eq!(s.whisper_server_url, "https://whisper.example/v1");
        assert_eq!(s.whisper_server_token, "token-abc");
        assert_eq!(s.whisper_server_model, "whisper-1");
        assert_eq!(s.voice_shortcut, "option+space");
        assert_eq!(s.selected_microphone, "Built-in Microphone");
        assert!(s.voice_transcription_enabled);
        assert!(s.ai_tagging_enabled);
    }

    #[test]
    fn partial_update_preserves_other_settings() {
        let db = test_db();
        seed_full_settings(&db);

        db.update_app_settings(None, Some(30), None, None, None, None, None, None, None)
            .unwrap();
        let s = db.get_app_settings().unwrap();
        assert_eq!(s.retention_days, 30);
        assert_eq!(s.ollama_model, "custom-model");
        assert_eq!(s.whisper_server_url, "https://whisper.example/v1");
        assert_eq!(s.whisper_server_token, "token-abc");
        assert_eq!(s.whisper_server_model, "whisper-1");
        assert_eq!(s.voice_shortcut, "option+space");
        assert_eq!(s.selected_microphone, "Built-in Microphone");
        assert!(s.voice_transcription_enabled);
        assert!(s.ai_tagging_enabled);
    }

    #[test]
    fn partial_update_whisper_url_only() {
        let db = test_db();
        seed_full_settings(&db);

        db.update_app_settings(
            None,
            None,
            Some("https://new.example"),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let s = db.get_app_settings().unwrap();
        assert_eq!(s.whisper_server_url, "https://new.example");
        assert_eq!(s.ollama_model, "custom-model");
        assert_eq!(s.retention_days, 7);
        assert_eq!(s.voice_shortcut, "option+space");
        assert_eq!(s.selected_microphone, "Built-in Microphone");
        assert!(s.voice_transcription_enabled);
        assert!(s.ai_tagging_enabled);
    }

    #[test]
    fn voice_transcription_enabled_toggle() {
        let db = test_db();
        assert!(!db.get_app_settings().unwrap().voice_transcription_enabled);

        db.update_app_settings(None, None, None, None, None, None, None, Some(true), None)
            .unwrap();
        assert!(db.get_app_settings().unwrap().voice_transcription_enabled);

        db.update_app_settings(None, None, None, None, None, None, None, Some(false), None)
            .unwrap();
        assert!(!db.get_app_settings().unwrap().voice_transcription_enabled);
    }

    #[test]
    fn ai_tagging_enabled_toggle() {
        let db = test_db();
        assert!(!db.is_ai_tagging_enabled());
        assert!(!db.get_app_settings().unwrap().ai_tagging_enabled);

        db.update_app_settings(None, None, None, None, None, None, None, None, Some(true))
            .unwrap();
        assert!(db.is_ai_tagging_enabled());
        assert!(db.get_app_settings().unwrap().ai_tagging_enabled);

        db.update_app_settings(None, None, None, None, None, None, None, None, Some(false))
            .unwrap();
        assert!(!db.is_ai_tagging_enabled());
        assert!(!db.get_app_settings().unwrap().ai_tagging_enabled);
    }

    #[test]
    fn partial_update_voice_enabled_preserves_whisper_settings() {
        let db = test_db();
        seed_full_settings(&db);

        db.update_app_settings(None, None, None, None, None, None, None, Some(false), None)
            .unwrap();
        let s = db.get_app_settings().unwrap();
        assert!(!s.voice_transcription_enabled);
        assert_eq!(s.whisper_server_url, "https://whisper.example/v1");
        assert_eq!(s.voice_shortcut, "option+space");
    }

    #[test]
    fn partial_update_ai_tagging_enabled_preserves_ollama_settings() {
        let db = test_db();
        seed_full_settings(&db);

        db.update_app_settings(None, None, None, None, None, None, None, None, Some(false))
            .unwrap();
        let s = db.get_app_settings().unwrap();
        assert!(!s.ai_tagging_enabled);
        assert_eq!(s.ollama_model, "custom-model");
        assert!(s.voice_transcription_enabled);
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
        assert_eq!(cols[0].color, Some("#ff0000".to_owned()));
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
        let bundle_id = "org.telegram.desktop";
        assert!(!db.is_app_excluded(bundle_id).unwrap());

        db.add_excluded_app(bundle_id).unwrap();
        assert!(db.is_app_excluded(bundle_id).unwrap());

        let apps = db.get_excluded_apps().unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].bundle_id, bundle_id);
        assert_eq!(apps[0].display_name, "Telegram");

        db.remove_excluded_app(apps[0].id).unwrap();
        assert!(!db.is_app_excluded(bundle_id).unwrap());
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
        let bundle_id = "com.apple.Safari";
        assert!(db.add_excluded_app(bundle_id).unwrap());
        assert!(!db.add_excluded_app(bundle_id).unwrap());
        assert_eq!(db.get_excluded_apps().unwrap().len(), 1);
    }
}
