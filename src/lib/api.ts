import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, AudioInputDevice, ClipboardEntry, Collection, ExcludedApp, ModelCatalog } from "./types";

export async function getEntries(opts?: {
  limit?: number;
  offset?: number;
  collection_id?: number | null;
  pinned_only?: boolean;
  search?: string | null;
}): Promise<ClipboardEntry[]> {
  return invoke("get_entries", {
    limit: opts?.limit ?? 50,
    offset: opts?.offset ?? 0,
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

export async function setEntryCollection(entryId: number, collectionId: number | null): Promise<void> {
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
}): Promise<AppSettings> {
  return invoke("update_app_settings", {
    ollamaModel: opts.ollama_model ?? null,
    retentionDays: opts.retention_days ?? null,
    whisperServerUrl: opts.whisper_server_url ?? null,
    whisperServerToken: opts.whisper_server_token ?? null,
    whisperServerModel: opts.whisper_server_model ?? null,
    voiceShortcut: opts.voice_shortcut ?? null,
    selectedMicrophone: opts.selected_microphone ?? null,
  });
}

export async function listMicrophones(): Promise<AudioInputDevice[]> {
  return invoke("list_microphones");
}

export async function rebindVoiceShortcut(): Promise<string> {
  return invoke("rebind_voice_shortcut");
}

export async function getModelCatalog(): Promise<ModelCatalog> {
  return invoke("get_model_catalog");
}

export async function getExcludedApps(): Promise<ExcludedApp[]> {
  return invoke("get_excluded_apps");
}

export async function addExcludedApp(bundleId: string): Promise<void> {
  return invoke("add_excluded_app", { bundleId });
}

export async function removeExcludedApp(id: number): Promise<void> {
  return invoke("remove_excluded_app", { id });
}

export async function addFrontmostAppToExcluded(): Promise<string | null> {
  return invoke("add_frontmost_app_to_excluded");
}

export async function retagEntry(entryId: number): Promise<void> {
  return invoke("retag_entry", { entryId });
}

export async function copyEntry(entryId: number): Promise<void> {
  return invoke("copy_entry", { entryId });
}

export async function activateEntry(entryId: number): Promise<void> {
  return invoke("activate_entry", { entryId });
}

export async function openSettingsWindow(): Promise<void> {
  return invoke("open_settings_window");
}

export async function quitApp(): Promise<void> {
  return invoke("quit_app");
}

export async function pasteEntry(text: string): Promise<void> {
  return invoke("paste_entry", { text });
}

export interface OllamaStatus {
  cli_installed: boolean;
  server_running: boolean;
  model_installed: boolean;
  model_name: string;
}

export async function checkAccessibility(): Promise<boolean> {
  return invoke("check_accessibility");
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
