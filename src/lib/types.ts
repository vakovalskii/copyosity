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
}
