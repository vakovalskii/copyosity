<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { AppSettings, ClipboardEntry, Collection, ModelCatalog, ModelOption } from "$lib/types";
  import { clearHistory, getAppSettings, getEntries, getCollections, getModelCatalog, updateAppSettings } from "$lib/api";
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
  let showSettings = $state(false);
  let revealCycle = $state(0);
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let settings = $state<AppSettings>({
    ollama_model: "qwen3:4b-instruct-2507-q4_K_M",
    retention_days: 30,
  });
  let modelCatalog = $state<ModelCatalog>({
    total_memory_gb: 0,
    recommended_memory_gb: 0,
    options: [],
  });
  let selectedModelPreset = $state("__custom__");
  let savingSettings = $state(false);
  let settingsNotice = $state("");
  const retentionOptions = [
    { label: "1 day", value: 1 },
    { label: "1 week", value: 7 },
    { label: "1 month", value: 30 },
    { label: "6 months", value: 180 },
  ];

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

  async function loadSettings() {
    settings = await getAppSettings();
    selectedModelPreset = settings.ollama_model;
  }

  async function loadModelCatalog() {
    modelCatalog = await getModelCatalog();
    if (!modelCatalog.options.some((option) => option.value === settings.ollama_model)) {
      selectedModelPreset = "__custom__";
    }
  }

  function showWindow() {
    clearTimeout(hideTimer);
    searchQuery = "";
    activeTag = null;
    selectedIndex = -1;
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
    searchQuery = "";
    activeTag = null;
    selectedIndex = -1;
    visible = false;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(async () => {
      await getCurrentWindow().hide();
      await getCurrentWindow().setFocus().catch(() => undefined);
    }, 240);
  }

  async function forceHideWindow() {
    showSettings = false;
    searchQuery = "";
    activeTag = null;
    selectedIndex = -1;
    visible = false;
    clearTimeout(hideTimer);
    await getCurrentWindow().hide();
  }

  onMount(() => {
    loadEntries();
    loadCollections();
    loadSettings();
    loadModelCatalog();

    // Tell Rust we're loaded — it will hide the off-screen warmup window
    invoke("frontend_ready");

    const unlistenClipboard = listen("clipboard-changed", () => {
      loadEntries();
    });

    const unlistenTagged = listen("entry-tagged", () => {
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
            hideWindow();
          });
        }
      }
    };

    window.addEventListener("keydown", handleKeydown);

    return () => {
      clearTimeout(hideTimer);
      unlistenClipboard.then((fn) => fn());
      unlistenTagged.then((fn) => fn());
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
    activeTag = null;
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

  async function saveSettings() {
    savingSettings = true;
    settingsNotice = "";

    try {
      settings = await updateAppSettings(settings);
      await loadModelCatalog();
      settingsNotice = "Saved";
      loadEntries();
    } finally {
      savingSettings = false;
    }
  }

  function handleModelPresetChange(value: string) {
    selectedModelPreset = value;
    if (value !== "__custom__") {
      settings.ollama_model = value;
    }
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

  let selectedModelMeta = $derived.by<ModelOption | null>(() => {
    return modelCatalog.options.find((option) => option.value === settings.ollama_model) ?? null;
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
          <label class="settings-field">
            <span class="settings-label">Ollama model</span>
            <select
              class="settings-select"
              bind:value={selectedModelPreset}
              onchange={(event) =>
                handleModelPresetChange((event.currentTarget as HTMLSelectElement).value)}
            >
              {#each modelCatalog.options as option}
                <option value={option.value}>
                  {option.label} · ~{option.memory_gb.toFixed(1)} GB · {option.fits ? "fits" : "tight"}{option.installed ? " · installed" : ""}
                </option>
              {/each}
              <option value="__custom__">Custom model</option>
            </select>
            {#if selectedModelPreset === "__custom__"}
              <input
                class="settings-input"
                type="text"
                bind:value={settings.ollama_model}
                placeholder="qwen3:4b-instruct-2507-q4_K_M"
              />
            {/if}
            <div class="settings-hint">
              Machine RAM: {modelCatalog.total_memory_gb.toFixed(1)} GB. Recommended Ollama budget:
              {modelCatalog.recommended_memory_gb.toFixed(1)} GB.
            </div>
            {#if selectedModelMeta}
              <div class="settings-hint" class:fits={selectedModelMeta.fits} class:tight={!selectedModelMeta.fits}>
                {selectedModelMeta.label} needs about {selectedModelMeta.memory_gb.toFixed(1)} GB and
                {selectedModelMeta.fits ? " should fit this machine." : " may be too heavy for this machine."}
              </div>
            {/if}
          </label>
          <label class="settings-field">
            <span class="settings-label">History retention</span>
            <select class="settings-select" bind:value={settings.retention_days}>
              {#each retentionOptions as option}
                <option value={option.value}>{option.label}</option>
              {/each}
            </select>
          </label>
          <button class="settings-item settings-save" type="button" disabled={savingSettings} onclick={saveSettings}>
            {savingSettings ? "Saving..." : "Save settings"}
          </button>
          {#if settingsNotice}
            <div class="settings-note">{settingsNotice}</div>
          {/if}
          <button class="settings-item" type="button" onclick={loadCollections}>Refresh collections</button>
          <button class="settings-item" type="button" onclick={handleClearHistory}>Clear unpinned history</button>
        </div>
      {/if}
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

  .settings-menu {
    position: absolute;
    top: calc(100% + 10px);
    right: 0;
    min-width: 280px;
    padding: 8px;
    background: rgba(34, 34, 40, 0.78);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    box-shadow: 0 18px 40px rgba(0, 0, 0, 0.35);
    z-index: 10;
    animation: settings-pop 0.18s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .settings-field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 6px 10px;
  }

  .settings-label {
    font-size: 11px;
    color: #b4b7c2;
  }

  .settings-input,
  .settings-select {
    width: 100%;
    padding: 10px 11px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 9px;
    color: #edf0f8;
    font: inherit;
    outline: none;
  }

  .settings-input::placeholder {
    color: rgba(237, 240, 248, 0.35);
  }

  .settings-hint {
    font-size: 11px;
    line-height: 1.35;
    color: #97a0b4;
  }

  .settings-hint.fits {
    color: #8fd1a1;
  }

  .settings-hint.tight {
    color: #e3b370;
  }

  .settings-save {
    margin-top: 4px;
    background: rgba(94, 140, 255, 0.14);
    border: 1px solid rgba(120, 160, 255, 0.24);
  }

  .settings-save:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .settings-note {
    padding: 6px 12px 8px;
    font-size: 11px;
    color: #91d6a6;
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
