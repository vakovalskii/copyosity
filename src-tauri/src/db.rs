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
    /// When false, hide the footer shortcut strip on the clipboard overlay (default on).
    pub overlay_shortcut_hints_enabled: bool,
    // --- NeuralDeep hub integration ---
    /// When false, hub API calls and the agent-search shortcut are disabled (default off).
    pub hub_enabled: bool,
    /// Base URL of the NeuralDeep hub (OpenAI-compatible), e.g. https://neuraldeep.ru
    pub hub_url: String,
    /// Per-user API token for the hub (Bearer).
    pub hub_token: String,
    /// Chat model id used for tagging via the hub.
    pub hub_chat_model: String,
    /// Use the hub (instead of local Ollama) for clipboard tagging.
    pub hub_tagging_enabled: bool,
    /// Use the hub for voice transcription.
    pub hub_transcribe_enabled: bool,
    // --- Context-aware voice polishing (stolen from opentypeless) ---
    /// Run transcribed voice through the LLM to clean/format it before pasting.
    pub voice_polish_enabled: bool,
    /// Multimodal model used for polishing (must accept images for screenshot context).
    pub voice_polish_model: String,
    /// Send a screenshot of the target window so the model matches surrounding context.
    pub voice_polish_screenshot: bool,
    /// Extra user instructions appended to the polish prompt.
    pub voice_polish_prompt: String,
    /// If non-empty, translate the polished result into this language code (e.g. "en", "ru").
    pub voice_translate_lang: String,
    /// Newline-separated custom terms with exact spellings to preserve.
    pub voice_dictionary: String,
    /// When text is selected in the target app, treat the spoken transcription as
    /// an instruction to apply to that selection (summarize/fix/translate/rewrite).
    pub voice_selected_text: bool,
    /// Render the clipboard board vertically (a tall mini-clipboard docked to the
    /// screen edge) instead of the default horizontal bottom bar.
    pub board_vertical: bool,
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
    /// Text recognized from an image via on-device OCR (Vision). None for text entries.
    #[serde(default)]
    pub ocr_text: Option<String>,
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
pub struct HistoryCounts {
    pub total: i64,
    pub unpinned: i64,
    pub pinned: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct OverlayTagCounts {
    pub semantic: Vec<TagCount>,
    pub format: Vec<TagCount>,
    pub has_text: bool,
    pub has_images: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryTaggedPayload {
    pub entry_id: i64,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryOcrPayload {
    pub entry_id: i64,
    pub ocr_text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i64,
}

/// A snippet folder — a named group of reusable snippets shown as a submenu in
/// the native quick menu (Clipy-style).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SnippetFolder {
    pub id: i64,
    pub name: String,
    pub position: i64,
}

/// A reusable text snippet (email, address, phone, prompt, …) that pastes in
/// two clicks from the quick menu.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Snippet {
    pub id: i64,
    pub folder_id: i64,
    pub title: String,
    pub content: String,
    pub position: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExcludedApp {
    pub id: i64,
    pub bundle_id: String,
    pub display_name: String,
}

fn lowercase_search_text(text: &str) -> String {
    text.to_lowercase()
}

const FORMAT_TAG_ORDER: &[&str] = &["gif", "jpg", "png"];
const SEMANTIC_TAG_LIMIT: i64 = 8;

fn push_entry_list_filters(
    sql: &mut String,
    param_values: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
    table_prefix: &str,
    collection_id: Option<i64>,
    pinned_only: bool,
    search: Option<&str>,
) {
    if let Some(cid) = collection_id {
        sql.push_str(&format!(" AND {table_prefix}collection_id = ?"));
        param_values.push(Box::new(cid));
    }
    if pinned_only {
        sql.push_str(&format!(" AND {table_prefix}is_pinned = 1"));
    }
    if let Some(q) = search {
        let q_lower = lowercase_search_text(q);
        if !q_lower.is_empty() {
            sql.push_str(&format!(
                " AND ({table_prefix}text_content_search LIKE ? OR {table_prefix}ocr_text LIKE ?)"
            ));
            let pattern = format!("%{q_lower}%");
            param_values.push(Box::new(pattern.clone()));
            param_values.push(Box::new(pattern));
        }
    }
}

fn is_format_tag(tag: &str) -> bool {
    matches!(tag, "gif" | "jpg" | "png")
}

fn push_content_kind_filter(sql: &mut String, table_prefix: &str, content_kind: &str) {
    match content_kind {
        "text" => sql.push_str(&format!(" AND {table_prefix}content_type = 'text'")),
        "image" => sql.push_str(&format!(" AND {table_prefix}content_type = 'image'")),
        _ => {}
    }
}

fn push_entry_tag_filter(
    sql: &mut String,
    param_values: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
    table_prefix: &str,
    tag: &str,
    tag_variants: Option<&[String]>,
) {
    let id_col = format!("{table_prefix}id");
    if is_format_tag(tag) {
        sql.push_str(&format!(
            " AND {table_prefix}content_type = 'image' AND (
              CASE
                WHEN UPPER({table_prefix}image_format) IN ('JPG', 'JPEG') THEN 'jpg'
                WHEN UPPER({table_prefix}image_format) = 'GIF' THEN 'gif'
                WHEN UPPER({table_prefix}image_format) = 'PNG' THEN 'png'
                ELSE NULL
              END = ?
              OR EXISTS (
                SELECT 1 FROM clipboard_tags ct
                WHERE ct.entry_id = {id_col} AND ct.tag = ?
              )
            )"
        ));
        param_values.push(Box::new(tag.to_owned()));
        param_values.push(Box::new(tag.to_owned()));
    } else {
        let tags: Vec<String> = match tag_variants {
            Some(variants) if !variants.is_empty() => variants.to_vec(),
            _ => vec![tag.to_owned()],
        };
        let placeholders = tags.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        sql.push_str(&format!(
            " AND {table_prefix}content_type = 'text' AND EXISTS (
                SELECT 1 FROM clipboard_tags ct
                WHERE ct.entry_id = {id_col} AND ct.tag IN ({placeholders})
              )"
        ));
        for variant in tags {
            param_values.push(Box::new(variant));
        }
    }
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

fn hydrate_image_entry(entry: &mut ClipboardEntry) {
    resolve_image_format(entry);
}

pub struct Database {
    pub conn: Mutex<Connection>,
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
                collection_id INTEGER REFERENCES collections(id) ON DELETE SET NULL,
                text_content_search TEXT,
                ocr_text TEXT,
                image_format TEXT,
                image_width INTEGER,
                image_height INTEGER,
                image_byte_size INTEGER
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

            CREATE TABLE IF NOT EXISTS snippet_folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS snippets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                folder_id INTEGER NOT NULL REFERENCES snippet_folders(id) ON DELETE CASCADE,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_snippets_folder ON snippets(folder_id);

            CREATE INDEX IF NOT EXISTS idx_clipboard_tags_entry ON clipboard_tags(entry_id);
            CREATE INDEX IF NOT EXISTS idx_clipboard_tags_tag ON clipboard_tags(tag);
            CREATE INDEX IF NOT EXISTS idx_clipboard_tag_state_status ON clipboard_tag_state(status);
        ")?;

        Self::run_migrations(&conn)?;

        #[cfg(target_os = "macos")]
        crate::macos_app::migrate_legacy_excluded_app_names(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Versioned schema migrations via PRAGMA user_version.
    fn run_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
        let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

        // v1: add ocr_text to clipboard_entries for databases created before it
        // existed in the CREATE TABLE above. New DBs already have the column, so
        // the ALTER fails with a duplicate-column error which we intentionally ignore.
        if version < 1 {
            let _ = conn.execute("ALTER TABLE clipboard_entries ADD COLUMN ocr_text TEXT", []);
            conn.execute_batch("PRAGMA user_version = 1;")?;
        }

        if version < 2 {
            let _ = conn.execute(
                "ALTER TABLE clipboard_entries ADD COLUMN text_content_search TEXT",
                [],
            );
            backfill_text_content_search(conn)?;
            conn.execute_batch("PRAGMA user_version = 2;")?;
        }

        if version < 3 {
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
            conn.execute_batch("PRAGMA user_version = 3;")?;
        }

        // v4: snippet folders + snippets for the native quick menu.
        if version < 4 {
            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS snippet_folders (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    position INTEGER NOT NULL DEFAULT 0
                );
                CREATE TABLE IF NOT EXISTS snippets (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    folder_id INTEGER NOT NULL REFERENCES snippet_folders(id) ON DELETE CASCADE,
                    title TEXT NOT NULL,
                    content TEXT NOT NULL,
                    position INTEGER NOT NULL DEFAULT 0
                );
                CREATE INDEX IF NOT EXISTS idx_snippets_folder ON snippets(folder_id);
                PRAGMA user_version = 4;
                ",
            )?;
        }

        Ok(())
    }

    /// One-time migration: pre-0.6.0 hub users had no `hub_enabled` master switch.
    fn migrate_hub_enabled_if_needed(&self) -> Result<(), rusqlite::Error> {
        if self.get_setting("hub_enabled")?.is_some() {
            return Ok(());
        }
        let legacy_search = self
            .get_setting("hub_search_enabled")?
            .map(|v| v == "true")
            .unwrap_or(false);
        let legacy_tagging = self
            .get_setting("hub_tagging_enabled")?
            .map(|v| v == "true")
            .unwrap_or(false);
        let has_token = !self
            .get_setting("hub_token")?
            .unwrap_or_default()
            .trim()
            .is_empty();
        let hub_transcribe = self
            .get_setting("hub_transcribe_enabled")?
            .map(|v| v == "true")
            .unwrap_or(false);
        let voice_polish = self
            .get_setting("voice_polish_enabled")?
            .map(|v| v == "true")
            .unwrap_or(false);
        let enabled = legacy_search
            || (legacy_tagging && has_token)
            || (has_token && (hub_transcribe || voice_polish));
        self.set_setting("hub_enabled", if enabled { "true" } else { "false" })
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
        self.migrate_hub_enabled_if_needed()?;
        let ollama_model = self
            .get_setting("ollama_model")?
            .unwrap_or_else(|| "qwen3:4b-instruct-2507-q4_K_M".to_string());
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
            .unwrap_or_else(|| "whisper-1".to_string());

        let voice_shortcut = self
            .get_setting("voice_shortcut")?
            .unwrap_or_else(|| "option+space".to_string());
        let selected_microphone = self.get_setting("selected_microphone")?.unwrap_or_default();
        let voice_transcription_enabled = self
            .get_setting("voice_transcription_enabled")?
            .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
            .unwrap_or(false);
        let ai_tagging_enabled = self
            .get_setting("ai_tagging_enabled")?
            .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
            .unwrap_or(false);
        let overlay_shortcut_hints_enabled = self
            .get_setting("overlay_shortcut_hints_enabled")?
            .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
            .unwrap_or(true);

        let hub_enabled = self
            .get_setting("hub_enabled")?
            .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
            .unwrap_or(false);

        let hub_url = self
            .get_setting("hub_url")?
            .unwrap_or_else(|| "https://api.neuraldeep.ru".to_string());
        let hub_token = self.get_setting("hub_token")?.unwrap_or_default();
        let hub_chat_model = self
            .get_setting("hub_chat_model")?
            .unwrap_or_else(|| "qwen3.6-35b-a3b".to_string());
        let hub_tagging_enabled = self
            .get_setting("hub_tagging_enabled")?
            .map(|v| v == "true")
            .unwrap_or(false);
        let hub_transcribe_enabled = self
            .get_setting("hub_transcribe_enabled")?
            .map(|v| v == "true")
            .unwrap_or(false);

        let voice_polish_enabled = self
            .get_setting("voice_polish_enabled")?
            .map(|v| v == "true")
            .unwrap_or(false);
        let voice_polish_model = self
            .get_setting("voice_polish_model")?
            .unwrap_or_else(|| "qwen3.6-35b-a3b".to_string());
        let voice_polish_screenshot = self
            .get_setting("voice_polish_screenshot")?
            .map(|v| v == "true")
            .unwrap_or(true);
        let voice_polish_prompt = self.get_setting("voice_polish_prompt")?.unwrap_or_default();
        let voice_translate_lang = self
            .get_setting("voice_translate_lang")?
            .unwrap_or_default();
        let voice_dictionary = self.get_setting("voice_dictionary")?.unwrap_or_default();
        let voice_selected_text = self
            .get_setting("voice_selected_text")?
            .map(|v| v == "true")
            .unwrap_or(false);
        let board_vertical = self
            .get_setting("board_vertical")?
            .map(|v| v == "true")
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
            overlay_shortcut_hints_enabled,
            hub_enabled,
            hub_url,
            hub_token,
            hub_chat_model,
            hub_tagging_enabled,
            hub_transcribe_enabled,
            voice_polish_enabled,
            voice_polish_model,
            voice_polish_screenshot,
            voice_polish_prompt,
            voice_translate_lang,
            voice_dictionary,
            voice_selected_text,
            board_vertical,
        })
    }

    #[allow(clippy::too_many_arguments)]
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
        overlay_shortcut_hints_enabled: Option<bool>,
        hub_enabled: Option<bool>,
        hub_url: Option<&str>,
        hub_token: Option<&str>,
        hub_chat_model: Option<&str>,
        hub_tagging_enabled: Option<bool>,
        hub_transcribe_enabled: Option<bool>,
        voice_polish_enabled: Option<bool>,
        voice_polish_model: Option<&str>,
        voice_polish_screenshot: Option<bool>,
        voice_polish_prompt: Option<&str>,
        voice_translate_lang: Option<&str>,
        voice_dictionary: Option<&str>,
        voice_selected_text: Option<bool>,
        board_vertical: Option<bool>,
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
        if let Some(enabled) = overlay_shortcut_hints_enabled {
            self.set_setting(
                "overlay_shortcut_hints_enabled",
                if enabled { "true" } else { "false" },
            )?;
        }
        if let Some(enabled) = hub_enabled {
            self.set_setting("hub_enabled", if enabled { "true" } else { "false" })?;
        }
        if let Some(url) = hub_url {
            self.set_setting("hub_url", url.trim())?;
        }
        if let Some(token) = hub_token {
            self.set_setting("hub_token", token.trim())?;
        }
        if let Some(model) = hub_chat_model {
            self.set_setting("hub_chat_model", model.trim())?;
        }
        if let Some(enabled) = hub_tagging_enabled {
            self.set_setting(
                "hub_tagging_enabled",
                if enabled { "true" } else { "false" },
            )?;
        }
        if let Some(enabled) = hub_transcribe_enabled {
            self.set_setting(
                "hub_transcribe_enabled",
                if enabled { "true" } else { "false" },
            )?;
        }
        if let Some(enabled) = voice_polish_enabled {
            self.set_setting(
                "voice_polish_enabled",
                if enabled { "true" } else { "false" },
            )?;
        }
        if let Some(model) = voice_polish_model {
            self.set_setting("voice_polish_model", model.trim())?;
        }
        if let Some(enabled) = voice_polish_screenshot {
            self.set_setting(
                "voice_polish_screenshot",
                if enabled { "true" } else { "false" },
            )?;
        }
        if let Some(p) = voice_polish_prompt {
            self.set_setting("voice_polish_prompt", p.trim())?;
        }
        if let Some(lang) = voice_translate_lang {
            self.set_setting("voice_translate_lang", lang.trim())?;
        }
        if let Some(dict) = voice_dictionary {
            self.set_setting("voice_dictionary", dict.trim())?;
        }
        if let Some(enabled) = voice_selected_text {
            self.set_setting(
                "voice_selected_text",
                if enabled { "true" } else { "false" },
            )?;
        }
        if let Some(enabled) = board_vertical {
            self.set_setting("board_vertical", if enabled { "true" } else { "false" })?;
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
            // Re-copying content that's already in history: bump it to the top
            // so it resurfaces (a clipboard manager must show the latest copy
            // as newest), instead of silently leaving it buried at its old time.
            conn.execute(
                "UPDATE clipboard_entries SET created_at = ?1 WHERE id = ?2",
                params![entry.created_at, id],
            )?;
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
        tag: Option<&str>,
        tag_variants: Option<&[String]>,
        content_kind: Option<&str>,
    ) -> Result<Vec<ClipboardEntry>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let mut sql = String::from(
            "SELECT id, content_type, text_content, NULL as image_data, COALESCE(image_thumb, image_data) as image_thumb, source_app, NULL as source_app_icon, content_hash, char_count, created_at, is_pinned, collection_id,
             COALESCE((SELECT GROUP_CONCAT(tag, '|') FROM clipboard_tags WHERE entry_id = clipboard_entries.id), '') as tags,
             ocr_text, image_format, image_width, image_height, image_byte_size
             FROM clipboard_entries WHERE 1=1"
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        push_entry_list_filters(
            &mut sql,
            &mut param_values,
            "",
            collection_id,
            pinned_only,
            search,
        );

        if let Some(kind) = content_kind {
            push_content_kind_filter(&mut sql, "", kind);
        }
        if let Some(tag) = tag {
            push_entry_tag_filter(&mut sql, &mut param_values, "", tag, tag_variants);
        }

        sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?");
        param_values.push(Box::new(limit));
        param_values.push(Box::new(offset));

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let mut entries = stmt
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
                        .map(|tag| tag.to_string())
                        .collect(),
                    ocr_text: row.get(13)?,
                    image_format: row.get(14)?,
                    image_width: row.get(15)?,
                    image_height: row.get(16)?,
                    image_byte_size: row.get(17)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        for entry in &mut entries {
            hydrate_image_entry(entry);
        }

        Ok(entries)
    }

    pub fn get_overlay_tag_counts(
        &self,
        collection_id: Option<i64>,
        pinned_only: bool,
        search: Option<&str>,
    ) -> Result<OverlayTagCounts, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        let mut semantic_sql = String::from(
            "SELECT ct.tag, COUNT(DISTINCT ce.id) AS cnt
             FROM clipboard_tags ct
             INNER JOIN clipboard_entries ce ON ce.id = ct.entry_id
             WHERE ce.content_type = 'text'
               AND ct.tag NOT IN ('code', 'otp', 'token', 'log', 'gif', 'jpg', 'png', 'jpeg')",
        );
        let mut semantic_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        push_entry_list_filters(
            &mut semantic_sql,
            &mut semantic_params,
            "ce.",
            collection_id,
            pinned_only,
            search,
        );
        semantic_sql.push_str(" GROUP BY ct.tag ORDER BY cnt DESC, ct.tag ASC LIMIT ?");
        semantic_params.push(Box::new(SEMANTIC_TAG_LIMIT));

        let semantic_params_ref: Vec<&dyn rusqlite::types::ToSql> =
            semantic_params.iter().map(|p| p.as_ref()).collect();
        let semantic = conn
            .prepare(&semantic_sql)?
            .query_map(semantic_params_ref.as_slice(), |row| {
                Ok(TagCount {
                    tag: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut format_sql = String::from(
            "SELECT fmt, COUNT(DISTINCT id) AS cnt FROM (
                SELECT ce.id,
                  CASE
                    WHEN UPPER(ce.image_format) IN ('JPG', 'JPEG') THEN 'jpg'
                    WHEN UPPER(ce.image_format) = 'GIF' THEN 'gif'
                    WHEN UPPER(ce.image_format) = 'PNG' THEN 'png'
                    ELSE NULL
                  END AS fmt
                FROM clipboard_entries ce
                WHERE ce.content_type = 'image' AND ce.image_format IS NOT NULL",
        );
        let mut format_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        push_entry_list_filters(
            &mut format_sql,
            &mut format_params,
            "ce.",
            collection_id,
            pinned_only,
            search,
        );
        format_sql.push_str(
            "
                UNION
                SELECT ce.id, ct.tag AS fmt
                FROM clipboard_entries ce
                INNER JOIN clipboard_tags ct ON ct.entry_id = ce.id
                WHERE ce.content_type = 'image'
                  AND ct.tag IN ('gif', 'jpg', 'png')",
        );
        push_entry_list_filters(
            &mut format_sql,
            &mut format_params,
            "ce.",
            collection_id,
            pinned_only,
            search,
        );
        format_sql.push_str(") WHERE fmt IS NOT NULL GROUP BY fmt ORDER BY cnt DESC, fmt ASC");

        let format_params_ref: Vec<&dyn rusqlite::types::ToSql> =
            format_params.iter().map(|p| p.as_ref()).collect();
        let format_rows = conn
            .prepare(&format_sql)?
            .query_map(format_params_ref.as_slice(), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let format_map: std::collections::HashMap<String, i64> = format_rows.into_iter().collect();
        let mut format: Vec<TagCount> = FORMAT_TAG_ORDER
            .iter()
            .filter_map(|tag| {
                format_map.get(*tag).map(|count| TagCount {
                    tag: (*tag).to_owned(),
                    count: *count,
                })
            })
            .collect();
        format.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));

        let mut kind_sql = String::from(
            "SELECT
                EXISTS(SELECT 1 FROM clipboard_entries WHERE content_type = 'text'",
        );
        let mut kind_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        push_entry_list_filters(
            &mut kind_sql,
            &mut kind_params,
            "",
            collection_id,
            pinned_only,
            search,
        );
        kind_sql.push_str(
            "),
                EXISTS(SELECT 1 FROM clipboard_entries WHERE content_type = 'image'",
        );
        push_entry_list_filters(
            &mut kind_sql,
            &mut kind_params,
            "",
            collection_id,
            pinned_only,
            search,
        );
        kind_sql.push(')');

        let kind_params_ref: Vec<&dyn rusqlite::types::ToSql> =
            kind_params.iter().map(|p| p.as_ref()).collect();
        let (has_text, has_images): (i32, i32) =
            conn.query_row(&kind_sql, kind_params_ref.as_slice(), |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;

        Ok(OverlayTagCounts {
            semantic,
            format,
            has_text: has_text != 0,
            has_images: has_images != 0,
        })
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

    // ---- Snippets ----

    pub fn get_snippet_folders(&self) -> Result<Vec<SnippetFolder>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, name, position FROM snippet_folders ORDER BY position, id")?;
        let folders = stmt
            .query_map([], |row| {
                Ok(SnippetFolder {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    position: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(folders)
    }

    pub fn get_snippets(&self) -> Result<Vec<Snippet>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, folder_id, title, content, position FROM snippets ORDER BY folder_id, position, id",
        )?;
        let snippets = stmt
            .query_map([], |row| {
                Ok(Snippet {
                    id: row.get(0)?,
                    folder_id: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get(3)?,
                    position: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(snippets)
    }

    pub fn get_snippet_by_id(&self, id: i64) -> Result<Option<Snippet>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, folder_id, title, content, position FROM snippets WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Snippet {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
                position: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn create_snippet_folder(&self, name: &str) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let next_pos: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(position) + 1, 0) FROM snippet_folders",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO snippet_folders (name, position) VALUES (?1, ?2)",
            params![name, next_pos],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn rename_snippet_folder(&self, id: i64, name: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE snippet_folders SET name = ?2 WHERE id = ?1",
            params![id, name],
        )?;
        Ok(())
    }

    pub fn delete_snippet_folder(&self, id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM snippet_folders WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn create_snippet(
        &self,
        folder_id: i64,
        title: &str,
        content: &str,
    ) -> Result<i64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let next_pos: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(position) + 1, 0) FROM snippets WHERE folder_id = ?1",
                params![folder_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO snippets (folder_id, title, content, position) VALUES (?1, ?2, ?3, ?4)",
            params![folder_id, title, content, next_pos],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_snippet(
        &self,
        id: i64,
        title: &str,
        content: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE snippets SET title = ?2, content = ?3 WHERE id = ?1",
            params![id, title, content],
        )?;
        Ok(())
    }

    pub fn delete_snippet(&self, id: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM snippets WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear_history(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM clipboard_entries WHERE is_pinned = 0", [])?;
        Ok(())
    }

    pub fn clear_all_history(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM clipboard_entries", [])?;
        Ok(())
    }

    pub fn get_history_counts(&self) -> Result<HistoryCounts, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM clipboard_entries", [], |row| {
            row.get(0)
        })?;
        let pinned: i64 = conn.query_row(
            "SELECT COUNT(*) FROM clipboard_entries WHERE is_pinned = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(HistoryCounts {
            total,
            pinned,
            unpinned: total - pinned,
        })
    }

    pub fn get_excluded_apps(&self) -> Result<Vec<ExcludedApp>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, bundle_id FROM excluded_apps ORDER BY bundle_id COLLATE NOCASE")?;
        let rows: Vec<(i64, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        drop(conn);

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
                        .map(|tag| tag.to_string())
                        .collect(),
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
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
                    ocr_text, image_format, image_width, image_height, image_byte_size
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
                    ocr_text: row.get(13)?,
                    image_format: row.get(14)?,
                    image_width: row.get(15)?,
                    image_height: row.get(16)?,
                    image_byte_size: row.get(17)?,
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
                hydrate_image_entry(&mut entry);
                entry
            })
        })
    }

    /// Store OCR-recognized text for an image entry.
    pub fn set_ocr_text(&self, entry_id: i64, text: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE clipboard_entries SET ocr_text = ?1 WHERE id = ?2",
            params![text, entry_id],
        )?;
        Ok(())
    }

    /// Persist image meta for legacy image rows missing width/height/size. Returns rows updated.
    pub fn backfill_missing_image_meta(&self, batch_size: i64) -> Result<i64, rusqlite::Error> {
        let rows: Vec<(i64, Option<String>, Option<String>)> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, image_data, image_thumb FROM clipboard_entries
                 WHERE content_type = 'image'
                   AND (image_width IS NULL OR image_height IS NULL OR image_byte_size IS NULL)
                 LIMIT ?1",
            )?;
            let mapped = stmt.query_map(params![batch_size], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };

        let mut updates: Vec<(Option<i64>, Option<i64>, Option<i64>, i64)> = Vec::new();
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
                ocr_text: None,
                image_format: None,
                image_width: None,
                image_height: None,
                image_byte_size: None,
            };
            resolve_image_meta(&mut entry);
            updates.push((
                entry.image_width,
                entry.image_height,
                entry.image_byte_size,
                id,
            ));
        }

        if updates.is_empty() {
            return Ok(0);
        }

        let conn = self.conn.lock().unwrap();
        let mut updated = 0i64;
        for (width, height, byte_size, id) in updates {
            let changed = conn.execute(
                "UPDATE clipboard_entries
                 SET image_width = COALESCE(image_width, ?1),
                     image_height = COALESCE(image_height, ?2),
                     image_byte_size = COALESCE(image_byte_size, ?3)
                 WHERE id = ?4",
                params![width, height, byte_size, id],
            )?;
            updated += changed as i64;
        }
        Ok(updated)
    }

    /// Persist image_format for legacy image rows missing the column. Returns rows updated.
    pub fn backfill_missing_image_formats(&self, batch_size: i64) -> Result<i64, rusqlite::Error> {
        let rows: Vec<(i64, Option<String>, Option<String>, String)> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT ce.id, ce.image_data, ce.image_thumb,
                        COALESCE((SELECT GROUP_CONCAT(ct.tag, '|') FROM clipboard_tags ct WHERE ct.entry_id = ce.id), '')
                 FROM clipboard_entries ce
                 WHERE ce.content_type = 'image' AND ce.image_format IS NULL
                 LIMIT ?1",
            )?;
            let mapped = stmt.query_map(params![batch_size], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };

        let mut updates: Vec<(String, i64)> = Vec::new();
        for (id, image_data, image_thumb, tags_joined) in rows {
            let format = if let Some(b64) = image_data.as_deref().or(image_thumb.as_deref()) {
                Some(crate::image_format::detect_from_b64(b64).to_owned())
            } else {
                tags_joined
                    .split('|')
                    .find_map(crate::image_format::detect_from_format_tag)
                    .map(str::to_owned)
            };
            let Some(format) = format else {
                continue;
            };
            updates.push((format, id));
        }

        if updates.is_empty() {
            return Ok(0);
        }

        let conn = self.conn.lock().unwrap();
        let mut updated = 0i64;
        for (format, id) in updates {
            let changed = conn.execute(
                "UPDATE clipboard_entries SET image_format = ?1 WHERE id = ?2 AND image_format IS NULL",
                params![format, id],
            )?;
            updated += changed as i64;
        }
        Ok(updated)
    }

    #[allow(dead_code)]
    pub fn cleanup_old_entries(&self, max_age_days: i64) -> Result<u64, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            // datetime(created_at) normalizes the stored RFC3339 ('T' + tz) so
            // the comparison is correct — a raw string compare against
            // datetime('now') (space-separated) never matched, so retention
            // cleanup silently never ran.
            "DELETE FROM clipboard_entries WHERE is_pinned = 0 AND datetime(created_at) < datetime('now', ?1)",
            params![format!("-{} days", max_age_days)],
        )?;
        Ok(conn.changes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
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
                text_content_search TEXT,
                ocr_text TEXT,
                image_format TEXT,
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
        Database::run_migrations(&conn).unwrap();
        Database {
            conn: Mutex::new(conn),
        }
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
            ocr_text: None,
            image_format: None,
            image_width: None,
            image_height: None,
            image_byte_size: None,
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
    fn insert_duplicate_bumps_created_at_to_top() {
        let db = test_db();
        let old_time = "2020-01-01T00:00:00Z".to_string();
        let mut first = make_entry("hello", "hash_resurface");
        first.created_at = old_time.clone();
        let (id, _) = db.insert_entry(&first).unwrap();

        db.insert_entry(&make_entry("other", "hash_other")).unwrap();

        let mut resurface = make_entry("hello", "hash_resurface");
        resurface.created_at = chrono::Utc::now().to_rfc3339();
        let (same_id, is_new) = db.insert_entry(&resurface).unwrap();
        assert!(!is_new);
        assert_eq!(id, same_id);

        let entries = db
            .get_entries(10, 0, None, false, None, None, None, None)
            .unwrap();
        assert_eq!(entries[0].id, id);
        assert!(entries[0].created_at > old_time);
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
            db.insert_entry(&make_entry(&format!("text {}", i), &format!("h{}", i)))
                .unwrap();
        }
        let entries = db
            .get_entries(3, 0, None, false, None, None, None, None)
            .unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn get_entries_with_search() {
        let db = test_db();
        db.insert_entry(&make_entry("rust programming", "h1"))
            .unwrap();
        db.insert_entry(&make_entry("python script", "h2")).unwrap();
        db.insert_entry(&make_entry("rust cargo", "h3")).unwrap();

        let results = db
            .get_entries(50, 0, None, false, Some("rust"), None, None, None)
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn get_entries_pinned_only() {
        let db = test_db();
        let (id1, _) = db.insert_entry(&make_entry("pinned", "h1")).unwrap();
        db.insert_entry(&make_entry("not pinned", "h2")).unwrap();
        db.pin_entry(id1, true).unwrap();

        let pinned = db
            .get_entries(50, 0, None, true, None, None, None, None)
            .unwrap();
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
        let all = db
            .get_entries(50, 0, None, false, None, None, None, None)
            .unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].is_pinned);
    }

    // --- Tags ---

    #[test]
    fn set_and_get_tags() {
        let db = test_db();
        let (id, _) = db.insert_entry(&make_entry("tagged text", "h1")).unwrap();
        db.set_entry_tags(id, &["rust".to_string(), "code".to_string()])
            .unwrap();

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
        assert!(!s.hub_enabled);
    }

    #[test]
    fn update_settings() {
        let db = test_db();
        db.update_app_settings(
            Some("custom-model"),
            Some(7),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
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

    #[test]
    fn hub_tagging_config_enables_retag_without_ollama() {
        let db = test_db();
        db.set_setting("hub_enabled", "true").unwrap();
        db.set_setting("hub_tagging_enabled", "true").unwrap();
        db.set_setting("hub_token", "secret").unwrap();
        assert!(crate::tagging::is_retag_ready(&db));
    }

    #[test]
    fn hub_text_auto_tag_requires_chat_model() {
        let db = test_db();
        db.set_setting("ai_tagging_enabled", "false").unwrap();
        db.set_setting("hub_enabled", "true").unwrap();
        db.set_setting("hub_tagging_enabled", "true").unwrap();
        db.set_setting("hub_token", "secret").unwrap();
        db.set_setting("hub_chat_model", "").unwrap();
        assert!(!crate::tagging::should_auto_tag_text_on_capture(&db));
        db.set_setting("hub_chat_model", "gpt-oss-120b").unwrap();
        assert!(crate::tagging::should_auto_tag_text_on_capture(&db));
    }

    #[test]
    fn retag_not_ready_when_tagging_disabled_everywhere() {
        let db = test_db();
        assert!(!crate::tagging::is_retag_ready(&db));
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

        assert!(db.add_excluded_app("Telegram").unwrap());
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

    // --- Image meta ---

    fn make_image_entry(hash: &str, format: &str) -> ClipboardEntry {
        ClipboardEntry {
            id: 0,
            content_type: "image".to_string(),
            text_content: None,
            image_data: Some("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==".to_string()),
            image_thumb: None,
            source_app: None,
            source_app_icon: None,
            content_hash: hash.to_string(),
            char_count: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            is_pinned: false,
            collection_id: None,
            tags: vec![format.to_string()],
            ocr_text: None,
            image_format: Some(format.to_string()),
            image_width: Some(1),
            image_height: Some(1),
            image_byte_size: Some(68),
        }
    }

    #[test]
    fn image_format_round_trips_on_insert_and_fetch() {
        let db = test_db();
        let entry = make_image_entry("img_hash_1", "PNG");
        let (id, is_new) = db.insert_entry(&entry).unwrap();
        assert!(is_new);

        let fetched = db.get_entry_by_id(id).unwrap().unwrap();
        assert_eq!(fetched.image_format.as_deref(), Some("PNG"));
        assert_eq!(fetched.image_width, Some(1));
        assert_eq!(fetched.image_height, Some(1));
        assert_eq!(fetched.image_byte_size, Some(68));
    }

    #[test]
    fn get_entries_resolves_image_format_from_thumb() {
        let db = test_db();
        let mut entry = make_image_entry("img_hash_2", "PNG");
        entry.image_format = None;
        let (id, _) = db.insert_entry(&entry).unwrap();

        let entries = db
            .get_entries(10, 0, None, false, None, None, None, None)
            .unwrap();
        let found = entries.iter().find(|e| e.id == id).unwrap();
        assert_eq!(found.image_format.as_deref(), Some("PNG"));
    }

    #[test]
    fn backfill_missing_image_formats_persists_rows() {
        let db = test_db();
        let mut entry = make_image_entry("img_hash_3", "PNG");
        entry.image_format = None;
        db.insert_entry(&entry).unwrap();

        let updated = db.backfill_missing_image_formats(10).unwrap();
        assert!(updated >= 1);

        let entries = db
            .get_entries(10, 0, None, false, None, None, None, None)
            .unwrap();
        let found = entries
            .iter()
            .find(|e| e.content_hash == "img_hash_3")
            .expect("backfilled row");
        assert_eq!(found.image_format.as_deref(), Some("PNG"));
    }

    #[test]
    fn get_entries_format_tag_matches_image_format_column() {
        let db = test_db();
        let mut entry = make_image_entry("img_hash_4", "JPG");
        entry.tags = vec!["jpg".to_string()];
        let (id, _) = db.insert_entry(&entry).unwrap();
        db.set_entry_tags(id, &["jpg".to_string()]).unwrap();

        let filtered = db
            .get_entries(10, 0, None, false, None, Some("jpg"), None, None)
            .unwrap();
        assert!(filtered.iter().any(|e| e.id == id));
    }

    #[test]
    fn get_entries_format_tag_matches_column_without_format_tag() {
        let db = test_db();
        let mut entry = make_image_entry("img_hash_5", "PNG");
        entry.tags = vec!["screenshot".to_string()];
        let (id, _) = db.insert_entry(&entry).unwrap();

        let filtered = db
            .get_entries(10, 0, None, false, None, Some("png"), None, None)
            .unwrap();
        assert!(filtered.iter().any(|e| e.id == id));
    }

    #[test]
    fn backfill_missing_image_meta_persists_dimensions_and_byte_size() {
        let db = test_db();
        let mut entry = make_image_entry("img_hash_meta", "PNG");
        entry.image_width = None;
        entry.image_height = None;
        entry.image_byte_size = None;
        db.insert_entry(&entry).unwrap();

        let updated = db.backfill_missing_image_meta(10).unwrap();
        assert!(updated >= 1);

        let entries = db
            .get_entries(10, 0, None, false, None, None, None, None)
            .unwrap();
        let found = entries
            .iter()
            .find(|e| e.content_hash == "img_hash_meta")
            .unwrap();
        assert_eq!(found.image_width, Some(1));
        assert_eq!(found.image_height, Some(1));
        assert!(found.image_byte_size.unwrap_or(0) > 0);
    }

    #[test]
    fn migration_v3_normalizes_legacy_jpeg() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            PRAGMA foreign_keys=ON;
            CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT);
            CREATE TABLE collections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                color TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE clipboard_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL,
                text_content TEXT,
                image_data TEXT,
                image_thumb TEXT,
                source_app TEXT,
                source_app_icon TEXT,
                content_hash TEXT NOT NULL UNIQUE,
                char_count INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                is_pinned INTEGER NOT NULL DEFAULT 0,
                collection_id INTEGER,
                ocr_text TEXT,
                image_format TEXT,
                image_width INTEGER,
                image_height INTEGER,
                image_byte_size INTEGER
            );
            CREATE TABLE clipboard_tags (
                entry_id INTEGER NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (entry_id, tag)
            );
            PRAGMA user_version = 2;
            ",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO clipboard_entries (content_type, content_hash, image_format)
             VALUES ('image', 'legacy_jpeg', 'JPEG')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO clipboard_tags (entry_id, tag) VALUES (1, 'jpeg')",
            [],
        )
        .unwrap();

        Database::run_migrations(&conn).unwrap();

        let format: String = conn
            .query_row(
                "SELECT image_format FROM clipboard_entries WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(format, "JPG");
        let tag: String = conn
            .query_row(
                "SELECT tag FROM clipboard_tags WHERE entry_id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tag, "jpg");
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        // v3 (and any later migrations) applied.
        assert!(version >= 3);
    }

    #[test]
    fn migrate_hub_enabled_from_legacy_flags() {
        let db = test_db();
        db.set_setting("hub_search_enabled", "true").unwrap();
        let settings = db.get_app_settings().unwrap();
        assert!(settings.hub_enabled);
    }

    #[test]
    fn migrate_hub_enabled_from_tagging_and_token() {
        let db = test_db();
        db.set_setting("hub_tagging_enabled", "true").unwrap();
        db.set_setting("hub_token", "secret").unwrap();
        let settings = db.get_app_settings().unwrap();
        assert!(settings.hub_enabled);
    }

    #[test]
    fn migrate_hub_enabled_from_voice_hub_and_token() {
        let db = test_db();
        db.set_setting("hub_token", "secret").unwrap();
        db.set_setting("hub_transcribe_enabled", "true").unwrap();
        let settings = db.get_app_settings().unwrap();
        assert!(settings.hub_enabled);
    }

    #[test]
    fn backfill_image_format_from_tag_when_bytes_missing() {
        let db = test_db();
        let mut entry = make_image_entry("img_tag_only", "PNG");
        entry.image_data = None;
        entry.image_thumb = None;
        entry.image_format = None;
        entry.tags = vec!["png".to_string()];
        let (id, _) = db.insert_entry(&entry).unwrap();
        db.set_entry_tags(id, &["png".to_string()]).unwrap();

        let updated = db.backfill_missing_image_formats(10).unwrap();
        assert_eq!(updated, 1);

        let entries = db
            .get_entries(10, 0, None, false, None, None, None, None)
            .unwrap();
        let found = entries.iter().find(|e| e.id == id).expect("row");
        assert_eq!(found.image_format.as_deref(), Some("PNG"));
    }

    #[test]
    fn migrate_hub_enabled_stays_false_without_token() {
        let db = test_db();
        db.set_setting("hub_tagging_enabled", "true").unwrap();
        let settings = db.get_app_settings().unwrap();
        assert!(!settings.hub_enabled);
    }

    #[test]
    fn migrate_hub_enabled_skips_when_already_set() {
        let db = test_db();
        db.set_setting("hub_enabled", "false").unwrap();
        db.set_setting("hub_search_enabled", "true").unwrap();
        let settings = db.get_app_settings().unwrap();
        assert!(!settings.hub_enabled);
    }

    #[test]
    fn get_overlay_tag_counts_dedupes_format_column_and_tag() {
        let db = test_db();
        let mut dual = make_image_entry("img_counts_dual", "PNG");
        dual.tags = vec!["png".to_string(), "screenshot".to_string()];
        db.insert_entry(&dual).unwrap();

        let counts = db.get_overlay_tag_counts(None, false, None).unwrap();
        let png = counts
            .format
            .iter()
            .find(|c| c.tag == "png")
            .expect("png chip");
        assert_eq!(png.count, 1, "one entry must not double-count column + tag");
    }

    #[test]
    fn get_overlay_tag_counts_sorts_format_by_count_desc() {
        let db = test_db();
        for i in 0..5 {
            db.insert_entry(&make_image_entry(&format!("jpg_{i}"), "jpg"))
                .unwrap();
        }
        for i in 0..87 {
            db.insert_entry(&make_image_entry(&format!("gif_{i}"), "gif"))
                .unwrap();
        }
        for i in 0..256 {
            db.insert_entry(&make_image_entry(&format!("png_{i}"), "png"))
                .unwrap();
        }

        let counts = db.get_overlay_tag_counts(None, false, None).unwrap();
        let format_tags: Vec<_> = counts.format.iter().map(|c| c.tag.as_str()).collect();
        assert_eq!(format_tags, vec!["png", "gif", "jpg"]);
        assert_eq!(counts.format[0].count, 256);
        assert_eq!(counts.format[1].count, 87);
        assert_eq!(counts.format[2].count, 5);
    }

    #[test]
    fn update_settings_round_trips_hub_enabled() {
        let db = test_db();
        db.update_app_settings(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(true),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert!(db.get_app_settings().unwrap().hub_enabled);
        db.update_app_settings(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(false),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert!(!db.get_app_settings().unwrap().hub_enabled);
    }

    #[test]
    fn get_entries_tag_variants_match_synonym_tags() {
        let db = test_db();
        let (id, _) = db
            .insert_entry(&make_entry("const x = 1", "hash_syn"))
            .unwrap();
        db.set_entry_tags(id, &["javascript".to_string()]).unwrap();

        let filtered = db
            .get_entries(
                10,
                0,
                None,
                false,
                None,
                Some("js"),
                Some(&["javascript".to_string(), "js".to_string()]),
                None,
            )
            .unwrap();
        assert!(filtered.iter().any(|e| e.id == id));
    }

    #[test]
    fn get_entries_search_matches_ocr_text() {
        let db = test_db();
        let (id, _) = db
            .insert_entry(&make_image_entry("img_ocr", "PNG"))
            .unwrap();
        db.set_ocr_text(id, "Hello from OCR").unwrap();

        let filtered = db
            .get_entries(10, 0, None, false, Some("OCR"), None, None, None)
            .unwrap();
        assert!(filtered.iter().any(|e| e.id == id));
    }
}
