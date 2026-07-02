<script lang="ts">
  import { onMount } from "svelte";
  import type { TagChip } from "$lib/overlay-filters";

  const {
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
    const fade = "var(--overlay-grid-pad-x)";
    if (!start && !end) return "none";
    if (start && end) {
      return `linear-gradient(90deg, transparent, #000 ${fade}, #000 calc(100% - ${fade}), transparent)`;
    }
    if (end) {
      return `linear-gradient(90deg, #000 calc(100% - ${fade}), transparent)`;
    }
    return `linear-gradient(90deg, transparent, #000 ${fade}, #000)`;
  }

  const scrollMask = $derived(scrollFadeMask(fadeStart, fadeEnd));

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
      class="filter-chip filter-chip-reset app-btn"
      class:active={!activeTag}
      aria-pressed={!activeTag}
      onclick={() => onreset?.()}
    >
      {resetLabel}
    </button>

    {#each formatChips as [tag, count] (tag)}
      <button
        type="button"
        class="filter-chip filter-chip-format app-btn"
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
        class="filter-chip filter-chip-semantic app-btn"
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
  .format-icon {
    width: var(--icon-size-chip);
    height: var(--icon-size-chip);
    opacity: 0.85;
  }
</style>
