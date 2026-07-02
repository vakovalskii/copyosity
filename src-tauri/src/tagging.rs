//! Tagging dispatcher: route clipboard tagging to the NeuralDeep hub when it is
//! enabled and configured, otherwise fall back to the local Ollama model.

use crate::db::{ClipboardEntry, Database};

/// Hub multimodal model used for image classification (matches capture pipeline).
pub const HUB_IMAGE_TAG_MODEL: &str = "qwen3.6-35b-a3b";

/// Hub tagging is configured when the master switch, tagging toggle, and token are set.
pub(crate) fn hub_tagging_configured(s: &crate::db::AppSettings) -> bool {
    s.hub_enabled && s.hub_tagging_enabled && !s.hub_token.trim().is_empty()
}

/// Hub text tagging (auto-tag / retag on text) needs a chat model in addition to hub tagging.
pub(crate) fn hub_text_tagging_ready(s: &crate::db::AppSettings) -> bool {
    hub_tagging_configured(s) && !s.hub_chat_model.trim().is_empty()
}

/// Retag is available when hub tagging is configured or local Ollama tagging is ready.
pub fn is_retag_ready(db: &Database) -> bool {
    should_auto_tag_on_capture(db)
}

/// Whether clipboard capture should spawn auto-tagging (hub and/or local Ollama).
pub fn should_auto_tag_on_capture(db: &Database) -> bool {
    if let Ok(s) = db.get_app_settings() {
        if hub_tagging_configured(&s) || hub_text_tagging_ready(&s) {
            return true;
        }
    }
    crate::ollama::is_tagging_ready(db)
}

/// Text clipboard capture: hub needs chat model; images use [`should_auto_tag_on_capture`].
pub fn should_auto_tag_text_on_capture(db: &Database) -> bool {
    if let Ok(s) = db.get_app_settings() {
        if hub_text_tagging_ready(&s) {
            return true;
        }
    }
    crate::ollama::is_tagging_ready(db)
}

/// Tag clipboard `text`. Prefers the hub when `hub_tagging_enabled` and a token
/// + chat model are set; on any hub failure it falls back to local Ollama so
///   tagging keeps working offline.
pub fn tag(db: &Database, text: &str) -> Option<Vec<String>> {
    if let Ok(s) = db.get_app_settings() {
        if hub_text_tagging_ready(&s) {
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

/// Tag an image entry using hub vision (thumbnail) with OCR-text fallback.
pub fn tag_image_entry(db: &Database, entry: &ClipboardEntry) -> Option<Vec<String>> {
    let settings = db.get_app_settings().ok()?;
    let use_hub = hub_tagging_configured(&settings);

    if use_hub {
        let image_b64 = entry
            .image_thumb
            .as_deref()
            .or(entry.image_data.as_deref())?;
        if let Some(tags) = crate::hub::tag_image(
            &settings.hub_url,
            &settings.hub_token,
            HUB_IMAGE_TAG_MODEL,
            image_b64,
        ) {
            return Some(tags);
        }
    }

    if !db.is_ai_tagging_enabled() {
        return None;
    }

    entry
        .ocr_text
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .and_then(|text| tag(db, text))
}
