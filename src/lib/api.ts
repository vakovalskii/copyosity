import { invoke } from "@tauri-apps/api/core";

import type {
  AppSettings,
  AudioInputDevice,
  ClipboardEntry,
  Collection,
  ExcludedApp,
  ExcludableAppCandidate,
  ExcludeAppResult,
  HistoryCounts,
  ModelCatalog,
  OverlayTagCounts,
  Snippet,
  SnippetFolder,
} from "./types";

/** Page size for entry list pagination; mirrored by the backend default. */
export const ENTRY_PAGE_SIZE = 50;

export async function getEntries(opts?: {
  limit?: number;
  offset?: number;
  collection_id?: number | null;
  pinned_only?: boolean;
  search?: string | null;
  tag?: string | null;
  /** DB tag strings that share the UI label in `tag` (semantic filters only). */
  tag_variants?: string[] | null;
  content_kind?: "text" | "image" | null;
}): Promise<ClipboardEntry[]> {
  return invoke("get_entries", {
    limit: opts?.limit ?? ENTRY_PAGE_SIZE,
    offset: opts?.offset ?? 0,
    collectionId: opts?.collection_id ?? null,
    pinnedOnly: opts?.pinned_only ?? false,
    search: opts?.search ?? null,
    tag: opts?.tag ?? null,
    tagVariants: opts?.tag_variants ?? null,
    contentKind: opts?.content_kind ?? null,
  });
}

export async function getOverlayTagCounts(opts?: {
  collection_id?: number | null;
  pinned_only?: boolean;
  search?: string | null;
}): Promise<OverlayTagCounts> {
  return invoke("get_overlay_tag_counts", {
    collectionId: opts?.collection_id ?? null,
    pinnedOnly: opts?.pinned_only ?? false,
    search: opts?.search ?? null,
  });
}

export async function deleteEntry(id: number): Promise<void> {
  return invoke("delete_entry", { id });
}

export async function pinEntry(id: number, pinned: boolean): Promise<void> {
  return invoke("pin_entry", { id, pinned });
}

export async function setEntryCollection(
  entryId: number,
  collectionId: number | null,
): Promise<void> {
  return invoke("set_entry_collection", { entryId, collectionId });
}

export async function getCollections(): Promise<Collection[]> {
  return invoke("get_collections");
}

export async function createCollection(name: string, color?: string): Promise<number> {
  return invoke("create_collection", { name, color: color ?? null });
}

export async function deleteCollection(id: number): Promise<void> {
  return invoke("delete_collection", { id });
}

export async function clearHistory(): Promise<void> {
  return invoke("clear_history");
}

export async function clearAllHistory(): Promise<void> {
  return invoke("clear_all_history");
}

export async function getHistoryCounts(): Promise<HistoryCounts> {
  return invoke("get_history_counts");
}

export async function hideMainWindow(): Promise<void> {
  return invoke("hide_main_window");
}

export async function getAppSettings(): Promise<AppSettings> {
  return invoke("get_app_settings");
}

export async function updateAppSettings(opts: {
  ollama_model?: string | null;
  retention_days?: number | null;
  whisper_server_url?: string | null;
  whisper_server_token?: string | null;
  whisper_server_model?: string | null;
  voice_shortcut?: string | null;
  selected_microphone?: string | null;
  hub_enabled?: boolean | null;
  hub_url?: string | null;
  hub_token?: string | null;
  hub_chat_model?: string | null;
  hub_tagging_enabled?: boolean | null;
  hub_transcribe_enabled?: boolean | null;
  voice_polish_enabled?: boolean | null;
  voice_polish_model?: string | null;
  voice_polish_screenshot?: boolean | null;
  voice_polish_prompt?: string | null;
  voice_translate_lang?: string | null;
  voice_dictionary?: string | null;
  voice_selected_text?: boolean | null;
  board_vertical?: boolean | null;
  voice_transcription_enabled?: boolean | null;
  ai_tagging_enabled?: boolean | null;
  overlay_shortcut_hints_enabled?: boolean | null;
}): Promise<AppSettings> {
  return invoke("update_app_settings", {
    ollamaModel: opts.ollama_model ?? null,
    retentionDays: opts.retention_days ?? null,
    whisperServerUrl: opts.whisper_server_url ?? null,
    whisperServerToken: opts.whisper_server_token ?? null,
    whisperServerModel: opts.whisper_server_model ?? null,
    voiceShortcut: opts.voice_shortcut ?? null,
    selectedMicrophone: opts.selected_microphone ?? null,
    hubEnabled: opts.hub_enabled ?? null,
    hubUrl: opts.hub_url ?? null,
    hubToken: opts.hub_token ?? null,
    hubChatModel: opts.hub_chat_model ?? null,
    hubTaggingEnabled: opts.hub_tagging_enabled ?? null,
    hubTranscribeEnabled: opts.hub_transcribe_enabled ?? null,
    voicePolishEnabled: opts.voice_polish_enabled ?? null,
    voicePolishModel: opts.voice_polish_model ?? null,
    voicePolishScreenshot: opts.voice_polish_screenshot ?? null,
    voicePolishPrompt: opts.voice_polish_prompt ?? null,
    voiceTranslateLang: opts.voice_translate_lang ?? null,
    voiceDictionary: opts.voice_dictionary ?? null,
    voiceSelectedText: opts.voice_selected_text ?? null,
    boardVertical: opts.board_vertical ?? null,
    voiceTranscriptionEnabled: opts.voice_transcription_enabled ?? null,
    aiTaggingEnabled: opts.ai_tagging_enabled ?? null,
    overlayShortcutHintsEnabled: opts.overlay_shortcut_hints_enabled ?? null,
  });
}

/** Test the NeuralDeep hub connection. Returns the number of available models. */
export async function hubTestConnection(url?: string, token?: string): Promise<number> {
  return invoke("hub_test_connection", { url: url ?? null, token: token ?? null });
}

/** List available hub model ids (from /v1/models). */
export async function hubListModels(url?: string, token?: string): Promise<string[]> {
  return invoke("hub_list_models", { url: url ?? null, token: token ?? null });
}

export async function listMicrophones(): Promise<AudioInputDevice[]> {
  return invoke("list_microphones");
}

export async function rebindVoiceShortcut(): Promise<string> {
  return invoke("rebind_voice_shortcut");
}

export async function rebindPaletteShortcut(): Promise<void> {
  return invoke("rebind_palette_shortcut");
}

// ---- Quick menu (Clipy-style native menu) ----

export async function getQuickMenuShortcut(): Promise<string> {
  return invoke("get_quick_menu_shortcut");
}

/** Persist the quick-menu hotkey and re-register it. Returns the stored string. */
export async function setQuickMenuShortcut(shortcut: string): Promise<string> {
  return invoke("set_quick_menu_shortcut", { shortcut });
}

// ---- Snippets ----

export async function getSnippetFolders(): Promise<SnippetFolder[]> {
  return invoke("get_snippet_folders");
}

export async function getSnippets(): Promise<Snippet[]> {
  return invoke("get_snippets");
}

export async function createSnippetFolder(name: string): Promise<number> {
  return invoke("create_snippet_folder", { name });
}

export async function renameSnippetFolder(id: number, name: string): Promise<void> {
  return invoke("rename_snippet_folder", { id, name });
}

export async function deleteSnippetFolder(id: number): Promise<void> {
  return invoke("delete_snippet_folder", { id });
}

export async function createSnippet(
  folderId: number,
  title: string,
  content: string,
): Promise<number> {
  return invoke("create_snippet", { folderId, title, content });
}

export async function updateSnippet(id: number, title: string, content: string): Promise<void> {
  return invoke("update_snippet", { id, title, content });
}

export async function deleteSnippet(id: number): Promise<void> {
  return invoke("delete_snippet", { id });
}

export async function pasteSnippet(id: number): Promise<void> {
  return invoke("paste_snippet", { id });
}

export async function getModelCatalog(): Promise<ModelCatalog> {
  return invoke("get_model_catalog");
}

export async function getExcludedApps(): Promise<ExcludedApp[]> {
  return invoke("get_excluded_apps");
}

export async function addExcludedApp(appNameOrBundleId: string): Promise<ExcludeAppResult> {
  return invoke("add_excluded_app", { appNameOrBundleId });
}

export async function removeExcludedApp(id: number): Promise<void> {
  return invoke("remove_excluded_app", { id });
}

export async function getExcludableAppCandidate(): Promise<ExcludableAppCandidate | null> {
  return invoke("get_excludable_app_candidate");
}

export async function addExcludableAppCandidate(): Promise<ExcludeAppResult | null> {
  return invoke("add_excludable_app_candidate");
}

export async function openCommandPalette(): Promise<void> {
  return invoke("open_command_palette");
}

export async function pickAppToExclude(): Promise<ExcludeAppResult | null> {
  return invoke("pick_app_to_exclude");
}

export async function retagEntry(entryId: number): Promise<string[]> {
  return invoke("retag_entry", { entryId });
}

export async function isTaggingReady(): Promise<boolean> {
  return invoke("is_tagging_ready");
}

export async function copyText(text: string): Promise<void> {
  return invoke("copy_text", { text });
}

export async function copyEntry(entryId: number): Promise<void> {
  return invoke("copy_entry", { entryId });
}

export async function activateEntry(entryId: number): Promise<void> {
  return invoke("activate_entry", { entryId });
}

export async function openSettingsWindow(initialPane?: string): Promise<void> {
  return invoke("open_settings_window", { initialPane: initialPane ?? null });
}

export async function quitApp(): Promise<void> {
  return invoke("quit_app");
}

/** @deprecated Use activateEntry for paste-into-target behavior. */
export async function pasteEntry(text: string): Promise<void> {
  return invoke("paste_entry", { text });
}

export interface OllamaStatus {
  cli_installed: boolean;
  server_running: boolean;
  model_installed: boolean;
  model_loaded: boolean;
  model_name: string;
}

export async function checkAccessibility(prompt = false): Promise<boolean> {
  return invoke("check_accessibility", { prompt });
}

export async function openAccessibilitySettings(): Promise<void> {
  return invoke("open_accessibility_settings");
}

export async function checkOllamaStatus(): Promise<OllamaStatus> {
  return invoke("check_ollama_status");
}

export async function startOllamaServer(): Promise<boolean> {
  return invoke("start_ollama_server");
}

export async function pullOllamaModel(): Promise<void> {
  return invoke("pull_ollama_model");
}

export async function unloadOllamaModel(): Promise<boolean> {
  return invoke("unload_ollama_model");
}

export async function testOllamaTagging(): Promise<string[] | null> {
  return invoke("test_ollama_tagging");
}
