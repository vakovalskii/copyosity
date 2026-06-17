<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { parseEntryTaggedEvent, type Collection, type ExcludableAppCandidate } from "$lib/types";
  import {
    getCollections,
    hideMainWindow,
    openSettingsWindow,
    activateEntry,
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
  import KeyboardHints, { type KeyboardHint } from "$lib/components/KeyboardHints.svelte";
  import SearchBar from "$lib/components/SearchBar.svelte";
  import CollectionTabs from "$lib/components/CollectionTabs.svelte";
  import ContentKindSegment from "$lib/components/ContentKindSegment.svelte";
  import TagFilterBar from "$lib/components/TagFilterBar.svelte";
  import {
    buildTagBarModel,
    isFormatTag,
    contentKindEmptyLabel,
    formatTagEmptyLabel,
  } from "$lib/overlay-filters";
  import { createOverlayEntriesStore } from "$lib/overlay-entries.svelte";
  import { overlayEscapeAction } from "$lib/overlay-search";
  import { setInputModality } from "$lib/input-modality";
  import { overlayHeightForLayout } from "$lib/overlay-layout";
  import {
    animateOverlayResize,
    resetOverlayResizeState,
    resizeMainWindow,
  } from "$lib/overlay-resize";
  import { panelCloseFallbackMs, panelOpenMs, scrollBehavior } from "$lib/motion";
  import { shouldLoadNextEntryPage } from "$lib/overlay-pagination";

  const overlayShortcutHints: KeyboardHint[] = [
    { prefix: "Click", action: "copy" },
    { keys: "↵", action: "paste" },
    { prefix: "Double-click", action: "paste" },
    { keys: ["←", "→"], action: "browse" },
    { keys: "Esc", action: "clear search / dismiss" },
  ];

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
  let excludeCandidate: ExcludableAppCandidate | null = $state(null);
  let excludeNotice = $state("");
  let excludeNoticeTone = $state<"neutral" | "warn">("neutral");
  let excludeBusy = $state(false);
  let searchBar: SearchBar | undefined = $state();
  let lastLayoutHeight = $state<number | null>(null);
  let collections: Collection[] = $state([]);
  let activating = $state(false);

  const SETTINGS_SYNC_USER_NOTICE =
    "Couldn't load app settings. Tags and filters may not work properly. Restart Copyosity.";

  const overlay = createOverlayEntriesStore({
    getVisible: () => visible,
    getIsRevealing: () => isRevealing,
    onSelectionRequested: (_selectFirst, scrollToFirst) => {
      selectedIndex = filteredEntries.length > 0 ? 0 : -1;
      if (scrollToFirst) scrollToSelected();
    },
    onClampSelection: () => {
      if (selectedIndex >= overlay.entries.length) {
        selectedIndex = overlay.entries.length > 0 ? overlay.entries.length - 1 : -1;
      }
    },
  });

  const settingsSyncNotice = $derived(
    overlay.settingsLoadError !== null ? SETTINGS_SYNC_USER_NOTICE : null,
  );

  const tagBarModel = $derived(
    buildTagBarModel({
      entries: overlay.entries,
      layoutEntries: overlay.catalogEntries,
      contentKind: overlay.contentKind,
      aiTaggingEnabled: overlay.aiTaggingEnabled,
      activeTag: overlay.activeTag,
      displayTagCounts: overlay.searchQuery ? overlay.searchTagCounts : overlay.catalogTagCounts,
      layoutTagCounts: overlay.catalogTagCounts,
    }),
  );

  const overlayLayoutHeight = $derived(
    overlayHeightForLayout({
      tagBar: tagBarModel,
      hasSettingsNotice: settingsSyncNotice !== null,
      showShortcutHints: overlay.overlayShortcutHintsEnabled,
    }),
  );

  const filteredEntries = $derived(overlay.entries);

  $effect(() => {
    if (selectedIndex < 0) return;
    if (selectedIndex < filteredEntries.length) return;
    selectedIndex = filteredEntries.length > 0 ? filteredEntries.length - 1 : -1;
    if (filteredEntries.length > 0) scrollToSelected();
  });

  $effect(() => {
    if (overlay.displayListPending) selectedIndex = -1;
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

  async function activateSelectedEntry(entryId: number) {
    if (activating) return;
    activating = true;
    try {
      await activateEntry(entryId);
    } finally {
      activating = false;
    }
  }

  function emptyStateCopy(): { title: string; hint?: string } {
    if (overlay.displayFetchFailed) {
      if (overlay.searchQuery) {
        return {
          title: `Couldn't search for “${overlay.searchQuery}”`,
          hint: "Something went wrong — try again or clear the search",
        };
      }
      if (overlay.activeTag) {
        return {
          title: `Couldn't load tag “${overlay.activeTag}”`,
          hint: "Something went wrong — try again or clear the filter",
        };
      }
      if (overlay.aiTaggingEnabled && overlay.contentKind !== "all") {
        const kindLabel = contentKindEmptyLabel(overlay.contentKind);
        return {
          title: kindLabel ? `Couldn't load ${kindLabel.toLowerCase()}` : "Couldn't load filter",
          hint: "Something went wrong — try again or clear filters",
        };
      }
      return {
        title: "Couldn't load clipboard entries",
        hint: "Something went wrong — try again",
      };
    }
    if (overlay.loadMoreFailed) {
      return {
        title: "Couldn't load more entries",
        hint: "Something went wrong — try again",
      };
    }
    if (overlay.searchQuery && overlay.activeTag) {
      return {
        title: `No results for “${overlay.searchQuery}” in tag “${overlay.activeTag}”`,
        hint: "Try a different search or tag",
      };
    }
    if (overlay.searchQuery) {
      return {
        title: `No results for “${overlay.searchQuery}”`,
        hint: "Try a different search term",
      };
    }
    if (overlay.activeTag) {
      if (isFormatTag(overlay.activeTag)) {
        return {
          title: formatTagEmptyLabel(overlay.activeTag),
          hint: "Try another format or clear the filter",
        };
      }
      return {
        title: `No results for tag “${overlay.activeTag}”`,
        hint: "Try another tag or clear the filter",
      };
    }
    if (overlay.aiTaggingEnabled) {
      const kindLabel = contentKindEmptyLabel(overlay.contentKind);
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
      void (async () => {
        await overlay.loadEntries(true, false);
        scrollToSelected();
      })();
    }
  }

  function resetOverlayMotionState() {
    revealSeq += 1;
    clearRevealTimer();
    isRevealing = false;
    visible = false;
    overlay.resetDisplayStateOnHide();
    overlay.clearSearch({ reload: false });
    overlay.resetOverlayFilters();
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

  async function prepareOverlayLayout(seq: number): Promise<boolean> {
    const loaded = await overlay.prepareCatalogAndDisplay(() => seq === revealSeq);
    if (!loaded || seq !== revealSeq) return false;
    const height = overlayHeightForLayout({
      tagBar: tagBarModel,
      hasSettingsNotice: settingsSyncNotice !== null,
      showShortcutHints: overlay.overlayShortcutHintsEnabled,
    });
    await applyOverlayHeight(height, false);
    lastLayoutHeight = height;
    return true;
  }

  function showWindow() {
    const seq = ++revealSeq;
    window.getSelection()?.removeAllRanges();
    clearHideTimer();
    clearHideTransitionHandler();
    clearRevealTimer();
    overlay.clearSearch({ reload: false });
    overlay.resetOverlayFilters();
    resetOverlayResizeState();

    isRevealing = true;
    const hadPendingReload = pendingReload;
    pendingReload = false;
    if (gridEl) gridEl.scrollLeft = 0;

    visible = false;
    void (async () => {
      const ready = await prepareOverlayLayout(seq);
      if (!ready || seq !== revealSeq) {
        isRevealing = false;
        return;
      }
      await nextPaint();
      if (seq !== revealSeq) return;
      visible = true;
      searchBar?.blur();
      await nextPaint();
      if (seq !== revealSeq) return;
      if (hadPendingReload) {
        void overlay.loadEntries(true, false);
      }
      scrollToSelected();
      revealTimer = setTimeout(finishReveal, panelOpenMs());
      void loadExcludeCandidate();
    })();
  }

  function startVisualHide() {
    revealSeq += 1;
    clearRevealTimer();
    isRevealing = false;
    visible = false;
    overlay.resetDisplayStateOnHide();
  }

  function animateOut() {
    startVisualHide();
    requestNativeHide();
  }

  function forceHideWindow() {
    animateOut();
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

  // TEST-NOTE (+page integration): reveal/hide, keyboard, scroll prefetch, and Tauri
  // events are not automated. Playwright + running app would be needed (not installed).
  // Manual QA: docs/plans/05-overlay-content-and-tag-filters.md §7.
  onMount(() => {
    void overlay.syncOverlaySettings();
    void overlay.warmCatalog();
    loadCollections();

    invoke("frontend_ready");

    let reloadTimer: ReturnType<typeof setTimeout>;
    function scheduleReload() {
      if (isRevealing || !visible) {
        pendingReload = true;
        return;
      }
      clearTimeout(reloadTimer);
      reloadTimer = setTimeout(() => {
        if (isRevealing || !visible) {
          pendingReload = true;
          return;
        }
        overlay.loadEntries();
      }, 100);
    }

    function handleEntryTagged(event: { payload: unknown }) {
      const parsed = parseEntryTaggedEvent(event.payload);
      if (!parsed) return;
      if (parsed.kind === "legacy-id") {
        // Pre-EntryTaggedPayload emitters only sent the entry id; reload matches old behavior.
        void overlay.reloadDisplayList(false, false);
        return;
      }
      overlay.applyEntryTags(parsed.payload.entryId, parsed.payload.tags);
    }

    const unlistenClipboard = listen("clipboard-changed", scheduleReload);
    const unlistenHistory = listen("history-changed", () => {
      void overlay.syncOverlaySettings();
    });
    const unlistenTagged = listen("entry-tagged", handleEntryTagged);

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
        if (overlayEscapeAction(overlay.searchQuery.length > 0) === "clear-search") {
          overlay.clearSearch({ immediate: true });
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
        if (typingInField && !searchFocused) return;
        if (overlay.displayListPending || overlay.displayFetchFailed) return;
        e.preventDefault();
        setInputModality("keyboard");
        if (e.key === "ArrowRight") {
          selectedIndex = Math.min(selectedIndex + 1, filteredEntries.length - 1);
          if (selectedIndex === filteredEntries.length - 1) {
            void overlay.loadNextEntryPage();
          }
        } else {
          selectedIndex = Math.max(selectedIndex - 1, 0);
        }
        scrollToSelected();
        return;
      }

      if (e.key === "Enter") {
        if (overlay.displayListPending || overlay.displayFetchFailed) return;
        if (selectedIndex < 0 || selectedIndex >= filteredEntries.length) return;
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
      overlay.dispose();
      unlistenClipboard.then((fn) => fn());
      unlistenHistory.then((fn) => fn());
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

  function handleGridScroll(event: Event) {
    const target = event.currentTarget;
    if (!(target instanceof HTMLElement)) return;

    if (
      shouldLoadNextEntryPage({
        scrollLeft: target.scrollLeft,
        clientWidth: target.clientWidth,
        scrollWidth: target.scrollWidth,
        hasMore: overlay.entriesHasMore && !overlay.displayFetchFailed,
        loading: overlay.loadingMoreEntries || overlay.displayListPending,
      })
    ) {
      void overlay.loadNextEntryPage();
    }
  }

  function getGridScrollInsets(container: HTMLElement) {
    const style = getComputedStyle(container);
    return {
      left: parseFloat(style.paddingLeft) || 0,
      right: parseFloat(style.paddingRight) || 0,
    };
  }

  function scrollMeasureEl(card: HTMLElement): HTMLElement {
    const wrapper = card.parentElement;
    return wrapper instanceof HTMLElement ? wrapper : card;
  }

  function snapCardIntoPaddedViewport(
    card: HTMLElement,
    container: HTMLElement,
    behavior: ScrollBehavior,
  ) {
    const measureEl = scrollMeasureEl(card);
    const { left: padLeft, right: padRight } = getGridScrollInsets(container);
    const containerRect = container.getBoundingClientRect();
    const cardRect = measureEl.getBoundingClientRect();
    const slack = 2;
    const visibleLeft = containerRect.left + padLeft;
    const visibleRight = containerRect.right - padRight;

    if (cardRect.left >= visibleLeft - slack && cardRect.right <= visibleRight + slack) {
      return;
    }

    let delta = 0;
    if (cardRect.right > visibleRight + slack) {
      delta = cardRect.right - visibleRight;
    } else if (cardRect.left < visibleLeft - slack) {
      delta = cardRect.left - visibleLeft;
    }
    if (delta === 0) return;

    container.scrollTo({ left: container.scrollLeft + delta, behavior });
  }

  function blurDeselectedCards(cards: NodeListOf<Element>) {
    cards.forEach((c, i) => {
      if (i === selectedIndex || !(c instanceof HTMLElement)) return;
      if (c === document.activeElement || c.contains(document.activeElement)) {
        const active = document.activeElement;
        if (active instanceof HTMLElement) active.blur();
        c.blur();
      }
    });
  }

  function scrollToSelected() {
    void (async () => {
      if (!gridEl) return;
      await tick();
      const cards = gridEl.querySelectorAll(".card");
      const card = cards[selectedIndex];
      if (!(card instanceof HTMLElement)) return;

      blurDeselectedCards(cards);

      const behavior = scrollBehavior();
      snapCardIntoPaddedViewport(card, gridEl, behavior);

      const keepSearchFocus = searchBar?.isFocused() ?? false;
      if (!keepSearchFocus) {
        card.focus({ preventScroll: true });
      }
    })();
  }
</script>

<div class="app" class:visible bind:this={appEl}>
  <header class="header">
    <SearchBar bind:this={searchBar} value={overlay.searchQuery} onchange={overlay.debouncedSearch} />
    <CollectionTabs
      {collections}
      activeId={overlay.activeCollectionId}
      activePinned={overlay.pinnedOnly}
      onselect={overlay.handleCollectionSelect}
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
          <ContentKindSegment
            value={overlay.contentKind}
            onchange={overlay.handleContentKindChange}
          />
        </div>
      {/if}
      {#if tagBarModel.showRowB}
        <TagFilterBar
          resetLabel={tagBarModel.resetLabel}
          activeTag={overlay.activeTag}
          formatChips={tagBarModel.formatChips}
          semanticChips={tagBarModel.semanticChips}
          showDivider={tagBarModel.showDivider}
          onreset={overlay.handleTagReset}
          onselect={overlay.handleTagSelect}
        />
      {/if}
    </div>
  {/if}

  <div class="grid-container" bind:this={gridEl} onscroll={handleGridScroll}>
    {#if filteredEntries.length === 0}
      {@const empty = emptyStateCopy()}
      {@const listPending = overlay.displayListPending}
      {@const searchingMore = overlay.loadingMoreEntries}
      <div class="empty-state" role="status" aria-live="polite">
        <p class="empty-title">
          {#if listPending && overlay.searchQuery}
            Searching for “{overlay.searchQuery}”…
          {:else if listPending}
            Loading entries…
          {:else if searchingMore}
            Loading more entries…
          {:else}
            {empty.title}
          {/if}
        </p>
        {#if listPending && overlay.searchQuery}
          <p class="hint">Matching entries will appear here</p>
        {:else if listPending}
          <p class="hint">Updating the list</p>
        {:else if searchingMore}
          <p class="hint">Fetching the next page of matching entries</p>
        {:else if empty.hint}
          <p class="hint">{empty.hint}</p>
        {/if}
        {#if overlay.displayFetchFailed}
          <button
            class="empty-retry-btn"
            type="button"
            onclick={() => overlay.retryDisplayFetch()}
          >
            Try again
          </button>
        {:else if overlay.loadMoreFailed}
          <button
            class="empty-retry-btn"
            type="button"
            onclick={() => overlay.retryLoadMore()}
          >
            Try again
          </button>
        {/if}
      </div>
    {:else}
      {#each filteredEntries as entry, i (entry.id)}
        <div class="card-wrapper">
          <ClipboardCard
            {entry}
            retagAvailable={overlay.retagAvailable}
            aiTaggingEnabled={overlay.aiTaggingEnabled}
            selected={i === selectedIndex}
            onselect={() => handleCardSelect(i)}
            ondeleted={() => overlay.removeEntry(entry.id)}
            onpinned={() => overlay.handlePinned(entry.id, !entry.is_pinned)}
            onretagged={(tags) => overlay.applyEntryTags(entry.id, tags)}
          />
        </div>
      {/each}
    {/if}
  </div>
  {#if filteredEntries.length > 0 && overlay.loadMoreFailed}
    <div class="load-more-banner" role="status" aria-live="polite">
      <p class="hint">Couldn't load more entries</p>
      <button class="empty-retry-btn" type="button" onclick={() => overlay.retryLoadMore()}>
        Try again
      </button>
    </div>
  {/if}
  {#if overlay.overlayShortcutHintsEnabled}
    <footer class="overlay-shortcuts">
      <KeyboardHints hints={overlayShortcutHints} />
    </footer>
  {/if}
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
    min-height: 0;
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

  .empty-retry-btn {
    margin-top: 12px;
    padding: 6px 14px;
    font-size: var(--font-size-md);
    font-weight: 500;
    color: var(--color-text-body);
    background: var(--surface-2);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-control);
    cursor: pointer;
  }

  .empty-retry-btn:hover {
    background: var(--surface-3);
  }

  .empty-retry-btn:focus-visible {
    outline: none;
    box-shadow: var(--ring-accent);
  }

  .load-more-banner {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 8px 16px;
    border-top: 1px solid var(--border-default);
    background: var(--surface-1);
  }

  .load-more-banner .hint {
    margin: 0;
  }

  .overlay-shortcuts {
    flex-shrink: 0;
    padding: 6px 16px 8px;
    border-top: 1px solid var(--border-default);
    background: var(--surface-1);
  }
</style>
