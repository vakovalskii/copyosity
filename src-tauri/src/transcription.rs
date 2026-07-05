//! Voice transcription endpoint resolution (hub vs standalone Whisper).

use crate::db::AppSettings;

/// Resolve hub or standalone Whisper transcription endpoint from settings.
pub(crate) fn transcription_endpoint(settings: &AppSettings) -> Result<(String, String), String> {
    let use_hub = settings.hub_enabled
        && settings.hub_transcribe_enabled
        && !settings.hub_token.is_empty()
        && !settings.hub_url.trim().is_empty();
    let (url, tok) = if use_hub {
        (
            format!(
                "{}/v1/audio/transcriptions",
                settings.hub_url.trim_end_matches('/')
            ),
            settings.hub_token.clone(),
        )
    } else {
        (
            settings.whisper_server_url.clone(),
            settings.whisper_server_token.clone(),
        )
    };
    if url.is_empty() {
        return Err("Transcription endpoint not configured".to_string());
    }
    Ok((url, tok))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::AppSettings;

    fn base_settings() -> AppSettings {
        AppSettings {
            ollama_model: String::new(),
            retention_days: 30,
            whisper_server_url: "http://localhost:8080/v1/audio/transcriptions".into(),
            whisper_server_token: String::new(),
            whisper_server_model: String::new(),
            voice_shortcut: String::new(),
            selected_microphone: String::new(),
            voice_transcription_enabled: false,
            ai_tagging_enabled: false,
            overlay_shortcut_hints_enabled: true,
            hub_enabled: false,
            hub_url: String::new(),
            hub_token: String::new(),
            hub_chat_model: String::new(),
            hub_tagging_enabled: false,
            hub_transcribe_enabled: false,
            voice_polish_enabled: false,
            voice_polish_model: String::new(),
            voice_polish_screenshot: false,
            voice_polish_prompt: String::new(),
            voice_translate_lang: String::new(),
            voice_dictionary: String::new(),
            voice_selected_text: false,
            board_vertical: false,
        }
    }

    #[test]
    fn uses_whisper_when_hub_transcribe_off() {
        let s = base_settings();
        let (url, tok) = transcription_endpoint(&s).unwrap();
        assert_eq!(url, "http://localhost:8080/v1/audio/transcriptions");
        assert!(tok.is_empty());
    }

    #[test]
    fn uses_hub_when_enabled() {
        let mut s = base_settings();
        s.hub_enabled = true;
        s.hub_transcribe_enabled = true;
        s.hub_url = "https://hub.example/".into();
        s.hub_token = "tok".into();
        let (url, tok) = transcription_endpoint(&s).unwrap();
        assert_eq!(url, "https://hub.example/v1/audio/transcriptions");
        assert_eq!(tok, "tok");
    }

    #[test]
    fn falls_back_to_whisper_when_hub_url_empty() {
        let mut s = base_settings();
        s.hub_enabled = true;
        s.hub_transcribe_enabled = true;
        s.hub_token = "tok".into();
        let (url, _) = transcription_endpoint(&s).unwrap();
        assert_eq!(url, "http://localhost:8080/v1/audio/transcriptions");
    }

    #[test]
    fn errors_when_no_endpoint() {
        let mut s = base_settings();
        s.whisper_server_url.clear();
        assert!(transcription_endpoint(&s).is_err());
    }
}
