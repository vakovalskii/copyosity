<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import type { ClipboardEntry, Collection, ExcludableAppCandidate } from "$lib/types";
  import {
    getEntries,
    getCollections,
    getAppSettings,
    hideMainWindow,
    openSettingsWindow,
    activateEntry,
    isTaggingReady,
    getExcludableAppCandidate,
    addExcludableAppCandidate,
  } from "$lib/api";
  import {
    alreadyExcludedFromHistoryLabel,
    excludeFromClipboardHistoryAriaLabel,
    excludeFromHistoryLabel,
    invokeErrorMessage,
  } from "$lib/exclusion-label";
  import ClipboardCard from "$lib/components/ClipboardCard.svelte";
  import SearchBar from "$lib/components/SearchBar.svelte";
  import CollectionTabs from "$lib/components/CollectionTabs.svelte";
  import ContentKindSegment from "$lib/components/ContentKindSegment.svelte";
  import TagFilterBar from "$lib/components/TagFilterBar.svelte";
  import {
    type ContentKind,
    buildTagBarModel,
    filterKindPool,
    filterByActiveTag,
    activeTagCompatibleWithKind,
    activeTagCompatibleWithAi,
    isFormatTag,
    contentKindEmptyLabel,
    formatTagEmptyLabel,
  } from "$lib/overlay-filters";
  import { overlayEscapeAction } from "$lib/overlay-search";
  import { overlayHeightForLayout } from "$lib/overlay-layout";
  import {
    animateOverlayResize,
    resetOverlayResizeState,
    resizeMainWindow,
  } from "$lib/overlay-resize";
  import { panelCloseFallbackMs, panelOpenMs, scrollBehavior } from "$lib/motion";

  let entries: ClipboardEntry[] = $state([]);
  /** Collection pool without search — keeps filter rows and overlay height stable. */
  let catalogEntries: ClipboardEntry[] = $state([]);
  let collections: Collection[] = $state([]);
  let searchQuery = $state("");
  let activeCollectionId: number | null = $state(null);
  let pinnedOnly = $state(false);
  let activeTag = $state<string | null>(null);
  let contentKind = $state<ContentKind>("all");
  let aiTaggingEnabled = $state(false);
  let selectedIndex = $state(-1);
  let gridEl: HTMLDivElement | undefined = $state();
  let appEl: HTMLDivElement | undefined = $state();
  let visible = $state(false);
  let isRevealing = $state(false);
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let revealTimer: ReturnType<typeof setTimeout> | undefined;
  let pendingReload = false;
  let revealSeq = 0;
  let hideTransitionHandler: ((e: TransitionEvent) => void) | undefined;
  let retagAvailable = $state(false);
  let excludeCandidate: ExcludableAppCandidate | null = $state(null);
  let excludeNotice = $state("");
  let excludeNoticeTone = $state<"neutral" | "warn">("neutral");
  let excludeBusy = $state(false);
  let searchBar: SearchBar | undefined = $state();
  let settingsLoadError = $state<string | null>(null);
  let lastLayoutHeight = $state<number | null>(null);
  let dataFetchGen = 0;
  let activating = $state(false);

  const SETTINGS_SYNC_USER_NOTICE =
    "Couldn't load app settings. Tags and filters may not work properly. Restart Copyosity.";

  const settingsSyncNotice = $derived(
    settingsLoadError !== null ? SETTINGS_SYNC_USER_NOTICE : null,
  );

  async function syncRetagAvailability() {
    retagAvailable = await isTaggingReady();
  }

  async function syncAiTaggingSettings() {
    try {
      const settings = await getAppSettings();
      settingsLoadError = null;
      const enabled = settings.ai_tagging_enabled;
      if (enabled !== aiTaggingEnabled) {
        if (!enabled) {
          if (activeTag && !activeTagCompatibleWithAi(activeTag, false)) {
            activeTag = null;
          }
        } else {
          contentKind = "all";
          activeTag = null;
        }
      }
      aiTaggingEnabled = enabled;
    } catch (err) {
      aiTaggingEnabled = false;
      settingsLoadError = invokeErrorMessage(err) || "unknown";
    }
  }

  async function syncOverlaySettings() {
    await Promise.all([syncRetagAvailability(), syncAiTaggingSettings()]);
  }

  async function loadExcludeCandidate() {
    try {
      const candidate = await getExcludableAppCandidate();
      excludeCandidate = candidate;
      if (candidate?.alreadyExcluded) {
        excludeNotice = alreadyExcludedFromHistoryLabel(candidate.displayName);
        excludeNoticeTone = "neutral";
        return;
      }
      excludeNotice = "";
    } catch (err) {
      excludeCandidate = null;
      excludeNotice = invokeErrorMessage(err) || "Could not detect active app";
      excludeNoticeTone = "warn";
    }
  }

  async function handleExcludeFromPanel() {
    if (excludeBusy) return;
    excludeBusy = true;
    try {
      const added = await addExcludableAppCandidate();
      if (added) {
        await loadExcludeCandidate();
        return;
      }
      excludeNotice = "No active app";
      excludeNoticeTone = "warn";
    } catch (err) {
      excludeNotice = invokeErrorMessage(err) || "Could not exclude this app";
      excludeNoticeTone = "warn";
    } finally {
      excludeBusy = false;
    }
  }

  function entryQuery() {
    return {
      collection_id: activeCollectionId,
      pinned_only: pinnedOnly,
    };
  }

  async function activateSelectedEntry(entryId: number) {
    if (activating) return;
    activating = true;
    try {
      await activateEntry(entryId);
    } finally {
      activating = false;
    }
  }

  function applyEntrySelection(selectFirst: boolean, scrollToFirst: boolean) {
    if (selectFirst) {
      selectedIndex = filteredEntries.length > 0 ? 0 : -1;
      if (scrollToFirst) scrollToSelected();
    }
  }

  async function loadCatalog(gen: number) {
    const data = await getEntries({ ...entryQuery(), search: null });
    if (gen !== dataFetchGen) return false;
    catalogEntries = data;
    if (!searchQuery) entries = data;
    return true;
  }

  async function loadSearchEntries(
    selectFirst = false,
    scrollToFirst = true,
    gen = dataFetchGen,
  ) {
    if (gen !== dataFetchGen) return;

    if (!searchQuery) {
      entries = catalogEntries;
      applyEntrySelection(selectFirst, scrollToFirst);
      return;
    }

    const data = await getEntries({ ...entryQuery(), search: searchQuery });
    if (gen !== dataFetchGen) return;
    entries = data;
    applyEntrySelection(selectFirst, scrollToFirst);
  }

  /** Refresh catalog; when search is active, also refresh filtered entries. */
  async function loadEntries(selectFirst = false, scrollToFirst = true) {
    const gen = ++dataFetchGen;
    if (!(await loadCatalog(gen))) return;
    if (searchQuery) {
      await loadSearchEntries(selectFirst, scrollToFirst, gen);
    } else {
      applyEntrySelection(selectFirst, scrollToFirst);
    }
  }

  function nextPaint(): Promise<void> {
    return new Promise((resolve) => {
      requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
    });
  }

  function clearHideTimer() {
    if (hideTimer !== undefined) {
      clearTimeout(hideTimer);
      hideTimer = undefined;
    }
  }

  function clearRevealTimer() {
    if (revealTimer !== undefined) {
      clearTimeout(revealTimer);
      revealTimer = undefined;
    }
  }

  function clearHideTransitionHandler() {
    if (hideTransitionHandler && appEl) {
      appEl.removeEventListener("transitionend", hideTransitionHandler);
      hideTransitionHandler = undefined;
    }
  }

  function requestNativeHide() {
    clearHideTimer();
    clearHideTransitionHandler();

    let committed = false;
    const commit = () => {
      if (committed) return;
      committed = true;
      clearHideTimer();
      clearHideTransitionHandler();
      void hideMainWindow();
    };

    const onTransitionEnd = (e: TransitionEvent) => {
      if (e.target !== appEl || e.propertyName !== "opacity") return;
      commit();
    };

    hideTransitionHandler = onTransitionEnd;
    appEl?.addEventListener("transitionend", onTransitionEnd);
    hideTimer = setTimeout(() => {
      hideTimer = undefined;
      commit();
    }, panelCloseFallbackMs());
  }

  async function loadCollections() {
    collections = await getCollections();
  }

  function finishReveal() {
    isRevealing = false;
    revealTimer = undefined;
    if (pendingReload) {
      pendingReload = false;
      void loadEntries(true, false);
    }
  }

  function resetOverlayMotionState() {
    revealSeq += 1;
    clearRevealTimer();
    isRevealing = false;
    visible = false;
    clearSearch({ reload: false });
    activeTag = null;
    selectedIndex = -1;
    resetOverlayResizeState();
  }

  async function applyOverlayHeight(height: number, animated: boolean) {
    if (animated && visible) {
      await animateOverlayResize(height);
    } else {
      await resizeMainWindow(height);
    }
  }

  async function prepareOverlayLayout() {
    await syncOverlaySettings();
    await loadEntries(true, false);
    const height = overlayHeightForLayout({
      tagBar: tagBarModel,
      hasSettingsNotice: settingsSyncNotice !== null,
    });
    await applyOverlayHeight(height, false);
    lastLayoutHeight = height;
  }

  function showWindow() {
    const seq = ++revealSeq;
    window.getSelection()?.removeAllRanges();
    clearHideTimer();
    clearHideTransitionHandler();
    clearRevealTimer();
    clearSearch({ reload: false });
    activeTag = null;
    resetOverlayResizeState();

    isRevealing = true;
    pendingReload = false;
    if (gridEl) gridEl.scrollLeft = 0;

    // Always reset to hidden first so CSS transition replays on every open.
    visible = false;
    void (async () => {
      await prepareOverlayLayout();
      if (seq !== revealSeq) return;
      await nextPaint();
      if (seq !== revealSeq) return;
      visible = true;
      searchBar?.blur();
      revealTimer = setTimeout(finishReveal, panelOpenMs());
      void loadExcludeCandidate();
    })();
  }

  function startVisualHide() {
    revealSeq += 1;
    clearRevealTimer();
    isRevealing = false;
    pendingReload = false;
    visible = false;
  }

  function animateOut() {
    startVisualHide();
    requestNativeHide();
  }

  function forceHideWindow() {
    animateOut();
  }

  onMount(() => {
    void syncOverlaySettings();
    loadEntries();
    loadCollections();

    // Tell Rust we're loaded — it will hide the off-screen warmup window
    invoke("frontend_ready");

    // Debounce entry reloads — clipboard-changed and entry-tagged can fire together
    let reloadTimer: ReturnType<typeof setTimeout>;
    function scheduleReload() {
      if (isRevealing) {
        pendingReload = true;
        return;
      }
      clearTimeout(reloadTimer);
      reloadTimer = setTimeout(() => loadEntries(), 100);
    }

    const unlistenClipboard = listen("clipboard-changed", scheduleReload);
    const unlistenTagged = listen("entry-tagged", scheduleReload);

    const unlistenShow = listen("window-show", () => {
      showWindow();
    });

    const unlistenHideRequest = listen("window-hide-request", () => {
      startVisualHide();
      requestNativeHide();
    });

    const unlistenHide = listen("window-hide", () => {
      clearHideTimer();
      clearHideTransitionHandler();
      resetOverlayMotionState();
    });

    const unlistenOpenSettings = listen("open-settings", () => {
      openSettingsWindow();
    });

    const handleKeydown = (e: KeyboardEvent) => {
      if (!visible) return;

      const searchFocused = searchBar?.isFocused() ?? false;
      const target = e.target;
      const typingInField =
        target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;

      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        if (overlayEscapeAction(searchQuery.length > 0) === "clear-search") {
          clearSearch({ immediate: true });
          searchBar?.blur();
          return;
        }
        forceHideWindow();
        return;
      }

      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        e.stopPropagation();
        searchBar?.focus();
        return;
      }

      if (
        e.key === "/" &&
        !searchFocused &&
        !typingInField &&
        !e.metaKey &&
        !e.ctrlKey &&
        !e.altKey
      ) {
        e.preventDefault();
        searchBar?.focus();
        return;
      }

      if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
        // ←/→ always browse cards (including while search is focused); block in other text inputs.
        if (typingInField && !searchFocused) return;
        e.preventDefault();
        if (e.key === "ArrowRight") {
          selectedIndex = Math.min(selectedIndex + 1, filteredEntries.length - 1);
        } else {
          selectedIndex = Math.max(selectedIndex - 1, 0);
        }
        scrollToSelected();
        return;
      }

      if (e.key === "Enter") {
        if (selectedIndex < 0 || selectedIndex >= filteredEntries.length) return;
        // Paste from search when a result is selected; block Enter in other text fields.
        if (typingInField && !searchFocused) return;
        e.preventDefault();
        e.stopPropagation();
        const entry = filteredEntries[selectedIndex];
        if (entry.content_type === "text" || entry.content_type === "image") {
          void activateSelectedEntry(entry.id);
        }
        return;
      }
    };

    window.addEventListener("keydown", handleKeydown, true);

    return () => {
      clearHideTimer();
      clearHideTransitionHandler();
      clearRevealTimer();
      clearTimeout(reloadTimer);
      clearTimeout(debounceTimer);
      unlistenClipboard.then((fn) => fn());
      unlistenTagged.then((fn) => fn());
      unlistenShow.then((fn) => fn());
      unlistenHideRequest.then((fn) => fn());
      unlistenHide.then((fn) => fn());
      unlistenOpenSettings.then((fn) => fn());
      window.removeEventListener("keydown", handleKeydown, true);
    };
  });

  function handleCardSelect(index: number) {
    selectedIndex = index;
  }

  function getGridScrollInsets(container: HTMLElement) {
    const style = getComputedStyle(container);
    return {
      left: parseFloat(style.paddingLeft) || 0,
      right: parseFloat(style.paddingRight) || 0,
    };
  }

  function snapCardIntoPaddedViewport(
    card: HTMLElement,
    container: HTMLElement,
    behavior: ScrollBehavior,
  ) {
    const { left: padLeft, right: padRight } = getGridScrollInsets(container);
    const containerRect = container.getBoundingClientRect();
    const cardRect = card.getBoundingClientRect();
    const slack = 1;
    const visibleLeft = containerRect.left + padLeft;
    const visibleRight = containerRect.right - padRight;

    let delta = 0;
    if (cardRect.right > visibleRight + slack) {
      delta = cardRect.right - visibleRight;
    } else if (cardRect.left < visibleLeft - slack) {
      delta = cardRect.left - visibleLeft;
    }
    if (delta === 0) return;

    container.scrollTo({ left: container.scrollLeft + delta, behavior });
  }

  function snapCardToGridEnd(
    card: HTMLElement,
    container: HTMLElement,
    behavior: ScrollBehavior,
  ) {
    const { right: padRight } = getGridScrollInsets(container);
    const containerRect = container.getBoundingClientRect();
    const cardRect = card.getBoundingClientRect();
    const visibleRight = containerRect.right - padRight;
    const delta = cardRect.right - visibleRight;
    if (Math.abs(delta) <= 1) return;
    container.scrollTo({ left: container.scrollLeft + delta, behavior });
  }

  function scrollToSelected() {
    if (!gridEl) return;
    const cards = gridEl.querySelectorAll(".card");
    const card = cards[selectedIndex];
    if (!(card instanceof HTMLElement)) return;
    const keepSearchFocus = searchBar?.isFocused() ?? false;
    if (!keepSearchFocus) {
      card.focus({ preventScroll: true });
    }

    const behavior = scrollBehavior();
    const lastIndex = cards.length - 1;

    if (selectedIndex === 0) {
      gridEl.scrollTo({ left: 0, behavior });
      return;
    }

    if (selectedIndex === lastIndex) {
      snapCardToGridEnd(card, gridEl, behavior);
      return;
    }

    snapCardIntoPaddedViewport(card, gridEl, behavior);
  }

  function setSearchQuery(
    q: string,
    options: { reload?: boolean; immediate?: boolean } = {},
  ) {
    const { reload = true, immediate = false } = options;
    searchQuery = q;
    clearTimeout(debounceTimer);
    if (!reload) return;
    if (immediate || q === "") {
      const gen = ++dataFetchGen;
      void loadSearchEntries(true, true, gen);
      return;
    }
    debounceTimer = setTimeout(() => {
      const gen = ++dataFetchGen;
      void loadSearchEntries(true, true, gen);
    }, 150);
  }

  function queueSearch(q: string) {
    setSearchQuery(q);
  }

  function clearSearch(options: { reload?: boolean; immediate?: boolean } = {}) {
    setSearchQuery("", options);
  }

  function handleCollectionSelect(id: number | null) {
    pinnedOnly = id === -1;
    activeCollectionId = id === -1 ? null : id;
    activeTag = null;
    void loadEntries(true);
  }

  function handleEntryAction() {
    loadEntries();
  }

  let debounceTimer: ReturnType<typeof setTimeout>;
  function debouncedSearch(q: string) {
    if (q === "") {
      clearSearch({ immediate: true });
      return;
    }
    queueSearch(q);
  }

  function emptyStateCopy(): { title: string; hint?: string } {
    if (searchQuery && activeTag) {
      return {
        title: `No results for “${searchQuery}” in tag “${activeTag}”`,
        hint: "Try a different search or tag",
      };
    }
    if (searchQuery) {
      return {
        title: `No results for “${searchQuery}”`,
        hint: "Try a different search term",
      };
    }
    if (activeTag) {
      if (isFormatTag(activeTag)) {
        return {
          title: formatTagEmptyLabel(activeTag),
          hint: "Try another format or clear the filter",
        };
      }
      return {
        title: `No results for tag “${activeTag}”`,
        hint: "Try another tag or clear the filter",
      };
    }
    if (aiTaggingEnabled) {
      const kindLabel = contentKindEmptyLabel(contentKind);
      if (kindLabel) {
        return {
          title: kindLabel,
          hint: "Try another content type or clear filters",
        };
      }
    }
    return {
      title: "Clipboard history is empty",
      hint: "Copy something to get started",
    };
  }

  const tagBarModel = $derived(
    buildTagBarModel({
      entries,
      layoutEntries: catalogEntries,
      contentKind,
      aiTaggingEnabled,
      activeTag,
    }),
  );

  const kindPool = $derived(
    filterKindPool(entries, aiTaggingEnabled && tagBarModel.showRowA, contentKind),
  );

  const overlayLayoutHeight = $derived(
    overlayHeightForLayout({
      tagBar: tagBarModel,
      hasSettingsNotice: settingsSyncNotice !== null,
    }),
  );

  $effect(() => {
    if (!tagBarModel.showRowA && contentKind !== "all") {
      contentKind = "all";
      if (activeTag && !activeTagCompatibleWithKind(activeTag, "all")) {
        activeTag = null;
      }
    }
  });

  $effect(() => {
    const height = overlayLayoutHeight;
    if (!visible) {
      lastLayoutHeight = null;
      return;
    }
    if (lastLayoutHeight === height) return;
    const previous = lastLayoutHeight;
    lastLayoutHeight = height;
    const animate = previous !== null && !isRevealing;
    void applyOverlayHeight(height, animate);
  });

  const filteredEntries = $derived(filterByActiveTag(kindPool, activeTag));

  function handleContentKindChange(kind: ContentKind) {
    contentKind = kind;
    if (!activeTagCompatibleWithKind(activeTag, kind)) {
      activeTag = null;
    }
    resetKeyboardSelection();
  }

  function handleTagSelect(tag: string) {
    activeTag = tag;
    resetKeyboardSelection();
  }

  function handleTagReset() {
    activeTag = null;
    resetKeyboardSelection();
  }

  function resetKeyboardSelection() {
    selectedIndex = filteredEntries.length > 0 ? 0 : -1;
    scrollToSelected();
  }
</script>

<div class="app" class:visible bind:this={appEl}>
  <header class="header">
    <SearchBar bind:this={searchBar} value={searchQuery} onchange={debouncedSearch} />
    <CollectionTabs
      {collections}
      activeId={activeCollectionId}
      activePinned={pinnedOnly}
      onselect={handleCollectionSelect}
      onupdate={loadCollections}
    />
    <div class="header-actions">
      {#if excludeCandidate && !excludeCandidate.alreadyExcluded}
        {@const excludeLabel = excludeFromClipboardHistoryAriaLabel(
          excludeCandidate.displayName,
        )}
        <button
          class="form-btn-restrict exclude-app-btn app-btn"
          type="button"
          aria-label={excludeLabel}
          aria-busy={excludeBusy}
          disabled={excludeBusy}
          onclick={() => void handleExcludeFromPanel()}
        >
          <span class="exclude-app-btn-text"
            >{excludeFromHistoryLabel(excludeCandidate.displayName)}</span
          >
        </button>
      {/if}
      {#if excludeNotice}
        <span
          class="status-hint exclude-notice"
          class:neutral={excludeNoticeTone === "neutral"}
          class:warn={excludeNoticeTone === "warn"}
          aria-live="polite"
        >
          {excludeNotice}
        </span>
      {/if}
      <button
        class="settings-btn app-btn"
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
      <button
        class="close-btn app-btn"
        type="button"
        aria-label="Close overlay"
        onclick={() => forceHideWindow()}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M6.4 6.4 17.6 17.6M17.6 6.4 6.4 17.6" />
        </svg>
      </button>
    </div>
  </header>

  {#if settingsSyncNotice || tagBarModel.showRowA || tagBarModel.showRowB}
    <div class="filter-zone">
      {#if settingsSyncNotice}
        <p
          class="status-hint settings-sync-notice warn"
          role="status"
          aria-live="polite"
        >
          {settingsSyncNotice}
        </p>
      {/if}
      {#if tagBarModel.showRowA}
        <div class="filter-row-a">
          <ContentKindSegment value={contentKind} onchange={handleContentKindChange} />
        </div>
      {/if}
      {#if tagBarModel.showRowB}
        <TagFilterBar
          resetLabel={tagBarModel.resetLabel}
          {activeTag}
          formatChips={tagBarModel.formatChips}
          semanticChips={tagBarModel.semanticChips}
          showDivider={tagBarModel.showDivider}
          onreset={handleTagReset}
          onselect={handleTagSelect}
        />
      {/if}
    </div>
  {/if}

  <div class="grid-container" bind:this={gridEl}>
    {#if filteredEntries.length === 0}
      {@const empty = emptyStateCopy()}
      <div class="empty-state" role="status" aria-live="polite">
        <p class="empty-title">{empty.title}</p>
        {#if empty.hint}
          <p class="hint">{empty.hint}</p>
        {/if}
      </div>
    {:else}
      {#each filteredEntries as entry, i (entry.id)}
        <div class="card-wrapper">
          <ClipboardCard
            {entry}
            {retagAvailable}
            {aiTaggingEnabled}
            selected={i === selectedIndex}
            onselect={() => handleCardSelect(i)}
            ondeleted={handleEntryAction}
            onpinned={handleEntryAction}
            onretagged={handleEntryAction}
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
    color: var(--color-text-body);
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
    background: var(--surface-app);
    backdrop-filter: blur(var(--panel-blur-visible)) saturate(1.15);
    -webkit-backdrop-filter: blur(var(--panel-blur-visible)) saturate(1.15);
    border-radius: var(--radius-panel);
    border: 1px solid var(--border-strong);
    box-shadow:
      var(--shadow-elevated),
      var(--shadow-inset-highlight);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    backface-visibility: hidden;
    transform: translate3d(0, var(--panel-open-travel), 0);
    opacity: 0;
    will-change: transform, opacity;
    /* Transition on hidden state runs when opening (visible added). */
    transition:
      transform var(--duration-panel-open) var(--ease-apple-panel),
      opacity var(--duration-panel-opacity-open) var(--ease-apple-panel);
  }

  .app.visible {
    transform: translate3d(0, 0, 0);
    opacity: 1;
    will-change: auto;
    /* Transition on visible state runs when closing (visible removed). */
    transition:
      transform var(--duration-panel-close) var(--ease-panel-dismiss),
      opacity var(--duration-panel-opacity-close) var(--ease-panel-dismiss);
  }

  @media (prefers-reduced-motion: reduce) {
    .app,
    .app.visible {
      transition-duration: 0.01ms;
    }
  }

  @media (prefers-reduced-transparency: reduce) {
    .app {
      backdrop-filter: none;
      -webkit-backdrop-filter: none;
    }
  }

  .header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-default);
    background: var(--surface-1);
    flex-shrink: 0;
  }

  .filter-zone {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 10px;
    transition:
      opacity var(--duration-fast) var(--ease-interactive),
      transform var(--duration-fast) var(--ease-interactive);
  }

  @media (prefers-reduced-motion: reduce) {
    .filter-zone {
      transition: none;
    }
  }

  .filter-row-a {
    padding: 0 16px;
  }

  .settings-sync-notice {
    margin: 0 16px;
  }

  .header-actions {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
    flex-shrink: 0;
  }

  .exclude-app-btn {
    height: 36px;
    max-width: min(220px, 42vw);
    padding: 0 12px;
    border-radius: var(--radius-control);
    font: inherit;
    font-size: var(--font-size-xs);
    font-weight: 600;
    cursor: pointer;
  }

  .exclude-app-btn-text {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .exclude-notice {
    margin: 0;
    white-space: nowrap;
  }

  .settings-btn,
  .close-btn {
    width: 36px;
    height: 36px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--surface-6);
    border: 1px solid var(--border-soft);
    border-radius: var(--radius-control);
    color: var(--color-text-body);
    cursor: pointer;
  }

  .settings-btn:hover:not(:disabled, [aria-busy="true"]),
  .close-btn:hover:not(:disabled, [aria-busy="true"]) {
    background: var(--surface-10);
    border-color: var(--border-emphasis);
  }

  .settings-btn svg {
    width: 18px;
    height: 18px;
    fill: currentcolor;
  }

  .close-btn svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentcolor;
    stroke-width: 2;
    stroke-linecap: round;
  }

  .grid-container {
    flex: 1;
    display: flex;
    gap: 12px;
    padding: 14px 16px var(--space-section);
    scroll-padding-inline: 16px;
    overflow: auto hidden;
    align-items: flex-start;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-thumb) transparent;
    min-height: 0;
  }

  .grid-container::-webkit-scrollbar {
    height: 6px;
  }

  .grid-container::-webkit-scrollbar-track {
    background: transparent;
  }

  .grid-container::-webkit-scrollbar-thumb {
    background: var(--scrollbar-thumb);
    border-radius: var(--radius-scrollbar);
  }

  .card-wrapper {
    flex-shrink: 0;
  }

  .empty-state {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    padding: 0 24px;
    text-align: center;
    color: var(--color-text-tertiary);
  }

  .empty-title {
    margin: 0;
    font-size: var(--font-size-lg);
    font-weight: 500;
    color: var(--color-text-secondary);
  }

  .hint {
    margin: 8px 0 0;
    font-size: var(--font-size-md);
    color: var(--color-text-label);
  }
</style>
