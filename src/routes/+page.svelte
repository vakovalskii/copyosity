<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { parseEntryOcrEvent, parseEntryTaggedEvent, type Collection, type ExcludableAppCandidate } from "$lib/types";
  import {
    getCollections,
    getAppSettings,
    hideMainWindow,
    openSettingsWindow,
    activateEntry,
    getExcludableAppCandidate,
    addExcludableAppCandidate,
    openCommandPalette,
  } from "$lib/api";
  import {
    alreadyExcludedFromHistoryLabel,
    excludeFromClipboardHistoryAriaLabel,
    excludeFromHistoryLabel,
    invokeErrorMessage,
  } from "$lib/exclusion-label";
  import { autoUpdateOnLaunch, notify } from "$lib/updater";
  import ClipboardCard from "$lib/components/ClipboardCard.svelte";
  import QuickLookPanel from "$lib/components/QuickLookPanel.svelte";
  import KeyboardHints, { type KeyboardHint } from "$lib/components/KeyboardHints.svelte";
  import { t } from "$lib/i18n";
  import SearchBar from "$lib/components/SearchBar.svelte";
  import CollectionTabs from "$lib/components/CollectionTabs.svelte";
  // TEMP: re-enable with Content Kind segment block below.
  // import ContentKindSegment from "$lib/components/ContentKindSegment.svelte";
  import TagFilterBar from "$lib/components/TagFilterBar.svelte";
  import {
    buildTagBarModel,
    isFormatTag,
    contentKindEmptyLabel,
    formatTagEmptyLabel,
    type TagChip,
  } from "$lib/overlay-filters";
  import { createOverlayEntriesStore } from "$lib/overlay-entries.svelte";
  import { shouldShowOverlayEntryGrid } from "$lib/overlay-entries-logic";
  import { closeAllCardContextMenus, isCardContextMenuOpen } from "$lib/overlay-card-context-menu";
  import {
    canToggleQuickLook,
    resolveOverlayEscapeAction,
    shouldBlockOverlayActionWhileQuickLookOpen,
    shouldExitSearchToGrid,
    shouldHandleQuickLookCmdY,
    shouldHandleQuickLookSpace,
  } from "$lib/quick-look-keyboard";
  import { setInputModality, resetFocusState } from "$lib/input-modality";
  import { overlayHeightForLayout } from "$lib/overlay-layout";
  import {
    animateOverlayResize,
    resetOverlayResizeState,
    resizeMainWindow,
  } from "$lib/overlay-resize";
  import { panelCloseFallbackMs, panelOpenMs, scrollBehavior, afterLayoutFlush } from "$lib/motion";
  import {
    createPanelTransitionEpoch,
    planInstantNativeHide,
    type PanelMotionMode,
  } from "$lib/overlay-motion";
  import { shouldLoadNextEntryPage } from "$lib/overlay-pagination";
  import {
    indexOfLeadingVisibleCard,
    indexOfLeadingVisibleCardVertical,
    isCardOffScreen,
    isCardOffScreenVertical,
    nextIndexAfterKeyboardArrow,
    verticalCardViewportPosition,
    verticalScrollDeltaForKeyboardNav,
    verticalScrollDeltaToSnapCard,
  } from "$lib/overlay-grid-scroll";
  import {
    handleScrollEndBrowseSync,
    shouldClearStuckSuppressOnUserScroll,
    shouldRunScrollToSelectedGeneration,
    shouldScheduleTrackpadLeadingSync,
  } from "$lib/overlay-browse-sync";
  import { initPlatform, platformIsMacOS } from "$lib/platform.svelte";

  const overlayShortcutHints = $derived<KeyboardHint[]>([
    { prefix: "Click", action: $t("overlay.hint.copy") },
    { keys: "Space", action: $t("overlay.hint.preview") },
    { keys: "⌘Y", action: $t("overlay.hint.preview") },
    { keys: "↵", action: $t("overlay.hint.paste") },
    { prefix: "Double-click", action: $t("overlay.hint.paste") },
    { keys: ["←", "→"], action: $t("overlay.hint.browse") },
    { keys: "Esc", action: $t("overlay.hint.dismiss") },
  ]);

  const overlayVerticalShortcutHints = $derived<KeyboardHint[]>([
    { prefix: "Click", action: $t("overlay.hint.copy") },
    { keys: "Space", action: $t("overlay.hint.preview") },
    { keys: "⌘Y", action: $t("overlay.hint.preview") },
    { keys: "↵", action: $t("overlay.hint.paste") },
    { prefix: "2× click", action: $t("overlay.hint.paste") },
    { keys: ["↑", "↓"], action: $t("overlay.hint.browse") },
    { keys: "Esc", action: $t("overlay.hint.dismiss") },
  ]);

  let boardVertical = $state(false);
  let hubEnabled = $state(false);

  const activeOverlayShortcutHints = $derived(
    boardVertical ? overlayVerticalShortcutHints : overlayShortcutHints,
  );
  let selectedIndex = $state(-1);
  let quickLookOpen = $state(false);
  let quickLookReturnFocus: HTMLElement | null = null;
  let gridEl: HTMLDivElement | undefined = $state();
  let appEl: HTMLDivElement | undefined = $state();
  let visible = $state(false);
  let isRevealing = $state(false);
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let revealTimer: ReturnType<typeof setTimeout> | undefined;
  let pendingReload = false;
  let revealSeq = 0;
  let hideGeneration = 0;
  let nativeHidePending = false;
  let hideTransitionHandler: ((e: TransitionEvent) => void) | undefined;
  let excludeCandidate: ExcludableAppCandidate | null = $state(null);
  let excludeNotice = $state("");
  let excludeNoticeTone = $state<"neutral" | "warn">("neutral");
  let excludeBusy = $state(false);
  let searchBar: SearchBar | undefined = $state();
  // During reveal, macOS can grab first-responder focus for the search input from a
  // separate process (WKWebView) with unpredictable IPC latency relative to our own
  // `searchBar?.blur()` call below — it can land either before or after it. Rather than
  // relying on winning that timing race, treat any focus gained while this is true as
  // unwanted and immediately reverse it; explicit shortcuts (Cmd+F, `/`) clear it first.
  let suppressAutoSearchFocus = false;
  let lastLayoutHeight = $state<number | null>(null);
  let collections: Collection[] = $state([]);
  let activating = $state(false);
  let suppressSelectionSyncCount = 0;
  let scrollToSelectedGeneration = 0;
  let scrollIdleSyncTimer: ReturnType<typeof setTimeout> | undefined;
  let keyboardBrowseUntil = 0;
  const SCROLL_IDLE_SYNC_MS = 120; // debounce fallback when scrollend is late (WKWebView)
  const KEYBOARD_BROWSE_GUARD_MS = 350; // block trackpad leading-sync during rapid ←/→
  /**
   * Panel motion coordination (settings instant-hide vs animated hide vs in-flight reveal):
   * - revealSeq: invalidates async showWindow pipeline; bumped on hide reset and new show.
   * - panelMotionMode: `instant` snaps CSS pose; `animate` runs open/close transitions.
   * - panelTransitionEpoch: stale-guard for deferred motion-mode release after instant hide.
   */
  let panelMotionMode = $state<PanelMotionMode>("animate");
  const panelTransitionEpoch = createPanelTransitionEpoch();

  const SETTINGS_SYNC_USER_NOTICE =
    "Couldn't load app settings. Tags and filters may not work properly. Restart Copyosity.";

  async function loadLayout() {
    try {
      const previous = boardVertical;
      const settings = await getAppSettings();
      boardVertical = settings.board_vertical;
      hubEnabled = settings.hub_enabled;
      if (!visible || previous === boardVertical) return;
      const height = overlayHeightForLayout({
        showShortcutHints: overlay.overlayShortcutHintsEnabled,
      });
      await applyOverlayHeight(height, false);
      if (gridEl) {
        gridEl.scrollLeft = 0;
        gridEl.scrollTop = 0;
      }
    } catch (e) {
      console.error("Failed to load layout:", e);
    }
  }

  const overlay = createOverlayEntriesStore({
    getVisible: () => visible,
    getIsRevealing: () => isRevealing,
    onSelectionRequested: () => {
      // No card is preselected on open/reload — the first arrow press selects card 0.
      selectedIndex = -1;
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
      searchQuery: overlay.searchQuery,
      searchPending: overlay.displayListPending,
    }),
  );

  const verticalTagChips = $derived<TagChip[]>([
    ...tagBarModel.formatChips,
    ...tagBarModel.semanticChips,
  ]);

  const overlayLayoutHeight = $derived(
    overlayHeightForLayout({
      showShortcutHints: overlay.overlayShortcutHintsEnabled,
    }),
  );

  const filteredEntries = $derived(overlay.entries);
  const showEntryGrid = $derived(
    shouldShowOverlayEntryGrid(
      filteredEntries.length,
      overlay.displayListPending,
      overlay.searchQuery,
    ),
  );

  /** Quick Look always mirrors the current selection — arrow keys refresh it while open. */
  const quickLookEntry = $derived(
    quickLookOpen && selectedIndex >= 0 && selectedIndex < filteredEntries.length
      ? filteredEntries[selectedIndex]
      : null,
  );

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
    if (quickLookOpen && !quickLookEntry) closeQuickLook();
  });

  $effect(() => {
    const height = overlayLayoutHeight;
    if (!visible) {
      lastLayoutHeight = null;
      return;
    }
    // Vertical panel height is fixed in Rust; the hints footer flexes inside it.
    if (boardVertical) return;
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

  function openQuickLook() {
    if (quickLookOpen) return;
    closeAllCardContextMenus();
    quickLookReturnFocus =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    quickLookOpen = true;
  }

  function closeQuickLook() {
    if (!quickLookOpen) return;
    quickLookOpen = false;
    const focusEl = quickLookReturnFocus;
    quickLookReturnFocus = null;
    if (focusEl?.isConnected) {
      requestAnimationFrame(() => focusEl.focus({ preventScroll: true }));
    } else {
      scrollToSelected();
    }
  }

  function toggleQuickLook() {
    if (quickLookOpen) {
      closeQuickLook();
    } else {
      openQuickLook();
    }
  }

  function handleQuickLookShortcut(e: KeyboardEvent): boolean {
    const toggleCtx = {
      displayListPending: overlay.displayListPending,
      displayFetchFailed: overlay.displayFetchFailed,
      selectedIndex,
      entryCount: filteredEntries.length,
    };
    if (!canToggleQuickLook(toggleCtx)) return false;
    e.preventDefault();
    e.stopPropagation();
    toggleQuickLook();
    return true;
  }

  /** Blur search and anchor selection on the first visible card (↓ or → from search). */
  function tryExitSearchToGrid(e: KeyboardEvent): boolean {
    if (
      !shouldExitSearchToGrid(e.key, { metaKey: e.metaKey, ctrlKey: e.ctrlKey, altKey: e.altKey }, {
        boardVertical,
        searchFocused: searchBar?.isFocused() ?? false,
      })
    ) {
      return false;
    }

    e.preventDefault();
    e.stopPropagation();
    searchBar?.blur();
    if (overlay.displayListPending || overlay.displayFetchFailed) return true;
    if (filteredEntries.length === 0) return true;

    const leading = leadingVisibleCardIndex();
    selectedIndex = leading >= 0 ? leading : 0;
    setInputModality("keyboard");
    scrollToSelected({ behavior: "auto", keyboardScroll: true });
    return true;
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
    const gen = ++hideGeneration;
    nativeHidePending = true;
    clearHideTimer();
    clearHideTransitionHandler();

    let committed = false;
    const commit = () => {
      if (committed || gen !== hideGeneration) return;
      committed = true;
      nativeHidePending = false;
      clearHideTimer();
      clearHideTransitionHandler();
      void hideMainWindow();
    };

    if (panelMotionMode === "instant") {
      void afterLayoutFlush().then(commit);
      return;
    }

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

  async function finalizePendingNativeHide(): Promise<void> {
    if (!nativeHidePending) return;
    hideGeneration += 1;
    nativeHidePending = false;
    clearHideTimer();
    clearHideTransitionHandler();
    await hideMainWindow();
  }

  async function loadCollections() {
    collections = await getCollections();
  }

  /**
   * AGENT NOTE — WKWebView-only workaround, not reproducible in Chromium/Playwright:
   *
   * WKWebView occasionally leaves the grid's compositing layer stale after a hide/show cycle
   * (window moves screens, or a card's on-demand `backdrop-filter` layer — see
   * `.type-preview-btn` in ClipboardCard.svelte — gets added/removed while hidden), painting
   * the reopened panel blank/see-through until something forces a new layout+paint pass. A
   * plain reflow read (`offsetHeight`) isn't always enough; briefly toggling `display` forces
   * WebKit to rebuild the layer tree from scratch. Vertical board's higher on-screen card count
   * makes stale layers far more likely to be visible, hence noticeable there first.
   *
   * This is a WebKit compositor quirk that Chromium (and therefore Playwright) does not exhibit,
   * so this fix cannot be verified by browser automation — only by manually hiding/reshowing the
   * real macOS overlay (`make dev` + tray toggle) and confirming vertical cards still paint.
   * Do not remove this call believing it's dead code just because Playwright runs don't need it.
   */
  function forceWebviewRepaint(el: HTMLElement) {
    const previousDisplay = el.style.display;
    el.style.display = "none";
    void el.offsetHeight;
    el.style.display = previousDisplay;
  }

  function finishReveal() {
    isRevealing = false;
    revealTimer = undefined;
    const reload = pendingReload;
    pendingReload = false;
    // Scroll/focus after open animation so horizontal scroll does not fight panel motion.
    void (async () => {
      overlay.syncDisplayFromCatalog();
      if (reload) await overlay.loadEntries(true, false);
      scrollToSelected();
      if (gridEl) forceWebviewRepaint(gridEl);
    })();
  }

  function schedulePanelMotionRelease(epoch: number) {
    void (async () => {
      await afterLayoutFlush();
      if (!panelTransitionEpoch.isCurrent(epoch)) return;
      panelMotionMode = "animate";
    })();
  }

  function handleNativeWindowHide() {
    hideGeneration += 1;
    nativeHidePending = false;
    clearHideTimer();
    clearHideTransitionHandler();
    const plan = planInstantNativeHide(visible, () => panelTransitionEpoch.bump());
    panelMotionMode = plan.motionMode;
    resetOverlayMotionState();
    if (plan.releaseEpoch === null) {
      panelMotionMode = "animate";
      return;
    }
    schedulePanelMotionRelease(plan.releaseEpoch);
  }

  function resetOverlayMotionState() {
    revealSeq += 1;
    clearRevealTimer();
    isRevealing = false;
    visible = false;
    quickLookOpen = false;
    quickLookReturnFocus = null;
    resetFocusState();
    overlay.resetDisplayStateOnHide();
    overlay.clearSearch({ reload: false });
    overlay.resetOverlayFilters();
    selectedIndex = -1;
    suppressSelectionSyncCount = 0;
    keyboardBrowseUntil = 0;
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
      showShortcutHints: overlay.overlayShortcutHintsEnabled,
    });
    await applyOverlayHeight(height, false);
    lastLayoutHeight = height;
    return true;
  }

  function showWindow() {
    const seq = ++revealSeq;
    panelTransitionEpoch.bump();
    const pendingNativeHide = nativeHidePending;
    resetFocusState();
    window.getSelection()?.removeAllRanges();
    clearRevealTimer();
    overlay.clearSearch({ reload: false });
    overlay.resetOverlayFilters();
    resetOverlayResizeState();

    isRevealing = true;
    const hadPendingReload = pendingReload;
    pendingReload = false;
    if (gridEl) {
      gridEl.scrollLeft = 0;
      gridEl.scrollTop = 0;
    }
    suppressSelectionSyncCount = 0;
    keyboardBrowseUntil = 0;

    panelMotionMode = "instant";
    visible = false;
    suppressAutoSearchFocus = true;
    void (async () => {
      let revealed = false;
      try {
        if (pendingNativeHide) {
          await finalizePendingNativeHide();
          if (seq !== revealSeq) return;
        }
        // Layout settings and catalog data are independent — load concurrently
        // instead of chaining two IPC round-trips before the panel can animate in.
        const [, ready] = await Promise.all([loadLayout(), prepareOverlayLayout(seq)]);
        if (!ready || seq !== revealSeq) return;
        await afterLayoutFlush();
        if (seq !== revealSeq) return;
        panelMotionMode = "animate";
        // Reset scroll and force the WebKit repaint while still invisible (opacity 0)
        // so the reflow lands before the transition starts, not mid-animation.
        if (gridEl) {
          gridEl.scrollLeft = 0;
          gridEl.scrollTop = 0;
          forceWebviewRepaint(gridEl);
        }
        await afterLayoutFlush();
        if (seq !== revealSeq) return;
        visible = true;
        overlay.syncDisplayFromCatalog();
        searchBar?.blur();
        if (hadPendingReload) {
          void overlay.loadEntries(true, false);
        }
        revealTimer = setTimeout(finishReveal, panelOpenMs());
        void loadExcludeCandidate();
        revealed = true;
      } finally {
        suppressAutoSearchFocus = false;
        if (!revealed) {
          panelMotionMode = "animate";
          if (seq === revealSeq) isRevealing = false;
        }
      }
    })();
  }

  /** Reverse a focus grab that lands on the search input during reveal (see
   * `suppressAutoSearchFocus`). Real Cmd+F/`/` shortcuts clear the flag first. */
  function handleSearchBarFocusGained() {
    if (suppressAutoSearchFocus) {
      searchBar?.blur();
    }
  }

  function startVisualHide() {
    revealSeq += 1;
    clearRevealTimer();
    isRevealing = false;
    panelMotionMode = "animate";
    visible = false;
    quickLookOpen = false;
    quickLookReturnFocus = null;
    suppressAutoSearchFocus = false;
    resetFocusState();
    overlay.resetDisplayStateOnHide();
  }

  function animateOut() {
    startVisualHide();
    requestNativeHide();
  }

  function forceHideWindow() {
    animateOut();
  }

  // Drag bottom-right corner to resize the native panel (borderless NSPanel has no
  // obvious OS resize edge). Horizontal board locks vertical size natively (Rust
  // min==max height), so this drag is effectively width-only there. Vertical board
  // allows height drag too, but only width is persisted (see `apply_overlay_size_limits`
  // in lib.rs) — height always resets to the preferred value on next reveal.
  function startOverlayResize(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    void getCurrentWindow().startResizeDragging(boardVertical ? "SouthWest" : "SouthEast");
  }

  async function loadExcludeCandidate() {
    if (!platformIsMacOS()) return;
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
  // panelMotionMode / settings instant-hide: manual QA — overlay → settings → reopen.
  // Manual QA: docs/plans/feature-overlay-content-tag-filters.md §7.
  onMount(() => {
    void initPlatform();
    void overlay.syncOverlaySettings();
    void overlay.warmCatalog();
    void loadLayout();
    loadCollections();

    // Background: check for updates on launch, auto-install, and notify.
    void autoUpdateOnLaunch();

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

    function handleEntryOcr(event: { payload: unknown }) {
      const parsed = parseEntryOcrEvent(event.payload);
      if (!parsed) return;
      if (parsed.kind === "legacy-id") {
        void overlay.reloadDisplayList(false, false);
        return;
      }
      overlay.applyEntryOcr(parsed.payload.entryId, parsed.payload.ocrText);
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
      void loadLayout();
    });
    const unlistenOcr = listen("entry-ocr", handleEntryOcr);
    const unlistenTagged = listen("entry-tagged", handleEntryTagged);

    const unlistenShow = listen("window-show", () => {
      showWindow();
    });

    const unlistenHideRequest = listen("window-hide-request", () => {
      startVisualHide();
      requestNativeHide();
    });

    const unlistenHide = listen("window-hide", handleNativeWindowHide);

    const unlistenOpenSettings = listen("open-settings", () => {
      openSettingsWindow();
    });

    const unlistenSizesReset = listen("overlay-board-sizes-reset", () => {
      void (async () => {
        const height = overlayHeightForLayout({
          showShortcutHints: overlay.overlayShortcutHintsEnabled,
        });
        await applyOverlayHeight(height, false);
        lastLayoutHeight = height;
      })();
    });

    // Voice transcription failures (e.g. a hub 429 asking to raise the tariff).
    const unlistenVoiceError = listen<string>("voice-error", (e) => {
      void notify("Voice transcription failed", e.payload);
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
        const escapeAction = resolveOverlayEscapeAction({
          cardContextMenuOpen: isCardContextMenuOpen(),
          quickLookOpen,
          hasSearchQuery: overlay.searchQuery.length > 0,
        });
        if (escapeAction === "close-context-menu") {
          closeAllCardContextMenus();
          return;
        }
        if (escapeAction === "close-quick-look") {
          closeQuickLook();
          return;
        }
        if (escapeAction === "clear-search") {
          overlay.clearSearch({ immediate: true });
          searchBar?.blur();
          return;
        }
        forceHideWindow();
        return;
      }

      // Cmd+↑ hides the overlay (matches the native global monitor for when the
      // panel is not the key window).
      if (e.metaKey && e.key === "ArrowUp") {
        e.preventDefault();
        e.stopPropagation();
        forceHideWindow();
        return;
      }

      if (shouldBlockOverlayActionWhileQuickLookOpen(quickLookOpen)) {
        if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f") return;
        if (
          e.key === "/" &&
          !searchFocused &&
          !typingInField &&
          !e.metaKey &&
          !e.ctrlKey &&
          !e.altKey
        ) {
          return;
        }
        if (e.key === "Enter") return;
      }

      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        e.stopPropagation();
        suppressAutoSearchFocus = false;
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
        suppressAutoSearchFocus = false;
        searchBar?.focus();
        return;
      }

      if ((e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "y") {
        if (
          shouldHandleQuickLookCmdY(
            e.key,
            { altKey: e.altKey, metaKey: e.metaKey, ctrlKey: e.ctrlKey, shiftKey: e.shiftKey },
            target,
            {
              displayListPending: overlay.displayListPending,
              displayFetchFailed: overlay.displayFetchFailed,
              selectedIndex,
              entryCount: filteredEntries.length,
            },
          )
        ) {
          handleQuickLookShortcut(e);
        }
        return;
      }

      if (e.key === "ArrowRight" || e.key === "ArrowLeft" || e.key === "ArrowDown" || e.key === "ArrowUp") {
        if (tryExitSearchToGrid(e)) return;
        if (typingInField && !searchFocused) return;
        if (overlay.displayListPending || overlay.displayFetchFailed) return;
        e.preventDefault();
        setInputModality("keyboard");
        touchKeyboardBrowseScroll();
        const scrollCtx = keyboardArrowScrollContext();
        const direction =
          e.key === "ArrowRight" || e.key === "ArrowDown"
            ? "right"
            : "left";
        selectedIndex = nextIndexAfterKeyboardArrow({
          direction,
          selectedIndex,
          leadingIndex: scrollCtx.leadingIndex,
          selectedOffScreen: scrollCtx.selectedOffScreen,
          wrapperMissing: scrollCtx.wrapperMissing,
          entryCount: filteredEntries.length,
          boardVertical,
          verticalPosition: scrollCtx.verticalPosition,
        });
        if (direction === "right" && selectedIndex === filteredEntries.length - 1) {
          void overlay.loadNextEntryPage();
        }
        scrollToSelected({
          behavior: "auto",
          keyboardScroll: true,
          verticalNavDirection:
            boardVertical && (e.key === "ArrowDown" || e.key === "ArrowUp")
              ? e.key === "ArrowDown"
                ? "down"
                : "up"
              : undefined,
        });
        return;
      }

      if (
        shouldHandleQuickLookSpace(
          e.key,
          { altKey: e.altKey, metaKey: e.metaKey, ctrlKey: e.ctrlKey },
          target,
          {
            displayListPending: overlay.displayListPending,
            displayFetchFailed: overlay.displayFetchFailed,
            selectedIndex,
            entryCount: filteredEntries.length,
            searchFocused,
            typingInField,
          },
        )
      ) {
        handleQuickLookShortcut(e);
        return;
      }

      if (e.key === "Enter") {
        if (quickLookOpen) return;
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
      clearTimeout(scrollIdleSyncTimer);
      overlay.dispose();
      unlistenClipboard.then((fn) => fn());
      unlistenHistory.then((fn) => fn());
      unlistenOcr.then((fn) => fn());
      unlistenTagged.then((fn) => fn());
      unlistenShow.then((fn) => fn());
      unlistenHideRequest.then((fn) => fn());
      unlistenHide.then((fn) => fn());
      unlistenOpenSettings.then((fn) => fn());
      unlistenSizesReset.then((fn) => fn());
      unlistenVoiceError.then((fn) => fn());
      window.removeEventListener("keydown", handleKeydown, true);
    };
  });

  function handleCardSelect(index: number) {
    if (overlay.displayListPending) return;
    selectedIndex = index;
  }

  function handleCardPreview(index: number) {
    if (overlay.displayListPending) return;
    selectedIndex = index;
    if (!quickLookOpen) {
      openQuickLook();
    }
  }

  function getGridVerticalScrollInsets(container: HTMLElement) {
    const style = getComputedStyle(container);
    return {
      top: parseFloat(style.paddingTop) || 0,
      bottom: parseFloat(style.paddingBottom) || 0,
    };
  }

  function leadingVisibleCardIndex(): number {
    if (!gridEl || filteredEntries.length === 0) return -1;

    const wrappers = gridEl.querySelectorAll(".card-wrapper");
    const viewport = gridEl.getBoundingClientRect();

    if (boardVertical) {
      const cardRects: { top: number; bottom: number }[] = [];
      wrappers.forEach((wrapper) => {
        if (wrapper instanceof HTMLElement) {
          const rect = wrapper.getBoundingClientRect();
          cardRects.push({ top: rect.top, bottom: rect.bottom });
        }
      });
      const { top: padTop, bottom: padBottom } = getGridVerticalScrollInsets(gridEl);
      return indexOfLeadingVisibleCardVertical(viewport, padTop, padBottom, cardRects);
    }

    const cardRects: { left: number; right: number }[] = [];
    wrappers.forEach((wrapper) => {
      if (wrapper instanceof HTMLElement) {
        const rect = wrapper.getBoundingClientRect();
        cardRects.push({ left: rect.left, right: rect.right });
      }
    });

    const { left: padLeft, right: padRight } = getGridScrollInsets(gridEl);
    return indexOfLeadingVisibleCard(viewport, padLeft, padRight, cardRects);
  }

  function handleGridScroll(event: Event) {
    const target = event.currentTarget;
    if (!(target instanceof HTMLElement)) return;

    const now = performance.now();
    suppressSelectionSyncCount = shouldClearStuckSuppressOnUserScroll({
      suppressSelectionSyncCount,
      keyboardBrowseUntil,
      now,
      isTrusted: event.isTrusted,
    });

    if (boardVertical) {
      if (
        shouldLoadNextEntryPage({
          scrollLeft: target.scrollTop,
          clientWidth: target.clientHeight,
          scrollWidth: target.scrollHeight,
          hasMore: overlay.entriesHasMore && !overlay.displayFetchFailed,
          loading: overlay.loadingMoreEntries || overlay.displayListPending,
          // Prefetch ~2 viewports ahead so fast scrolling never outruns the loader.
          prefetchPx: target.clientHeight * 2,
        })
      ) {
        void overlay.loadNextEntryPage();
      }
    } else if (
      shouldLoadNextEntryPage({
        scrollLeft: target.scrollLeft,
        clientWidth: target.clientWidth,
        scrollWidth: target.scrollWidth,
        hasMore: overlay.entriesHasMore && !overlay.displayFetchFailed,
        loading: overlay.loadingMoreEntries || overlay.displayListPending,
        // Prefetch ~2 viewports ahead so fast scrolling never outruns the loader.
        prefetchPx: target.clientWidth * 2,
      })
    ) {
      void overlay.loadNextEntryPage();
    }

    if (
      !boardVertical &&
      shouldScheduleTrackpadLeadingSync({ keyboardBrowseUntil, now })
    ) {
      scheduleTrackpadScrollSync();
    }
  }

  function scheduleTrackpadScrollSync() {
    clearTimeout(scrollIdleSyncTimer);
    scrollIdleSyncTimer = setTimeout(() => {
      scrollIdleSyncTimer = undefined;
      finishIdleScrollSync();
    }, SCROLL_IDLE_SYNC_MS);
  }

  /**
   * Shared by scrollend (primary) and idle debounce when scrollend is late.
   * Mouse/trackpad scrolling never auto-selects a card — it only clears a stale
   * selection so the next arrow press re-anchors on whatever is now on screen.
   */
  function finishIdleScrollSync() {
    const result = handleScrollEndBrowseSync({
      suppressSelectionSyncCount,
      keyboardBrowseUntil,
      now: performance.now(),
    });
    suppressSelectionSyncCount = result.nextSuppressCount;
    if (result.shouldSyncLeading && selectedIndex >= 0) selectedIndex = -1;
  }

  function keyboardArrowScrollContext(): {
    leadingIndex: number;
    selectedOffScreen: boolean;
    wrapperMissing: boolean;
    verticalPosition?: ReturnType<typeof verticalCardViewportPosition>;
  } {
    const leadingIndex = leadingVisibleCardIndex();
    if (!gridEl || filteredEntries.length === 0) {
      return { leadingIndex, selectedOffScreen: false, wrapperMissing: false };
    }

    const wrappers = gridEl.querySelectorAll(".card-wrapper");
    const wrapper = wrappers[selectedIndex];
    const wrapperMissing = selectedIndex >= 0 && !(wrapper instanceof HTMLElement);

    let selectedOffScreen = false;
    let verticalPosition: ReturnType<typeof verticalCardViewportPosition> | undefined;
    if (wrapper instanceof HTMLElement) {
      const viewport = gridEl.getBoundingClientRect();
      const rect = wrapper.getBoundingClientRect();
      if (boardVertical) {
        const { top: padTop, bottom: padBottom } = getGridVerticalScrollInsets(gridEl);
        selectedOffScreen = isCardOffScreenVertical(viewport, padTop, padBottom, rect);
        verticalPosition = verticalCardViewportPosition(viewport, padTop, padBottom, rect);
      } else {
        const { left: padLeft, right: padRight } = getGridScrollInsets(gridEl);
        selectedOffScreen = isCardOffScreen(viewport, padLeft, padRight, rect);
      }
    }

    return { leadingIndex, selectedOffScreen, wrapperMissing, verticalPosition };
  }

  function touchKeyboardBrowseScroll() {
    keyboardBrowseUntil = performance.now() + KEYBOARD_BROWSE_GUARD_MS;
    clearTimeout(scrollIdleSyncTimer);
    scrollIdleSyncTimer = undefined;
  }

  function handleGridScrollEnd() {
    if (boardVertical) return;
    clearTimeout(scrollIdleSyncTimer);
    scrollIdleSyncTimer = undefined;
    finishIdleScrollSync();
  }

  function getGridScrollInsets(container: HTMLElement) {
    const style = getComputedStyle(container);
    return {
      left: parseFloat(style.paddingLeft) || 0,
      right: parseFloat(style.paddingRight) || 0,
    };
  }

  /**
   * `.card`'s DOM parent is `ClipboardCard`'s `display: contents` host (kept so the card
   * context menu can render as an unclipped sibling) — it has no box, so
   * `getBoundingClientRect()` on it returns an empty rect at (0,0). Walk up to the real
   * `.card-wrapper` (the scroll/snap unit) instead of trusting `parentElement`.
   */
  function scrollMeasureEl(card: HTMLElement): HTMLElement {
    const wrapper = card.closest(".card-wrapper");
    return wrapper instanceof HTMLElement ? wrapper : card;
  }

  function snapCardIntoPaddedViewport(
    card: HTMLElement,
    container: HTMLElement,
    behavior: ScrollBehavior,
  ): boolean {
    const measureEl = scrollMeasureEl(card);
    const { left: padLeft, right: padRight } = getGridScrollInsets(container);
    const containerRect = container.getBoundingClientRect();
    const cardRect = measureEl.getBoundingClientRect();
    const slack = 2;
    const visibleLeft = containerRect.left + padLeft;
    const visibleRight = containerRect.right - padRight;

    if (cardRect.left >= visibleLeft - slack && cardRect.right <= visibleRight + slack) {
      return false;
    }

    let delta = 0;
    if (cardRect.right > visibleRight + slack) {
      delta = cardRect.right - visibleRight;
    } else if (cardRect.left < visibleLeft - slack) {
      delta = cardRect.left - visibleLeft;
    }
    if (delta === 0) return false;

    container.scrollTo({ left: container.scrollLeft + delta, behavior });
    return true;
  }

  function snapCardIntoPaddedViewportVertical(
    card: HTMLElement,
    container: HTMLElement,
    behavior: ScrollBehavior,
    navDirection?: "up" | "down",
  ): boolean {
    const measureEl = scrollMeasureEl(card);
    const { top: padTop, bottom: padBottom } = getGridVerticalScrollInsets(container);
    const containerRect = container.getBoundingClientRect();
    const cardRect = measureEl.getBoundingClientRect();
    const delta = navDirection
      ? verticalScrollDeltaForKeyboardNav(containerRect, padTop, padBottom, cardRect, navDirection)
      : verticalScrollDeltaToSnapCard(containerRect, padTop, padBottom, cardRect);
    if (delta === 0) return false;

    container.scrollTo({ top: container.scrollTop + delta, behavior });
    return true;
  }

  function blurDeselectedCards(cards: NodeListOf<Element>, keepIndex: number) {
    cards.forEach((c, i) => {
      if (i === keepIndex || !(c instanceof HTMLElement)) return;
      if (c === document.activeElement || c.contains(document.activeElement)) {
        const active = document.activeElement;
        if (active instanceof HTMLElement) active.blur();
        c.blur();
      }
    });
  }

  type ScrollToSelectedOptions = {
    behavior?: ScrollBehavior;
    keyboardScroll?: boolean;
    /** Vertical keyboard ↑/↓ alignment (list browse). */
    verticalNavDirection?: "up" | "down";
  };

  function scrollToSelected(options?: ScrollBehavior | ScrollToSelectedOptions) {
    const resolved: ScrollToSelectedOptions =
      typeof options === "string" ? { behavior: options } : (options ?? {});
    const behaviorOverride = resolved.behavior;
    const keyboardScroll = resolved.keyboardScroll ?? false;
    const verticalNavDirection = resolved.verticalNavDirection;
    const generation = ++scrollToSelectedGeneration;
    const targetIndex = selectedIndex;

    void (async () => {
      if (!gridEl || targetIndex < 0) return;
      await tick();
      if (!shouldRunScrollToSelectedGeneration(generation, scrollToSelectedGeneration)) return;

      const cards = gridEl.querySelectorAll(".card");
      const card = cards[targetIndex];
      if (!(card instanceof HTMLElement)) return;

      blurDeselectedCards(cards, targetIndex);

      const behavior = behaviorOverride ?? scrollBehavior();
      const didScroll = boardVertical
        ? snapCardIntoPaddedViewportVertical(card, gridEl, behavior, verticalNavDirection)
        : snapCardIntoPaddedViewport(card, gridEl, behavior);
      // Blocks one leading-sync clear on scrollend for this programmatic scroll.
      if (didScroll) suppressSelectionSyncCount += 1;

      if (boardVertical && keyboardScroll && gridEl) {
        if (
          shouldLoadNextEntryPage({
            scrollLeft: gridEl.scrollTop,
            clientWidth: gridEl.clientHeight,
            scrollWidth: gridEl.scrollHeight,
            hasMore: overlay.entriesHasMore && !overlay.displayFetchFailed,
            loading: overlay.loadingMoreEntries || overlay.displayListPending,
          })
        ) {
          void overlay.loadNextEntryPage();
        }
      }

      const keepSearchFocus = searchBar?.isFocused() ?? false;
      if (!keepSearchFocus) {
        if (keyboardScroll) setInputModality("keyboard");
        card.focus({ preventScroll: true });
      }
    })();
  }
</script>

<div
  class="app"
  class:visible
  class:vertical={boardVertical}
  data-panel-motion={panelMotionMode}
  bind:this={appEl}
>
  <header class="header overlay-header">
    <SearchBar
      bind:this={searchBar}
      value={overlay.searchQuery}
      onchange={overlay.debouncedSearch}
      onfocus={handleSearchBarFocusGained}
    />
    {#if boardVertical}
      <div class="header-actions">
        {#if platformIsMacOS() && hubEnabled}
          <button
            class="overlay-icon-btn overlay-icon-btn--agent app-btn"
            type="button"
            aria-label="Open command palette (Cmd+Shift+Space)"
            title="Open command palette · ⌘⇧Space"
            onclick={() => void openCommandPalette()}
          >
            <svg class="overlay-icon-btn-icon" viewBox="0 0 24 24" aria-hidden="true">
              <path
                d="m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275L12 3Z"
              />
              <path d="M5 3v4M3 5h4" />
              <path d="M19 17v4M17 19h4" />
            </svg>
          </button>
        {/if}
        <button
          class="overlay-icon-btn overlay-icon-btn--settings app-btn"
          type="button"
          aria-label="Open settings"
          title="Settings"
          onclick={() => openSettingsWindow()}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path
              d="M19.14 12.94c.04-.31.06-.62.06-.94s-.02-.63-.06-.94l2.03-1.58a.5.5 0 0 0 .12-.64l-1.92-3.32a.5.5 0 0 0-.6-.22l-2.39.96a7.03 7.03 0 0 0-1.63-.94l-.36-2.54a.5.5 0 0 0-.5-.42h-3.84a.5.5 0 0 0-.5.42l-.36 2.54c-.58.22-1.13.53-1.63.94l-2.39-.96a.5.5 0 0 0-.6.22L2.71 8.84a.5.5 0 0 0 .12.64l2.03 1.58c-.04.31-.06.62-.06.94s.02.63.06.94l-2.03 1.58a.5.5 0 0 0-.12.64l1.92 3.32a.5.5 0 0 0 .6.22l2.39-.96c.5.41 1.05.72 1.63.94l.36 2.54a.5.5 0 0 0 .5.42h3.84a.5.5 0 0 0 .5-.42l.36-2.54c.58-.22 1.13-.53 1.63-.94l2.39.96a.5.5 0 0 0 .6-.22l1.92-3.32a.5.5 0 0 0-.12-.64zM12 15.5A3.5 3.5 0 1 1 12 8.5a3.5 3.5 0 0 1 0 7z"
            />
          </svg>
        </button>
      </div>
    {:else}
      <CollectionTabs
        {collections}
        activeId={overlay.activeCollectionId}
        activePinned={overlay.pinnedOnly}
        onselect={overlay.handleCollectionSelect}
        onupdate={loadCollections}
      />
      <div class="header-actions">
        {#if platformIsMacOS() && excludeCandidate && !excludeCandidate.alreadyExcluded}
          {@const excludeLabel = excludeFromClipboardHistoryAriaLabel(
            excludeCandidate.displayName,
          )}
          <button
            class="exclude-app-btn app-btn"
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
        {#if platformIsMacOS() && excludeNotice}
          <span
            class="status-hint exclude-notice"
            class:neutral={excludeNoticeTone === "neutral"}
            class:warn={excludeNoticeTone === "warn"}
            aria-live="polite"
          >
            {excludeNotice}
          </span>
        {/if}
        {#if platformIsMacOS() && hubEnabled}
          <button
            class="overlay-icon-btn overlay-icon-btn--agent app-btn"
            type="button"
            aria-label="Open command palette (Cmd+Shift+Space)"
            title="Open command palette · ⌘⇧Space"
            onclick={() => void openCommandPalette()}
          >
            <svg class="overlay-icon-btn-icon" viewBox="0 0 24 24" aria-hidden="true">
              <path
                d="m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275L12 3Z"
              />
              <path d="M5 3v4M3 5h4" />
              <path d="M19 17v4M17 19h4" />
            </svg>
          </button>
        {/if}
        <button
          class="overlay-icon-btn overlay-icon-btn--settings app-btn"
          type="button"
          aria-label="Open settings"
          title="Settings"
          onclick={() => openSettingsWindow()}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path
              d="M19.14 12.94c.04-.31.06-.62.06-.94s-.02-.63-.06-.94l2.03-1.58a.5.5 0 0 0 .12-.64l-1.92-3.32a.5.5 0 0 0-.6-.22l-2.39.96a7.03 7.03 0 0 0-1.63-.94l-.36-2.54a.5.5 0 0 0-.5-.42h-3.84a.5.5 0 0 0-.5.42l-.36 2.54c-.58.22-1.13.53-1.63.94l-2.39-.96a.5.5 0 0 0-.6.22L2.71 8.84a.5.5 0 0 0 .12.64l2.03 1.58c-.04.31-.06.62-.06.94s.02.63.06.94l-2.03 1.58a.5.5 0 0 0-.12.64l1.92 3.32a.5.5 0 0 0 .6.22l2.39-.96c.5.41 1.05.72 1.63.94l.36 2.54a.5.5 0 0 0 .5.42h3.84a.5.5 0 0 0 .5-.42l.36-2.54c.58-.22 1.13-.53 1.63-.94l2.39.96a.5.5 0 0 0 .6-.22l1.92-3.32a.5.5 0 0 0-.12-.64zM12 15.5A3.5 3.5 0 1 1 12 8.5a3.5 3.5 0 0 1 0 7z"
            />
          </svg>
        </button>
        <button
          class="close-btn overlay-icon-btn overlay-icon-btn--close app-btn"
          type="button"
          aria-label="Close overlay"
          title="Close"
          onclick={() => forceHideWindow()}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M5 5 19 19M19 5 5 19" />
          </svg>
        </button>
      </div>
    {/if}
  </header>

  {#if boardVertical && tagBarModel.showRowB}
    <div class="vertical-tag-groups">
      <button
        class="filter-chip filter-chip--compact filter-chip-reset app-btn"
        class:active={!overlay.activeTag}
        type="button"
        onclick={() => overlay.handleTagReset()}
      >
        All tags
      </button>
      {#each verticalTagChips as [tag, count] (tag)}
        <button
          class="filter-chip filter-chip--compact filter-chip--muted app-btn"
          class:active={overlay.activeTag === tag}
          type="button"
          onclick={() => overlay.handleTagSelect(tag)}
        >
          <span>{tag}</span>
          <span class="tag-count">{count}</span>
        </button>
      {/each}
    </div>
  {:else if settingsSyncNotice || tagBarModel.showRowB}
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
      <!-- TEMP: Content Kind segment (All / Text / Images) hidden pending user feedback
           on whether to keep, evolve, or remove. Re-enable filter-row-a block when decided.
        <div class="filter-row-a">
          <ContentKindSegment
            value={overlay.contentKind}
            onchange={overlay.handleContentKindChange}
          />
        </div>
      -->
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

  <div
    class="grid-container"
    class:vertical={boardVertical}
    bind:this={gridEl}
    onscroll={handleGridScroll}
    onscrollend={handleGridScrollEnd}
  >
    {#if !showEntryGrid}
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
            compactVertical={boardVertical}
            retagAvailable={overlay.retagAvailable}
            aiTaggingEnabled={overlay.aiTaggingEnabled}
            selected={i === selectedIndex}
            onselect={() => handleCardSelect(i)}
            onpreview={() => handleCardPreview(i)}
            ondeleted={() => overlay.removeEntry(entry.id)}
            onpinned={() => overlay.handlePinned(entry.id, !entry.is_pinned)}
            onretagged={(tags) => overlay.applyEntryTags(entry.id, tags)}
          />
        </div>
      {/each}
    {/if}
  </div>
  {#if showEntryGrid && overlay.loadMoreFailed}
    <div class="load-more-banner overlay-footer-strip" role="status" aria-live="polite">
      <p class="hint">Couldn't load more entries</p>
      <button class="empty-retry-btn" type="button" onclick={() => overlay.retryLoadMore()}>
        Try again
      </button>
    </div>
  {/if}
  {#if overlay.overlayShortcutHintsEnabled}
    <footer class="overlay-shortcuts overlay-footer-strip" class:vertical={boardVertical}>
      <KeyboardHints hints={activeOverlayShortcutHints} />
    </footer>
  {/if}
  {#if quickLookEntry}
    <QuickLookPanel
      entry={quickLookEntry}
      aiTaggingEnabled={overlay.aiTaggingEnabled}
      compact={boardVertical}
      showHints={overlay.overlayShortcutHintsEnabled}
      onclose={closeQuickLook}
    />
  {/if}
  <button
    class="resize-grip"
    class:vertical={boardVertical}
    type="button"
    tabindex="-1"
    aria-label="Resize"
    title="Drag to resize"
    onmousedown={startOverlayResize}
  ></button>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent;
    font-family: var(--font-family-system);
    color: var(--color-text-body);
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
  }

  .app {
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    max-width: 100%;
    max-height: 100%;
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
    transition: none;
  }

  .resize-grip {
    position: fixed;
    right: var(--space-segment-inset);
    bottom: var(--space-segment-inset);
    width: var(--size-resize-grip);
    height: var(--size-resize-grip);
    padding: 0;
    border: none;
    cursor: nwse-resize;
    background: var(--gradient-resize-grip);
    z-index: 10;
  }

  .resize-grip.vertical {
    right: auto;
    left: var(--space-segment-inset);
    cursor: nesw-resize;
    transform: scaleX(-1);
  }

  .app[data-panel-motion="animate"] {
    /* Open transition runs when `.visible` is added. */
    transition:
      transform var(--duration-panel-open) var(--ease-apple-panel),
      opacity var(--duration-panel-opacity-open) var(--ease-apple-panel);
  }

  .app.visible {
    transform: translate3d(0, 0, 0);
    opacity: 1;
    will-change: auto;
  }

  .app[data-panel-motion="animate"].visible {
    /* Close transition runs when `.visible` is removed. */
    transition:
      transform var(--duration-panel-close) var(--ease-panel-dismiss),
      opacity var(--duration-panel-opacity-close) var(--ease-panel-dismiss);
  }

  @media (prefers-reduced-motion: reduce) {
    .app,
    .app.visible,
    .app[data-panel-motion="animate"],
    .app[data-panel-motion="animate"].visible {
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
    overflow: hidden;
  }

  .app.vertical .header {
    flex: 0 0 auto;
    gap: var(--space-stack);
    height: auto;
    max-height: none;
    min-height: calc(var(--overlay-header-control-height) + var(--overlay-header-pad-block));
    overflow: visible;
  }

  .app.vertical .header :global(.search-bar) {
    flex: 1;
    min-width: 0;
  }

  .vertical-tag-groups {
    display: flex;
    gap: var(--space-stack);
    padding:
      var(--overlay-filter-pad-top)
      calc(var(--overlay-grid-pad-x) + var(--overlay-scrollbar-gutter))
      var(--space-stack)
      var(--overlay-grid-pad-x);
    overflow-x: auto;
    flex-shrink: 0;
    scrollbar-width: none;
    scroll-padding-inline: var(--overlay-grid-pad-x);
  }

  .vertical-tag-groups::-webkit-scrollbar {
    display: none;
  }

  .filter-zone {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: var(--overlay-filter-pad-top);
    transition:
      opacity var(--duration-fast) var(--ease-interactive),
      transform var(--duration-fast) var(--ease-interactive);
  }

  @media (prefers-reduced-motion: reduce) {
    .filter-zone {
      transition: none;
    }
  }

  /* TEMP: re-enable with Content Kind segment block above. */
  /*
  .filter-row-a {
    padding: 0 16px;
  }
  */

  .settings-sync-notice {
    margin: 0 var(--overlay-grid-pad-x);
  }

  .header-actions {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-stack);
    margin-left: auto;
    flex-shrink: 1;
    min-width: 0;
    overflow: hidden;
  }

  .exclude-app-btn {
    box-sizing: border-box;
    height: var(--overlay-header-control-height);
    flex-shrink: 0;
    padding: 0 10px;
    border-radius: var(--radius-control);
    border: 1px solid var(--border-soft);
    background: var(--surface-3);
    font: inherit;
    font-size: var(--font-size-sm);
    font-weight: 500;
    color: var(--color-text-secondary);
    opacity: var(--opacity-muted-control);
    cursor: pointer;
    white-space: nowrap;
  }

  .exclude-app-btn:hover:not(:disabled, [aria-busy="true"]) {
    opacity: var(--opacity-muted-control-hover);
    background: var(--surface-warning-subtle);
    border-color: var(--border-warning);
    color: var(--color-text-body);
  }

  .exclude-app-btn:focus {
    outline: none;
  }

  .exclude-app-btn-text {
    display: block;
  }

  .exclude-notice {
    margin: 0;
    white-space: nowrap;
  }

  .grid-container {
    flex: 1;
    display: flex;
    gap: 12px;
    padding: var(--overlay-grid-pad-y) var(--overlay-grid-pad-x);
    scroll-padding-inline: var(--overlay-grid-pad-x);
    overflow: auto hidden;
    scroll-snap-type: x mandatory;
    align-items: flex-start;
    min-height: 0;
    min-width: 0;
  }

  .grid-container.vertical {
    flex-direction: column;
    overflow: hidden auto;
    align-items: stretch;
    padding-inline: var(--overlay-grid-pad-x);
    scroll-padding-inline: 0;
    scroll-padding-block: var(--overlay-grid-pad-y);
    /* Base rule snaps the x axis (horizontal board) — vertical board snaps y instead so the
       top card always sits flush against the top edge instead of scrolling to an arbitrary
       partial position. */
    scroll-snap-type: y mandatory;
    min-height: 0;
    scrollbar-gutter: stable;
  }

  .grid-container.vertical .card-wrapper {
    width: 100%;
  }

  /* Compact vertical rows (upstream max-height); preview area shrinks so footer fits. */
  .grid-container.vertical :global(.card) {
    width: 100%;
    min-width: 0;
    height: auto;
    min-height: 60px;
    max-height: var(--card-max-height-vertical);
  }

  .grid-container.vertical :global(.card-header) {
    margin-bottom: 6px;
  }

  .grid-container.vertical :global(.card-body) {
    flex: 0 0 var(--card-body-height-vertical);
    height: var(--card-body-height-vertical);
    min-height: 0;
  }

  .grid-container.vertical :global(.text-preview) {
    padding: 8px 10px;
  }

  .grid-container.vertical :global(.text-content) {
    font-size: var(--card-preview-font-size-vertical);
    line-height: var(--card-preview-line-height-vertical);
    max-height: calc(
      var(--card-preview-line-height-vertical) * var(--card-preview-line-count-vertical)
    );
    -webkit-line-clamp: var(--card-preview-line-count-vertical);
    line-clamp: var(--card-preview-line-count-vertical);
  }

  .grid-container.vertical :global(.image-preview) {
    gap: 0;
  }

  .grid-container.vertical :global(.image-preview img),
  .grid-container.vertical :global(.image-placeholder) {
    height: var(--card-image-height-vertical);
  }

  .grid-container.vertical :global(.card-footer) {
    padding-top: 6px;
    gap: 6px;
  }

  .card-wrapper {
    flex-shrink: 0;
    scroll-snap-align: start;
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

  .empty-retry-btn:focus {
    outline: none;
  }

  .load-more-banner {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 8px var(--overlay-grid-pad-x);
    border-top: 1px solid var(--border-default);
    background: var(--surface-1);
  }

  .load-more-banner .hint {
    margin: 0;
  }

</style>
