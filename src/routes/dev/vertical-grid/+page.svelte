<script lang="ts">
  import { onMount } from "svelte";
  import {
    nextIndexAfterKeyboardArrow,
    verticalCardViewportPosition,
    verticalScrollDeltaForKeyboardNav,
  } from "$lib/overlay-grid-scroll";
  import { shouldLoadNextEntryPage } from "$lib/overlay-pagination";

  const PAGE_SIZE = 12;
  const TOTAL = 36;

  let gridEl: HTMLDivElement | undefined = $state();
  let selectedIndex = $state(0);
  let loadedCount = $state(PAGE_SIZE);
  let hasMore = $derived(loadedCount < TOTAL);

  const entries = $derived(Array.from({ length: loadedCount }, (_, i) => i));

  function getVerticalInsets(container: HTMLElement) {
    const style = getComputedStyle(container);
    return {
      top: parseFloat(style.paddingTop) || 0,
      bottom: parseFloat(style.paddingBottom) || 0,
    };
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
    const selectedOffScreen = verticalPosition !== "inside";

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

  function scrollToSelected(direction: "up" | "down") {
    if (!gridEl || selectedIndex < 0) return;
    const wrapper = gridEl.querySelectorAll(".card-wrapper")[selectedIndex];
    if (!(wrapper instanceof HTMLElement)) return;

    const { top: padTop, bottom: padBottom } = getVerticalInsets(gridEl);
    const containerRect = gridEl.getBoundingClientRect();
    const cardRect = wrapper.getBoundingClientRect();
    const delta = verticalScrollDeltaForKeyboardNav(
      containerRect,
      padTop,
      padBottom,
      cardRect,
      direction,
    );
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
  <title>Vertical grid browse (dev)</title>
</svelte:head>

<div class="page" data-testid="vertical-grid-harness">
  <p class="status" data-testid="vertical-grid-status">
    selected={selectedIndex} loaded={loadedCount}/{TOTAL}
  </p>
  <div class="grid-container vertical" bind:this={gridEl} data-testid="vertical-grid">
    {#each entries as i (i)}
      <div class="card-wrapper" class:selected={i === selectedIndex} data-index={i}>
        <div class="card">Entry {i + 1}</div>
      </div>
    {/each}
  </div>
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    height: 100vh;
    margin: 0;
    font-family: system-ui, sans-serif;
    background: var(--surface-1, #1c1c1e);
    color: var(--color-text-body, white);
  }

  .status {
    margin: 0;
    padding: 12px 16px;
    font-size: 14px;
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

  .card {
    min-height: 72px;
    padding: 16px;
    border-radius: 12px;
    background: var(--surface-2, #2c2c2e);
    border: 2px solid transparent;
  }

  .card-wrapper.selected .card {
    border-color: var(--border-accent, #0a84ff);
  }
</style>
