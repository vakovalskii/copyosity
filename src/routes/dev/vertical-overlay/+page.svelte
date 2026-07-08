<script lang="ts">
  import { onMount } from "svelte";
  import type { ClipboardEntry } from "$lib/types";
  import ClipboardCard from "$lib/components/ClipboardCard.svelte";
  import QuickLookPanel from "$lib/components/QuickLookPanel.svelte";
  import {
    isCardOffScreenVertical,
    nextIndexAfterKeyboardArrow,
    verticalCardViewportPosition,
    verticalScrollDeltaForKeyboardNav,
    verticalScrollDeltaToSnapCard,
  } from "$lib/overlay-grid-scroll";
  import { shouldLoadNextEntryPage } from "$lib/overlay-pagination";

  /**
   * Real-component regression harness for the vertical board (mirrors +page.svelte's
   * boardVertical wiring exactly, minus Tauri IPC) — reproduces the `.clipboard-card-host`
   * (display: contents) DOM nesting so the same `scrollMeasureEl` bug/fix applies here too.
   */

  const PAGE_SIZE = 10;
  const TOTAL = 30;

  const TINY_PNG_B64 =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

  function makeEntry(i: number): ClipboardEntry {
    if (i === 3) {
      return {
        id: i,
        content_type: "image",
        text_content: null,
        image_data: null,
        image_thumb: TINY_PNG_B64,
        source_app: "Preview",
        source_app_icon: null,
        content_hash: `hash-${i}`,
        char_count: null,
        created_at: new Date().toISOString(),
        is_pinned: false,
        collection_id: null,
        tags: ["screenshot"],
        ocr_text: "Choose Application…\nmeeting\nui",
        image_format: "png",
        image_width: 800,
        image_height: 600,
        image_byte_size: 245_000,
      };
    }
    return {
      id: i,
      content_type: "text",
      text_content: `Entry ${i + 1} — clipboard row content for scroll testing.`,
      image_data: null,
      image_thumb: null,
      source_app: "TextEdit",
      source_app_icon: null,
      content_hash: `hash-${i}`,
      char_count: 48,
      created_at: new Date().toISOString(),
      is_pinned: false,
      collection_id: null,
      tags: [],
    };
  }

  let gridEl: HTMLDivElement | undefined = $state();
  let selectedIndex = $state(0);
  let loadedCount = $state(PAGE_SIZE);
  let quickLookOpen = $state(false);
  let showHints = $state(true);
  const hasMore = $derived(loadedCount < TOTAL);
  const entries = $derived(Array.from({ length: loadedCount }, (_, i) => makeEntry(i)));
  const quickLookEntry = $derived(
    quickLookOpen && selectedIndex >= 0 && selectedIndex < entries.length
      ? entries[selectedIndex]
      : null,
  );

  function getVerticalInsets(container: HTMLElement) {
    const style = getComputedStyle(container);
    return {
      top: parseFloat(style.paddingTop) || 0,
      bottom: parseFloat(style.paddingBottom) || 0,
    };
  }

  /** Same fix as +page.svelte: `.card`'s real parent is a `display: contents` host. */
  function scrollMeasureEl(card: HTMLElement): HTMLElement {
    const wrapper = card.closest(".card-wrapper");
    return wrapper instanceof HTMLElement ? wrapper : card;
  }

  function scrollContext() {
    if (!gridEl) {
      return {
        leadingIndex: 0,
        selectedOffScreen: false,
        wrapperMissing: false,
        verticalPosition: "inside" as const,
      };
    }

    const wrappers = gridEl.querySelectorAll(".card-wrapper");
    const wrapper = wrappers[selectedIndex];
    const wrapperMissing = selectedIndex >= 0 && !(wrapper instanceof HTMLElement);
    const viewport = gridEl.getBoundingClientRect();
    const { top: padTop, bottom: padBottom } = getVerticalInsets(gridEl);

    if (!(wrapper instanceof HTMLElement)) {
      return { leadingIndex: 0, selectedOffScreen: false, wrapperMissing, verticalPosition: "inside" as const };
    }

    const rect = wrapper.getBoundingClientRect();
    const verticalPosition = verticalCardViewportPosition(viewport, padTop, padBottom, rect);
    const selectedOffScreen = isCardOffScreenVertical(viewport, padTop, padBottom, rect);

    let leadingIndex = 0;
    for (let i = 0; i < wrappers.length; i++) {
      const row = wrappers[i];
      if (!(row instanceof HTMLElement)) continue;
      const rowRect = row.getBoundingClientRect();
      if (verticalCardViewportPosition(viewport, padTop, padBottom, rowRect) !== "inside") continue;
      leadingIndex = i;
      break;
    }

    return { leadingIndex, selectedOffScreen, wrapperMissing, verticalPosition };
  }

  function scrollToSelected(direction?: "up" | "down") {
    if (!gridEl || selectedIndex < 0) return;
    const cards = gridEl.querySelectorAll(".card");
    const card = cards[selectedIndex];
    if (!(card instanceof HTMLElement)) return;
    const measureEl = scrollMeasureEl(card);

    const { top: padTop, bottom: padBottom } = getVerticalInsets(gridEl);
    const containerRect = gridEl.getBoundingClientRect();
    const cardRect = measureEl.getBoundingClientRect();
    const delta = direction
      ? verticalScrollDeltaForKeyboardNav(containerRect, padTop, padBottom, cardRect, direction)
      : verticalScrollDeltaToSnapCard(containerRect, padTop, padBottom, cardRect);
    if (delta !== 0) {
      gridEl.scrollTo({ top: gridEl.scrollTop + delta, behavior: "auto" });
    }

    if (
      shouldLoadNextEntryPage({
        scrollLeft: gridEl.scrollTop,
        clientWidth: gridEl.clientHeight,
        scrollWidth: gridEl.scrollHeight,
        hasMore,
        loading: false,
      })
    ) {
      loadedCount = Math.min(TOTAL, loadedCount + PAGE_SIZE);
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (quickLookOpen) {
      if (e.key === "Escape" || e.key === " ") {
        e.preventDefault();
        quickLookOpen = false;
      }
      if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    }
    if (e.key === " ") {
      e.preventDefault();
      quickLookOpen = true;
      return;
    }
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();

    const ctx = scrollContext();
    const direction = e.key === "ArrowDown" ? "right" : "left";
    selectedIndex = nextIndexAfterKeyboardArrow({
      direction,
      selectedIndex,
      leadingIndex: ctx.leadingIndex,
      selectedOffScreen: ctx.selectedOffScreen,
      wrapperMissing: ctx.wrapperMissing,
      entryCount: entries.length,
      boardVertical: true,
      verticalPosition: ctx.verticalPosition,
    });

    if (direction === "right" && selectedIndex === entries.length - 1 && hasMore) {
      loadedCount = Math.min(TOTAL, loadedCount + PAGE_SIZE);
    }

    scrollToSelected(e.key === "ArrowDown" ? "down" : "up");
  }

  onMount(() => {
    window.addEventListener("keydown", onKeydown, true);
    return () => window.removeEventListener("keydown", onKeydown, true);
  });
</script>

<svelte:head>
  <title>Vertical overlay (dev)</title>
</svelte:head>

<div class="page">
  <p class="status" data-testid="vertical-overlay-status">
    selected={selectedIndex} loaded={loadedCount}/{TOTAL} quickLook={quickLookOpen} showHints={showHints}
  </p>
  <label class="status">
    <input type="checkbox" data-testid="toggle-hints" bind:checked={showHints} />
    Show hints
  </label>
  <div class="panel" data-testid="vertical-overlay-panel">
    <div class="grid-container vertical" bind:this={gridEl} data-testid="vertical-overlay-grid">
      {#each entries as entry, i (entry.id)}
        <div class="card-wrapper" data-index={i}>
          <ClipboardCard
            {entry}
            compactVertical
            selected={i === selectedIndex}
            onselect={() => (selectedIndex = i)}
            onpreview={() => {
              selectedIndex = i;
              quickLookOpen = true;
            }}
          />
        </div>
      {/each}
    </div>
    {#if quickLookEntry}
      <QuickLookPanel
        entry={quickLookEntry}
        compact
        showHints={showHints}
        onclose={() => (quickLookOpen = false)}
      />
    {/if}
  </div>
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    height: 100vh;
    margin: 0;
    font-family: system-ui, sans-serif;
    background: #111;
    color: var(--color-text-body, #fff);
  }

  .status {
    margin: 0;
    padding: 12px 16px;
    font-size: 14px;
  }

  /* Matches the real vertical panel width from position_window_bottom (360px CSS).
     `transform` makes this the containing block for `position: fixed` descendants
     (QuickLookPanel's backdrop), exactly like `.app` does in the real overlay. */
  .panel {
    position: relative;
    width: 360px;
    height: 700px;
    margin: 0 auto;
    background: var(--surface-app);
    border-radius: var(--radius-panel);
    border: 1px solid var(--border-strong);
    overflow: hidden;
    display: flex;
    flex-direction: column;
    transform: translateZ(0);
  }

  .grid-container.vertical {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow: hidden auto;
    padding: 12px 16px;
    min-height: 0;
  }

  .card-wrapper {
    width: 100%;
  }
</style>
