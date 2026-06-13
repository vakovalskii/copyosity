<script lang="ts">
  import type { Collection } from "$lib/types";
  import { createCollection, deleteCollection } from "$lib/api";

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

<div class="tabs-container">
  <button
    class="tab app-btn"
    class:active={activeId === null && !activePinned}
    type="button"
    onclick={() => onselect?.(null)}
  >
    Clipboard History
  </button>

  <button
    class="tab app-btn"
    class:active={activePinned}
    type="button"
    onclick={() => onselect?.(-1)}
  >
    Starred
  </button>

  {#each collections as col}
    <div class="tab-item">
      <button
        class="tab app-btn"
        class:active={activeId === col.id && !activePinned}
        type="button"
        onclick={() => onselect?.(col.id)}
      >
        <span class="tab-dot" style:background={col.color ?? "var(--color-text-subtle)"}></span>
        {col.name}
      </button>
      <button
        class="tab-delete app-btn"
        type="button"
        aria-label="Delete collection {col.name}"
        onclick={(e) => handleDelete(e, col.id)}
      >
        ×
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
    <button class="tab add-tab app-btn" type="button" onclick={() => (showAdd = true)}>+</button>
  {/if}
</div>

<style>
  .tabs-container {
    display: flex;
    align-items: center;
    gap: 4px;
    overflow-x: auto;
    padding: 0 4px;
    scrollbar-width: none;
  }

  .tabs-container::-webkit-scrollbar {
    display: none;
  }

  .tab-item {
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  .tab-item .tab {
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
    padding-right: 6px;
  }

  .tab-item .tab-delete {
    align-self: stretch;
    display: inline-flex;
    align-items: center;
    border: none;
    border-top-right-radius: 6px;
    border-bottom-right-radius: 6px;
    padding: 0 6px;
    margin-left: -2px;
    background: none;
    opacity: 0;
  }

  .tab-item:hover .tab-delete {
    opacity: 1;
    background: var(--surface-6);
  }

  .tab-item .tab.active + .tab-delete {
    background: var(--surface-10);
  }

  .tab-item .tab.active:hover + .tab-delete {
    background: var(--surface-10);
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 12px;
    border-radius: var(--radius-control-sm);
    background: none;
    border: none;
    color: var(--color-text-tab);
    font-size: var(--font-size-sm);
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    font-family: inherit;
    transition:
      color var(--duration-fast) var(--ease-interactive),
      background var(--duration-fast) var(--ease-interactive);
  }

  .tab:hover:not(:disabled, [aria-busy="true"]) {
    color: var(--color-text-tab-hover);
    background: var(--surface-6);
  }

  .tab.active {
    color: var(--color-text-bright);
    background: var(--surface-10);
  }

  .tab:focus-visible {
    outline: none;
    box-shadow: var(--ring-accent);
  }

  .tab-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .tab-delete {
    background: none;
    border: none;
    color: var(--color-text-subtle);
    cursor: pointer;
    font-size: var(--font-size-base);
    padding: 0 2px;
    line-height: 1;
    opacity: 0;
    transition: opacity var(--duration-fast) var(--ease-interactive);
  }

  .tab-delete:hover:not(:disabled, [aria-busy="true"]) {
    color: var(--color-danger);
  }

  .add-tab {
    font-size: var(--font-size-xl);
    color: var(--color-text-subtle);
  }

  .add-form .form-input {
    width: 120px;
    min-height: 28px;
    padding: 4px 10px;
    font-size: var(--font-size-sm);
    border-radius: var(--radius-control-sm);
    color: var(--color-text-body);
  }
</style>
