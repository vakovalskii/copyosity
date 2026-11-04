//! Clipboard tagging via the NeuralDeep hub (cloud only).
//!
//! Every automatic upload goes through [`upload_guard`] first: text that looks
//! like a credential / secret / one-time code is never sent to the hub — it is
//! tagged locally with [`heuristic_tags`] (or left untagged) instead.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::db::{AppSettings, ClipboardEntry, Database, EntryTaggedPayload};

/// Hub multimodal model used for image classification (matches capture pipeline).
pub const HUB_IMAGE_TAG_MODEL: &str = "qwen3.6-35b-a3b";

/// Hub tagging is configured when the master switch, tagging toggle, and token are set.
pub(crate) fn hub_tagging_configured(s: &AppSettings) -> bool {
    s.hub_enabled && s.hub_tagging_enabled && !s.hub_token.trim().is_empty()
}

/// Hub text tagging (auto-tag / retag on text) needs a chat model in addition to hub tagging.
pub(crate) fn hub_text_tagging_ready(s: &AppSettings) -> bool {
    hub_tagging_configured(s) && !s.hub_chat_model.trim().is_empty()
}

/// Retag (and the overlay's retag affordance) is available when hub tagging is configured.
pub fn is_retag_ready(db: &Database) -> bool {
    db.get_app_settings()
        .map(|s| hub_tagging_configured(&s))
        .unwrap_or(false)
}

/// Text clipboard capture: hub tagging must be configured with a chat model.
pub fn should_auto_tag_text_on_capture(db: &Database) -> bool {
    db.get_app_settings()
        .map(|s| hub_text_tagging_ready(&s))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Upload guard
// ---------------------------------------------------------------------------

/// Whether a piece of clipboard text may be sent to the hub for tagging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadGuard {
    /// Looks like a credential / secret / opaque code: tag locally only.
    Blocked,
    /// Ordinary content: eligible for cloud tagging.
    Eligible,
}

/// Classify `text` before any automatic hub upload. Heuristic, not a guarantee.
pub fn upload_guard(text: &str) -> UploadGuard {
    if looks_like_secret(text) {
        UploadGuard::Blocked
    } else {
        UploadGuard::Eligible
    }
}

const API_KEY_PREFIXES: &[&str] = &[
    "sk-",
    "sk_live_",
    "sk_test_",
    "rk_live_",
    "pk_live_",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    concat!("gl", "pat-"),
    "xoxa-",
    "xoxb-",
    "xoxp-",
    "xoxr-",
    "xoxs-",
    "xapp-",
    "AKIA",
    "ASIA",
    "AIza",
    "hf_",
    "npm_",
];

/// Explicit credential keys: any literal value assigned to one of these blocks.
const SECRET_KEYS: &[&str] = &[
    "password",
    "passwd",
    "pwd",
    "passphrase",
    "secret",
    "token",
    "apikey",
    "api_key",
    "api-key",
    "access_key",
    "private_key",
];

/// Start of a PEM block (`-----BEGIN … KEY-----`).
const PEM_MARKER: &str = "-----BEGIN ";

fn looks_like_secret(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    if looks_like_opaque_code(trimmed) || trimmed.contains(PEM_MARKER) {
        return true;
    }
    if contains_secret_assignment(trimmed) || contains_auth_header_token(trimmed) {
        return true;
    }
    // A whole clipboard that is one long random-looking token.
    if !trimmed.contains(char::is_whitespace) && is_high_entropy_token(trimmed) {
        return true;
    }
    // Credentials embedded anywhere in the text, incl. `NAME=value` pieces.
    candidate_tokens(trimmed).any(|token| {
        is_api_key_token(token) || is_jwt_token(token) || is_embedded_random_token(token)
    })
}

fn candidate_tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '"' | '\''
                    | '`'
                    | ','
                    | ';'
                    | '('
                    | ')'
                    | '<'
                    | '>'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '='
                    | ':'
                    | '&'
                    | '?'
            )
    })
    .filter(|t| !t.is_empty())
}

/// `sk-…`, `ghp_…`, `xoxb-…`, `AKIA…` etc. with a long enough body.
fn is_api_key_token(token: &str) -> bool {
    let token = token.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    if token.len() < 20 {
        return false;
    }
    API_KEY_PREFIXES.iter().any(|prefix| {
        token.starts_with(prefix)
            && token.len() >= prefix.len() + 10
            && token[prefix.len()..]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    })
}

/// `eyJ<header>.<payload>.<signature>` (base64url segments).
fn is_jwt_token(token: &str) -> bool {
    if !token.starts_with("eyJ") {
        return false;
    }
    let parts: Vec<&str> = token.split('.').collect();
    parts.len() == 3
        && parts[0].len() >= 10
        && parts[1].len() >= 10
        && parts.iter().all(|p| {
            p.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '='))
        })
}

/// `Authorization: Bearer <token>` / `Basic <token>` headers. With an explicit
/// `Authorization` header any credential blocks, regardless of length or entropy
/// (`Basic dXNlcjpwYXNz` is just `user:pass`). A bare `Bearer`/`Basic` word pair
/// without the header blocks when the token is credential-shaped (≥ 16 token chars).
fn contains_auth_header_token(text: &str) -> bool {
    // Header syntax only (`Authorization: …`, `"Authorization": "…"`, `authorization=…`),
    // not the word in prose ("sent in the Authorization header").
    let lower = text.to_ascii_lowercase();
    let explicit_header = [
        "authorization:",
        "authorization\":",
        "authorization=",
        "authorization':",
    ]
    .iter()
    .any(|h| lower.contains(h));
    // JSON / minified JSON: `{"Authorization":"Basic …"}` — treat structural punctuation as
    // whitespace so the scheme and its token split into words.
    let spaced: String = text
        .chars()
        .map(|c| {
            if matches!(c, '"' | '{' | '}' | '[' | ']' | ',') {
                ' '
            } else {
                c
            }
        })
        .collect();
    let words: Vec<&str> = spaced.split_whitespace().collect();
    words.windows(2).any(|pair| {
        let scheme = pair[0]
            .trim_matches(|c: char| !c.is_ascii_alphanumeric())
            .to_ascii_lowercase();
        if scheme != "bearer" && scheme != "basic" {
            return false;
        }
        let token = pair[1].trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | ';' | ')'));
        let token_chars = !token.is_empty()
            && token.chars().all(|c| {
                c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~' | '+' | '/' | '=')
            });
        token_chars && (explicit_header || token.chars().count() >= 16)
    })
}

/// `password=hunter2`, `"api_key": "…"`, `DB_TOKEN=…`, `secret: s3cr3t` — an
/// explicit credential key followed by a literal value (any strength). Code
/// such as `token = getToken()`, `token: String` or
/// `const token = process.env.API_TOKEN;` is not a literal and is allowed.
fn contains_secret_assignment(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let looks_like_code = [
        "let ", "const ", "var ", "self.", "this.", "fn ", "def ", "{",
    ]
    .iter()
    .any(|m| lower.contains(m))
        // A trailing `;` marks code — except shell lines (`export DB_PASSWORD=a.b.c;`).
        || (lower.contains(';') && !lower.trim_start().starts_with("export "));
    for key in SECRET_KEYS {
        let mut start = 0;
        while let Some(pos) = lower[start..].find(key) {
            let at = start + pos;
            start = at + key.len();
            let mut i = at + key.len();
            // Reject longer identifiers like `tokenizer` / `token_type`.
            if i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                continue;
            }
            // Optional closing quote of a JSON/YAML key, then spaces.
            if i < bytes.len() && matches!(bytes[i], b'"' | b'\'') {
                i += 1;
            }
            let space_before = i < bytes.len() && bytes[i] == b' ';
            while i < bytes.len() && bytes[i] == b' ' {
                i += 1;
            }
            if i >= bytes.len() {
                continue;
            }
            let sep = match bytes[i] {
                b':' if bytes.get(i + 1) == Some(&b'=') => {
                    i += 1;
                    b'='
                }
                b @ (b'=' | b':') => b,
                _ => continue,
            };
            i += 1;
            // Comparisons / paths / arrows: `==`, `=>`, `::`.
            if i < bytes.len()
                && ((sep == b'=' && matches!(bytes[i], b'=' | b'>'))
                    || (sep == b':' && bytes[i] == b':'))
            {
                continue;
            }
            let space_after = i < bytes.len() && bytes[i] == b' ';
            // Use the original text for the value (indices match: ASCII lowercase).
            let rest = text[i..].trim_start_matches([' ', '\t']);
            if literal_value_follows(rest, sep, space_before && space_after, looks_like_code) {
                return true;
            }
        }
    }
    false
}

fn literal_value_follows(rest: &str, sep: u8, spaced: bool, looks_like_code: bool) -> bool {
    let Some(first) = rest.chars().next() else {
        return false;
    };
    if matches!(first, '"' | '\'' | '`') {
        // Quoted literal: the full quoted value counts, whatever it contains.
        let inner = &rest[first.len_utf8()..];
        let value = inner.split(first).next().unwrap_or("");
        let value = value.lines().next().unwrap_or("");
        return !value.trim().is_empty();
    }
    let raw: &str = rest
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | ')' | '}' | ']'))
        .next()
        .unwrap_or("");
    if raw.is_empty() {
        return false;
    }
    // Placeholders / env references: `$TOKEN`, `${TOKEN}`, `<your-token>`, `{{token}}`.
    if raw.starts_with(['$', '%', '<', '{']) {
        return false;
    }
    if matches!(
        raw.to_ascii_lowercase().as_str(),
        "none" | "null" | "nil" | "undefined" | "true" | "false"
    ) {
        return false;
    }
    // Member expressions only count as code in a code context or under a known env/config
    // root — `DB_PASSWORD=correct.horse.battery.staple` in a .env file is a literal.
    if is_code_expression(raw) && (looks_like_code || raw.contains('(') || has_code_root(raw)) {
        return false;
    }
    if sep == b':' {
        if is_type_annotation(raw) {
            return false;
        }
        // Prose like "The secret: we ship on Monday" — a YAML-style value is
        // the last thing on its line.
        let line_rest = rest[raw.len()..].lines().next().unwrap_or("").trim();
        // A trailing YAML comment (`password: hunter2 # prod`) is not prose.
        if !line_rest.is_empty() && !line_rest.starts_with('#') {
            return false;
        }
    }
    // `token = otherVar;` in code is a variable reference, not a literal.
    if sep == b'=' && spaced && looks_like_code && is_identifier(raw) {
        return false;
    }
    true
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// `process.env.X`, `os.environ…`, `config.x` — references, not literal values.
fn has_code_root(raw: &str) -> bool {
    const ROOTS: &[&str] = &[
        "process.env.",
        "os.environ",
        "env.",
        "config.",
        "settings.",
        "self.",
        "this.",
        "import.meta.env.",
    ];
    let lower = raw.to_ascii_lowercase();
    ROOTS.iter().any(|root| lower.starts_with(root))
}

/// Function calls and member expressions: `getToken(`, `process.env.API_TOKEN`.
fn is_code_expression(raw: &str) -> bool {
    if raw.contains('(') {
        return true;
    }
    raw.contains('.') && raw.split('.').all(is_identifier)
}

fn is_type_annotation(raw: &str) -> bool {
    const PRIMITIVES: &[&str] = &[
        "str", "string", "number", "boolean", "bool", "any", "unknown", "int", "float", "i32",
        "i64", "u32", "u64", "usize", "f64", "bytes",
    ];
    let raw = raw.trim_end_matches(['?', '!']);
    if raw.contains('<') || raw.starts_with('&') || raw.ends_with("[]") {
        return true;
    }
    if PRIMITIVES.contains(&raw) {
        return true;
    }
    // `String`, `SecretString`, `Token` — capitalised type names without digits.
    raw.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && raw.chars().all(|c| c.is_ascii_alphabetic())
}

/// A long single token (≥ 20 chars) that mixes letters and digits and looks
/// random. URLs, e-mails, file names and paths (anything with `.`) are excluded.
fn is_high_entropy_token(token: &str) -> bool {
    let len = token.chars().count();
    if len < 20 || token.contains('.') || token.contains("://") || token.contains('@') {
        return false;
    }
    if !token
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '/' | '='))
    {
        return false;
    }
    let digits = token.chars().filter(|c| c.is_ascii_digit()).count();
    let letters = token.chars().filter(|c| c.is_ascii_alphabetic()).count();
    if digits < 2 || letters < 2 {
        return false;
    }
    shannon_entropy(token) >= 3.5
}

/// Stricter check for tokens inside longer text (so git SHAs, UUIDs and long
/// identifiers in prose or code stay eligible): mixed case, several digits,
/// high entropy.
fn is_embedded_random_token(token: &str) -> bool {
    if !is_high_entropy_token(token) {
        return false;
    }
    let digits = token.chars().filter(|c| c.is_ascii_digit()).count();
    let upper = token.chars().any(|c| c.is_ascii_uppercase());
    let lower = token.chars().any(|c| c.is_ascii_lowercase());
    digits >= 3 && upper && lower && shannon_entropy(token) >= 4.0
}

fn shannon_entropy(s: &str) -> f64 {
    let mut counts = std::collections::HashMap::new();
    let mut total = 0usize;
    for c in s.chars() {
        *counts.entry(c).or_insert(0usize) += 1;
        total += 1;
    }
    if total == 0 {
        return 0.0;
    }
    counts
        .values()
        .map(|&n| {
            let p = n as f64 / total as f64;
            -p * p.log2()
        })
        .sum()
}

// ---------------------------------------------------------------------------
// Local heuristics
// ---------------------------------------------------------------------------

pub(crate) fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();

    for tag in tags {
        let cleaned = tag
            .trim()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>();

        if cleaned.is_empty() || cleaned.len() > 24 {
            continue;
        }

        if !normalized.contains(&cleaned) {
            normalized.push(cleaned);
        }

        if normalized.len() >= 5 {
            break;
        }
    }

    normalized
}

fn heuristic_tags(text: &str) -> Option<Vec<String>> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains(char::is_whitespace) {
        return None;
    }

    let len = trimmed.chars().count();
    let digits_only = trimmed.chars().all(|ch| ch.is_ascii_digit());
    if digits_only && (4..=8).contains(&len) {
        return Some(vec!["otp".to_owned()]);
    }

    let ascii_only = trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '+' | '/' | '='));
    let has_uppercase = trimmed.chars().any(|ch| ch.is_ascii_uppercase());
    let has_digits = trimmed.chars().any(|ch| ch.is_ascii_digit());
    let has_dash = trimmed.contains('-') || trimmed.contains('_');

    if ascii_only && has_uppercase && has_digits && has_dash && (6..=20).contains(&len) {
        return Some(vec!["code".to_owned()]);
    }

    if ascii_only
        && has_digits
        && (trimmed.contains('+') || trimmed.contains('/') || trimmed.contains('='))
    {
        return Some(vec!["token".to_owned()]);
    }

    None
}

fn looks_like_opaque_code(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains(char::is_whitespace) {
        return false;
    }

    let len = trimmed.chars().count();
    if !(1..=32).contains(&len) {
        return false;
    }

    let mut alnum = 0usize;
    let mut uppercase = 0usize;
    let mut digits = 0usize;
    let mut separators = 0usize;
    let mut other = 0usize;

    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() {
            alnum += 1;
            if ch.is_ascii_uppercase() {
                uppercase += 1;
            }
            if ch.is_ascii_digit() {
                digits += 1;
            }
        } else if matches!(ch, '-' | '_' | ':' | '/') {
            separators += 1;
        } else {
            other += 1;
        }
    }

    if other > 0 || alnum == 0 {
        return false;
    }

    let strong_code_shape =
        digits >= len.saturating_div(2) || uppercase + digits >= len.saturating_sub(separators);
    strong_code_shape && (digits > 0 || uppercase > 0)
}

// ---------------------------------------------------------------------------
// Tagging entry points
// ---------------------------------------------------------------------------

/// Result of one tagging attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagOutcome {
    /// Tags to store (from the hub, or local heuristics for blocked text).
    Tags(Vec<String>),
    /// Nothing meaningful to tag — mark the entry `skipped`.
    Empty,
    /// Transient failure (hub unreachable / not configured) — retry later.
    Failed,
}

impl TagOutcome {
    pub fn into_tags(self) -> Option<Vec<String>> {
        match self {
            TagOutcome::Tags(tags) => Some(tags),
            _ => None,
        }
    }
}

fn local_outcome(text: &str) -> TagOutcome {
    match heuristic_tags(text) {
        Some(tags) if !tags.is_empty() => TagOutcome::Tags(tags),
        _ => TagOutcome::Empty,
    }
}

/// Tag clipboard `text`. Secrets are tagged locally and never uploaded.
pub fn tag_text_outcome(settings: &AppSettings, text: &str) -> TagOutcome {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return TagOutcome::Empty;
    }
    if upload_guard(trimmed) == UploadGuard::Blocked {
        return local_outcome(trimmed);
    }
    if !hub_text_tagging_ready(settings) {
        return TagOutcome::Failed;
    }
    match crate::hub::tag_text(
        &settings.hub_url,
        &settings.hub_token,
        &settings.hub_chat_model,
        trimmed,
    ) {
        Ok(tags) if tags.is_empty() => TagOutcome::Empty,
        Ok(tags) => TagOutcome::Tags(tags),
        Err(_) => TagOutcome::Failed,
    }
}

/// Convenience wrapper reading current settings.
pub fn tag(db: &Database, text: &str) -> TagOutcome {
    match db.get_app_settings() {
        Ok(s) => tag_text_outcome(&s, text),
        Err(_) => TagOutcome::Failed,
    }
}

/// OCR status of an image at tagging time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageOcr<'a> {
    /// OCR completed successfully; the text may be empty (no text in the image).
    Done(&'a str),
    /// OCR is still pending or failed: the image must not be uploaded.
    Unavailable,
}

/// Run on-device OCR on a base64 image and report the status.
pub fn ocr_b64(image_b64: &str) -> Result<String, ()> {
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, image_b64)
        .map_err(|_| ())?;
    crate::ocr::ocr_image_png_result(&bytes)
}

/// Tag an image: hub vision on the thumbnail, then hub text tagging on OCR
/// text. The image is uploaded only after OCR has completed and its text
/// passed the upload guard; blocked OCR text is tagged locally instead.
pub fn tag_image(db: &Database, image_b64: &str, ocr: ImageOcr<'_>) -> Option<Vec<String>> {
    let settings = db.get_app_settings().ok()?;
    tag_image_with(
        &settings,
        image_b64,
        ocr,
        |s, b64| crate::hub::tag_image(&s.hub_url, &s.hub_token, HUB_IMAGE_TAG_MODEL, b64),
        // The vision request can take a while: re-read settings before the OCR-text fallback
        // so an opt-out (or cleared token) in the meantime stops the second upload.
        |_, text| match db.get_app_settings() {
            Ok(fresh) => tag_text_outcome(&fresh, text),
            Err(_) => TagOutcome::Failed,
        },
    )
}

fn tag_image_with(
    settings: &AppSettings,
    image_b64: &str,
    ocr: ImageOcr<'_>,
    upload_image: impl FnOnce(&AppSettings, &str) -> Option<Vec<String>>,
    tag_text: impl FnOnce(&AppSettings, &str) -> TagOutcome,
) -> Option<Vec<String>> {
    if !hub_tagging_configured(settings) {
        return None;
    }
    let ImageOcr::Done(ocr_text) = ocr else {
        return None;
    };
    let ocr_text = Some(ocr_text.trim()).filter(|t| !t.is_empty());
    if let Some(text) = ocr_text {
        if upload_guard(text) == UploadGuard::Blocked {
            return local_outcome(text).into_tags();
        }
    }
    if !image_b64.is_empty() {
        if let Some(tags) = upload_image(settings, image_b64) {
            return Some(tags);
        }
    }
    ocr_text.and_then(|text| tag_text(settings, text).into_tags())
}

/// Retag an existing image entry. Uses stored OCR text when present;
/// otherwise runs OCR synchronously first (and stores the text). If OCR
/// fails, nothing is uploaded.
pub fn tag_image_entry(db: &Database, entry: &ClipboardEntry) -> Option<Vec<String>> {
    let image_b64 = entry
        .image_thumb
        .as_deref()
        .or(entry.image_data.as_deref())
        .unwrap_or("");
    if let Some(text) = entry.ocr_text.as_deref().filter(|t| !t.trim().is_empty()) {
        return tag_image(db, image_b64, ImageOcr::Done(text));
    }
    let full = entry.image_data.as_deref().unwrap_or(image_b64);
    match ocr_b64(full) {
        Ok(text) => {
            let text = text.trim().to_string();
            if !text.is_empty() {
                let _ = db.set_ocr_text(entry.id, &text);
            }
            tag_image(db, image_b64, ImageOcr::Done(&text))
        }
        Err(()) => None,
    }
}

// ---------------------------------------------------------------------------
// Backfill of existing untagged text entries
// ---------------------------------------------------------------------------

/// Serializes backfill runs: one at a time; a request during a run schedules
/// exactly one more run after it.
struct BackfillGate {
    running: AtomicBool,
    rerun: AtomicBool,
}

impl BackfillGate {
    const fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            rerun: AtomicBool::new(false),
        }
    }

    /// Returns true when the caller must start a run.
    fn request(&self) -> bool {
        self.rerun.store(true, Ordering::SeqCst);
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            self.rerun.store(false, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    /// Call after a run. Returns true when the runner must run again.
    fn finish(&self) -> bool {
        self.running.store(false, Ordering::SeqCst);
        self.rerun.swap(false, Ordering::SeqCst)
            && self
                .running
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
    }
}

static BACKFILL_GATE: BackfillGate = BackfillGate::new();

/// Stop a backfill run after this many consecutive hub failures (hub down).
const BACKFILL_MAX_CONSECUTIVE_FAILURES: usize = 3;
const BACKFILL_BATCH: i64 = 24;

/// Tag old text entries that have no tags yet. No-op unless hub tagging is
/// configured; runs are serialized. Entries with tags are never touched;
/// transient failures leave the entry without state so a later run retries it.
pub fn spawn_backfill_if_configured(app: AppHandle, db: Arc<Database>) {
    let ready = db
        .get_app_settings()
        .map(|s| hub_text_tagging_ready(&s))
        .unwrap_or(false);
    if !ready || !BACKFILL_GATE.request() {
        return;
    }
    std::thread::spawn(move || loop {
        run_backfill(&db, tag_text_outcome, |entry_id, tags| {
            let _ = app.emit_to(
                "main",
                "entry-tagged",
                EntryTaggedPayload {
                    entry_id,
                    tags: tags.to_vec(),
                },
            );
        });
        if !BACKFILL_GATE.finish() {
            break;
        }
    });
}

/// Synchronous backfill body. Settings are re-read before every request so
/// an opt-out or credential change takes effect immediately.
fn run_backfill(
    db: &Database,
    mut tagger: impl FnMut(&AppSettings, &str) -> TagOutcome,
    mut on_tagged: impl FnMut(i64, &[String]),
) {
    let mut before_id: Option<i64> = None;
    let mut consecutive_failures = 0usize;
    loop {
        let batch = match db.get_untagged_text_entries(BACKFILL_BATCH, before_id) {
            Ok(entries) => entries,
            Err(err) => {
                eprintln!("copyosity[tagging]: failed to load untagged entries: {err}");
                return;
            }
        };
        if batch.is_empty() {
            return;
        }
        for (entry_id, text) in batch {
            before_id = Some(entry_id);
            let Ok(settings) = db.get_app_settings() else {
                return;
            };
            if !hub_text_tagging_ready(&settings) {
                return;
            }
            match tagger(&settings, &text) {
                TagOutcome::Tags(tags) => {
                    consecutive_failures = 0;
                    if let Ok(true) = db.set_entry_tags_if_untagged(entry_id, &tags) {
                        on_tagged(entry_id, &tags);
                    }
                }
                TagOutcome::Empty => {
                    consecutive_failures = 0;
                    let _ = db.mark_entry_skipped_if_untagged(entry_id);
                }
                TagOutcome::Failed => {
                    consecutive_failures += 1;
                    if consecutive_failures >= BACKFILL_MAX_CONSECUTIVE_FAILURES {
                        eprintln!("copyosity[tagging]: backfill paused: hub unavailable");
                        return;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- normalize_tags ---

    #[test]
    fn normalize_lowercases_and_strips() {
        let result = normalize_tags(vec!["  Rust ".to_owned(), "CODE".to_owned()]);
        assert_eq!(result, vec!["rust", "code"]);
    }

    #[test]
    fn normalize_deduplicates() {
        let result = normalize_tags(vec![
            "rust".to_owned(),
            "Rust".to_owned(),
            "RUST".to_owned(),
        ]);
        assert_eq!(result, vec!["rust"]);
    }

    #[test]
    fn normalize_limits_to_5() {
        let tags: Vec<String> = (0..10).map(|i| format!("tag{}", i)).collect();
        let result = normalize_tags(tags);
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn normalize_skips_empty_and_long() {
        let result = normalize_tags(vec![
            "".to_owned(),
            "a".to_owned(),
            "x".repeat(25), // too long
        ]);
        assert_eq!(result, vec!["a"]);
    }

    #[test]
    fn normalize_strips_special_chars() {
        let result = normalize_tags(vec!["hello world!".to_owned()]);
        assert_eq!(result, vec!["helloworld"]);
    }

    // --- heuristic_tags ---

    #[test]
    fn heuristic_otp_4_digits() {
        assert_eq!(heuristic_tags("1234"), Some(vec!["otp".to_owned()]));
    }

    #[test]
    fn heuristic_otp_6_digits() {
        assert_eq!(heuristic_tags("482917"), Some(vec!["otp".to_owned()]));
    }

    #[test]
    fn heuristic_not_otp_too_long() {
        assert_eq!(heuristic_tags("123456789"), None);
    }

    #[test]
    fn heuristic_code_pattern() {
        assert_eq!(heuristic_tags("AB3-XY7_Z"), Some(vec!["code".to_owned()]));
    }

    #[test]
    fn heuristic_token_with_base64() {
        assert_eq!(
            heuristic_tags("abc123+def/ghi="),
            Some(vec!["token".to_owned()])
        );
    }

    #[test]
    fn heuristic_none_for_words() {
        assert_eq!(heuristic_tags("hello world"), None);
    }

    #[test]
    fn heuristic_none_for_empty() {
        assert_eq!(heuristic_tags(""), None);
    }

    // --- looks_like_opaque_code ---

    #[test]
    fn opaque_code_numeric() {
        assert!(looks_like_opaque_code("482917"));
    }

    #[test]
    fn opaque_code_mixed() {
        assert!(looks_like_opaque_code("A3B7C9"));
    }

    #[test]
    fn opaque_not_words() {
        assert!(!looks_like_opaque_code("hello world"));
    }

    #[test]
    fn opaque_not_long_text() {
        assert!(!looks_like_opaque_code(&"x".repeat(33)));
    }

    #[test]
    fn opaque_not_empty() {
        assert!(!looks_like_opaque_code(""));
    }

    #[test]
    fn opaque_not_lowercase_only() {
        assert!(!looks_like_opaque_code("abcdef"));
    }

    // --- upload_guard: blocked ---

    fn blocked(text: &str) -> bool {
        upload_guard(text) == UploadGuard::Blocked
    }

    /// Credential-shaped fixtures are assembled at runtime so no contiguous
    /// secret-looking literal exists in the source (secret scanners).
    fn fake(prefix: &[&str], body: &str) -> String {
        format!("{}{}", prefix.concat(), body)
    }

    fn fake_openai() -> String {
        fake(&["s", "k-", "proj-"], "AbCdEf0123456789AbCdEf0123456789")
    }

    fn fake_jwt() -> String {
        let header = fake(&["ey", "J"], "hbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
        let payload = fake(&["ey", "J"], "zdWIiOiIxMjM0NTY3ODkwIn0");
        format!("{header}.{payload}.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U")
    }

    #[test]
    fn guard_blocks_otp_and_short_codes() {
        assert!(blocked("482917"));
        assert!(blocked("AB3-XY7_Z"));
    }

    #[test]
    fn guard_blocks_api_keys() {
        assert!(blocked(&fake_openai()));
        assert!(blocked(&fake(
            &["gh", "p_"],
            "1234567890abcdefghijABCDEFGHIJ123456"
        )));
        assert!(blocked(&fake(
            &["xo", "xb-"],
            "123456789012-1234567890123-AbCdEfGhIjKlMnOp"
        )));
        assert!(blocked(&fake(&["AK", "IA"], "IOSFODNN7EXAMPLE")));
        assert!(blocked(&fake(&["gl", "pat-"], "AbCdEf0123456789xyzQ")));
        assert!(blocked(&fake(
            &["github", "_pat_"],
            "11ABCDEFG0123456789_abcdefghijklmnop"
        )));
        assert!(blocked(&format!("export KEY {} then run", fake_openai())));
    }

    #[test]
    fn guard_blocks_prefixed_key_inside_assignment() {
        assert!(blocked(&format!("export OPENAI_KEY={}", fake_openai())));
        assert!(blocked(&format!(
            "OPENAI_API_KEY: {}\nMODEL=gpt",
            fake_openai()
        )));
    }

    #[test]
    fn guard_blocks_jwt() {
        assert!(blocked(&fake_jwt()));
        assert!(blocked(&format!("Authorization: Bearer {}", fake_jwt())));
    }

    #[test]
    fn guard_blocks_bearer_tokens() {
        assert!(blocked(
            "curl -H 'Authorization: Bearer q8Zt3Lp0Vx7Kc2Nw9Rb4Jm6Hd1Fs5Gy' https://api.example.com"
        ));
        assert!(blocked("Authorization: Basic dXNlcjpwYXNzd29yZDEyMw=="));
        // Short / digit-free credentials under an explicit header.
        assert!(blocked("Authorization: Basic dXNlcjpwYXNz"));
        assert!(blocked("Authorization: Bearer abcdefghijklmnop"));
        assert!(blocked("authorization: bearer abc"));
    }

    #[test]
    fn guard_blocks_json_shell_and_yaml_credentials() {
        assert!(blocked(r#"{"Authorization": "Basic dXNlcjpwYXNz"}"#));
        assert!(blocked(r#"{"Authorization":"Bearer abc"}"#));
        assert!(blocked("export DB_PASSWORD=correct.horse.battery.staple;"));
        assert!(blocked("password: hunter2 # production"));
    }

    #[test]
    fn guard_blocks_dotted_env_passwords() {
        assert!(blocked("DB_PASSWORD=correct.horse.battery.staple"));
        assert!(!blocked("const token = process.env.API_TOKEN;"));
        assert!(!blocked("token = config.api.token"));
    }

    #[test]
    fn guard_blocks_embedded_random_token() {
        assert!(blocked(
            "my key is q8Zt3Lp0Vx7Kc2Nw9Rb4Jm6Hd1Fs5Gy keep it safe"
        ));
    }

    #[test]
    fn guard_blocks_pem() {
        let pem = format!(
            "{}{}\nb3BlbnNzaC1rZXktdjEAAAAA\n-----END OPENSSH PRIVATE KEY-----",
            "-----BEGIN", " OPENSSH PRIVATE KEY-----"
        );
        assert!(blocked(&pem));
    }

    #[test]
    fn guard_blocks_secret_assignments() {
        assert!(blocked("password=hunter2"));
        assert!(blocked("DB_PASSWORD=s3cr3t!pass"));
        assert!(blocked("{\"api_key\": \"AbCdEf1234567890\"}"));
        assert!(blocked("login: admin\npassword: Tr0ub4dor&3"));
        assert!(blocked("https://example.com/cb?token=abc123def456"));
    }

    #[test]
    fn guard_blocks_weak_literal_credentials() {
        assert!(blocked("password=correcthorsebatterystaple"));
        assert!(blocked("password=12345"));
        assert!(blocked("password=\"abc def 123\""));
        assert!(blocked("pwd: hunter"));
        assert!(blocked("secret = 'letmein'"));
        assert!(blocked("api_key := abc"));
        assert!(blocked("[db]\npassword = hunter"));
    }

    #[test]
    fn guard_blocks_long_high_entropy_token() {
        assert!(blocked("q8Zt3Lp0Vx7Kc2Nw9Rb4Jm6Hd1Fs5Gy"));
        assert!(blocked("3f786850e387550fdab836ed7e6dc881de23001b"));
    }

    // --- upload_guard: eligible ---

    #[test]
    fn guard_allows_normal_sentences() {
        assert!(!blocked("Meeting moved to Thursday at 3pm, see you there."));
        assert!(!blocked("Встреча перенесена на четверг"));
        assert!(!blocked(
            "Please reset your password using the link we sent."
        ));
        assert!(!blocked("The secret: we ship on Monday"));
        assert!(!blocked(
            "Bearer tokens are sent in the Authorization header."
        ));
        assert!(!blocked(
            "Fixed in commit 3f786850e387550fdab836ed7e6dc881de23001b today"
        ));
    }

    #[test]
    fn guard_allows_urls_and_paths() {
        assert!(!blocked("https://github.com/vakovalskii/copyosity/pull/12"));
        assert!(!blocked("https://example.com/search?q=rust+tauri&page=2"));
        assert!(!blocked(
            "/Users/me/Projects/copyosity/src-tauri/src/tagging.rs"
        ));
        assert!(!blocked("john.doe1987@example.com"));
    }

    #[test]
    fn guard_allows_code_snippets() {
        assert!(!blocked("fn main() {\n    println!(\"hello\");\n}"));
        assert!(!blocked("const token = getToken();"));
        assert!(!blocked("const token = process.env.API_TOKEN;"));
        assert!(!blocked("token = self.token"));
        assert!(!blocked("let token = otherToken;"));
        assert!(!blocked("pub hub_token: String,"));
        assert!(!blocked("token: string;"));
        assert!(!blocked("let password: Option<String> = None;"));
        assert!(!blocked("if token == expected { return; }"));
        assert!(!blocked("use crate::token::Token;"));
        assert!(!blocked("const tokenizer = new Tokenizer(config);"));
        assert!(!blocked("headers: { Authorization: `Bearer ${token}` }"));
        assert!(!blocked("API_TOKEN=${API_TOKEN}"));
        assert!(!blocked("SELECT id, name FROM users WHERE active = 1;"));
    }

    #[test]
    fn guard_allows_plain_words() {
        assert!(!blocked("hello"));
        assert!(!blocked("copyosity"));
        assert!(!blocked("handleClipboardChange"));
    }

    // --- tagging outcomes ---

    fn hub_ready_settings(db: &Database) -> AppSettings {
        db.set_setting("hub_enabled", "true").unwrap();
        db.set_setting("hub_tagging_enabled", "true").unwrap();
        db.set_setting("hub_token", "tok").unwrap();
        db.set_setting("hub_chat_model", "m").unwrap();
        db.set_setting("hub_url", "http://127.0.0.1:9").unwrap();
        db.get_app_settings().unwrap()
    }

    #[test]
    fn blocked_text_is_tagged_locally_without_hub() {
        // Hub not configured: blocked text still gets local heuristic tags,
        // eligible text reports a transient failure (retry later).
        let db = crate::db::tests::test_db();
        let s = db.get_app_settings().unwrap();
        assert_eq!(
            tag_text_outcome(&s, "482917"),
            TagOutcome::Tags(vec!["otp".to_owned()])
        );
        assert_eq!(tag_text_outcome(&s, &fake_openai()), TagOutcome::Empty);
        assert_eq!(
            tag_text_outcome(&s, "Meeting notes for Thursday"),
            TagOutcome::Failed
        );
    }

    // --- images ---

    #[test]
    fn image_not_uploaded_while_ocr_pending_or_failed() {
        let db = crate::db::tests::test_db();
        let s = hub_ready_settings(&db);
        let mut uploads = 0;
        let tags = tag_image_with(
            &s,
            "iVBORw0KGgo",
            ImageOcr::Unavailable,
            |_, _| {
                uploads += 1;
                Some(vec!["photo".to_owned()])
            },
            |_, _| TagOutcome::Failed,
        );
        assert_eq!(tags, None);
        assert_eq!(uploads, 0);
    }

    #[test]
    fn image_with_blocked_ocr_text_is_never_uploaded() {
        let db = crate::db::tests::test_db();
        let s = hub_ready_settings(&db);
        let mut uploads = 0;
        let ocr = format!("Your API key\n{}", fake_openai());
        let tags = tag_image_with(
            &s,
            "iVBORw0KGgo",
            ImageOcr::Done(&ocr),
            |_, _| {
                uploads += 1;
                None
            },
            |_, _| panic!("blocked OCR text must not be uploaded"),
        );
        assert_eq!(tags, None);
        assert_eq!(uploads, 0);
    }

    #[test]
    fn image_uploaded_after_ocr_done_without_text() {
        let db = crate::db::tests::test_db();
        let s = hub_ready_settings(&db);
        let tags = tag_image_with(
            &s,
            "iVBORw0KGgo",
            ImageOcr::Done(""),
            |_, _| Some(vec!["photo".to_owned()]),
            |_, _| TagOutcome::Failed,
        );
        assert_eq!(tags, Some(vec!["photo".to_owned()]));
    }

    // --- backfill ---

    #[test]
    fn backfill_tags_blocked_locally_and_leaves_failures_for_retry() {
        let db = crate::db::tests::test_db();
        let _ = hub_ready_settings(&db);
        let otp = crate::db::tests::insert_text(&db, "482917", "h1");
        let secret = crate::db::tests::insert_text(&db, &fake_openai(), "h2");
        let plain = crate::db::tests::insert_text(&db, "Meeting notes", "h3");

        let mut tagged = Vec::new();
        // Real tagger against an unroutable hub: eligible uploads fail fast.
        run_backfill(&db, tag_text_outcome, |id, tags| {
            tagged.push((id, tags.to_vec()))
        });

        assert_eq!(tagged, vec![(otp, vec!["otp".to_owned()])]);
        let remaining: Vec<i64> = db
            .get_untagged_text_entries(50, None)
            .unwrap()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        // Secret got `skipped` (blocked, no local tags); plain text failed
        // transiently and stays eligible for the next run.
        assert_eq!(remaining, vec![plain]);
        assert!(!remaining.contains(&secret));
    }

    #[test]
    fn backfill_noop_when_hub_tagging_off() {
        let db = crate::db::tests::test_db();
        let id = crate::db::tests::insert_text(&db, "482917", "h1");
        let mut calls = 0;
        run_backfill(&db, tag_text_outcome, |_, _| calls += 1);
        assert_eq!(calls, 0);
        assert_eq!(db.get_untagged_text_entries(50, None).unwrap()[0].0, id);
    }

    #[test]
    fn backfill_stops_immediately_on_opt_out() {
        let db = crate::db::tests::test_db();
        let _ = hub_ready_settings(&db);
        for i in 0..5 {
            crate::db::tests::insert_text(&db, &format!("note {i}"), &format!("h{i}"));
        }
        let mut requests = 0;
        run_backfill(
            &db,
            |_, _| {
                requests += 1;
                // User turns cloud tagging off while the first request runs.
                db.set_setting("hub_tagging_enabled", "false").unwrap();
                TagOutcome::Tags(vec!["note".to_owned()])
            },
            |_, _| {},
        );
        assert_eq!(requests, 1);
    }

    #[test]
    fn backfill_uses_fresh_credentials_per_request() {
        let db = crate::db::tests::test_db();
        let _ = hub_ready_settings(&db);
        crate::db::tests::insert_text(&db, "first note", "h1");
        crate::db::tests::insert_text(&db, "second note", "h2");
        let mut tokens = Vec::new();
        run_backfill(
            &db,
            |s, _| {
                tokens.push(s.hub_token.clone());
                db.set_setting("hub_token", "rotated").unwrap();
                TagOutcome::Empty
            },
            |_, _| {},
        );
        assert_eq!(tokens, vec!["tok".to_owned(), "rotated".to_owned()]);
    }

    #[test]
    fn backfill_does_not_overwrite_concurrent_manual_retag() {
        let db = crate::db::tests::test_db();
        let _ = hub_ready_settings(&db);
        let id = crate::db::tests::insert_text(&db, "release notes", "h1");
        let mut emitted = Vec::new();
        run_backfill(
            &db,
            |_, _| {
                // A manual Retag lands while the hub request is in flight.
                db.set_entry_tags(id, &["manual".to_owned()]).unwrap();
                TagOutcome::Tags(vec!["backfill".to_owned()])
            },
            |id, _| emitted.push(id),
        );
        assert!(emitted.is_empty());
        assert_eq!(
            db.get_entry_by_id(id).unwrap().unwrap().tags,
            vec!["manual".to_owned()]
        );
    }

    #[test]
    fn backfill_gate_serializes_and_reruns_once() {
        let gate = BackfillGate::new();
        assert!(gate.request(), "idle gate starts a run");
        assert!(
            !gate.request(),
            "request during a run does not start another"
        );
        assert!(!gate.request(), "requests coalesce");
        assert!(gate.finish(), "one queued re-run");
        assert!(!gate.finish(), "nothing queued after the re-run");
        assert!(gate.request(), "idle again");
        assert!(!gate.finish());
    }
}
