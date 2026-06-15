//! NeuralDeep hub integration — an OpenAI-compatible LLM proxy.
//!
//! The hub exposes standard OpenAI routes (`/v1/models`, `/v1/chat/completions`,
//! `/v1/audio/transcriptions`). Each user configures their own base URL + Bearer
//! token in Settings. This module provides a connection test and chat-based
//! clipboard tagging that mirrors the local Ollama tagging behaviour.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Trim a trailing slash so we can safely append `/v1/...`.
fn normalize_base(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .build()
}

/// Probe the hub by listing models. Returns the number of available models on
/// success, or a human-readable error.
pub fn test_connection(base_url: &str, token: &str) -> Result<usize, String> {
    let base = normalize_base(base_url);
    if base.is_empty() {
        return Err("Hub URL is empty".to_string());
    }
    if token.trim().is_empty() {
        return Err("Hub token is empty".to_string());
    }

    let url = format!("{}/v1/models", base);
    let response = agent()
        .get(&url)
        .set("Authorization", &format!("Bearer {}", token.trim()))
        .set("Accept", "application/json")
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("Hub returned HTTP {}", code),
            other => format!("Hub request failed: {}", other),
        })?;

    let json: serde_json::Value = response
        .into_json()
        .map_err(|e| format!("Failed to parse hub response: {}", e))?;

    let count = json["data"].as_array().map(|a| a.len()).unwrap_or(0);
    Ok(count)
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    temperature: f32,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: String,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct TagResponse {
    tags: Vec<String>,
}

const TAG_SYSTEM_PROMPT: &str = "You classify clipboard text. Return strict JSON only in the shape {\"tags\":[\"tag1\",\"tag2\"]}. Use 2 to 5 short lowercase tags. Prefer practical tags like bash, ssh, docker, sql, json, url, ai, meeting, credentials, error, python, rust, javascript, html, api. If the text is just an opaque token, otp, code, short id, password, or random identifier with no semantic meaning, return {\"tags\":[]}. Do not explain.";

/// Tag clipboard text via the hub chat API. Returns `None` on any failure so the
/// caller can fall back to local tagging.
pub fn tag_text(base_url: &str, token: &str, model: &str, text: &str) -> Option<Vec<String>> {
    let base = normalize_base(base_url);
    if base.is_empty() || token.trim().is_empty() || model.trim().is_empty() {
        return None;
    }

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let truncated = trimmed.chars().take(1200).collect::<String>();

    let request = ChatRequest {
        model: model.trim(),
        temperature: 0.0,
        stream: false,
        messages: vec![
            ChatMessage {
                role: "system",
                content: TAG_SYSTEM_PROMPT.to_string(),
            },
            ChatMessage {
                role: "user",
                content: format!("Text:\n{}", truncated),
            },
        ],
    };

    let url = format!("{}/v1/chat/completions", base);
    let response = agent()
        .post(&url)
        .set("Authorization", &format!("Bearer {}", token.trim()))
        .set("Accept", "application/json")
        .send_json(request)
        .ok()?;

    let chat: ChatResponse = response.into_json().ok()?;
    let content = chat.choices.first()?.message.content.clone();

    // The model may wrap JSON in prose/code fences — extract the JSON object.
    let json_slice = extract_json_object(&content)?;
    let parsed: TagResponse = serde_json::from_str(json_slice).ok()?;

    let tags = crate::ollama::normalize_tags(parsed.tags);
    if tags.is_empty() {
        None
    } else {
        Some(tags)
    }
}

/// Find the first balanced `{...}` JSON object in a string.
fn extract_json_object(s: &str) -> Option<&str> {
    let start = s.find('{')?;
    let mut depth = 0usize;
    for (i, c) in s[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[start..start + i + 1]);
                }
            }
            _ => {}
        }
    }
    None
}
