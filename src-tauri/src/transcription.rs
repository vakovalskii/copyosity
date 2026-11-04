//! Voice transcription endpoint resolution (NeuralDeep hub only).

use crate::db::AppSettings;

/// Resolve the hub transcription endpoint from settings.
pub(crate) fn transcription_endpoint(settings: &AppSettings) -> Result<(String, String), String> {
    if !settings.hub_enabled || !settings.hub_transcribe_enabled {
        return Err(
            "Voice transcription needs the NeuralDeep hub — enable it in Settings".to_string(),
        );
    }
    let base = settings.hub_url.trim().trim_end_matches('/');
    if base.is_empty() || settings.hub_token.trim().is_empty() {
        return Err("Transcription endpoint not configured".to_string());
    }
    Ok((
        format!("{base}/v1/audio/transcriptions"),
        settings.hub_token.clone(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::AppSettings;

    fn base_settings() -> AppSettings {
        AppSettings {
            retention_days: 30,
            whisper_server_model: String::new(),
            voice_shortcut: String::new(),
            selected_microphone: String::new(),
            voice_transcription_enabled: false,
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
            setup_notice_local_tagging_removed: false,
            setup_notice_custom_whisper_removed: false,
        }
    }

    #[test]
    fn errors_when_hub_transcribe_off() {
        let s = base_settings();
        assert!(transcription_endpoint(&s).is_err());
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
    fn errors_when_hub_url_empty() {
        let mut s = base_settings();
        s.hub_enabled = true;
        s.hub_transcribe_enabled = true;
        s.hub_token = "tok".into();
        assert!(transcription_endpoint(&s).is_err());
    }

    #[test]
    fn errors_when_token_missing() {
        let mut s = base_settings();
        s.hub_enabled = true;
        s.hub_transcribe_enabled = true;
        s.hub_url = "https://hub.example".into();
        assert!(transcription_endpoint(&s).is_err());
    }
}
