//! Tagging dispatcher: route clipboard tagging to the NeuralDeep hub when it is
//! enabled and configured, otherwise fall back to the local Ollama model.

use crate::db::Database;

/// Tag clipboard `text`. Prefers the hub when `hub_tagging_enabled` and a token
/// + chat model are set; on any hub failure it falls back to local Ollama so
/// tagging keeps working offline.
pub fn tag(db: &Database, text: &str) -> Option<Vec<String>> {
    if let Ok(s) = db.get_app_settings() {
        if s.hub_tagging_enabled && !s.hub_token.is_empty() && !s.hub_chat_model.is_empty() {
            if let Some(tags) =
                crate::hub::tag_text(&s.hub_url, &s.hub_token, &s.hub_chat_model, text)
            {
                return Some(tags);
            }
            // hub unreachable / errored — fall through to local tagging
        }
    }
    crate::ollama::tag_text(text)
}
