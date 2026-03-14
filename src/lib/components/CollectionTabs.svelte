<script lang="ts">
  import type { Collection } from "$lib/types";
  import { createCollection, deleteCollection } from "$lib/api";

  let {
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
    class="tab"
    class:active={activeId === null && !activePinned}
    onclick={() => onselect?.(null)}
  >
    Clipboard History
  </button>

  <button
    class="tab"
    class:active={activePinned}
    onclick={() => onselect?.(-1)}
  >
    Starred
  </button>

  {#each collections as col}
    <div
      class="tab"
      class:active={activeId === col.id && !activePinned}
      onclick={() => onselect?.(col.id)}
      onkeydown={(e) => e.key === 'Enter' && onselect?.(col.id)}
      role="button"
      tabindex="0"
    >
      <span class="tab-dot" style:background={col.color ?? '#666'}></span>
      {col.name}
      <button class="tab-delete" onclick={(e) => handleDelete(e, col.id)}>×</button>
    </div>
  {/each}

  {#if showAdd}
    <form class="add-form" onsubmit={(e) => { e.preventDefault(); handleAdd(); }}>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        bind:value={newName}
        placeholder="Name..."
        autofocus
        onblur={() => { if (!newName) showAdd = false; }}
      />
    </form>
  {:else}
    <button class="tab add-tab" onclick={() => (showAdd = true)}>+</button>
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

  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 12px;
    border-radius: 6px;
    background: none;
    border: none;
    color: #999;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    font-family: inherit;
    transition: all 0.15s;
  }

  .tab:hover {
    color: #ddd;
    background: rgba(255, 255, 255, 0.06);
  }

  .tab.active {
    color: #fff;
    background: rgba(255, 255, 255, 0.1);
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
    color: #666;
    cursor: pointer;
    font-size: 14px;
    padding: 0 2px;
    line-height: 1;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .tab:hover .tab-delete {
    opacity: 1;
  }

  .tab-delete:hover {
    color: #ff6b6b;
  }

  .add-tab {
    font-size: 16px;
    color: #666;
  }

  .add-form input {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    color: #e0e0e0;
    padding: 4px 10px;
    font-size: 12px;
    outline: none;
    width: 120px;
    font-family: inherit;
  }
</style>
