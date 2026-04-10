<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { ClipboardEntry, Collection } from "$lib/types";
  import {
    getEntries,
    getCollections,
    hideMainWindow,
    openSettingsWindow,
  } from "$lib/api";
  import ClipboardCard from "$lib/components/ClipboardCard.svelte";
  import SearchBar from "$lib/components/SearchBar.svelte";
  import CollectionTabs from "$lib/components/CollectionTabs.svelte";

  let entries: ClipboardEntry[] = $state([]);
  let collections: Collection[] = $state([]);
  let searchQuery = $state("");
  let activeCollectionId: number | null = $state(null);
  let pinnedOnly = $state(false);
  let activeTag = $state<string | null>(null);
  let selectedIndex = $state(-1);
  let gridEl: HTMLDivElement | undefined = $state();
  let visible = $state(false);
  let revealCycle = $state(0);
  const hiddenTopTags = new Set(["code", "otp", "token", "log"]);

  async function loadEntries() {
    entries = await getEntries({
      collection_id: activeCollectionId,
      pinned_only: pinnedOnly,
      search: searchQuery || null,
    });
  }

  async function loadCollections() {
    collections = await getCollections();
  }

  function showWindow() {
    window.getSelection()?.removeAllRanges();
    searchQuery = "";
    activeTag = null;
    selectedIndex = -1;
    loadEntries();
    revealCycle += 1;
    // Reset scroll to start
    if (gridEl) gridEl.scrollLeft = 0;
    // Start hidden, then animate in next frame
    visible = false;
    requestAnimationFrame(() => {
      visible = true;
    });
  }

  function animateOut() {
    visible = false;
    searchQuery = "";
    activeTag = null;
    selectedIndex = -1;
    hideMainWindow();
  }

  function forceHideWindow() {
    visible = false;
    searchQuery = "";
    activeTag = null;
    selectedIndex = -1;
    hideMainWindow();
  }

  onMount(() => {
    loadEntries();
    loadCollections();

    // Tell Rust we're loaded — it will hide the off-screen warmup window
    invoke("frontend_ready");

    // Debounce entry reloads — clipboard-changed and entry-tagged can fire together
    let reloadTimer: ReturnType<typeof setTimeout>;
    function scheduleReload() {
      clearTimeout(reloadTimer);
      reloadTimer = setTimeout(() => loadEntries(), 100);
    }

    const unlistenClipboard = listen("clipboard-changed", scheduleReload);
    const unlistenTagged = listen("entry-tagged", scheduleReload);

    const unlistenShow = listen("window-show", () => {
      showWindow();
    });

    const unlistenOpenSettings = listen("open-settings", () => {
      openSettingsWindow();
    });

    const handleKeydown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        forceHideWindow();
        return;
      }
      if (e.key === "ArrowRight") {
        e.preventDefault();
        selectedIndex = Math.min(selectedIndex + 1, filteredEntries.length - 1);
        scrollToSelected();
      }
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        selectedIndex = Math.max(selectedIndex - 1, 0);
        scrollToSelected();
      }
      if (e.key === "Enter" && selectedIndex >= 0 && selectedIndex < filteredEntries.length) {
        e.preventDefault();
        const entry = filteredEntries[selectedIndex];
        if (entry.text_content) {
          import("$lib/api").then(({ pasteEntry }) => {
            pasteEntry(entry.text_content!);
            animateOut();
          });
        }
      }
    };

    window.addEventListener("keydown", handleKeydown);

    return () => {
      clearTimeout(reloadTimer);
      clearTimeout(debounceTimer);
      unlistenClipboard.then((fn) => fn());
      unlistenTagged.then((fn) => fn());
      unlistenShow.then((fn) => fn());
      unlistenOpenSettings.then((fn) => fn());
      window.removeEventListener("keydown", handleKeydown);
    };
  });

  function scrollToSelected() {
    if (!gridEl) return;
    const cards = gridEl.querySelectorAll(".card");
    if (cards[selectedIndex]) {
      cards[selectedIndex].scrollIntoView({ behavior: "smooth", block: "nearest", inline: "center" });
    }
  }

  function handleSearch(q: string) {
    searchQuery = q;
    selectedIndex = -1;
    loadEntries();
  }

  function handleCollectionSelect(id: number | null) {
    pinnedOnly = id === -1;
    activeCollectionId = id === -1 ? null : id;
    activeTag = null;
    selectedIndex = -1;
    loadEntries();
  }

  function handleEntryAction() {
    loadEntries();
  }

  function handlePasted() {
    animateOut();
  }

  let debounceTimer: ReturnType<typeof setTimeout>;
  function debouncedSearch(q: string) {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => handleSearch(q), 150);
  }

  let topTags = $derived.by(() => {
    const counts = new Map<string, number>();

    for (const entry of entries) {
      for (const tag of entry.tags ?? []) {
        if (hiddenTopTags.has(tag)) continue;
        counts.set(tag, (counts.get(tag) ?? 0) + 1);
      }
    }

    return [...counts.entries()]
      .sort((a, b) => {
        if (b[1] !== a[1]) return b[1] - a[1];
        return a[0].localeCompare(b[0]);
      })
      .slice(0, 8);
  });

  let filteredEntries = $derived.by(() => {
    if (!activeTag) return entries;
    const tag = activeTag;
    return entries.filter((entry) => (entry.tags ?? []).includes(tag));
  });
</script>

<div class="app" class:visible>
  <header class="header">
    <SearchBar value={searchQuery} onchange={debouncedSearch} />
    <CollectionTabs
      {collections}
      activeId={activeCollectionId}
      activePinned={pinnedOnly}
      onselect={handleCollectionSelect}
      onupdate={loadCollections}
    />
    <div class="header-actions">
      <button
        class="settings-btn"
        type="button"
        aria-label="Open settings"
        onclick={() => openSettingsWindow()}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path
            d="M19.14 12.94c.04-.31.06-.62.06-.94s-.02-.63-.06-.94l2.03-1.58a.5.5 0 0 0 .12-.64l-1.92-3.32a.5.5 0 0 0-.6-.22l-2.39.96a7.03 7.03 0 0 0-1.63-.94l-.36-2.54a.5.5 0 0 0-.5-.42h-3.84a.5.5 0 0 0-.5.42l-.36 2.54c-.58.22-1.13.53-1.63.94l-2.39-.96a.5.5 0 0 0-.6.22L2.71 8.84a.5.5 0 0 0 .12.64l2.03 1.58c-.04.31-.06.62-.06.94s.02.63.06.94l-2.03 1.58a.5.5 0 0 0-.12.64l1.92 3.32a.5.5 0 0 0 .6.22l2.39-.96c.5.41 1.05.72 1.63.94l.36 2.54a.5.5 0 0 0 .5.42h3.84a.5.5 0 0 0 .5-.42l.36-2.54c.58-.22 1.13-.53 1.63-.94l2.39.96a.5.5 0 0 0 .6-.22l1.92-3.32a.5.5 0 0 0-.12-.64zM12 15.5A3.5 3.5 0 1 1 12 8.5a3.5 3.5 0 0 1 0 7z"
          />
        </svg>
      </button>
    </div>
  </header>

  {#if topTags.length > 0}
    <div class="tag-groups">
      <button
        class="tag-group-chip"
        class:active={!activeTag}
        type="button"
        onclick={() => {
          activeTag = null;
          selectedIndex = -1;
        }}
      >
        All tags
      </button>

      {#each topTags as [tag, count]}
        <button
          class="tag-group-chip"
          class:active={activeTag === tag}
          type="button"
          onclick={() => {
            activeTag = tag;
            selectedIndex = -1;
          }}
        >
          <span>{tag}</span>
          <span class="tag-group-count">{count}</span>
        </button>
      {/each}
    </div>
  {/if}

  <div class="grid-container" bind:this={gridEl}>
    {#if filteredEntries.length === 0}
      <div class="empty-state">
        {#if searchQuery || activeTag}
          <p>No results for "{searchQuery}"</p>
        {:else}
          <p>Clipboard history is empty</p>
          <p class="hint">Copy something to get started</p>
        {/if}
      </div>
    {:else}
      {#each filteredEntries as entry, i (`${revealCycle}-${activeTag ?? 'all'}-${entry.id}`)}
        <div class="card-wrapper" style="animation-delay: {Math.min(i * 30, 300)}ms">
          <ClipboardCard
            {entry}
            selected={i === selectedIndex}
            onpasted={handlePasted}
            ondeleted={handleEntryAction}
            onpinned={handleEntryAction}
          />
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    background: transparent;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", sans-serif;
    color: #e0e0e0;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
  }

  :global(*) {
    box-sizing: border-box;
    outline: none;
  }

  :global(::selection) {
    background: transparent;
  }

  .app {
    width: 100vw;
    height: 100vh;
    background:
      linear-gradient(180deg, rgba(44, 44, 50, 0.68), rgba(24, 24, 30, 0.58));
    backdrop-filter: blur(34px) saturate(1.15);
    -webkit-backdrop-filter: blur(34px) saturate(1.15);
    border-radius: 18px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow:
      0 18px 50px rgba(0, 0, 0, 0.28),
      inset 0 1px 0 rgba(255, 255, 255, 0.08);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    transform: translateY(26px) scale(0.985);
    opacity: 0;
    transition:
      transform 0.24s cubic-bezier(0.22, 1, 0.36, 1),
      opacity 0.22s ease,
      box-shadow 0.24s ease;
  }

  .app.visible {
    transform: translateY(0) scale(1);
    opacity: 1;
  }

  .header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(255, 255, 255, 0.015);
    flex-shrink: 0;
  }

  .tag-groups {
    display: flex;
    gap: 8px;
    padding: 10px 16px 0;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .tag-groups::-webkit-scrollbar {
    display: none;
  }

  .tag-group-chip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 7px 11px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.035);
    color: #c9ccd8;
    cursor: pointer;
    white-space: nowrap;
    font: inherit;
    font-size: 11px;
    transition: background 0.15s ease, border-color 0.15s ease, color 0.15s ease;
  }

  .tag-group-chip:hover {
    background: rgba(255, 255, 255, 0.07);
    border-color: rgba(255, 255, 255, 0.12);
  }

  .tag-group-chip.active {
    background: rgba(94, 140, 255, 0.18);
    border-color: rgba(120, 160, 255, 0.28);
    color: #eef3ff;
  }

  .tag-group-count {
    display: inline-flex;
    min-width: 18px;
    justify-content: center;
    padding: 2px 5px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    font-size: 10px;
    line-height: 1;
  }

  .header-actions {
    position: relative;
    margin-left: auto;
    flex-shrink: 0;
  }

  .settings-btn {
    width: 36px;
    height: 36px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    color: #d8d8d8;
    cursor: pointer;
    transition: background 0.15s ease, border-color 0.15s ease, transform 0.15s ease;
  }

  .settings-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.16);
    transform: translateY(-1px);
  }

  .settings-btn svg {
    width: 18px;
    height: 18px;
    fill: currentColor;
  }

  .grid-container {
    flex: 1;
    display: flex;
    gap: 12px;
    padding: 14px 16px 16px;
    overflow-x: auto;
    overflow-y: hidden;
    align-items: flex-start;
    scrollbar-width: thin;
    scrollbar-color: rgba(255, 255, 255, 0.1) transparent;
  }

  .grid-container::-webkit-scrollbar {
    height: 6px;
  }

  .grid-container::-webkit-scrollbar-track {
    background: transparent;
  }

  .grid-container::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 3px;
  }

  .card-wrapper {
    animation: card-enter 0.35s cubic-bezier(0.16, 1, 0.3, 1) backwards;
  }

  @keyframes card-enter {
    from {
      opacity: 0;
      transform: translateY(20px) scale(0.95);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  .empty-state {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #666;
  }

  .empty-state p {
    margin: 4px 0;
  }

  .hint {
    font-size: 13px;
    color: #555;
  }
</style>
