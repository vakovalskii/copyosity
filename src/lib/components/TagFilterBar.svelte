<script lang="ts">
  import { onMount } from "svelte";
  import type { TagChip } from "$lib/overlay-filters";

  let {
    resetLabel = "All tags",
    activeTag = null,
    formatChips = [] as TagChip[],
    semanticChips = [] as TagChip[],
    showDivider = false,
    onreset,
    onselect,
  }: {
    resetLabel?: string;
    activeTag?: string | null;
    formatChips?: TagChip[];
    semanticChips?: TagChip[];
    showDivider?: boolean;
    onreset?: () => void;
    onselect?: (tag: string) => void;
  } = $props();

  let scrollEl: HTMLDivElement | undefined = $state();
  let fadeStart = $state(false);
  let fadeEnd = $state(false);

  function updateScrollFade() {
    if (!scrollEl) return;
    const { scrollLeft, scrollWidth, clientWidth } = scrollEl;
    fadeStart = scrollLeft > 1;
    fadeEnd = scrollLeft + clientWidth < scrollWidth - 1;
  }

  function scrollFadeMask(start: boolean, end: boolean): string {
    if (!start && !end) return "none";
    if (start && end) {
      return "linear-gradient(90deg, transparent, #000 16px, #000 calc(100% - 16px), transparent)";
    }
    if (end) {
      return "linear-gradient(90deg, #000 calc(100% - 16px), transparent)";
    }
    return "linear-gradient(90deg, transparent, #000 16px, #000)";
  }

  let scrollMask = $derived(scrollFadeMask(fadeStart, fadeEnd));

  $effect(() => {
    void formatChips;
    void semanticChips;
    void showDivider;
    void resetLabel;
    queueMicrotask(updateScrollFade);
  });

  onMount(() => {
    updateScrollFade();
    const ro = scrollEl ? new ResizeObserver(updateScrollFade) : null;
    ro?.observe(scrollEl!);
    return () => ro?.disconnect();
  });
</script>

<div class="tag-filter-bar">
  <div
    class="tag-filter-scroll"
    bind:this={scrollEl}
    style:mask-image={scrollMask}
    style:-webkit-mask-image={scrollMask}
    onscroll={updateScrollFade}
  >
    <button
      type="button"
      class="tag-chip tag-chip-reset app-btn"
      class:active={!activeTag}
      aria-pressed={!activeTag}
      onclick={() => onreset?.()}
    >
      {resetLabel}
    </button>

    {#each formatChips as [tag, count] (tag)}
      <button
        type="button"
        class="tag-chip tag-chip-format app-btn"
        class:active={activeTag === tag}
        aria-pressed={activeTag === tag}
        onclick={() => onselect?.(tag)}
      >
        <svg class="format-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
          <circle cx="8.5" cy="8.5" r="1.5" />
          <polyline points="21 15 16 10 5 21" />
        </svg>
        <span>{tag}</span>
        <span class="tag-count">{count}</span>
      </button>
    {/each}

    {#if showDivider}
      <span class="tag-filter-divider" role="separator" aria-hidden="true"></span>
    {/if}

    {#each semanticChips as [tag, count] (tag)}
      <button
        type="button"
        class="tag-chip tag-chip-semantic app-btn"
        class:active={activeTag === tag}
        aria-pressed={activeTag === tag}
        onclick={() => onselect?.(tag)}
      >
        <span>{tag}</span>
        <span class="tag-count">{count}</span>
      </button>
    {/each}
  </div>
</div>

<style>
  .tag-filter-bar {
    flex-shrink: 0;
    padding: 0 16px 10px;
  }

  .tag-filter-scroll {
    display: flex;
    align-items: center;
    gap: 8px;
    overflow-x: auto;
    scrollbar-width: none;
    padding: 2px 0;
  }

  .tag-filter-scroll::-webkit-scrollbar {
    display: none;
  }

  .tag-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 11px;
    border-radius: 999px;
    border: 1px solid var(--border-soft);
    cursor: pointer;
    white-space: nowrap;
    font: inherit;
    font-size: 12px;
    line-height: 1.2;
    flex-shrink: 0;
    transition:
      background var(--duration-fast) var(--ease-interactive),
      border-color var(--duration-fast) var(--ease-interactive),
      color var(--duration-fast) var(--ease-interactive);
  }

  .tag-chip-reset {
    background: var(--surface-3);
    color: var(--color-text-secondary);
  }

  .tag-chip-reset:hover:not(:disabled):not([aria-busy="true"]) {
    background: var(--surface-7);
    border-color: var(--border-strong);
  }

  .tag-chip-reset.active {
    background: var(--surface-accent);
    border-color: var(--border-accent-soft);
    color: var(--color-accent-chip);
  }

  .tag-chip-format {
    background: var(--surface-5);
    border-color: var(--border-default);
    color: var(--color-text-muted);
    font-family: "SF Mono", "Menlo", "Monaco", monospace;
    text-transform: lowercase;
  }

  .tag-chip-format:hover:not(:disabled):not([aria-busy="true"]) {
    background: var(--surface-7);
    border-color: var(--border-strong);
    color: var(--color-text-body);
  }

  .tag-chip-format.active {
    background: var(--surface-accent);
    border-color: var(--border-accent-soft);
    color: var(--color-accent-chip);
  }

  .tag-chip-format.active .format-icon {
    opacity: 1;
  }

  .tag-chip-format.active .tag-count {
    background: rgb(var(--rgb-accent) / 0.28);
    color: var(--color-accent-chip);
  }

  .tag-chip-semantic {
    background: var(--surface-3);
    color: var(--color-text-secondary);
    text-transform: lowercase;
  }

  .tag-chip-semantic:hover:not(:disabled):not([aria-busy="true"]) {
    background: var(--surface-7);
    border-color: var(--border-strong);
  }

  .tag-chip-semantic.active {
    background: var(--surface-accent);
    border-color: var(--border-accent-soft);
    color: var(--color-accent-chip);
  }

  .tag-chip-semantic.active .tag-count {
    background: rgb(var(--rgb-accent) / 0.28);
    color: var(--color-accent-chip);
  }

  .format-icon {
    width: 12px;
    height: 12px;
    flex-shrink: 0;
    opacity: 0.85;
  }

  .tag-count {
    display: inline-flex;
    min-width: 18px;
    justify-content: center;
    padding: 2px 5px;
    border-radius: 999px;
    background: var(--surface-8);
    font-size: 10px;
    line-height: 1;
    font-family: inherit;
  }

  .tag-filter-divider {
    flex-shrink: 0;
    align-self: center;
    width: 1px;
    height: 18px;
    margin: 0 2px;
    border-radius: 1px;
    background: var(--border-emphasis);
    box-shadow: 0 0 0 1px rgb(var(--rgb-white) / 0.06);
    opacity: 0.95;
  }
</style>
