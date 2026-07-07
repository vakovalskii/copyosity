import {
  ENTRY_PAGE_SIZE,
  getAppSettings,
  getEntries,
  getOverlayTagCounts,
  isTaggingReady,
} from "$lib/api";
import { cardTagDbVariants } from "$lib/card-tag-label";
import { invokeErrorMessage } from "$lib/exclusion-label";
import {
  clearContentKindSession,
  readContentKindSession,
  writeContentKindSession,
} from "$lib/overlay-content-kind-session";
import { displayQueryKey, tagCountsQueryKey } from "$lib/overlay-display-query";
import {
  isReconcileDepthExhausted,
  shouldBackfillEntriesAfterShrink,
  shouldRefetchTagCounts,
  shouldRefreshUnfilteredDisplayFromCatalog,
  shouldSyncDisplayFromCatalog,
} from "$lib/overlay-entries-logic";
import {
  type ContentKind,
  activeTagCompatibleWithAi,
  activeTagCompatibleWithKind,
  entryMatchesKind,
  entryMatchesTag,
  isFormatTag,
  isSemanticTagUiEnabled,
  hasImageEntries,
  hasTextEntries,
  reconcileOverlayFilterState,
} from "$lib/overlay-filters";
import type { ClipboardEntry, OverlayTagCounts } from "$lib/types";

/*
 * Store integration tests: see overlay-entries-logic.test.ts (pure helpers).
 * Full store (runes + invoke) still needs Vitest/Playwright per TEST-NOTE in +page.svelte.
 */

/** TEMP: Content Kind segment hidden in +page.svelte — keep false until product decision. */
const CONTENT_KIND_ROW_UI_ENABLED = false;

export interface OverlayEntriesDeps {
  getVisible: () => boolean;
  getIsRevealing: () => boolean;
  onSelectionRequested: (selectFirst: boolean, scrollToFirst: boolean) => void;
  onClampSelection: () => void;
}

type DisplayFetchGenKind = "data" | "display";

export function createOverlayEntriesStore(deps: OverlayEntriesDeps) {
  let entries = $state<ClipboardEntry[]>([]);
  let catalogEntries = $state<ClipboardEntry[]>([]);
  let catalogTagCounts = $state<OverlayTagCounts | null>(null);
  let searchTagCounts = $state<OverlayTagCounts | null>(null);
  let searchQuery = $state("");
  let activeCollectionId = $state<number | null>(null);
  let pinnedOnly = $state(false);
  let activeTag = $state<string | null>(null);
  let contentKind = $state<ContentKind>("all");
  let aiTaggingEnabled = $state(false);
  let overlayShortcutHintsEnabled = $state(true);
  let settingsLoadError = $state<string | null>(null);
  let retagAvailable = $state(false);
  let catalogHasMore = $state(true);
  let entriesHasMore = $state(true);
  let searchPageReady = $state(true);
  let displayFetchFailed = $state(false);
  let displayPageLoading = $state(false);
  let loadingMoreEntries = $state(false);
  let loadMoreFailed = $state(false);

  let dataFetchGen = 0;
  let displayFetchGen = 0;
  let tagCountsFetchGen = 0;
  let loadMoreGen = 0;
  let entriesPastFirstPage = false;
  let filteredPastFirstPage = false;
  let catalogQueryKey = "";
  let catalogTagCountsQueryKey = "";
  let searchTagCountsQueryKey = "";
  let mountCatalogPromise: Promise<void> | null = null;
  let tagCountsRefreshTimer: ReturnType<typeof setTimeout>;
  let debounceTimer: ReturnType<typeof setTimeout>;
  let suppressReconcileEffect = false;

  const showContentKindRow = $derived(
    CONTENT_KIND_ROW_UI_ENABLED &&
      aiTaggingEnabled &&
      (catalogTagCounts?.has_text ?? hasTextEntries(catalogEntries)) &&
      (catalogTagCounts?.has_images ?? hasImageEntries(catalogEntries)),
  );

  const searchPending = $derived(Boolean(searchQuery) && !searchPageReady);
  const displayListPending = $derived(searchPending || displayPageLoading);

  function currentDisplayQueryKey(): string {
    return displayQueryKey({
      searchQuery,
      activeTag,
      contentKind,
      showContentKindRow,
      collectionId: activeCollectionId,
      pinnedOnly,
    });
  }

  function entryQuery() {
    return {
      collection_id: activeCollectionId,
      pinned_only: pinnedOnly,
    };
  }

  function entryQueryKey(): string {
    return `${activeCollectionId ?? "null"}:${pinnedOnly}`;
  }

  function currentTagCountsQueryKey(): string {
    return tagCountsQueryKey({
      collectionId: activeCollectionId,
      pinnedOnly,
      searchQuery,
    });
  }

  function catalogMatchesCurrentQuery(): boolean {
    return catalogTagCounts !== null && catalogQueryKey === entryQueryKey();
  }

  function usesFilteredDisplayList(): boolean {
    return (
      Boolean(searchQuery) || activeTag !== null || (showContentKindRow && contentKind !== "all")
    );
  }

  /** Copy warm catalog rows into an empty unfiltered grid (tags can still show counts). */
  function syncDisplayFromCatalog(): boolean {
    if (
      !shouldSyncDisplayFromCatalog(
        entries.length,
        catalogEntries.length,
        usesFilteredDisplayList(),
      )
    ) {
      return false;
    }
    entries = catalogEntries;
    entriesHasMore = catalogHasMore;
    searchPageReady = true;
    displayFetchFailed = false;
    searchTagCounts = null;
    searchTagCountsQueryKey = "";
    return true;
  }

  /** Point the unfiltered grid at the warm catalog (e.g. after clearing a tag filter). */
  function refreshUnfilteredDisplayFromCatalog(): boolean {
    if (
      !shouldRefreshUnfilteredDisplayFromCatalog(catalogEntries.length, usesFilteredDisplayList())
    ) {
      return false;
    }
    entries = catalogEntries;
    entriesHasMore = catalogHasMore;
    searchPageReady = true;
    displayFetchFailed = false;
    searchTagCounts = null;
    searchTagCountsQueryKey = "";
    return true;
  }

  function displayListQuery() {
    const tag = activeTag;
    let tagVariants: string[] | null = null;
    if (tag && !isFormatTag(tag)) {
      const variants = cardTagDbVariants(tag);
      if (variants.length > 1) tagVariants = variants;
    }
    return {
      ...entryQuery(),
      search: searchQuery || null,
      tag,
      tag_variants: tagVariants,
      content_kind: showContentKindRow && contentKind !== "all" ? contentKind : null,
    };
  }

  function isDisplayFetchStale(expectedGen: number, genKind: DisplayFetchGenKind): boolean {
    return genKind === "data" ? expectedGen !== dataFetchGen : expectedGen !== displayFetchGen;
  }

  function invalidateInFlightFetches(): number {
    loadingMoreEntries = false;
    loadMoreFailed = false;
    clearTimeout(tagCountsRefreshTimer);
    tagCountsFetchGen += 1;
    dataFetchGen += 1;
    displayFetchGen += 1;
    loadMoreGen += 1;
    return dataFetchGen;
  }

  function applyEntrySelection(selectFirst: boolean, scrollToFirst: boolean) {
    if (selectFirst) {
      deps.onSelectionRequested(selectFirst, scrollToFirst);
    }
  }

  function beginFreshFetch(): number {
    loadingMoreEntries = false;
    loadMoreFailed = false;
    displayFetchGen += 1;
    tagCountsFetchGen += 1;
    clearTimeout(tagCountsRefreshTimer);
    entriesPastFirstPage = false;
    filteredPastFirstPage = false;
    return ++dataFetchGen;
  }

  function resetDisplayPaginationForReveal() {
    loadingMoreEntries = false;
    loadMoreFailed = false;
    loadMoreGen += 1;
    displayFetchGen += 1;
    entriesPastFirstPage = false;
    filteredPastFirstPage = false;
  }

  async function applyDisplayPage0(
    expectedGen: number,
    genKind: DisplayFetchGenKind,
  ): Promise<boolean> {
    const queryKeyAtStart = currentDisplayQueryKey();

    if (!usesFilteredDisplayList()) {
      if (isDisplayFetchStale(expectedGen, genKind)) return false;
      if (currentDisplayQueryKey() !== queryKeyAtStart) return false;

      if (syncDisplayFromCatalog()) {
        return true;
      }

      if (refreshUnfilteredDisplayFromCatalog()) {
        return true;
      }

      try {
        const data = await getEntries({
          ...entryQuery(),
          search: null,
          tag: null,
          content_kind: null,
          limit: ENTRY_PAGE_SIZE,
          offset: 0,
        });
        if (isDisplayFetchStale(expectedGen, genKind)) return false;
        if (currentDisplayQueryKey() !== queryKeyAtStart) return false;
        entries = data;
        entriesHasMore = data.length === ENTRY_PAGE_SIZE;
        catalogEntries = data;
        catalogHasMore = entriesHasMore;
        catalogQueryKey = entryQueryKey();
        searchPageReady = true;
        displayFetchFailed = false;
        searchTagCounts = null;
        searchTagCountsQueryKey = "";
        return true;
      } catch {
        if (
          !isDisplayFetchStale(expectedGen, genKind) &&
          currentDisplayQueryKey() === queryKeyAtStart
        ) {
          searchPageReady = true;
          displayFetchFailed = true;
          entries = [];
          entriesHasMore = false;
        }
        return false;
      }
    }

    try {
      const tagCountsKeyAtStart = currentTagCountsQueryKey();
      const cachedCounts = searchQuery ? searchTagCounts : catalogTagCounts;
      const cachedKey = searchQuery ? searchTagCountsQueryKey : catalogTagCountsQueryKey;
      const shouldFetchTagCounts = shouldRefetchTagCounts(
        cachedKey,
        tagCountsKeyAtStart,
        cachedCounts,
      );

      const entriesRequest = getEntries({
        ...displayListQuery(),
        limit: ENTRY_PAGE_SIZE,
        offset: 0,
      });
      const tagCountsRequest = shouldFetchTagCounts
        ? getOverlayTagCounts(
            searchQuery
              ? { ...entryQuery(), search: searchQuery }
              : { ...entryQuery(), search: null },
          )
        : null;

      const [data, tagCounts] = await Promise.all([
        entriesRequest,
        tagCountsRequest ?? Promise.resolve(cachedCounts!),
      ]);
      if (isDisplayFetchStale(expectedGen, genKind)) return false;
      if (currentDisplayQueryKey() !== queryKeyAtStart) return false;
      entries = data;
      entriesHasMore = data.length === ENTRY_PAGE_SIZE;
      if (searchQuery) {
        searchTagCounts = tagCounts;
        searchTagCountsQueryKey = tagCountsKeyAtStart;
      } else {
        catalogTagCounts = tagCounts;
        catalogTagCountsQueryKey = tagCountsKeyAtStart;
      }
      searchPageReady = true;
      displayFetchFailed = false;
      return true;
    } catch {
      if (
        !isDisplayFetchStale(expectedGen, genKind) &&
        currentDisplayQueryKey() === queryKeyAtStart
      ) {
        searchPageReady = true;
        displayFetchFailed = true;
        entriesHasMore = false;
        if (usesFilteredDisplayList()) {
          entries = [];
          if (searchQuery) {
            searchTagCounts = null;
            searchTagCountsQueryKey = "";
          }
        }
      }
      return false;
    }
  }

  async function loadCatalog(gen: number) {
    try {
      const [data, tagCounts] = await Promise.all([
        getEntries({
          ...entryQuery(),
          search: null,
          tag: null,
          content_kind: null,
          limit: ENTRY_PAGE_SIZE,
          offset: 0,
        }),
        getOverlayTagCounts({ ...entryQuery(), search: null }),
      ]);
      if (gen !== dataFetchGen) return false;
      catalogEntries = data;
      catalogTagCounts = tagCounts;
      catalogHasMore = data.length === ENTRY_PAGE_SIZE;
      catalogQueryKey = entryQueryKey();
      catalogTagCountsQueryKey = tagCountsQueryKey({
        collectionId: activeCollectionId,
        pinnedOnly,
        searchQuery: "",
      });
      if (!usesFilteredDisplayList()) {
        entries = data;
        entriesHasMore = catalogHasMore;
      }
      return true;
    } catch {
      if (gen === dataFetchGen) {
        catalogEntries = [];
        catalogTagCounts = null;
        catalogHasMore = false;
        catalogTagCountsQueryKey = "";
        entries = [];
        entriesHasMore = false;
        displayFetchFailed = true;
      }
      return false;
    }
  }

  async function loadDisplayEntries(selectFirst = false, scrollToFirst = true, gen = dataFetchGen) {
    if (gen !== dataFetchGen) return;
    displayPageLoading = true;
    displayFetchFailed = false;
    try {
      const ok = await applyDisplayPage0(gen, "data");
      if (gen !== dataFetchGen) return;
      if (ok) applyEntrySelection(selectFirst, scrollToFirst);
    } finally {
      displayPageLoading = false;
    }
  }

  async function refreshTagCounts(gen: number) {
    try {
      const catalogCounts = await getOverlayTagCounts({ ...entryQuery(), search: null });
      if (gen !== tagCountsFetchGen) return;
      catalogTagCounts = catalogCounts;
      catalogTagCountsQueryKey = tagCountsQueryKey({
        collectionId: activeCollectionId,
        pinnedOnly,
        searchQuery: "",
      });
      if (searchQuery) {
        const searchCounts = await getOverlayTagCounts({
          ...entryQuery(),
          search: searchQuery,
        });
        if (gen !== tagCountsFetchGen) return;
        searchTagCounts = searchCounts;
        searchTagCountsQueryKey = tagCountsQueryKey({
          collectionId: activeCollectionId,
          pinnedOnly,
          searchQuery,
        });
      }
    } catch {
      // Keep stale chip counts until the next full reload.
    }
  }

  function scheduleTagCountsRefresh() {
    clearTimeout(tagCountsRefreshTimer);
    const gen = ++tagCountsFetchGen;
    tagCountsRefreshTimer = setTimeout(() => {
      void refreshTagCounts(gen);
    }, 100);
  }

  function currentFilterAdjustment() {
    if (displayFetchFailed || displayListPending) return null;
    return reconcileOverlayFilterState({
      isRevealing: deps.getIsRevealing(),
      showContentKindRow,
      contentKind,
      activeTag,
      catalogEntries,
      catalogTagCounts,
      displayEntries: entries,
      hasMore: entriesHasMore,
      searchQuery,
    });
  }

  function applyFilterAdjustment(
    adjustment: NonNullable<ReturnType<typeof currentFilterAdjustment>>,
  ) {
    if (adjustment.clearContentKindSession) {
      writeContentKindSession("all");
    }
    contentKind = adjustment.contentKind;
    activeTag = adjustment.activeTag;
  }

  /** Apply reconcile adjustments in one reload (avoids empty intermediate filter states). */
  async function applyPendingFilterAdjustments(gen: number, depth = 0): Promise<boolean> {
    const adjustment = currentFilterAdjustment();
    if (isReconcileDepthExhausted(depth, adjustment, { contentKind, activeTag })) {
      displayFetchFailed = true;
      return false;
    }

    if (
      !adjustment?.needsReload ||
      (adjustment.contentKind === contentKind && adjustment.activeTag === activeTag)
    ) {
      return true;
    }

    applyFilterAdjustment(adjustment);
    if (!(await applyDisplayPage0(gen, "data"))) return false;
    if (entries.length > 0) return true;

    return applyPendingFilterAdjustments(gen, depth + 1);
  }

  async function reloadDisplayList(selectFirst = true, scrollToFirst = true) {
    suppressReconcileEffect = true;
    loadingMoreEntries = false;
    invalidateInFlightFetches();
    const hadFilteredPastFirstPage = filteredPastFirstPage;
    const hadEntriesPastFirstPage = entriesPastFirstPage;
    entriesPastFirstPage = false;
    filteredPastFirstPage = false;
    const gen = dataFetchGen;

    displayPageLoading = true;
    displayFetchFailed = false;

    try {
      if (!usesFilteredDisplayList()) {
        if (
          hadFilteredPastFirstPage &&
          catalogEntries.length <= ENTRY_PAGE_SIZE &&
          !hadEntriesPastFirstPage
        ) {
          try {
            const tagCountsKey = tagCountsQueryKey({
              collectionId: activeCollectionId,
              pinnedOnly,
              searchQuery: "",
            });
            const [data, tagCounts] = await Promise.all([
              getEntries({
                ...entryQuery(),
                search: null,
                tag: null,
                content_kind: null,
                limit: ENTRY_PAGE_SIZE,
                offset: 0,
              }),
              getOverlayTagCounts({ ...entryQuery(), search: null }),
            ]);
            if (gen !== dataFetchGen) return;
            entries = data;
            catalogEntries = data;
            entriesHasMore = data.length === ENTRY_PAGE_SIZE;
            catalogHasMore = entriesHasMore;
            catalogQueryKey = entryQueryKey();
            catalogTagCounts = tagCounts;
            catalogTagCountsQueryKey = tagCountsKey;
          } catch {
            if (gen === dataFetchGen) {
              displayFetchFailed = true;
              entries = [];
              catalogEntries = [];
              entriesHasMore = false;
              catalogHasMore = false;
              catalogTagCounts = null;
              catalogQueryKey = "";
              catalogTagCountsQueryKey = "";
            }
            return;
          }
        } else if (!(await applyDisplayPage0(gen, "data"))) {
          refreshUnfilteredDisplayFromCatalog();
          return;
        }
      } else if (!(await applyDisplayPage0(gen, "data"))) {
        return;
      }

      if (!(await applyPendingFilterAdjustments(gen))) return;

      applyEntrySelection(selectFirst, scrollToFirst);
    } finally {
      displayPageLoading = false;
      suppressReconcileEffect = false;
    }
  }

  async function loadEntries(selectFirst = false, scrollToFirst = true) {
    const gen = beginFreshFetch();
    if (!(await loadCatalog(gen))) return;
    await loadDisplayEntries(selectFirst, scrollToFirst, gen);
  }

  function warmCatalog(): Promise<void> {
    if (mountCatalogPromise) return mountCatalogPromise;
    mountCatalogPromise = loadEntries().finally(() => {
      mountCatalogPromise = null;
    });
    return mountCatalogPromise;
  }

  async function loadNextEntryPage() {
    if (loadingMoreEntries) return;
    if (displayListPending) return;
    if (displayFetchFailed) return;
    if (!entriesHasMore) return;

    loadMoreFailed = false;

    const dataFetchGenAtStart = dataFetchGen;
    const displayGenAtStart = displayFetchGen;
    const requestGen = ++loadMoreGen;
    const offset = entries.length;

    loadingMoreEntries = true;
    try {
      const data = await getEntries({
        ...displayListQuery(),
        limit: ENTRY_PAGE_SIZE,
        offset,
      });
      if (dataFetchGenAtStart !== dataFetchGen) return;
      if (displayGenAtStart !== displayFetchGen) return;
      if (requestGen !== loadMoreGen) return;

      if (offset === 0) {
        entriesPastFirstPage = false;
        filteredPastFirstPage = false;
      }

      const backfillFromEmpty = offset === 0 && entries.length === 0;
      entries = [...entries, ...data];
      entriesHasMore = data.length === ENTRY_PAGE_SIZE;
      if (offset >= ENTRY_PAGE_SIZE) {
        if (usesFilteredDisplayList()) {
          filteredPastFirstPage = true;
        } else {
          entriesPastFirstPage = true;
        }
      }

      if (!usesFilteredDisplayList()) {
        catalogEntries = entries;
        catalogHasMore = entriesHasMore;
      }
      loadMoreFailed = false;
      if (backfillFromEmpty && entries.length > 0) {
        applyEntrySelection(true, true);
      }
    } catch {
      if (dataFetchGenAtStart === dataFetchGen && displayGenAtStart === displayFetchGen) {
        loadMoreFailed = true;
      }
    } finally {
      if (requestGen === loadMoreGen) loadingMoreEntries = false;
    }
  }

  async function syncRetagAvailability() {
    retagAvailable = await isTaggingReady();
  }

  async function syncAiTaggingSettings(): Promise<boolean> {
    try {
      const settings = await getAppSettings();
      settingsLoadError = null;
      const enabled = isSemanticTagUiEnabled(settings);
      let filtersChanged = false;
      if (enabled !== aiTaggingEnabled) {
        if (!enabled) {
          if (activeTag && !activeTagCompatibleWithAi(activeTag, false)) {
            activeTag = null;
            filtersChanged = true;
          }
          if (contentKind !== "all") {
            contentKind = "all";
            filtersChanged = true;
          }
          clearContentKindSession();
        } else {
          contentKind = "all";
          activeTag = null;
          clearContentKindSession();
          filtersChanged = true;
        }
      }
      aiTaggingEnabled = enabled;
      return filtersChanged;
    } catch (err) {
      aiTaggingEnabled = false;
      settingsLoadError = invokeErrorMessage(err) || "unknown";
      return false;
    }
  }

  async function syncOverlayShortcutHints(): Promise<void> {
    try {
      const settings = await getAppSettings();
      settingsLoadError = null;
      overlayShortcutHintsEnabled = settings.overlay_shortcut_hints_enabled;
    } catch (err) {
      settingsLoadError = invokeErrorMessage(err) || "unknown";
    }
  }

  async function syncOverlaySettings(): Promise<boolean> {
    const [, filtersChanged] = await Promise.all([
      syncRetagAvailability(),
      syncAiTaggingSettings(),
      syncOverlayShortcutHints(),
    ]);
    if (filtersChanged && deps.getVisible()) {
      void loadEntries(true, true);
    }
    return filtersChanged;
  }

  function resetDisplayStateOnHide() {
    loadingMoreEntries = false;
    loadMoreFailed = false;
    clearTimeout(debounceTimer);
    invalidateInFlightFetches();
  }

  function resetOverlayFilters() {
    activeTag = null;
  }

  function restoreContentKindFromSession() {
    if (!aiTaggingEnabled || !CONTENT_KIND_ROW_UI_ENABLED) {
      contentKind = "all";
      return;
    }
    const saved = readContentKindSession();
    if (saved && saved !== "all" && showContentKindRow) {
      contentKind = saved;
      return;
    }
    contentKind = "all";
    if (saved && saved !== "all") {
      writeContentKindSession("all");
    }
  }

  async function prepareCatalogAndDisplay(isStillValid: () => boolean = () => true) {
    await syncOverlaySettings();
    if (!isStillValid()) return false;
    if (mountCatalogPromise) {
      await mountCatalogPromise;
      if (!isStillValid()) return false;
    }

    const reuseCatalog = catalogMatchesCurrentQuery() && catalogEntries.length > 0;
    contentKind = "all";

    if (!reuseCatalog) {
      const gen = beginFreshFetch();
      if (!(await loadCatalog(gen))) return false;
    } else {
      resetDisplayPaginationForReveal();
      entriesHasMore = catalogHasMore;
    }

    if (!isStillValid()) return false;
    restoreContentKindFromSession();

    displayPageLoading = true;
    displayFetchFailed = false;

    const gen = invalidateInFlightFetches();

    try {
      if (!(await applyDisplayPage0(gen, "data"))) return false;
      if (!(await applyPendingFilterAdjustments(gen))) return false;
      if (!isStillValid()) return false;
      applyEntrySelection(true, false);
      return true;
    } finally {
      displayPageLoading = false;
    }
  }

  function reconcileHasMoreAfterListShrink() {
    if (!shouldBackfillEntriesAfterShrink(entries.length, entriesHasMore)) return;
    // Same lazy-load path as scroll prefetch; empty state only when the fetch returns nothing.
    void loadNextEntryPage();
  }

  function entryMatchesActiveFilters(entry: ClipboardEntry): boolean {
    if (activeTag && !entryMatchesTag(entry, activeTag)) return false;
    if (showContentKindRow && contentKind !== "all" && !entryMatchesKind(entry, contentKind)) {
      return false;
    }
    return true;
  }

  function applyEntryTags(entryId: number, tags: string[]) {
    const wasInDisplay = entries.some((entry) => entry.id === entryId);
    const catalogEntry = catalogEntries.find((entry) => entry.id === entryId);
    const patch = (entry: ClipboardEntry) => (entry.id === entryId ? { ...entry, tags } : entry);
    entries = entries.map(patch);
    catalogEntries = catalogEntries.map(patch);
    scheduleTagCountsRefresh();

    if (searchQuery) {
      if (!wasInDisplay) {
        void reloadDisplayList(false, false);
      } else {
        const tagged = entries.find((entry) => entry.id === entryId);
        if (tagged && usesFilteredDisplayList() && !entryMatchesActiveFilters(tagged)) {
          entries = entries.filter((entry) => entry.id !== entryId);
          deps.onClampSelection();
          reconcileHasMoreAfterListShrink();
        }
      }
      return;
    }

    if (wasInDisplay) {
      const tagged = entries.find((entry) => entry.id === entryId);
      if (tagged && usesFilteredDisplayList() && !entryMatchesActiveFilters(tagged)) {
        entries = entries.filter((entry) => entry.id !== entryId);
        deps.onClampSelection();
        reconcileHasMoreAfterListShrink();
      }
      return;
    }

    if (!usesFilteredDisplayList() || !catalogEntry) return;
    const tagged: ClipboardEntry = { ...catalogEntry, tags };
    if (entryMatchesActiveFilters(tagged)) {
      void reloadDisplayList(false, false);
    }
  }

  function applyEntryOcr(entryId: number, ocrText: string) {
    const patch = (entry: ClipboardEntry) =>
      entry.id === entryId ? { ...entry, ocr_text: ocrText } : entry;
    entries = entries.map(patch);
    catalogEntries = catalogEntries.map(patch);

    if (searchQuery) {
      void reloadDisplayList(false, false);
    }
  }

  function removeEntry(entryId: number) {
    entries = entries.filter((entry) => entry.id !== entryId);
    catalogEntries = catalogEntries.filter((entry) => entry.id !== entryId);
    deps.onClampSelection();
    reconcileHasMoreAfterListShrink();
    scheduleTagCountsRefresh();
  }

  function handlePinned(entryId: number, pinned: boolean) {
    if (pinnedOnly && !pinned) {
      entries = entries.filter((entry) => entry.id !== entryId);
      catalogEntries = catalogEntries.filter((entry) => entry.id !== entryId);
      deps.onClampSelection();
      reconcileHasMoreAfterListShrink();
      scheduleTagCountsRefresh();
      return;
    }
    const patch = (entry: ClipboardEntry) =>
      entry.id === entryId ? { ...entry, is_pinned: pinned } : entry;
    entries = entries.map(patch);
    catalogEntries = catalogEntries.map(patch);
    scheduleTagCountsRefresh();
    // Pin does not change SQL order (created_at DESC); local patch keeps selection stable.
  }

  function setSearchQuery(q: string, options: { reload?: boolean; immediate?: boolean } = {}) {
    const { reload = true, immediate = false } = options;
    searchQuery = q;
    clearTimeout(debounceTimer);
    if (!reload) {
      if (q === "") syncDisplayFromCatalog();
      return;
    }
    searchPageReady = q === "";
    displayFetchFailed = false;

    if (!immediate && q) {
      invalidateInFlightFetches();
      entries = [];
      entriesHasMore = false;
      searchTagCounts = null;
      searchTagCountsQueryKey = "";
      debounceTimer = setTimeout(() => {
        const gen = beginFreshFetch();
        void loadDisplayEntries(true, true, gen);
      }, 150);
      return;
    }

    const gen = beginFreshFetch();
    void loadDisplayEntries(true, true, gen);
  }

  function clearSearch(options: { reload?: boolean; immediate?: boolean } = {}) {
    setSearchQuery("", options);
  }

  function debouncedSearch(q: string) {
    if (q === "") {
      clearSearch({ immediate: true });
      return;
    }
    setSearchQuery(q);
  }

  function handleCollectionSelect(id: number | null) {
    pinnedOnly = id === -1;
    activeCollectionId = id === -1 ? null : id;
    activeTag = null;
    void loadEntries(true);
  }

  function handleContentKindChange(kind: ContentKind) {
    contentKind = kind;
    if (aiTaggingEnabled) {
      writeContentKindSession(kind);
    }
    if (!activeTagCompatibleWithKind(activeTag, kind)) {
      activeTag = null;
    }
    void reloadDisplayList(true, true);
  }

  function handleTagSelect(tag: string) {
    activeTag = tag;
    void reloadDisplayList(true, true);
  }

  function retryDisplayFetch() {
    void reloadDisplayList(true, true);
  }

  function retryLoadMore() {
    loadMoreFailed = false;
    void loadNextEntryPage();
  }

  function handleTagReset() {
    activeTag = null;
    void reloadDisplayList(true, true);
  }

  $effect(() => {
    if (deps.getIsRevealing() || suppressReconcileEffect || displayFetchFailed) return;
    const adjustment = currentFilterAdjustment();
    if (!adjustment) return;
    suppressReconcileEffect = true;
    applyFilterAdjustment(adjustment);
    if (adjustment.needsReload) {
      void reloadDisplayList(false, false);
    } else {
      suppressReconcileEffect = false;
    }
  });

  $effect(() => {
    if (!deps.getVisible() || deps.getIsRevealing() || displayListPending || displayFetchFailed) {
      return;
    }
    syncDisplayFromCatalog();
  });

  function dispose() {
    clearTimeout(tagCountsRefreshTimer);
    clearTimeout(debounceTimer);
  }

  return {
    get entries() {
      return entries;
    },
    get catalogEntries() {
      return catalogEntries;
    },
    get catalogTagCounts() {
      return catalogTagCounts;
    },
    get searchTagCounts() {
      return searchTagCounts;
    },
    get searchQuery() {
      return searchQuery;
    },
    get activeCollectionId() {
      return activeCollectionId;
    },
    get pinnedOnly() {
      return pinnedOnly;
    },
    get activeTag() {
      return activeTag;
    },
    set activeTag(value: string | null) {
      activeTag = value;
    },
    get contentKind() {
      return contentKind;
    },
    set contentKind(value: ContentKind) {
      contentKind = value;
    },
    get aiTaggingEnabled() {
      return aiTaggingEnabled;
    },
    get overlayShortcutHintsEnabled() {
      return overlayShortcutHintsEnabled;
    },
    get settingsLoadError() {
      return settingsLoadError;
    },
    get retagAvailable() {
      return retagAvailable;
    },
    get entriesHasMore() {
      return entriesHasMore;
    },
    get loadingMoreEntries() {
      return loadingMoreEntries;
    },
    get searchPending() {
      return searchPending;
    },
    get displayListPending() {
      return displayListPending;
    },
    get displayFetchFailed() {
      return displayFetchFailed;
    },
    get loadMoreFailed() {
      return loadMoreFailed;
    },
    get showContentKindRow() {
      return showContentKindRow;
    },
    loadEntries,
    warmCatalog,
    loadNextEntryPage,
    reloadDisplayList,
    syncOverlaySettings,
    prepareCatalogAndDisplay,
    resetOverlayFilters,
    resetDisplayStateOnHide,
    clearSearch,
    debouncedSearch,
    removeEntry,
    applyEntryTags,
    applyEntryOcr,
    handlePinned,
    handleCollectionSelect,
    handleContentKindChange,
    handleTagSelect,
    handleTagReset,
    retryDisplayFetch,
    retryLoadMore,
    syncDisplayFromCatalog,
    dispose,
  };
}

export type OverlayEntriesStore = ReturnType<typeof createOverlayEntriesStore>;
