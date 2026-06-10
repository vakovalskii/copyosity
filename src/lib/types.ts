export interface ClipboardEntry {
  id: number;
  content_type: string;
  text_content: string | null;
  image_data: string | null;
  image_thumb: string | null;
  source_app: string | null;
  source_app_icon: string | null;
  content_hash: string;
  char_count: number | null;
  created_at: string;
  is_pinned: boolean;
  collection_id: number | null;
  tags: string[];
  image_format?: string | null;
}

export interface Collection {
  id: number;
  name: string;
  color: string | null;
  sort_order: number;
}

export interface AppSettings {
  ollama_model: string;
  retention_days: number;
  whisper_server_url: string;
  whisper_server_token: string;
  whisper_server_model: string;
  voice_shortcut: string;
  selected_microphone: string;
  voice_transcription_enabled: boolean;
  ai_tagging_enabled: boolean;
}

export interface AudioInputDevice {
  name: string;
  is_default: boolean;
}

export interface ModelOption {
  value: string;
  label: string;
  memory_gb: number;
  fits: boolean;
  installed: boolean;
}

export interface ModelCatalog {
  total_memory_gb: number;
  recommended_memory_gb: number;
  options: ModelOption[];
}

// Exclusion IPC types use camelCase (backend #[serde(rename_all = "camelCase")]).

export interface ExcludedApp {
  id: number;
  bundleId: string;
  displayName: string;
}

export interface ExcludableAppCandidate {
  bundleId: string;
  displayName: string;
  alreadyExcluded: boolean;
  source: "remembered" | "frontmost";
}

export interface ExcludeAppResult {
  displayName: string;
  alreadyExcluded: boolean;
}
