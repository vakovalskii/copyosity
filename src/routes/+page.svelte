<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { ClipboardEntry, Collection } from "$lib/types";
  import { clearHistory, getEntries, getCollections } from "$lib/api";
  import ClipboardCard from "$lib/components/ClipboardCard.svelte";
  import SearchBar from "$lib/components/SearchBar.svelte";
  import CollectionTabs from "$lib/components/CollectionTabs.svelte";

  let entries: ClipboardEntry[] = $state([]);
  let collections: Collection[] = $state([]);
  let searchQuery = $state("");
  let activeCollectionId: number | null = $state(null);
  let pinnedOnly = $state(false);
  let selectedIndex = $state(-1);
  let gridEl: HTMLDivElement | undefined = $state();
  let visible = $state(false);
  let showSettings = $state(false);
  let revealCycle = $state(0);
  let hideTimer: ReturnType<typeof setTimeout> | undefined;

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
    clearTimeout(hideTimer);
    loadEntries();
    revealCycle += 1;
    visible = false;
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        visible = true;
      });
    });
  }

  async function hideWindow() {
    showSettings = false;
    visible = false;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(async () => {
      await getCurrentWindow().hide();
      await getCurrentWindow().setFocus().catch(() => undefined);
    }, 240);
  }

  async function forceHideWindow() {
    showSettings = false;
    visible = false;
    clearTimeout(hideTimer);
    await getCurrentWindow().hide();
  }

  onMount(() => {
    loadEntries();
    loadCollections();

    // Tell Rust we're loaded — it will hide the off-screen warmup window
    invoke("frontend_ready");

    const unlistenClipboard = listen("clipboard-changed", () => {
      loadEntries();
    });

    const unlistenShow = listen("window-show", () => {
      showWindow();
    });

    const handleKeydown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        forceHideWindow();
        return;
      }
      if (e.key === "ArrowRight") {
        e.preventDefault();
        selectedIndex = Math.min(selectedIndex + 1, entries.length - 1);
        scrollToSelected();
      }
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        selectedIndex = Math.max(selectedIndex - 1, 0);
        scrollToSelected();
      }
      if (e.key === "Enter" && selectedIndex >= 0 && selectedIndex < entries.length) {
        e.preventDefault();
        const entry = entries[selectedIndex];
        if (entry.text_content) {
          import("$lib/api").then(({ pasteEntry }) => {
            pasteEntry(entry.text_content!);
            hideWindow();
          });
        }
      }
    };

    window.addEventListener("keydown", handleKeydown);

    return () => {
      clearTimeout(hideTimer);
      unlistenClipboard.then((fn) => fn());
      unlistenShow.then((fn) => fn());
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
    selectedIndex = -1;
    loadEntries();
  }

  function handleEntryAction() {
    loadEntries();
  }

  function handlePasted() {
    hideWindow();
  }

  async function handleClearHistory() {
    await clearHistory();
    showSettings = false;
    selectedIndex = -1;
    loadEntries();
  }

  let debounceTimer: ReturnType<typeof setTimeout>;
  function debouncedSearch(q: string) {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => handleSearch(q), 150);
  }
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
        onclick={() => (showSettings = !showSettings)}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path
            d="M19.14 12.94c.04-.31.06-.62.06-.94s-.02-.63-.06-.94l2.03-1.58a.5.5 0 0 0 .12-.64l-1.92-3.32a.5.5 0 0 0-.6-.22l-2.39.96a7.03 7.03 0 0 0-1.63-.94l-.36-2.54a.5.5 0 0 0-.5-.42h-3.84a.5.5 0 0 0-.5.42l-.36 2.54c-.58.22-1.13.53-1.63.94l-2.39-.96a.5.5 0 0 0-.6.22L2.71 8.84a.5.5 0 0 0 .12.64l2.03 1.58c-.04.31-.06.62-.06.94s.02.63.06.94l-2.03 1.58a.5.5 0 0 0-.12.64l1.92 3.32a.5.5 0 0 0 .6.22l2.39-.96c.5.41 1.05.72 1.63.94l.36 2.54a.5.5 0 0 0 .5.42h3.84a.5.5 0 0 0 .5-.42l.36-2.54c.58-.22 1.13-.53 1.63-.94l2.39.96a.5.5 0 0 0 .6-.22l1.92-3.32a.5.5 0 0 0-.12-.64zM12 15.5A3.5 3.5 0 1 1 12 8.5a3.5 3.5 0 0 1 0 7z"
          />
        </svg>
      </button>

      {#if showSettings}
        <div class="settings-menu">
          <button class="settings-item" type="button" onclick={loadCollections}>Refresh collections</button>
          <button class="settings-item" type="button" onclick={handleClearHistory}>Clear unpinned history</button>
        </div>
      {/if}
    </div>
  </header>

  <div class="grid-container" bind:this={gridEl}>
    {#if entries.length === 0}
      <div class="empty-state">
        {#if searchQuery}
          <p>No results for "{searchQuery}"</p>
        {:else}
          <p>Clipboard history is empty</p>
          <p class="hint">Copy something to get started</p>
        {/if}
      </div>
    {:else}
      {#each entries as entry, i (`${revealCycle}-${entry.id}`)}
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

  .settings-menu {
    position: absolute;
    top: calc(100% + 10px);
    right: 0;
    min-width: 220px;
    padding: 8px;
    background: rgba(34, 34, 40, 0.78);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    box-shadow: 0 18px 40px rgba(0, 0, 0, 0.35);
    z-index: 10;
    animation: settings-pop 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .settings-item {
    width: 100%;
    padding: 10px 12px;
    background: transparent;
    border: none;
    border-radius: 8px;
    color: #e8e8e8;
    text-align: left;
    cursor: pointer;
    font: inherit;
  }

  .settings-item:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  .grid-container {
    flex: 1;
    display: flex;
    gap: 12px;
    padding: 16px;
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

  @keyframes settings-pop {
    from {
      opacity: 0;
      transform: translateY(-8px) scale(0.96);
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
