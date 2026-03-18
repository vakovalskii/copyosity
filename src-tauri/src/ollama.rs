use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::db::{Database, ModelCatalog, ModelOption};

#[derive(Serialize, Clone)]
pub struct OllamaStatus {
    pub cli_installed: bool,
    pub server_running: bool,
    pub model_installed: bool,
    pub model_name: String,
}

pub fn check_status() -> OllamaStatus {
    let model = ollama_model();
    let cli = ollama_cli_available();
    let server = if cli { ollama_available() } else { false };
    let has_model = if server { model_installed(&model) } else { false };

    OllamaStatus {
        cli_installed: cli,
        server_running: server,
        model_installed: has_model,
        model_name: model,
    }
}

pub fn try_start_server() -> bool {
    if ollama_available() {
        return true;
    }
    if !ollama_cli_available() {
        return false;
    }
    spawn_ollama_serve();
    for _ in 0..20 {
        if ollama_available() {
            return true;
        }
        thread::sleep(Duration::from_millis(500));
    }
    false
}

pub fn try_pull_model(app: Option<&AppHandle>) -> bool {
    let model = ollama_model();
    if !ollama_available() {
        return false;
    }
    pull_model_via_api(&model, app);
    model_installed(&model)
}

const DEFAULT_OLLAMA_PULL_URL: &str = "http://127.0.0.1:11434/api/pull";

pub fn test_tagging() -> Option<Vec<String>> {
    // Use a longer timeout for test — model cold start can take 30+ seconds
    let model = ollama_model();
    let truncated = "Meeting with John tomorrow at 3pm to discuss the new API design for user authentication";

    let request = OllamaChatRequest {
        model: &model,
        stream: false,
        format: "json",
        messages: vec![
            OllamaMessage {
                role: "system",
                content: "You classify clipboard text. Return strict JSON only in the shape {\"tags\":[\"tag1\",\"tag2\"]}. Use 2 to 5 short lowercase tags.".to_string(),
            },
            OllamaMessage {
                role: "user",
                content: format!("Text:\n{}", truncated),
            },
        ],
    };

    // 60 second read timeout for cold model loading
    let agent = ollama_agent(2, 60);
    let response = agent.post(DEFAULT_OLLAMA_CHAT_URL).send_json(request).ok()?;
    let chat: OllamaChatResponse = response.into_json().ok()?;
    let parsed: TagResponse = serde_json::from_str(&chat.message.content).ok()?;
    let tags = normalize_tags(parsed.tags);
    if tags.is_empty() { None } else { Some(tags) }
}

const DEFAULT_OLLAMA_CHAT_URL: &str = "http://127.0.0.1:11434/api/chat";
const DEFAULT_OLLAMA_TAGS_URL: &str = "http://127.0.0.1:11434/api/tags";
const DEFAULT_OLLAMA_MODEL: &str = "qwen3:4b-instruct-2507-q4_K_M";

static ACTIVE_MODEL: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn set_active_model(model: &str) {
    *ACTIVE_MODEL.lock().unwrap() = Some(model.to_string());
}

#[derive(Serialize)]
struct OllamaChatRequest<'a> {
    model: &'a str,
    stream: bool,
    format: &'a str,
    messages: Vec<OllamaMessage<'a>>,
}

#[derive(Serialize)]
struct OllamaMessage<'a> {
    role: &'a str,
    content: String,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessageResponse,
}

#[derive(Deserialize)]
struct OllamaMessageResponse {
    content: String,
}

#[derive(Deserialize)]
struct TagResponse {
    tags: Vec<String>,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelTag>,
}

#[derive(Deserialize)]
struct OllamaModelTag {
    name: String,
}

fn ollama_model() -> String {
    ACTIVE_MODEL
        .lock()
        .unwrap()
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_OLLAMA_MODEL.to_string())
}

fn debug_enabled() -> bool {
    std::env::var("COPYOSITY_DEBUG_OLLAMA")
        .map(|v| {
            let value = v.trim().to_ascii_lowercase();
            value == "1" || value == "true" || value == "yes" || value == "on"
        })
        .unwrap_or(true)
}

fn log_debug(message: impl AsRef<str>) {
    if debug_enabled() {
        eprintln!("copyosity[ollama]: {}", message.as_ref());
    }
}

fn ollama_agent(connect_timeout_secs: u64, read_timeout_secs: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(connect_timeout_secs))
        .timeout_read(Duration::from_secs(read_timeout_secs))
        .build()
}

fn ollama_available() -> bool {
    let ok = ollama_agent(1, 2).get(DEFAULT_OLLAMA_TAGS_URL).call().ok().is_some();
    log_debug(format!("availability => {}", ok));
    ok
}

/// Find ollama binary — .app bundles don't inherit shell PATH
fn ollama_bin() -> &'static str {
    static BIN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BIN.get_or_init(|| {
        let candidates = [
            "/usr/local/bin/ollama",
            "/opt/homebrew/bin/ollama",
            "/usr/bin/ollama",
            "ollama", // fallback to PATH
        ];
        for path in candidates {
            if Command::new(path)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
            {
                log_debug(format!("found ollama at: {}", path));
                return path.to_string();
            }
        }
        "ollama".to_string()
    })
}

fn ollama_cli_available() -> bool {
    let ok = Command::new(ollama_bin())
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    log_debug(format!("cli available => {}", ok));
    ok
}

fn model_installed(model: &str) -> bool {
    log_debug(format!("checking model presence: {}", model));
    let response = match ollama_agent(1, 2).get(DEFAULT_OLLAMA_TAGS_URL).call() {
        Ok(response) => response,
        Err(err) => {
            log_debug(format!("failed /api/tags request: {}", err));
            return false;
        }
    };

    let tags: OllamaTagsResponse = match response.into_json() {
        Ok(tags) => tags,
        Err(err) => {
            log_debug(format!("failed to decode /api/tags: {}", err));
            return false;
        }
    };

    let installed = tags.models.iter().any(|candidate| candidate.name == model);
    log_debug(format!("model installed {} => {}", model, installed));
    installed
}

fn spawn_ollama_serve() {
    log_debug("starting background `ollama serve`");
    let _ = Command::new(ollama_bin())
        .arg("serve")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn();
}

fn pull_model(model: &str) {
    pull_model_via_api(model, None);
}

fn pull_model_via_api(model: &str, app: Option<&AppHandle>) {
    use std::io::BufRead;

    log_debug(format!("pulling model via API: {}", model));

    #[derive(Serialize)]
    struct PullRequest<'a> {
        name: &'a str,
        stream: bool,
    }

    #[derive(Deserialize)]
    struct PullProgress {
        status: Option<String>,
        total: Option<u64>,
        completed: Option<u64>,
    }

    let agent = ollama_agent(5, 600); // 10 min timeout for large models
    let response = match agent
        .post(DEFAULT_OLLAMA_PULL_URL)
        .send_json(PullRequest { name: model, stream: true })
    {
        Ok(r) => r,
        Err(err) => {
            log_debug(format!("pull API request failed: {}", err));
            if let Some(app) = app {
                let _ = app.emit("ollama-pull-progress", "Download failed");
            }
            return;
        }
    };

    let reader = std::io::BufReader::new(response.into_reader());
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }

        if let Ok(progress) = serde_json::from_str::<PullProgress>(trimmed) {
            let msg = match (&progress.status, progress.total, progress.completed) {
                (Some(status), Some(total), Some(completed)) if total > 0 => {
                    let pct = (completed as f64 / total as f64 * 100.0) as u32;
                    let mb_done = completed / 1_000_000;
                    let mb_total = total / 1_000_000;
                    format!("{} — {}MB / {}MB ({}%)", status, mb_done, mb_total, pct)
                }
                (Some(status), _, _) => status.clone(),
                _ => continue,
            };
            log_debug(format!("pull: {}", msg));
            if let Some(app) = app {
                let _ = app.emit("ollama-pull-progress", &msg);
            }
        }
    }

    log_debug("pull via API complete");
}

fn total_memory_gb() -> f64 {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
            if let Ok(raw) = String::from_utf8(output.stdout) {
                if let Ok(bytes) = raw.trim().parse::<u64>() {
                    return bytes as f64 / 1024.0 / 1024.0 / 1024.0;
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            if let Some(line) = meminfo.lines().find(|line| line.starts_with("MemTotal:")) {
                if let Some(kb) = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|value| value.parse::<u64>().ok())
                {
                    return kb as f64 / 1024.0 / 1024.0;
                }
            }
        }
    }

    8.0
}

fn installed_models() -> Vec<String> {
    match ollama_agent(1, 2).get(DEFAULT_OLLAMA_TAGS_URL).call() {
        Ok(response) => match response.into_json::<OllamaTagsResponse>() {
            Ok(tags) => tags.models.into_iter().map(|model| model.name).collect(),
            Err(_) => Vec::new(),
        },
        Err(_) => Vec::new(),
    }
}

pub fn model_catalog() -> ModelCatalog {
    let total_memory_gb = total_memory_gb();
    let recommended_memory_gb = ((total_memory_gb * 0.55) * 10.0).round() / 10.0;
    let installed = installed_models();
    let presets = [
        ("qwen3:1.7b-instruct-q4_K_M", "Qwen3 1.7B Q4", 1.8_f64),
        ("qwen3:4b-instruct-2507-q4_K_M", "Qwen3 4B Q4", 3.2_f64),
        ("qwen3:4b-instruct-2507-fp16", "Qwen3 4B FP16", 8.5_f64),
        ("qwen3:8b-instruct-q4_K_M", "Qwen3 8B Q4", 6.4_f64),
    ];

    let options = presets
        .into_iter()
        .map(|(value, label, memory_gb)| ModelOption {
            value: value.to_string(),
            label: label.to_string(),
            memory_gb,
            fits: memory_gb <= recommended_memory_gb,
            installed: installed.iter().any(|name| name == value),
        })
        .collect();

    ModelCatalog {
        total_memory_gb: (total_memory_gb * 10.0).round() / 10.0,
        recommended_memory_gb,
        options,
    }
}

pub fn ensure_runtime() {
    thread::spawn(|| {
        let model = ollama_model();
        log_debug(format!("ensure_runtime start model={}", model));

        if !ollama_cli_available() {
            eprintln!("copyosity: ollama cli not found, local tagging disabled");
            return;
        }

        if !ollama_available() {
            spawn_ollama_serve();
            for _ in 0..20 {
                if ollama_available() {
                    break;
                }
                thread::sleep(Duration::from_millis(500));
            }
        }

        if !ollama_available() {
            eprintln!("copyosity: ollama server did not start, local tagging disabled");
            return;
        }

        if !model_installed(&model) {
            eprintln!("copyosity: pulling ollama model {}", model);
            pull_model(&model);
        } else {
            log_debug(format!("model already installed: {}", model));
        }

        log_debug("ensure_runtime complete");
    });
}

pub fn backfill_existing_tags(app: AppHandle, db: Arc<Database>) {
    thread::spawn(move || {
        let model = ollama_model();
        log_debug(format!("backfill start model={}", model));

        if !ollama_cli_available() {
            log_debug("backfill skipped: ollama cli unavailable");
            return;
        }

        if !ollama_available() {
            log_debug("backfill skipped: ollama server unavailable");
            return;
        }

        let mut offset = 0i64;
        loop {
            let batch = match db.get_text_entries_for_retag(100, offset) {
                Ok(entries) => entries,
                Err(err) => {
                    eprintln!("copyosity[ollama]: failed to load entries for retag: {}", err);
                    return;
                }
            };

            if batch.is_empty() {
                break;
            }

            for (entry_id, text, tags) in batch {
                if looks_like_opaque_code(&text) {
                    let next_tags = heuristic_tags(&text).unwrap_or_default();
                    if tags != next_tags {
                        if let Err(err) = db.set_entry_tags(entry_id, &next_tags) {
                            eprintln!(
                                "copyosity[ollama]: failed to update heuristic tags for entry {}: {}",
                                entry_id, err
                            );
                            continue;
                        }
                        log_debug(format!("retag heuristic entry_id={} tags={:?}", entry_id, next_tags));
                        let _ = app.emit("entry-tagged", entry_id);
                    } else {
                        let _ = db.set_entry_tag_state(entry_id, "done");
                    }
                    continue;
                }

                if tags.is_empty() {
                    let _ = db.set_entry_tag_state(entry_id, "done");
                    continue;
                }
            }

            offset += 100;
        }

        loop {
            let batch = match db.get_untagged_text_entries(24) {
                Ok(entries) => entries,
                Err(err) => {
                    eprintln!("copyosity[ollama]: failed to load untagged entries: {}", err);
                    return;
                }
            };

            if batch.is_empty() {
                log_debug("backfill complete: no untagged entries left");
                return;
            }

            log_debug(format!("backfill batch size={}", batch.len()));

            for (entry_id, text) in batch {
                let preview = text.trim().chars().take(80).collect::<String>();
                log_debug(format!("backfill entry_id={} preview={:?}", entry_id, preview));

                match tag_text(&text) {
                    Some(tags) => {
                        if let Err(err) = db.set_entry_tags(entry_id, &tags) {
                            eprintln!(
                                "copyosity[ollama]: failed to save tags for entry {}: {}",
                                entry_id, err
                            );
                            continue;
                        }

                        log_debug(format!("backfill saved entry_id={} tags={:?}", entry_id, tags));
                        let _ = app.emit("entry-tagged", entry_id);
                    }
                    None => {
                        let _ = db.set_entry_tag_state(entry_id, "skipped");
                        log_debug(format!("backfill skipped entry_id={} tags empty", entry_id));
                    }
                }
            }
        }
    });
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();

    for tag in tags {
        let cleaned = tag
            .trim()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>();

        if cleaned.len() < 1 || cleaned.len() > 24 {
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
        return Some(vec!["otp".to_string()]);
    }

    let ascii_only = trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '+' | '/' | '='));
    let has_uppercase = trimmed.chars().any(|ch| ch.is_ascii_uppercase());
    let has_digits = trimmed.chars().any(|ch| ch.is_ascii_digit());
    let has_dash = trimmed.contains('-') || trimmed.contains('_');

    if ascii_only && has_uppercase && has_digits && has_dash && (6..=20).contains(&len) {
        return Some(vec!["code".to_string()]);
    }

    if ascii_only && has_digits && (trimmed.contains('+') || trimmed.contains('/') || trimmed.contains('=')) {
        return Some(vec!["token".to_string()]);
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

pub fn tag_text(text: &str) -> Option<Vec<String>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        log_debug("skip tagging: empty text");
        return None;
    }

    if looks_like_opaque_code(trimmed) {
        let tags = heuristic_tags(trimmed);
        log_debug(format!(
            "heuristic tagging: opaque code-like text {:?} => {:?}",
            trimmed, tags
        ));
        return tags;
    }

    let model = ollama_model();

    let truncated = trimmed.chars().take(1200).collect::<String>();
    log_debug(format!(
        "tag_text request model={} chars={} preview={:?}",
        model,
        truncated.chars().count(),
        truncated.chars().take(120).collect::<String>()
    ));
    let request = OllamaChatRequest {
        model: &model,
        stream: false,
        format: "json",
        messages: vec![
            OllamaMessage {
                role: "system",
                content: "You classify clipboard text. Return strict JSON only in the shape {\"tags\":[\"tag1\",\"tag2\"]}. Use 2 to 5 short lowercase tags. Prefer practical tags like bash, ssh, docker, sql, json, url, ai, meeting, credentials, error, python, rust, javascript, html, api. If the text is just an opaque token, otp, code, short id, password, or random identifier with no semantic meaning, return {\"tags\":[]}. Do not explain.".to_string(),
            },
            OllamaMessage {
                role: "user",
                content: format!("Text:\n{}", truncated),
            },
        ],
    };

    let agent = ollama_agent(1, 8);

    let response = agent
        .post(DEFAULT_OLLAMA_CHAT_URL)
        .send_json(request)
        .map_err(|err| {
            log_debug(format!("ollama request failed: {}", err));
            err
        })
        .ok()?;

    let chat: OllamaChatResponse = response
        .into_json()
        .map_err(|err| {
            log_debug(format!("failed to parse chat envelope: {}", err));
            err
        })
        .ok()?;
    log_debug(format!("raw model content={}", chat.message.content));
    let parsed: TagResponse = serde_json::from_str(&chat.message.content)
        .map_err(|err| {
            log_debug(format!("failed to parse model json: {}", err));
            err
        })
        .ok()?;
    let tags = normalize_tags(parsed.tags);
    log_debug(format!("normalized tags={:?}", tags));

    if tags.is_empty() {
        log_debug("skip tagging: normalized tags empty");
        None
    } else {
        Some(tags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- normalize_tags ---

    #[test]
    fn normalize_lowercases_and_strips() {
        let result = normalize_tags(vec!["  Rust ".to_string(), "CODE".to_string()]);
        assert_eq!(result, vec!["rust", "code"]);
    }

    #[test]
    fn normalize_deduplicates() {
        let result = normalize_tags(vec!["rust".to_string(), "Rust".to_string(), "RUST".to_string()]);
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
            "".to_string(),
            "a".to_string(),
            "x".repeat(25), // too long
        ]);
        assert_eq!(result, vec!["a"]);
    }

    #[test]
    fn normalize_strips_special_chars() {
        let result = normalize_tags(vec!["hello world!".to_string()]);
        assert_eq!(result, vec!["helloworld"]);
    }

    // --- heuristic_tags ---

    #[test]
    fn heuristic_otp_4_digits() {
        assert_eq!(heuristic_tags("1234"), Some(vec!["otp".to_string()]));
    }

    #[test]
    fn heuristic_otp_6_digits() {
        assert_eq!(heuristic_tags("482917"), Some(vec!["otp".to_string()]));
    }

    #[test]
    fn heuristic_not_otp_too_long() {
        assert_eq!(heuristic_tags("123456789"), None);
    }

    #[test]
    fn heuristic_code_pattern() {
        assert_eq!(heuristic_tags("AB3-XY7_Z"), Some(vec!["code".to_string()]));
    }

    #[test]
    fn heuristic_token_with_base64() {
        assert_eq!(heuristic_tags("abc123+def/ghi="), Some(vec!["token".to_string()]));
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

    // --- check_status (unit, no Ollama needed) ---

    #[test]
    fn check_status_returns_struct() {
        let status = check_status();
        // Just verify it returns without panic and model name is populated
        assert!(!status.model_name.is_empty());
    }
}
