import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, ClipboardEntry, Collection, ModelCatalog } from "./types";

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

export async function getAppSettings(): Promise<AppSettings> {
  return invoke("get_app_settings");
}

export async function updateAppSettings(opts: {
  ollama_model?: string | null;
  retention_days?: number | null;
}): Promise<AppSettings> {
  return invoke("update_app_settings", {
    ollamaModel: opts.ollama_model ?? null,
    retentionDays: opts.retention_days ?? null,
  });
}

export async function getModelCatalog(): Promise<ModelCatalog> {
  return invoke("get_model_catalog");
}

export async function pasteEntry(text: string): Promise<void> {
  return invoke("paste_entry", { text });
}
