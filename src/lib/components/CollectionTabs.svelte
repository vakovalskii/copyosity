<script lang="ts">
  import type { Collection } from "$lib/types";
  import { createCollection, deleteCollection } from "$lib/api";
  import {
    isCollectionsScrollable,
    isCustomCollectionActive,
    isHistoryCollectionActive,
  } from "$lib/collection-tabs";

  const {
    collections = [],
    activeId = null,
    activePinned = false,
    onselect,
    onupdate,
  }: {
    collections?: Collection[];
    activeId?: number | null;
    activePinned?: boolean;
    onselect?: (id: number | null) => void;
    onupdate?: () => void;
  } = $props();

  let showAdd = $state(false);
  let newName = $state("");

  const historySelected = $derived(isHistoryCollectionActive(activeId, activePinned));
  const collectionsScrollable = $derived(isCollectionsScrollable(collections.length, showAdd));

  async function handleAdd() {
    if (!newName.trim()) return;
    await createCollection(newName.trim());
    newName = "";
    showAdd = false;
    onupdate?.();
  }

  async function handleDelete(e: MouseEvent, id: number) {
    e.stopPropagation();
    await deleteCollection(id);
    if (activeId === id) onselect?.(null);
    onupdate?.();
  }
</script>

<!-- TEST-NOTE: Svelte markup/a11y not covered here; see collection-tabs.test.ts for scroll/selection helpers. -->
<div class="tabs-container">
  <div role="tablist" aria-label="Clipboard view" class="view-tablist">
    <div class="segment-track" role="presentation">
      <button
        type="button"
        class="segment-item app-btn"
        role="tab"
        aria-selected={historySelected}
        onclick={() => onselect?.(null)}
      >
        History
      </button>
      <button
        type="button"
        class="segment-item app-btn"
        role="tab"
        aria-selected={activePinned}
        onclick={() => onselect?.(-1)}
      >
        Starred
      </button>
    </div>
  </div>

  <div
    role="group"
    aria-label="Custom collections"
    class="collections-scroll"
    class:scrollable={collectionsScrollable}
  >
    {#each collections as col (col.id)}
      {@const selected = isCustomCollectionActive(col.id, activeId, activePinned)}
      <div class="collection-tab-item" class:selected role="presentation">
        <button
          type="button"
          class="collection-tab app-btn"
          aria-pressed={selected}
          title={col.name}
          onclick={() => onselect?.(col.id)}
        >
          <span class="tab-dot" style:background={col.color ?? "var(--color-text-subtle)"}></span>
          <span class="tab-label">{col.name}</span>
        </button>
        <button
          type="button"
          class="tab-delete"
          aria-label="Remove collection {col.name}"
          onclick={(e) => handleDelete(e, col.id)}
        >
          <svg class="tab-delete-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M18 6 6 18M6 6l12 12" />
          </svg>
        </button>
      </div>
    {/each}

    {#if showAdd}
      <form class="add-form" onsubmit={(e) => { e.preventDefault(); handleAdd(); }}>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="form-input"
          bind:value={newName}
          placeholder="Name..."
          aria-label="Collection name"
          autofocus
          onblur={() => { if (!newName) showAdd = false; }}
        />
      </form>
    {:else}
      <button
        class="add-tab app-btn"
        type="button"
        aria-label="Add collection"
        onclick={() => (showAdd = true)}
      >
        <svg class="add-tab-icon" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M12 5v14M5 12h14" />
        </svg>
      </button>
    {/if}
  </div>
</div>

<style>
  .tabs-container {
    display: flex;
    align-items: center;
    gap: var(--space-stack);
    flex-shrink: 1;
    min-width: 0;
    height: var(--overlay-header-control-height);
    overflow: hidden;
  }

  .view-tablist {
    flex-shrink: 0;
  }

  .collections-scroll {
    display: flex;
    align-items: center;
    gap: var(--space-stack);
    min-width: 0;
    flex: 0 0 auto;
  }

  .collections-scroll.scrollable {
    flex: 1 1 auto;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .collections-scroll.scrollable::-webkit-scrollbar {
    display: none;
  }

  /* ── Primary segmented control (History / Starred) — segment-control.css ── */

  /* ── Custom collection pills ── */
  .collection-tab-item {
    display: inline-flex;
    align-items: stretch;
    flex-shrink: 0;
    box-sizing: border-box;
    height: var(--overlay-header-control-height);
    border-radius: var(--radius-control-sm);
    background: var(--surface-3);
    border: 1px solid var(--border-soft);
    transition:
      background var(--duration-fast) var(--ease-interactive),
      border-color var(--duration-fast) var(--ease-interactive);
  }

  .collection-tab-item:hover {
    background: var(--surface-5);
    border-color: var(--border-default);
  }

  .collection-tab-item.selected {
    background: var(--surface-7);
    border-color: var(--border-default);
    box-shadow: var(--shadow-inset-highlight);
  }

  .collection-tab-item.selected:hover {
    background: var(--surface-8);
    border-color: var(--border-medium);
  }

  .collection-tab {
    display: inline-flex;
    align-items: center;
    gap: var(--space-chip-gap);
    height: 100%;
    padding: 0 4px 0 10px;
    border: none;
    border-radius: var(--radius-control-sm) 0 0 var(--radius-control-sm);
    background: transparent;
    color: var(--color-text-secondary);
    font: inherit;
    font-size: var(--font-size-sm);
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition: color var(--duration-fast) var(--ease-interactive);
  }

  .collection-tab-item:hover .collection-tab {
    color: var(--color-text-body);
  }

  .collection-tab[aria-pressed="true"] {
    color: var(--color-text-primary);
  }

  .collection-tab-item.selected:hover .collection-tab {
    color: var(--color-text-primary);
  }

  .collection-tab:focus-visible {
    outline: none;
    box-shadow: var(--ring-accent);
    z-index: 1;
  }

  .tab-label {
    max-width: 9rem;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tab-dot {
    width: var(--icon-size-collection-dot);
    height: var(--icon-size-collection-dot);
    border-radius: 50%;
    flex-shrink: 0;
  }

  .tab-delete-icon {
    width: var(--icon-size-chevron);
    height: var(--icon-size-chevron);
    display: block;
    fill: none;
    stroke: currentcolor;
    stroke-width: 2;
    stroke-linecap: round;
  }

  .tab-delete {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    align-self: stretch;
    flex-shrink: 0;
    width: calc(var(--overlay-header-control-height) - 4px);
    min-width: calc(var(--overlay-header-control-height) - 4px);
    border: none;
    border-radius: 0 var(--radius-control-sm) var(--radius-control-sm) 0;
    background: transparent;
    color: var(--color-text-subtle);
    cursor: pointer;
    padding: 0;
    opacity: 0.45;
    -webkit-tap-highlight-color: transparent;
    transition: color var(--duration-fast) var(--ease-interactive);
  }

  .collection-tab-item:hover .tab-delete,
  .collection-tab-item:focus-within .tab-delete,
  .tab-delete:focus-visible {
    opacity: 1;
  }

  .tab-delete:hover:not(:disabled) {
    color: var(--color-danger);
  }

  .tab-delete:active:not(:disabled) {
    color: var(--color-danger-text-hover);
  }

  .tab-delete:focus-visible {
    outline: none;
    box-shadow: var(--ring-accent);
  }

  .add-tab {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: var(--overlay-header-control-height);
    height: var(--overlay-header-control-height);
    padding: 0;
    border: 1px solid var(--border-soft);
    border-radius: var(--radius-control-sm);
    background: var(--surface-3);
    color: var(--color-text-subtle);
    cursor: pointer;
    transition:
      background var(--duration-fast) var(--ease-interactive),
      color var(--duration-fast) var(--ease-interactive),
      border-color var(--duration-fast) var(--ease-interactive);
  }

  .add-tab-icon {
    width: var(--icon-size-overlay-header-close);
    height: var(--icon-size-overlay-header-close);
    display: block;
    fill: none;
    stroke: currentcolor;
    stroke-width: 2;
    stroke-linecap: round;
  }

  .add-tab:hover:not(:disabled, [aria-busy="true"]) {
    color: var(--color-text-body);
    background: var(--surface-5);
    border-color: var(--border-default);
  }

  .add-tab:focus-visible {
    outline: none;
    box-shadow: var(--ring-accent-input);
  }

  .add-form {
    flex-shrink: 0;
  }

  .add-form .form-input {
    width: 120px;
    box-sizing: border-box;
    height: var(--overlay-header-control-height);
    min-height: var(--overlay-header-control-height);
    padding: 0 10px;
    font-size: var(--font-size-sm);
    border-radius: var(--radius-control-sm);
    color: var(--color-text-body);
  }
</style>
