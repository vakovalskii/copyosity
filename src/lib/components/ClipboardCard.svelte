<script lang="ts">
  import type { ClipboardEntry } from "$lib/types";
  import { pasteEntry, deleteEntry, pinEntry } from "$lib/api";

  let {
    entry,
    selected = false,
    onpasted,
    ondeleted,
    onpinned,
  }: {
    entry: ClipboardEntry;
    selected?: boolean;
    onpasted?: () => void;
    ondeleted?: () => void;
    onpinned?: () => void;
  } = $props();

  function timeAgo(dateStr: string): string {
    const now = Date.now();
    const then = new Date(dateStr).getTime();
    const diff = Math.floor((now - then) / 1000);

    if (diff < 60) return "just now";
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
    return new Date(dateStr).toLocaleDateString();
  }

  function truncate(text: string, max: number): string {
    if (text.length <= max) return text;
    return text.slice(0, max) + "...";
  }

  function detectTextKind(text: string | null): string {
    if (!text) return "Text";

    const sample = text.trim();
    const lower = sample.toLowerCase();

    if (/^(https?:\/\/|www\.)/.test(lower)) return "URL";
    if ((sample.startsWith("{") && sample.endsWith("}")) || (sample.startsWith("[") && sample.endsWith("]"))) {
      try {
        JSON.parse(sample);
        return "JSON";
      } catch {
        // fall through
      }
    }
    if (/^#!\/.*\b(bash|sh|zsh)\b/.test(lower)) return "Shell";
    if (/^(\$|#)\s+\S+/.test(sample) || /\b(curl|git|npm|pnpm|yarn|brew|ssh|docker|kubectl)\b/.test(lower)) {
      return "Bash";
    }
    if (/(^|\n)\s*(select|insert|update|delete|create table|alter table)\b/.test(lower)) return "SQL";
    if (/<[a-z][\s\S]*>/.test(lower)) return "HTML";
    if (/\b(function|const|let|import|export|=>)\b/.test(lower)) return "JavaScript";
    if (/\b(interface|type\s+\w+|implements|enum)\b/.test(lower)) return "TypeScript";
    if (/(^|\n)\s*(def |class |import |from .+ import )/.test(sample)) return "Python";
    if (/(^|\n)\s*(fn |let mut |impl |pub struct )/.test(sample)) return "Rust";

    return "Text";
  }

  async function handleClick() {
    if (entry.text_content) {
      await pasteEntry(entry.text_content);
      onpasted?.();
    }
  }

  async function handleDelete(e: MouseEvent) {
    e.stopPropagation();
    await deleteEntry(entry.id);
    ondeleted?.();
  }

  async function handlePin(e: MouseEvent) {
    e.stopPropagation();
    await pinEntry(entry.id, !entry.is_pinned);
    onpinned?.();
  }

  let preview = $derived(entry.text_content ? truncate(entry.text_content, 200) : "");
  let textKind = $derived(detectTextKind(entry.text_content));
  let typeLabel = $derived(entry.content_type === "text" ? textKind : entry.content_type === "image" ? "Image" : "File");
  let charLabel = $derived(entry.char_count ? `${entry.char_count.toLocaleString()} characters` : "");
  let tags = $derived(entry.tags ?? []);
</script>

<div
  class="card"
  class:selected
  class:pinned={entry.is_pinned}
  onclick={handleClick}
  onkeydown={(e) => e.key === 'Enter' && handleClick()}
  role="button"
  tabindex="0"
  title={entry.text_content ?? ""}
>
  <div class="card-header">
    <div class="card-type">
      <span class="type-label">{typeLabel}</span>
      <span class="time">{timeAgo(entry.created_at)}</span>
    </div>
    <div class="card-actions">
      <button class="action-btn" onclick={handlePin} title={entry.is_pinned ? "Unpin" : "Pin"}>
        {entry.is_pinned ? "★" : "☆"}
      </button>
      <button class="action-btn delete" onclick={handleDelete} title="Delete">×</button>
    </div>
  </div>

  <div class="card-body">
    {#if entry.content_type === "text"}
      <pre class="text-preview">{preview}</pre>
    {:else if entry.content_type === "image"}
      <div class="image-preview">
        {#if entry.image_thumb}
          <img src="data:image/png;base64,{entry.image_thumb}" alt="Copied content" />
        {:else}
          <div class="image-placeholder">Image</div>
        {/if}
        <div class="image-meta">
          Image preview
        </div>
      </div>
    {/if}
  </div>

  <div class="card-footer">
    <div class="footer-meta">
      {#if entry.source_app}
        <span class="source-app">{entry.source_app}</span>
      {/if}
      {#if tags.length > 0}
        <div class="tags">
          {#each tags.slice(0, 3) as tag}
            <span class="tag-chip">{tag}</span>
          {/each}
        </div>
      {/if}
    </div>
    {#if charLabel}
      <span class="char-count">{charLabel}</span>
    {/if}
  </div>
</div>

<style>
  .card {
    width: 220px;
    min-width: 220px;
    height: 280px;
    background: linear-gradient(180deg, rgba(58, 58, 66, 0.92), rgba(36, 36, 44, 0.88));
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 14px;
    padding: 12px;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    transition: all 0.15s ease;
    font-family: inherit;
    color: inherit;
    text-align: left;
    overflow: hidden;
    flex-shrink: 0;
  }

  .card:hover {
    border-color: rgba(120, 160, 255, 0.45);
    background: linear-gradient(180deg, rgba(66, 66, 76, 0.96), rgba(42, 42, 50, 0.92));
    transform: translateY(-2px);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.28);
  }

  .card.selected {
    border-color: rgba(100, 140, 255, 0.7);
    box-shadow: 0 0 0 2px rgba(100, 140, 255, 0.3);
  }

  .card.pinned {
    border-color: rgba(255, 200, 50, 0.3);
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 8px;
    flex-shrink: 0;
  }

  .card-type {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .type-label {
    display: inline-flex;
    align-items: center;
    width: fit-content;
    padding: 3px 8px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.07);
    font-weight: 600;
    font-size: 12px;
    letter-spacing: 0.02em;
    color: #e0e0e0;
  }

  .time {
    font-size: 11px;
    color: #888;
  }

  .card-actions {
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .card:hover .card-actions {
    opacity: 1;
  }

  .action-btn {
    background: none;
    border: none;
    color: #888;
    cursor: pointer;
    font-size: 16px;
    padding: 2px 4px;
    border-radius: 4px;
    line-height: 1;
  }

  .action-btn:hover {
    color: #fff;
    background: rgba(255, 255, 255, 0.1);
  }

  .action-btn.delete:hover {
    color: #ff6b6b;
  }

  .card-body {
    flex: 1;
    overflow: hidden;
    margin-bottom: 8px;
  }

  .text-preview {
    padding: 10px 12px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 10px;
    font-size: 12px;
    line-height: 1.55;
    color: #f0f0f2;
    white-space: pre-wrap;
    word-break: break-word;
    margin: 0;
    font-family: "SF Mono", "Menlo", "Monaco", monospace;
    overflow: hidden;
    max-height: 100%;
  }

  .image-preview {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .image-preview img {
    width: 100%;
    height: 86px;
    border-radius: 10px;
    object-fit: cover;
    display: block;
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.22);
  }

  .image-placeholder {
    width: 100%;
    height: 86px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #666;
    font-size: 13px;
  }

  .image-meta {
    padding: 7px 10px;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 10px;
    color: #d8d8dd;
    font-size: 11px;
    line-height: 1.45;
  }

  .card-footer {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 8px;
    flex-shrink: 0;
  }

  .footer-meta {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .source-app {
    font-size: 11px;
    color: #666;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tags {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .tag-chip {
    display: inline-flex;
    align-items: center;
    padding: 3px 7px;
    border-radius: 999px;
    background: rgba(90, 138, 255, 0.14);
    border: 1px solid rgba(120, 160, 255, 0.18);
    color: #cdddff;
    font-size: 10px;
    line-height: 1;
    text-transform: lowercase;
  }

  .char-count {
    font-size: 11px;
    color: #555;
  }
</style>
