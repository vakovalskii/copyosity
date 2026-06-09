<script lang="ts">
  import type { ClipboardEntry } from "$lib/types";
  import { copyEntry, activateEntry, deleteEntry, pinEntry, retagEntry } from "$lib/api";

  let {
    entry,
    selected = false,
    onpasted,
    ondeleted,
    onpinned,
    onretagged,
    retagAvailable = false,
  }: {
    entry: ClipboardEntry;
    selected?: boolean;
    onpasted?: () => void;
    ondeleted?: () => void;
    onpinned?: () => void;
    onretagged?: () => void;
    retagAvailable?: boolean;
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

  /** GIF thumbs are stored as raw GIF bytes; PNG/JPEG thumbs use image/png. */
  function imageThumbSrc(b64: string): string {
    const mime = b64.startsWith("R0lGOD") ? "image/gif" : "image/png";
    return `data:${mime};base64,${b64}`;
  }

  function detectTextKind(text: string | null): string {
    if (!text) return "Text";

    const sample = text.trim();
    const lower = sample.toLowerCase();

    if (/^(https?:\/\/|www\.)/.test(lower)) return "URL";
    if (sample.length < 10000 && ((sample.startsWith("{") && sample.endsWith("}")) || (sample.startsWith("[") && sample.endsWith("]")))) {
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

  let copied = $state(false);
  let clickTimer: ReturnType<typeof setTimeout> | undefined;

  function handleClick() {
    if (clickTimer) {
      clearTimeout(clickTimer);
      clickTimer = undefined;
      void handleDoubleClick();
      return;
    }
    clickTimer = setTimeout(() => {
      clickTimer = undefined;
      void handleSingleClick();
    }, 250);
  }

  async function handleSingleClick() {
    if (copied) return;
    if (entry.content_type === "text" || entry.content_type === "image") {
      await copyEntry(entry.id);
      copied = true;
      setTimeout(() => {
        copied = false;
      }, 800);
    }
  }

  async function handleDoubleClick() {
    if (copied) return;
    if (entry.content_type === "text" || entry.content_type === "image") {
      await activateEntry(entry.id);
      onpasted?.();
    }
  }

  async function handleCopy(e: MouseEvent) {
    e.stopPropagation();
    if (copied) return;
    await copyEntry(entry.id);
    copied = true;
    setTimeout(() => { copied = false; }, 800);
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

  async function handleRetag(e: MouseEvent) {
    e.stopPropagation();
    await retagEntry(entry.id);
    onretagged?.();
  }

  let textKind = $derived(detectTextKind(entry.text_content));
  let typeLabel = $derived(entry.content_type === "text" ? textKind : entry.content_type === "image" ? "Image" : "File");
  let imageFormat = $derived(entry.content_type === "image" ? entry.image_format : null);
  let charLabel = $derived(entry.char_count ? `${entry.char_count.toLocaleString()} characters` : "");
  let tags = $derived(entry.tags ?? []);
  let textLineClamp = $derived(tags.length > 0 ? 8 : 9);
</script>

<div
  class="card"
  class:selected
  class:pinned={entry.is_pinned}
  class:copied
  onclick={handleClick}
  onkeydown={(e) => e.key === "Enter" && handleDoubleClick()}
  role="button"
  tabindex="0"
  title={entry.text_content ?? ""}
>
  <div class="card-header">
    <div class="card-type">
      <span class="type-label">
        <span>{typeLabel}</span>
        {#if imageFormat}<span class="format-suffix">{imageFormat}</span>{/if}
      </span>
      <span class="time">{timeAgo(entry.created_at)}</span>
    </div>
    <div class="card-actions">
      <button class="action-btn app-btn" onclick={handleCopy} title="Copy">
        <svg class="action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
        </svg>
      </button>
      {#if entry.content_type === "text" && retagAvailable}
        <button class="action-btn app-btn" onclick={handleRetag} title="Retag">
          <svg class="action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" />
            <path d="M21 3v5h-5" />
            <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" />
            <path d="M8 16H3v5" />
          </svg>
        </button>
      {/if}
      <button
        class="action-btn app-btn"
        class:pinned={entry.is_pinned}
        onclick={handlePin}
        title={entry.is_pinned ? "Unpin" : "Pin"}
      >
        <svg class="action-icon" viewBox="0 0 24 24" fill={entry.is_pinned ? "currentColor" : "none"} stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
        </svg>
      </button>
      <button class="action-btn app-btn delete" onclick={handleDelete} title="Delete">
        <svg class="action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <line x1="18" y1="6" x2="6" y2="18" />
          <line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </button>
    </div>
  </div>

  <div class="card-body">
    {#if entry.content_type === "text"}
      <div class="text-preview">
        <div class="text-content" style:--line-clamp={textLineClamp}>{entry.text_content}</div>
      </div>
    {:else if entry.content_type === "image"}
      <div class="image-preview">
        {#if entry.image_thumb}
          <img src={imageThumbSrc(entry.image_thumb)} alt="Copied content" loading="lazy" decoding="async" />
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

  {#if copied}
    <div class="copied-overlay">
      <svg class="copied-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="20 6 9 17 4 12" />
      </svg>
      <span>Copied</span>
    </div>
  {/if}
</div>

<style>
  .card {
    position: relative;
    width: 220px;
    min-width: 220px;
    height: 280px;
    background: var(--surface-card);
    border: 1px solid var(--border-strong);
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
    border-color: var(--border-accent-selected);
    background: var(--surface-card-hover);
    transform: translateY(-2px);
    box-shadow: var(--shadow-card);
  }

  .card.selected {
    border-color: var(--border-accent-ring);
    box-shadow: 0 0 0 2px var(--shadow-accent-selected);
  }

  .card.pinned {
    border-color: var(--border-warning-pinned);
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
    gap: 5px;
    width: fit-content;
    padding: 3px 8px;
    border-radius: 999px;
    background: var(--surface-7);
    font-weight: 600;
    font-size: 12px;
    letter-spacing: 0.02em;
    color: var(--color-text-body);
  }

  .format-suffix {
    font-weight: 700;
    font-size: 10px;
    letter-spacing: 0.08em;
    color: var(--color-accent-text-soft);
    text-transform: uppercase;
  }

  .time {
    font-size: 11px;
    color: var(--color-text-muted);
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
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    background: none;
    border: none;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
    border-radius: 4px;
    flex-shrink: 0;
  }

  .action-icon {
    width: 16px;
    height: 16px;
    display: block;
  }

  .action-btn:hover:not(:disabled):not([aria-busy="true"]) {
    color: var(--color-text-bright);
    background: var(--surface-10);
  }

  .action-btn.pinned {
    color: var(--color-warning-bright);
  }

  .action-btn.delete:hover:not(:disabled):not([aria-busy="true"]) {
    color: var(--color-danger);
  }

  .card-body {
    flex: 1 1 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .text-preview {
    flex: 1 1 0;
    min-height: 0;
    display: grid;
    grid-template-rows: minmax(0, 1fr);
    box-sizing: border-box;
    padding: 10px 12px;
    background: var(--surface-4);
    border: 1px solid var(--border-default);
    border-radius: 10px;
    overflow: hidden;
  }

  .text-content {
    min-height: 0;
    margin: 0;
    overflow: hidden;
    font-size: 12px;
    line-height: 1.55;
    color: var(--color-text-primary);
    white-space: pre-line;
    word-break: break-word;
    font-family: "SF Mono", "Menlo", "Monaco", monospace;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: var(--line-clamp, 9);
    line-clamp: var(--line-clamp, 9);
    text-overflow: ellipsis;
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
    border: 1px solid var(--border-soft);
    box-shadow: var(--shadow-image);
  }

  .image-placeholder {
    width: 100%;
    height: 86px;
    background: var(--surface-5);
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-subtle);
    font-size: 13px;
  }

  .image-meta {
    padding: 7px 10px;
    background: var(--surface-3);
    border: 1px solid var(--border-default);
    border-radius: 10px;
    color: var(--color-text-body);
    font-size: 11px;
    line-height: 1.45;
  }

  .card-footer {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex-shrink: 0;
    margin-top: 8px;
  }

  .footer-meta {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .source-app {
    font-size: 11px;
    color: var(--color-text-subtle);
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
    background: var(--surface-accent-tag);
    border: 1px solid var(--border-accent-tag);
    color: var(--color-accent-text-tag);
    font-size: 10px;
    line-height: 1;
    text-transform: lowercase;
  }

  .char-count {
    font-size: 11px;
    color: var(--color-text-faint);
    align-self: flex-end;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .card.copied {
    border-color: var(--border-success);
    box-shadow: 0 0 0 2px var(--shadow-success);
  }

  .copied-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    background: var(--surface-overlay);
    backdrop-filter: blur(6px);
    border-radius: 14px;
    color: var(--color-success);
    font-size: 15px;
    font-weight: 700;
    letter-spacing: 0.02em;
    animation: copied-pop 0.3s cubic-bezier(0.16, 1, 0.3, 1);
    z-index: 5;
  }

  .copied-icon {
    width: 32px;
    height: 32px;
    animation: check-draw 0.35s ease forwards;
  }

  @keyframes copied-pop {
    from {
      opacity: 0;
      transform: scale(0.9);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  @keyframes check-draw {
    from {
      stroke-dasharray: 40;
      stroke-dashoffset: 40;
    }
    to {
      stroke-dasharray: 40;
      stroke-dashoffset: 0;
    }
  }
</style>
