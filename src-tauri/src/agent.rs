//! Minimal ReAct agent loop over the NeuralDeep hub.
//!
//! Uses qwen3.6 (native tool-calls) via /v1/chat/completions with a single
//! `web_search` tool backed by the hub Search API. Runs in a background thread
//! and streams progress to the frontend via Tauri events:
//!   - "agent-progress" : String (a human-readable step line)
//!   - "agent-final"    : String (the final answer)
//!   - "agent-error"    : String

use serde_json::{json, Value};
use std::time::Duration;
use tauri::Emitter;

/// Model with the best native tool-calling on the hub.
const AGENT_MODEL: &str = "qwen3.6-35b-a3b";
const MAX_STEPS: usize = 12;

fn agent_http() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        // qwen3.6 is a reasoning model and can take a while per step.
        .timeout(Duration::from_secs(180))
        .build()
}

/// Tool calls on an assistant turn (empty if none).
fn tool_calls_of(message: &Value) -> Vec<Value> {
    message["tool_calls"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// Final text of an assistant turn. Reasoning models sometimes leave `content`
/// null and put text in `reasoning_content`; fall back to it so we never show
/// an empty answer when the model actually responded.
fn final_content(message: &Value) -> String {
    let c = message["content"].as_str().unwrap_or("").trim();
    if !c.is_empty() {
        return c.to_string();
    }
    message["reasoning_content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn normalize_base(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

/// Parse an ISO-ish due date into seconds-from-now (None if past/unparseable).
fn parse_due_offset_secs(iso: &str) -> Option<i64> {
    use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
    let target_ts = if let Ok(dt) = DateTime::parse_from_rfc3339(iso) {
        dt.timestamp()
    } else {
        let ndt = NaiveDateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| NaiveDateTime::parse_from_str(iso, "%Y-%m-%d %H:%M:%S"))
            .or_else(|_| NaiveDateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M"))
            .ok()?;
        Local.from_local_datetime(&ndt).single()?.timestamp()
    };
    let offset = target_ts - Local::now().timestamp();
    (offset > 0).then_some(offset)
}

/// Run the agent loop to completion, emitting progress events on `app`.
pub fn run(app: &tauri::AppHandle, base_url: &str, token: &str, query: &str) {
    if let Err(e) = run_inner(app, base_url, token, query) {
        let _ = app.emit("agent-error", e);
    }
}

fn run_inner(
    app: &tauri::AppHandle,
    base_url: &str,
    token: &str,
    query: &str,
) -> Result<(), String> {
    let base = normalize_base(base_url);
    if base.is_empty() || token.trim().is_empty() {
        return Err("Set the NeuralDeep hub URL and token in Settings".to_string());
    }
    let query = query.trim();
    if query.is_empty() {
        return Err("Empty question".to_string());
    }

    let tools = json!([
        {
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Search the web for current/factual information. Use it whenever the question needs fresh facts, news, prices, docs or anything you are unsure about.",
                "parameters": {
                    "type": "object",
                    "properties": { "query": { "type": "string", "description": "search query" } },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "create_note",
                "description": "Create a note in the user's macOS Notes app.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "body": { "type": "string" }
                    },
                    "required": ["title", "body"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "create_reminder",
                "description": "Create a reminder in the user's macOS Reminders app.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "due": { "type": "string", "description": "optional due date/time as ISO 8601 (e.g. 2026-06-20T10:00:00)" }
                    },
                    "required": ["title"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_reminders",
                "description": "List the user's open (incomplete) reminders.",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "read_calendar",
                "description": "Read the user's upcoming macOS Calendar events for the next N days.",
                "parameters": {
                    "type": "object",
                    "properties": { "days": { "type": "integer", "description": "how many days ahead (1-60)" } },
                    "required": ["days"]
                }
            }
        }
    ]);

    let mut messages: Vec<Value> = vec![
        json!({
            "role": "system",
            "content": "You are a personal assistant agent on the user's Mac. You can search the web AND act on the user's apps: create notes (create_note), create/list reminders (create_reminder, list_reminders), and read their calendar (read_calendar). Use the right tool for the request — e.g. 'remind me tomorrow at 10 to call Bob' -> create_reminder; 'what's on my calendar this week' -> read_calendar; 'save this to notes' -> create_note. Call tools as needed, then give a concise confirmation/answer in the user's language. Do not invent facts — use web_search."
        }),
        json!({ "role": "user", "content": query }),
    ];

    let url = format!("{}/v1/chat/completions", base);

    for step in 0..MAX_STEPS {
        // On the last step, force a final answer (no more tool calls) so the
        // agent always responds instead of bailing with "step limit reached".
        let force_final = step == MAX_STEPS - 1;
        let _ = app.emit(
            "agent-progress",
            format!("🤔 Думаю… (шаг {}/{})", step + 1, MAX_STEPS),
        );

        let body = json!({
            "model": AGENT_MODEL,
            "messages": messages,
            "tools": tools,
            "tool_choice": if force_final { "none" } else { "auto" },
            "temperature": 0.2,
            "stream": false
        });

        let resp = agent_http()
            .post(&url)
            .set("Authorization", &format!("Bearer {}", token.trim()))
            .set("Content-Type", "application/json")
            .send_json(body)
            .map_err(|e| match e {
                ureq::Error::Status(code, _) => format!("Hub returned HTTP {}", code),
                other => format!("Hub request failed: {}", other),
            })?;

        let json: Value = resp
            .into_json()
            .map_err(|e| format!("Bad hub response: {}", e))?;
        let message = &json["choices"][0]["message"];

        let tool_calls = tool_calls_of(message);

        if tool_calls.is_empty() {
            // Final answer.
            let content = final_content(message);
            if content.is_empty() {
                return Err("Agent returned an empty answer".to_string());
            }
            let _ = app.emit("agent-final", content);
            return Ok(());
        }

        // Record the assistant turn (with its tool_calls) verbatim.
        messages.push(message.clone());

        for tc in &tool_calls {
            let name = tc["function"]["name"].as_str().unwrap_or("");
            let id = tc["id"].as_str().unwrap_or("").to_string();
            let args_raw = tc["function"]["arguments"].as_str().unwrap_or("{}");
            let args: Value = serde_json::from_str(args_raw).unwrap_or(json!({}));

            let result = match name {
                "web_search" => {
                    let q = args["query"].as_str().unwrap_or("").to_string();
                    let _ = app.emit("agent-progress", format!("🔎 Ищу: {}", q));
                    crate::hub::web_search(&base, token, &q, 5)
                        .unwrap_or_else(|e| format!("(search failed: {})", e))
                }
                "create_note" => {
                    let title = args["title"].as_str().unwrap_or("Note");
                    let body = args["body"].as_str().unwrap_or("");
                    let _ = app.emit("agent-progress", format!("📝 Заметка: {}", title));
                    crate::mactools::create_note(title, body)
                        .unwrap_or_else(|e| format!("(note failed: {})", e))
                }
                "create_reminder" => {
                    let title = args["title"].as_str().unwrap_or("Reminder");
                    let due_offset = args["due"].as_str().and_then(parse_due_offset_secs);
                    let _ = app.emit("agent-progress", format!("⏰ Напоминание: {}", title));
                    crate::mactools::create_reminder(title, due_offset)
                        .unwrap_or_else(|e| format!("(reminder failed: {})", e))
                }
                "list_reminders" => {
                    let _ = app.emit("agent-progress", "⏰ Читаю напоминания…".to_string());
                    crate::mactools::list_reminders()
                        .unwrap_or_else(|e| format!("(list failed: {})", e))
                }
                "read_calendar" => {
                    let days = args["days"].as_i64().unwrap_or(7);
                    let _ = app.emit("agent-progress", format!("📅 Календарь: {} дн.", days));
                    crate::mactools::read_calendar(days)
                        .unwrap_or_else(|e| format!("(calendar failed: {})", e))
                }
                other => format!("(unknown tool: {})", other),
            };

            messages.push(json!({
                "role": "tool",
                "tool_call_id": id,
                "content": result
            }));
        }
    }

    let _ = app.emit(
        "agent-error",
        "Достигнут лимит шагов — попробуй переформулировать вопрос".to_string(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_content_uses_content_when_present() {
        let m = json!({ "content": "  Париж  ", "reasoning_content": "thinking..." });
        assert_eq!(final_content(&m), "Париж");
    }

    #[test]
    fn final_content_falls_back_to_reasoning_when_content_null() {
        // Reasoning models sometimes return content: null.
        let m = json!({ "content": null, "reasoning_content": "  the answer is 42  " });
        assert_eq!(final_content(&m), "the answer is 42");
    }

    #[test]
    fn final_content_empty_when_both_missing() {
        let m = json!({ "content": "", "reasoning_content": "" });
        assert!(final_content(&m).is_empty());
    }

    #[test]
    fn tool_calls_parsed_when_present() {
        let m = json!({
            "content": null,
            "tool_calls": [{
                "id": "call_1",
                "function": { "name": "web_search", "arguments": "{\"query\":\"rust\"}" }
            }]
        });
        let tcs = tool_calls_of(&m);
        assert_eq!(tcs.len(), 1);
        assert_eq!(tcs[0]["function"]["name"], "web_search");
        // arguments arrive as a JSON string that we must parse.
        let args: Value =
            serde_json::from_str(tcs[0]["function"]["arguments"].as_str().unwrap()).unwrap();
        assert_eq!(args["query"], "rust");
    }

    #[test]
    fn tool_calls_empty_for_plain_answer() {
        let m = json!({ "content": "hello" });
        assert!(tool_calls_of(&m).is_empty());
    }
}
