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
  ocr_text?: string | null;
  image_format?: string | null;
  image_width?: number | null;
  image_height?: number | null;
  image_byte_size?: number | null;
}

export interface Collection {
  id: number;
  name: string;
  color: string | null;
  sort_order: number;
}

export interface SnippetFolder {
  id: number;
  name: string;
  position: number;
}

export interface Snippet {
  id: number;
  folder_id: number;
  title: string;
  content: string;
  position: number;
}

export interface HistoryCounts {
  total: number;
  unpinned: number;
  pinned: number;
}

export interface TagCount {
  tag: string;
  count: number;
}

export interface OverlayTagCounts {
  semantic: TagCount[];
  format: TagCount[];
  has_text: boolean;
  has_images: boolean;
}

export interface AppSettings {
  ollama_model: string;
  retention_days: number;
  whisper_server_url: string;
  whisper_server_token: string;
  whisper_server_model: string;
  voice_shortcut: string;
  selected_microphone: string;
  hub_enabled: boolean;
  hub_url: string;
  hub_token: string;
  hub_chat_model: string;
  hub_tagging_enabled: boolean;
  hub_transcribe_enabled: boolean;
  voice_polish_enabled: boolean;
  voice_polish_model: string;
  voice_polish_screenshot: boolean;
  voice_polish_prompt: string;
  voice_translate_lang: string;
  voice_dictionary: string;
  voice_selected_text: boolean;
  board_vertical: boolean;
  voice_transcription_enabled: boolean;
  ai_tagging_enabled: boolean;
  overlay_shortcut_hints_enabled: boolean;
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

/** `entry-tagged` Tauri event payload (Rust: `db::EntryTaggedPayload`). */
export interface EntryTaggedPayload {
  entryId: number;
  tags: string[];
}

export function isEntryTaggedPayload(payload: unknown): payload is EntryTaggedPayload {
  if (typeof payload !== "object" || payload === null) return false;
  const record = payload as Record<string, unknown>;
  return (
    typeof record.entryId === "number" &&
    Array.isArray(record.tags) &&
    record.tags.every((tag) => typeof tag === "string")
  );
}

export type ParsedEntryTaggedEvent =
  | { kind: "payload"; payload: EntryTaggedPayload }
  | { kind: "legacy-id"; entryId: number };

/**
 * Parse `entry-tagged` event payload. Accepts current `{ entryId, tags }` and
 * legacy bare entry id (pre-0.4 payload shape) for rolling upgrades.
 */
export function parseEntryTaggedEvent(payload: unknown): ParsedEntryTaggedEvent | null {
  if (isEntryTaggedPayload(payload)) {
    return { kind: "payload", payload };
  }
  if (typeof payload === "number" && Number.isInteger(payload) && payload > 0) {
    return { kind: "legacy-id", entryId: payload };
  }
  return null;
}

/** `entry-ocr` Tauri event payload (Rust: `db::EntryOcrPayload`). */
export interface EntryOcrPayload {
  entryId: number;
  ocrText: string;
}

export function isEntryOcrPayload(payload: unknown): payload is EntryOcrPayload {
  if (typeof payload !== "object" || payload === null) return false;
  const record = payload as Record<string, unknown>;
  return (
    typeof record.entryId === "number" &&
    typeof record.ocrText === "string" &&
    record.ocrText.length > 0
  );
}

export type ParsedEntryOcrEvent =
  | { kind: "payload"; payload: EntryOcrPayload }
  | { kind: "legacy-id"; entryId: number };

/**
 * Parse `entry-ocr` event payload. Accepts `{ entryId, ocrText }` and legacy bare id.
 */
export function parseEntryOcrEvent(payload: unknown): ParsedEntryOcrEvent | null {
  if (isEntryOcrPayload(payload)) {
    return { kind: "payload", payload };
  }
  if (typeof payload === "number" && Number.isInteger(payload) && payload > 0) {
    return { kind: "legacy-id", entryId: payload };
  }
  return null;
}

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
