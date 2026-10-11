//! Sync bookkeeping inside the app's SQLite database.
//!
//! Change tracking is done with triggers, so the existing insert/pin/delete code paths in
//! `db.rs` stay untouched:
//! - every entry gets a stable `entry_uuid` (random, assigned on insert or backfilled);
//! - inserts, pin/time changes and deletes append a row to `sync_log` with a monotonic
//!   `seq`; peers pull "everything after seq N";
//! - while a batch from a peer is being applied, `sync_state.applying_from` holds that
//!   peer's device id and the triggers record it as the row's `origin`, so the change is
//!   never sent back to the device it came from.
//!
//! Nothing here touches `PRAGMA user_version`: the release app keeps its own migrations, and
//! the extra column/tables are ignored by builds without the `lan-sync` feature.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Oldest log rows are trimmed past this many, so a build that stops syncing does not grow
/// the table forever (a peer that is further behind gets a full resync).
pub const LOG_KEEP: i64 = 20_000;

pub fn ensure_schema(conn: &Connection) -> rusqlite::Result<()> {
    // ADD COLUMN fails if it already exists; that is the "already migrated" case.
    let _ = conn.execute(
        "ALTER TABLE clipboard_entries ADD COLUMN entry_uuid TEXT",
        [],
    );
    conn.execute_batch(&format!(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_entries_uuid ON clipboard_entries(entry_uuid);

        CREATE TABLE IF NOT EXISTS sync_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);

        CREATE TABLE IF NOT EXISTS sync_log (
            seq        INTEGER PRIMARY KEY AUTOINCREMENT,
            entry_uuid TEXT NOT NULL,
            op         TEXT NOT NULL,            -- 'upsert' | 'delete'
            origin     TEXT,                     -- NULL = this device, else peer device id
            at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );

        CREATE TABLE IF NOT EXISTS sync_peers (
            device_id     TEXT PRIMARY KEY,
            name          TEXT NOT NULL,
            os            TEXT NOT NULL,
            secret_hex    TEXT NOT NULL,         -- 32-byte pair secret from SPAKE2 pairing
            last_recv_seq INTEGER NOT NULL DEFAULT 0,
            last_seen     TEXT,
            paired_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );

        UPDATE clipboard_entries SET entry_uuid = lower(hex(randomblob(16))) WHERE entry_uuid IS NULL;

        CREATE TRIGGER IF NOT EXISTS sync_entry_insert AFTER INSERT ON clipboard_entries
        BEGIN
            UPDATE clipboard_entries SET entry_uuid = lower(hex(randomblob(16)))
                WHERE id = NEW.id AND entry_uuid IS NULL;
            INSERT INTO sync_log (entry_uuid, op, origin)
                SELECT entry_uuid, 'upsert', (SELECT value FROM sync_state WHERE key = 'applying_from')
                FROM clipboard_entries WHERE id = NEW.id;
            DELETE FROM sync_log WHERE seq <= (SELECT max(seq) FROM sync_log) - {LOG_KEEP};
        END;

        CREATE TRIGGER IF NOT EXISTS sync_entry_update
        AFTER UPDATE OF is_pinned, created_at ON clipboard_entries
        WHEN NEW.entry_uuid IS NOT NULL
        BEGIN
            INSERT INTO sync_log (entry_uuid, op, origin)
                VALUES (NEW.entry_uuid, 'upsert', (SELECT value FROM sync_state WHERE key = 'applying_from'));
        END;

        CREATE TRIGGER IF NOT EXISTS sync_entry_delete AFTER DELETE ON clipboard_entries
        WHEN OLD.entry_uuid IS NOT NULL
        BEGIN
            INSERT INTO sync_log (entry_uuid, op, origin)
                VALUES (OLD.entry_uuid, 'delete', (SELECT value FROM sync_state WHERE key = 'applying_from'));
        END;
        "#
    ))
}

/// One history entry as it travels between devices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub seq: i64,
    pub entry_uuid: String,
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<EntryData>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntryData {
    pub content_type: String,
    pub text_content: Option<String>,
    /// Base64 payloads exactly as stored in the DB (image_data / image_thumb columns).
    pub image_data: Option<String>,
    pub image_thumb: Option<String>,
    pub image_format: Option<String>,
    pub image_width: Option<i64>,
    pub image_height: Option<i64>,
    pub image_byte_size: Option<i64>,
    pub source_app: Option<String>,
    pub content_hash: String,
    pub char_count: Option<i64>,
    pub created_at: String,
    pub is_pinned: bool,
}

/// Images bigger than this stay local (base64 length, ~20 MB of PNG).
pub const MAX_IMAGE_B64: usize = 27_000_000;

/// Local changes after `since` that did not come from `peer`, oldest first, the latest
/// state of each entry only. Deleted entries come as op 'delete' with no payload.
pub fn changes_since(
    conn: &Connection,
    since: i64,
    peer: &str,
    limit: i64,
) -> rusqlite::Result<Vec<Change>> {
    let mut stmt = conn.prepare(
        "SELECT max(seq), entry_uuid FROM sync_log
         WHERE seq > ?1 AND (origin IS NULL OR origin != ?2)
         GROUP BY entry_uuid ORDER BY max(seq) LIMIT ?3",
    )?;
    let rows: Vec<(i64, String)> = stmt
        .query_map(params![since, peer, limit], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut out = Vec::with_capacity(rows.len());
    for (seq, uuid) in rows {
        let entry = load_entry(conn, &uuid)?;
        let entry = entry.filter(|e| {
            e.image_data
                .as_ref()
                .is_none_or(|d| d.len() <= MAX_IMAGE_B64)
        });
        let op = if entry.is_some() { "upsert" } else { "delete" };
        out.push(Change {
            seq,
            entry_uuid: uuid,
            op: op.to_owned(),
            entry,
        });
    }
    Ok(out)
}

fn load_entry(conn: &Connection, uuid: &str) -> rusqlite::Result<Option<EntryData>> {
    conn.query_row(
        "SELECT content_type, text_content, image_data, image_thumb, image_format, image_width,
                image_height, image_byte_size, source_app, content_hash, char_count, created_at,
                COALESCE(is_pinned, 0)
         FROM clipboard_entries WHERE entry_uuid = ?1",
        params![uuid],
        |r| {
            Ok(EntryData {
                content_type: r.get(0)?,
                text_content: r.get(1)?,
                image_data: blob_text(r.get_ref(2)?),
                image_thumb: blob_text(r.get_ref(3)?),
                image_format: r.get(4)?,
                image_width: r.get(5)?,
                image_height: r.get(6)?,
                image_byte_size: r.get(7)?,
                source_app: r.get(8)?,
                content_hash: r.get(9)?,
                char_count: r.get(10)?,
                created_at: r.get(11)?,
                is_pinned: r.get::<_, i64>(12)? != 0,
            })
        },
    )
    .optional()
}

/// image columns are declared BLOB but hold base64 text; accept either storage class.
fn blob_text(v: rusqlite::types::ValueRef<'_>) -> Option<String> {
    match v {
        rusqlite::types::ValueRef::Text(t) | rusqlite::types::ValueRef::Blob(t) => {
            Some(String::from_utf8_lossy(t).into_owned())
        }
        _ => None,
    }
}

/// Apply a batch from `peer` in one transaction. Returns how many entries changed locally.
///
/// Rules: an entry is matched by uuid, or else by content hash (the same text copied on both
/// machines before they were paired), in which case it adopts the incoming uuid. Newer
/// `created_at` wins for time; pinned on either side stays pinned. Deletes remove the entry
/// unless it is pinned here.
pub fn apply_changes(
    conn: &mut Connection,
    peer: &str,
    changes: &[Change],
) -> rusqlite::Result<usize> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT OR REPLACE INTO sync_state (key, value) VALUES ('applying_from', ?1)",
        params![peer],
    )?;
    let mut changed = 0;
    for c in changes {
        match (&*c.op, &c.entry) {
            ("delete", _) => {
                changed += tx.execute(
                    "DELETE FROM clipboard_entries WHERE entry_uuid = ?1 AND COALESCE(is_pinned, 0) = 0",
                    params![c.entry_uuid],
                )?;
            }
            ("upsert", Some(e)) => {
                let local: Option<(i64, String, i64)> = tx
                    .query_row(
                        "SELECT id, created_at, COALESCE(is_pinned, 0) FROM clipboard_entries
                         WHERE entry_uuid = ?1 OR content_hash = ?2
                         ORDER BY (entry_uuid = ?1) DESC LIMIT 1",
                        params![c.entry_uuid, e.content_hash],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )
                    .optional()?;
                match local {
                    Some((id, created_at, pinned)) => {
                        let newest = if e.created_at > created_at {
                            &e.created_at
                        } else {
                            &created_at
                        };
                        let pin = i64::from(e.is_pinned || pinned != 0);
                        changed += tx.execute(
                            "UPDATE clipboard_entries SET entry_uuid = ?1, created_at = ?2, is_pinned = ?3
                             WHERE id = ?4 AND (entry_uuid IS NOT ?1 OR created_at != ?2 OR COALESCE(is_pinned, 0) != ?3)",
                            params![c.entry_uuid, newest, pin, id],
                        )?;
                    }
                    None => {
                        changed += tx.execute(
                            "INSERT INTO clipboard_entries (entry_uuid, content_type, text_content, text_content_search,
                                image_data, image_thumb, image_format, image_width, image_height, image_byte_size,
                                source_app, content_hash, char_count, created_at, is_pinned)
                             VALUES (?1, ?2, ?3, lower(?3), ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                            params![
                                c.entry_uuid, e.content_type, e.text_content, e.image_data, e.image_thumb,
                                e.image_format, e.image_width, e.image_height, e.image_byte_size,
                                e.source_app, e.content_hash, e.char_count, e.created_at, i64::from(e.is_pinned)
                            ],
                        )?;
                    }
                }
            }
            _ => {}
        }
    }
    tx.execute("DELETE FROM sync_state WHERE key = 'applying_from'", [])?;
    tx.commit()?;
    Ok(changed)
}

pub fn max_seq(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COALESCE(max(seq), 0) FROM sync_log", [], |r| {
        r.get(0)
    })
}

pub fn get_state(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM sync_state WHERE key = ?1",
        params![key],
        |r| r.get(0),
    )
    .optional()
}

pub fn set_state(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO sync_state (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub device_id: String,
    pub name: String,
    pub os: String,
    #[serde(skip)]
    pub secret_hex: String,
    pub last_recv_seq: i64,
    pub last_seen: Option<String>,
}

pub fn peers(conn: &Connection) -> rusqlite::Result<Vec<Peer>> {
    let mut stmt = conn.prepare(
        "SELECT device_id, name, os, secret_hex, last_recv_seq, last_seen FROM sync_peers ORDER BY paired_at",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Peer {
            device_id: r.get(0)?,
            name: r.get(1)?,
            os: r.get(2)?,
            secret_hex: r.get(3)?,
            last_recv_seq: r.get(4)?,
            last_seen: r.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn save_peer(
    conn: &Connection,
    device_id: &str,
    name: &str,
    os: &str,
    secret_hex: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO sync_peers (device_id, name, os, secret_hex) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(device_id) DO UPDATE SET name = ?2, os = ?3, secret_hex = ?4, last_recv_seq = 0",
        params![device_id, name, os, secret_hex],
    )?;
    Ok(())
}

pub fn remove_peer(conn: &Connection, device_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM sync_peers WHERE device_id = ?1",
        params![device_id],
    )?;
    Ok(())
}

pub fn mark_received(conn: &Connection, device_id: &str, upto_seq: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sync_peers SET last_recv_seq = max(last_recv_seq, ?2),
             last_seen = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE device_id = ?1",
        params![device_id, upto_seq],
    )?;
    Ok(())
}

pub fn touch_peer(conn: &Connection, device_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sync_peers SET last_seen = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE device_id = ?1",
        params![device_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The part of the real schema the sync code reads and writes.
    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE clipboard_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT, content_type TEXT NOT NULL DEFAULT 'text',
                text_content TEXT, image_data BLOB, image_thumb BLOB, source_app TEXT,
                source_app_icon BLOB, content_hash TEXT NOT NULL, char_count INTEGER,
                created_at TEXT NOT NULL, is_pinned INTEGER DEFAULT 0, collection_id INTEGER,
                text_content_search TEXT, ocr_text TEXT, image_format TEXT, image_width INTEGER,
                image_height INTEGER, image_byte_size INTEGER);",
        )
        .unwrap();
        conn
    }

    fn add(conn: &Connection, text: &str, at: &str) {
        conn.execute(
            "INSERT INTO clipboard_entries (text_content, content_hash, created_at) VALUES (?1, ?2, ?3)",
            params![text, format!("h-{text}"), at],
        )
        .unwrap();
    }

    #[test]
    fn existing_rows_get_uuids_and_new_rows_are_logged() {
        let conn = db();
        add(&conn, "before", "2026-10-11T10:00:00Z");
        ensure_schema(&conn).unwrap();
        ensure_schema(&conn).unwrap(); // idempotent
        let missing: i64 = conn
            .query_row(
                "SELECT count(*) FROM clipboard_entries WHERE entry_uuid IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(missing, 0);
        add(&conn, "after", "2026-10-11T10:01:00Z");
        let ch = changes_since(&conn, 0, "peer", 100).unwrap();
        assert_eq!(ch.len(), 1, "the backfill is not a change, the new copy is");
        assert_eq!(
            ch[0].entry.as_ref().unwrap().text_content.as_deref(),
            Some("after")
        );
    }

    #[test]
    fn two_devices_converge_and_do_not_echo() {
        let (mut a, mut b) = (db(), db());
        ensure_schema(&a).unwrap();
        ensure_schema(&b).unwrap();
        add(&a, "from a", "2026-10-11T10:00:00Z");
        add(&b, "from b", "2026-10-11T10:00:05Z");

        let a_out = changes_since(&a, 0, "B", 100).unwrap();
        assert_eq!(apply_changes(&mut b, "A", &a_out).unwrap(), 1);
        let b_out = changes_since(&b, 0, "A", 100).unwrap();
        assert_eq!(b_out.len(), 1, "what came from A is not sent back to A");
        assert_eq!(
            b_out[0].entry.as_ref().unwrap().text_content.as_deref(),
            Some("from b")
        );
        apply_changes(&mut a, "B", &b_out).unwrap();

        let count = |c: &Connection| -> i64 {
            c.query_row("SELECT count(*) FROM clipboard_entries", [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!((count(&a), count(&b)), (2, 2));
    }

    #[test]
    fn same_text_copied_on_both_sides_is_merged_by_hash() {
        let (a, mut b) = (db(), db());
        ensure_schema(&a).unwrap();
        ensure_schema(&b).unwrap();
        add(&a, "same", "2026-10-11T10:00:09Z");
        add(&b, "same", "2026-10-11T10:00:01Z");
        apply_changes(&mut b, "A", &changes_since(&a, 0, "B", 100).unwrap()).unwrap();
        let (n, at): (i64, String) = b
            .query_row(
                "SELECT count(*), max(created_at) FROM clipboard_entries",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(n, 1);
        assert_eq!(at, "2026-10-11T10:00:09Z", "newer copy time wins");
    }

    #[test]
    fn deletes_replicate_but_spare_pinned_entries() {
        let (a, mut b) = (db(), db());
        ensure_schema(&a).unwrap();
        ensure_schema(&b).unwrap();
        add(&a, "x", "2026-10-11T10:00:00Z");
        add(&a, "keep", "2026-10-11T10:00:01Z");
        apply_changes(&mut b, "A", &changes_since(&a, 0, "B", 100).unwrap()).unwrap();
        let seq = max_seq(&a).unwrap();
        b.execute(
            "UPDATE clipboard_entries SET is_pinned = 1 WHERE text_content = 'keep'",
            [],
        )
        .unwrap();
        a.execute("DELETE FROM clipboard_entries", []).unwrap();
        let dels = changes_since(&a, seq, "B", 100).unwrap();
        assert!(dels.iter().all(|c| c.op == "delete"));
        apply_changes(&mut b, "A", &dels).unwrap();
        let left: Vec<String> = b
            .prepare("SELECT text_content FROM clipboard_entries")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(left, vec!["keep".to_owned()]);
    }
}
