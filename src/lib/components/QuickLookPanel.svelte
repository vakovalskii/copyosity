<script lang="ts">
  import { tick } from "svelte";
  import type { ClipboardEntry } from "$lib/types";
  import { getEntry } from "$lib/api";
  import { cardDisplayTags } from "$lib/overlay-filters";
  import { cardTagDisplayLabel } from "$lib/card-tag-label";
  import {
    imageDataUrl,
    imageOcrPreviewText,
    resolveImageFooterMetaParts,
    resolveImageFormatBadge,
  } from "$lib/image-meta";
  import CardTypeBadge from "$lib/components/CardTypeBadge.svelte";
  import { detectTextKind, usesMonoPreview } from "$lib/text-kind";
  import {
    initialQuickLookImageTab,
    resolveFullImagePayload,
    shouldApplyFullImageResult,
  } from "$lib/quick-look-image-fetch";
  import SegmentControl from "$lib/components/SegmentControl.svelte";

  const {
    entry,
    aiTaggingEnabled = false,
    compact = false,
    showHints = true,
    onclose,
  }: {
    entry: ClipboardEntry;
    aiTaggingEnabled?: boolean;
    /** Narrow vertical board: shrink the mode segment and use short IMG/OCR labels. */
    compact?: boolean;
    /** Mirrors the overlay's "Keyboard shortcuts" setting — hides the Tab-switch hint row. */
    showHints?: boolean;
    onclose: () => void;
  } = $props();

  let dialogEl: HTMLDivElement | null = $state(null);
  let closeButtonEl = $state<HTMLButtonElement | null>(null);
  let imageTabEl = $state<HTMLButtonElement | null>(null);
  let textTabEl = $state<HTMLButtonElement | null>(null);
  /** Only meaningful when `showTabs` is true; reset to "image" whenever the entry changes. */
  let activeTab = $state<"image" | "text">("image");
  /** Lazy full-resolution fetch — `get_entries` omits `image_data` for list-fetch cost. */
  let fullImageB64 = $state<string | null>(null);
  let fullImageFetchFailed = $state(false);
  let fullImageRequestSeq = 0;

  const textKind = $derived(detectTextKind(entry.text_content));
  const isMonoPreview = $derived(usesMonoPreview(textKind));
  const tags = $derived(cardDisplayTags(entry, aiTaggingEnabled));
  const tagLabels = $derived(tags.map(cardTagDisplayLabel));
  const ocrText = $derived(imageOcrPreviewText(entry.content_type, entry.ocr_text));
  const showTabs = $derived(entry.content_type === "image" && ocrText.length > 0);
  const showTabHint = $derived(showTabs && showHints);
  const imageFormatBadge = $derived(
    entry.content_type === "image"
      ? resolveImageFormatBadge(entry.image_format, entry.image_thumb)
      : null,
  );
  const imageFooterMeta = $derived(
    entry.content_type === "image"
      ? resolveImageFooterMetaParts(entry.image_width, entry.image_height, entry.image_byte_size)
      : null,
  );
  const charLabel = $derived(
    entry.char_count ? `${entry.char_count.toLocaleString()} characters` : null,
  );
  const dialogLabel = $derived(
    entry.content_type === "image" ? "Image entry preview" : "Text entry preview",
  );
  const typeLabel = $derived(entry.content_type === "image" ? "Image" : "Text");
  /** Announced on open and whenever the previewed entry changes via arrow-key browsing. */
  const liveAnnouncement = $derived(
    `${typeLabel} preview${entry.source_app ? ` from ${entry.source_app}` : ""}`,
  );
  /** Full-res once the lazy fetch resolves; the list thumb until then (avoids a blank flash). */
  const imageSrc = $derived.by(() => {
    const b64 = fullImageB64 ?? entry.image_thumb;
    return b64 ? imageDataUrl(b64) : null;
  });

  $effect(() => {
    const id = entry.id;
    activeTab = initialQuickLookImageTab();
    fullImageB64 = null;
    fullImageFetchFailed = false;
    if (entry.content_type !== "image") return;

    const requestSeq = ++fullImageRequestSeq;
    let cancelled = false;

    void (async () => {
      try {
        const full = await getEntry(id);
        if (!shouldApplyFullImageResult(requestSeq, fullImageRequestSeq, cancelled)) return;
        const payload = resolveFullImagePayload(full?.image_data);
        if (payload) {
          fullImageB64 = payload;
          return;
        }
        if (!entry.image_thumb) fullImageFetchFailed = true;
      } catch {
        if (shouldApplyFullImageResult(requestSeq, fullImageRequestSeq, cancelled)) {
          fullImageFetchFailed = true;
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    // Re-run whenever `entry.id` changes so arrow-key browsing keeps focus on the dialog.
    void entry.id;
    void (async () => {
      await tick();
      dialogEl?.focus({ preventScroll: true });
    })();
  });

  $effect(() => {
    const root = document.documentElement;
    const body = document.body;
    const prevRootOverflow = root.style.overflow;
    const prevBodyOverflow = body.style.overflow;
    root.style.overflow = "hidden";
    body.style.overflow = "hidden";

    return () => {
      root.style.overflow = prevRootOverflow;
      body.style.overflow = prevBodyOverflow;
    };
  });

  function focusableControls(): HTMLButtonElement[] {
    const controls = [closeButtonEl];
    if (showTabs) {
      controls.push(imageTabEl, textTabEl);
    }
    return controls.filter((button): button is HTMLButtonElement => button !== null);
  }

  function trapDialogKey(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();
  }

  function handleWindowKeydown(e: KeyboardEvent) {
    const target = e.target;

    if (e.key === "Enter") {
      trapDialogKey(e);
      return;
    }

    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f") {
      trapDialogKey(e);
      return;
    }

    if (
      e.key === "/" &&
      !e.metaKey &&
      !e.ctrlKey &&
      !e.altKey &&
      !(target instanceof HTMLInputElement) &&
      !(target instanceof HTMLTextAreaElement)
    ) {
      trapDialogKey(e);
      return;
    }

    if (showTabs && e.key === "Tab" && !e.metaKey && !e.ctrlKey && !e.altKey) {
      handleDialogKeydown(e);
      return;
    }

    if (e.key !== "Tab") return;

    const buttons = focusableControls();
    if (buttons.length === 0) {
      trapDialogKey(e);
      return;
    }

    const first = buttons[0];
    const last = buttons[buttons.length - 1];
    const active = document.activeElement;
    const activeIsButton = buttons.includes(active as HTMLButtonElement);
    const activeInsideDialog =
      active instanceof Node && dialogEl ? dialogEl.contains(active) : false;

    if (activeInsideDialog && !activeIsButton) {
      return;
    }

    if (!activeInsideDialog || !activeIsButton) {
      trapDialogKey(e);
      if (e.shiftKey) {
        last.focus({ preventScroll: true });
      } else {
        first.focus({ preventScroll: true });
      }
      return;
    }

    if (e.shiftKey && active === first) {
      trapDialogKey(e);
      last.focus({ preventScroll: true });
      return;
    }

    if (!e.shiftKey && active === last) {
      trapDialogKey(e);
      first.focus({ preventScroll: true });
    }
  }

  function handleBackdropClick() {
    onclose();
  }

  function handleDialogClick(e: MouseEvent) {
    e.stopPropagation();
  }

  /** `Tab` cycles Image ↔ Recognised text; `←`/`→` stay on entry browse. */
  function handleDialogKeydown(e: KeyboardEvent) {
    if (!showTabs) return;
    if (e.key !== "Tab" || e.shiftKey || e.metaKey || e.ctrlKey || e.altKey) return;
    e.preventDefault();
    e.stopPropagation();
    activeTab = activeTab === "image" ? "text" : "image";
  }
</script>

<svelte:window onkeydowncapture={handleWindowKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="quicklook-backdrop" onclick={handleBackdropClick}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={dialogEl}
    class="quicklook-dialog"
    class:quicklook-dialog--horizontal={!compact}
    role="dialog"
    aria-modal="true"
    aria-label={dialogLabel}
    tabindex="-1"
    data-quick-look-dialog=""
    onclick={handleDialogClick}
    onkeydown={handleDialogKeydown}
  >
    <span class="sr-only" role="status" aria-live="polite">{liveAnnouncement}</span>

    <header class="quicklook-header">
      <div class="quicklook-header-row">
        <CardTypeBadge contentType={entry.content_type} formatLabel={imageFormatBadge} />
        {#if showTabs}
          <SegmentControl
            ariaLabel="Preview mode"
            ariaKind="selected"
            rootClass={compact ? "quicklook-mode-segment quicklook-mode-segment--compact" : "quicklook-mode-segment"}
            items={[
              { id: "image", label: compact ? "IMG" : "Image", title: "Image preview" },
              { id: "text", label: compact ? "OCR" : "Recognised text", title: "Recognised text" },
            ]}
            value={activeTab}
            compact={compact}
            onSelect={(id) => (activeTab = id === "image" ? "image" : "text")}
            onButtonMount={(id, el) => (id === "image" ? (imageTabEl = el) : (textTabEl = el))}
          />
        {:else}
          <div
            class="quicklook-mode-segment-spacer"
            class:quicklook-mode-segment-spacer--compact={compact}
            aria-hidden="true"
          ></div>
        {/if}
        <button
          bind:this={closeButtonEl}
          class="quicklook-close app-btn"
          type="button"
          aria-label="Close preview"
          title="Close preview · Space or Esc"
          onclick={onclose}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M5 5 19 19M19 5 5 19" />
          </svg>
        </button>
      </div>
      <div class="quicklook-header-tags">
        {#if tags.length > 0}
          {#each tagLabels as label, index (tags[index])}
            <span class="entry-tag">{label}</span>
          {/each}
        {/if}
      </div>
    </header>

    <div class="quicklook-body">
      {#if entry.content_type === "image"}
        {#if activeTab === "text" && showTabs}
          <div class="quicklook-text-preview">
            <p class="quicklook-text quicklook-text--ocr">{ocrText}</p>
          </div>
        {:else}
          <div class="quicklook-image-preview">
            {#if imageSrc}
              <img src={imageSrc} alt="Copied content" />
            {:else}
              <div class="quicklook-image-placeholder">Image</div>
            {/if}
            {#if fullImageFetchFailed}
              <p class="quicklook-fetch-warning" role="status">
                Full-resolution image unavailable — showing preview thumbnail.
              </p>
            {/if}
          </div>
        {/if}
      {:else}
        <div class="quicklook-text-preview">
          <p class="quicklook-text" class:mono={isMonoPreview}>{entry.text_content ?? ""}</p>
        </div>
      {/if}
    </div>

    <footer class="quicklook-footer">
      <div class="quicklook-footer-meta">
        {#if entry.source_app}
          <span class="source-app">{entry.source_app}</span>
        {/if}
        {#if charLabel}
          <span class="meta">{charLabel}</span>
        {:else if imageFooterMeta}
          <span class="meta">
            {#if imageFooterMeta.dimensions}{imageFooterMeta.dimensions}{/if}
            {#if imageFooterMeta.dimensions && imageFooterMeta.byteSize}
              <span aria-hidden="true"> · </span>
            {/if}
            {#if imageFooterMeta.byteSize}{imageFooterMeta.byteSize}{/if}
          </span>
        {/if}
      </div>
      {#if showTabHint}
        <div class="quicklook-footer-hints">
          <p class="quicklook-tab-hint" aria-hidden="true">
            <kbd class="hint-kbd">Tab</kbd>
            <span class="hint-action">switch view</span>
          </p>
        </div>
      {/if}
    </footer>
  </div>
</div>

<style>
  .quicklook-backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
    background: var(--surface-scrim);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
    border-radius: inherit;
  }

  @media (prefers-reduced-transparency: reduce) {
    .quicklook-backdrop {
      backdrop-filter: none;
      -webkit-backdrop-filter: none;
    }
  }

  .quicklook-dialog {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--surface-menu);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-surface);
    box-shadow: var(--shadow-elevated);
    overflow: hidden;
  }

  /* Horizontal overlay: one stable width for text and image while arrow-key browsing. */
  .quicklook-dialog--horizontal {
    max-width: 800px;
  }

  .quicklook-dialog:focus-visible {
    outline: none;
  }

  .quicklook-header {
    display: flex;
    flex-direction: column;
    gap: 8px;
    flex-shrink: 0;
    padding: 12px var(--radius-card-padding) 10px;
  }

  /* Three-column row: type chip | centered segments | close button. */
  .quicklook-header-row {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: var(--space-stack);
    min-height: var(--overlay-header-control-height);
  }

  .quicklook-header-row > :global(.card-type-badge) {
    grid-column: 1;
    justify-self: start;
  }

  .quicklook-header-row > :global(.quicklook-mode-segment) {
    grid-column: 2;
    justify-self: center;
  }

  .quicklook-header-row > .quicklook-close {
    grid-column: 3;
    justify-self: end;
  }

  /* Fixed two-column grid so "Image" / "Recognised text" never resize the track. */
  :global(.quicklook-mode-segment) {
    width: 15.75rem;
    flex-shrink: 0;
  }

  /* Vertical board dialog is docked-narrow (~360px) — the full-width English labels
     ("Image" / "Recognised text") never fit next to the type chip and close button. */
  :global(.quicklook-mode-segment--compact) {
    width: 6.5rem;
  }

  /* Reserve segment track space when OCR tabs are absent so header level-1 height stays stable. */
  .quicklook-mode-segment-spacer {
    grid-column: 2;
    justify-self: center;
    width: 15.75rem;
    height: var(--overlay-header-control-height);
    flex-shrink: 0;
    pointer-events: none;
    visibility: hidden;
  }

  .quicklook-mode-segment-spacer--compact {
    width: 6.5rem;
  }

  .quicklook-header-tags {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--space-row);
    /* One .entry-tag row — keeps level-2 height stable when tags are absent. */
    min-height: calc(var(--font-size-2xs) * 1.3 + 4px + 2px);
  }

  .entry-tag {
    display: inline-flex;
    align-items: center;
    padding: var(--card-entry-tag-pad);
    border-radius: var(--radius-control-sm);
    border: 1px solid var(--border-entry-tag);
    background: var(--surface-entry-tag);
    color: var(--color-entry-tag);
    font-size: var(--font-size-2xs);
    line-height: 1.3;
    font-weight: 500;
    text-transform: lowercase;
  }

  .quicklook-close {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--overlay-header-control-height);
    height: var(--overlay-header-control-height);
    border-radius: var(--radius-control);
    background: transparent;
    border: none;
    color: var(--color-text-secondary);
    cursor: pointer;
  }

  .quicklook-close svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentcolor;
    stroke-width: 2;
    stroke-linecap: round;
  }

  .quicklook-close:hover {
    background: var(--surface-3);
    color: var(--color-text-body);
  }

  .quicklook-close:focus-visible {
    outline: none;
    box-shadow: var(--ring-accent);
  }

  .quicklook-body {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-stack);
    padding: 0 var(--radius-card-padding) var(--space-stack);
    overflow-y: auto;
  }

  .quicklook-text-preview {
    flex: 1 1 auto;
    min-height: 0;
    box-sizing: border-box;
    padding: var(--card-text-preview-pad);
    background: var(--surface-4);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-inset);
    overflow: auto;
  }

  .quicklook-text {
    margin: 0;
    font-size: var(--font-size-md);
    line-height: 1.6;
    color: var(--color-text-primary);
    white-space: pre-wrap;
    word-break: normal;
    overflow-wrap: break-word;
  }

  .quicklook-text.mono {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
  }

  .quicklook-text--ocr {
    color: var(--color-text-secondary);
  }

  .quicklook-image-preview {
    position: relative;
    flex: 1 1 auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 0;
    border-radius: var(--radius-inset);
    background: var(--surface-4);
    border: 1px solid var(--border-default);
    overflow: hidden;
  }

  .quicklook-image-preview img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    display: block;
    flex: 1 1 auto;
    min-height: 0;
  }

  .quicklook-fetch-warning {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    margin: 0;
    padding: 8px 12px;
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    text-align: center;
    background: var(--surface-scrim);
  }

  .quicklook-image-placeholder {
    width: 100%;
    padding: 60px 0;
    text-align: center;
    color: var(--color-text-subtle);
    font-size: var(--font-size-md);
  }

  /* Two tiers so meta info (source app, size) never crowds the Tab-switch instruction —
     the vertical board's narrow (~318px) dialog has no room for both on one row. */
  .quicklook-footer {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    padding: 10px var(--radius-card-padding);
    border-top: 1px solid var(--border-default);
  }

  .quicklook-footer-meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-height: 1.125rem;
  }

  .quicklook-footer-hints {
    display: flex;
    justify-content: center;
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--border-default);
  }

  .quicklook-tab-hint {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0;
    margin: 0;
    font-size: var(--font-size-xs);
    white-space: nowrap;
  }

  .hint-kbd {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    box-sizing: border-box;
    height: 1.125rem;
    min-width: 1.125rem;
    margin-right: 0.3125rem;
    padding: 0 6px;
    font-family: inherit;
    font-size: 1em;
    font-weight: 500;
    line-height: 1;
    border-radius: var(--radius-code);
    background: var(--surface-kbd);
    border: 1px solid var(--border-kbd);
    color: var(--color-text-subtle);
    box-shadow: var(--shadow-kbd);
  }

  .hint-action {
    color: var(--color-text-faint);
    font-weight: 400;
  }

  .source-app {
    min-width: 0;
    font-size: var(--font-size-xs);
    color: var(--color-text-subtle);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    flex-shrink: 0;
    margin-left: auto;
    font-size: var(--font-size-xs);
    font-variant-numeric: tabular-nums;
    color: var(--color-text-faint);
    white-space: nowrap;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
    border: 0;
  }
</style>
