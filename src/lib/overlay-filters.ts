import type { ClipboardEntry, OverlayTagCounts } from "$lib/types";

import { cardTagDisplayLabel } from "./card-tag-label.ts";
import { resolveImageFormatBadge } from "./image-meta.ts";

export type ContentKind = "all" | "text" | "image";

/** Settings fields that gate semantic tag chips and card tag rows in the overlay. */
export type SemanticTagUiSettings = {
  ai_tagging_enabled: boolean;
  hub_enabled: boolean;
  hub_tagging_enabled: boolean;
  hub_token: string;
};

/** Semantic tags/filters are shown when local AI tagging or hub tagging is configured. */
export function isSemanticTagUiEnabled(settings: SemanticTagUiSettings): boolean {
  return (
    settings.ai_tagging_enabled ||
    (settings.hub_enabled && settings.hub_tagging_enabled && Boolean(settings.hub_token?.trim()))
  );
}

export type TagChip = [tag: string, count: number];

export const HIDDEN_TOP_TAGS = new Set(["code", "otp", "token", "log"]);
export const IMAGE_FORMAT_TAGS = ["gif", "jpg", "png"] as const;
export const IMAGE_FORMAT_TAG_SET = new Set<string>(IMAGE_FORMAT_TAGS);

const SEMANTIC_TAG_LIMIT = 8;

export function sortTagsByCount(tagCounts: TagChip[]): TagChip[] {
  return [...tagCounts].toSorted((a, b) => {
    if (b[1] !== a[1]) return b[1] - a[1];
    return a[0].localeCompare(b[0]);
  });
}

export function isFormatTag(tag: string): boolean {
  return IMAGE_FORMAT_TAG_SET.has(tag);
}

export function entryMatchesKind(entry: ClipboardEntry, kind: ContentKind): boolean {
  switch (kind) {
    case "all":
      return true;
    case "text":
      return entry.content_type === "text";
    case "image":
      return entry.content_type === "image";
    default: {
      return kind satisfies never;
    }
  }
}

export function entryMatchesTag(entry: ClipboardEntry, tag: string): boolean {
  if (!isFormatTag(tag)) {
    const target = cardTagDisplayLabel(tag);
    return (entry.tags ?? []).some((entryTag) => cardTagDisplayLabel(entryTag) === target);
  }
  if ((entry.tags ?? []).includes(tag)) return true;
  if (entry.content_type === "image") {
    const badge = resolveImageFormatBadge(entry.image_format ?? null, entry.image_thumb ?? null);
    if (badge && badge.toLowerCase() === tag.toLowerCase()) return true;
  }
  return false;
}

export function filterKindPool(
  entries: ClipboardEntry[],
  aiTaggingEnabled: boolean,
  contentKind: ContentKind,
): ClipboardEntry[] {
  if (!aiTaggingEnabled) return entries;
  return entries.filter((entry) => entryMatchesKind(entry, contentKind));
}

export function filterByActiveTag(
  entries: ClipboardEntry[],
  activeTag: string | null,
): ClipboardEntry[] {
  if (!activeTag) return entries;
  return entries.filter((entry) => entryMatchesTag(entry, activeTag));
}

function tagCountsToChips(counts: { tag: string; count: number }[]): TagChip[] {
  return counts.map((item) => [item.tag, item.count] as TagChip);
}

function sortFormatChips(chips: TagChip[]): TagChip[] {
  return sortTagsByCount(chips);
}

function mergeSemanticChipsByDisplayLabel(counts: { tag: string; count: number }[]): TagChip[] {
  const merged = new Map<string, number>();
  for (const { tag, count } of counts) {
    if (HIDDEN_TOP_TAGS.has(tag) || isFormatTag(tag)) continue;
    const label = cardTagDisplayLabel(tag);
    merged.set(label, (merged.get(label) ?? 0) + count);
  }
  return sortTagsByCount([...merged.entries()]).slice(0, SEMANTIC_TAG_LIMIT);
}

function chipsFromServerCounts(
  counts: OverlayTagCounts,
  contentKind: ContentKind,
  aiTaggingEnabled: boolean,
  showRowA: boolean,
): TagBarChips {
  const kindFilterActive = aiTaggingEnabled && showRowA;
  let formatChips = sortFormatChips(tagCountsToChips(counts.format));
  let semanticChips = mergeSemanticChipsByDisplayLabel(counts.semantic);
  let resetLabel = "All tags";

  if (!aiTaggingEnabled) {
    semanticChips = [];
    resetLabel = "All formats";
  } else if (!showRowA) {
    if (counts.has_images && !counts.has_text) {
      semanticChips = [];
      resetLabel = "All formats";
    } else if (counts.has_text) {
      formatChips = [];
      resetLabel = "All tags";
    }
  } else if (kindFilterActive) {
    if (contentKind === "text") {
      formatChips = [];
      resetLabel = "All tags";
    } else if (contentKind === "image") {
      semanticChips = [];
      resetLabel = "All formats";
    }
  }

  return {
    formatChips,
    semanticChips,
    resetLabel,
    hasChips: formatChips.length > 0 || semanticChips.length > 0,
  };
}

function countFormatTags(pool: ClipboardEntry[]): TagChip[] {
  const counts = new Map<string, number>();
  for (const entry of pool) {
    if (entry.content_type !== "image") continue;
    for (const tag of IMAGE_FORMAT_TAGS) {
      if (entryMatchesTag(entry, tag)) {
        counts.set(tag, (counts.get(tag) ?? 0) + 1);
      }
    }
  }
  return sortTagsByCount(
    IMAGE_FORMAT_TAGS.filter((tag) => counts.has(tag)).map(
      (tag) => [tag, counts.get(tag)!] as TagChip,
    ),
  );
}

function countSemanticTags(pool: ClipboardEntry[], aiTaggingEnabled: boolean): TagChip[] {
  if (!aiTaggingEnabled) return [];
  const counts = new Map<string, number>();
  for (const entry of pool) {
    if (entry.content_type !== "text") continue;
    const seenLabels = new Set<string>();
    for (const tag of entry.tags ?? []) {
      if (HIDDEN_TOP_TAGS.has(tag) || isFormatTag(tag)) continue;
      const label = cardTagDisplayLabel(tag);
      if (seenLabels.has(label)) continue;
      seenLabels.add(label);
      counts.set(label, (counts.get(label) ?? 0) + 1);
    }
  }
  return sortTagsByCount([...counts.entries()]).slice(0, SEMANTIC_TAG_LIMIT);
}

export type TagBarModel = {
  showRowA: boolean;
  showRowB: boolean;
  resetLabel: string;
  formatChips: TagChip[];
  semanticChips: TagChip[];
  showDivider: boolean;
};

export function hasTextEntries(entries: ClipboardEntry[]): boolean {
  return entries.some((entry) => entry.content_type === "text");
}

export function hasImageEntries(entries: ClipboardEntry[]): boolean {
  return entries.some((entry) => entry.content_type === "image");
}

type TagBarChips = Pick<TagBarModel, "formatChips" | "semanticChips" | "resetLabel"> & {
  hasChips: boolean;
};

function tagBarChipsForPool(options: {
  pool: ClipboardEntry[];
  contentKind: ContentKind;
  aiTaggingEnabled: boolean;
  showRowA: boolean;
  textAvailable: boolean;
  imagesAvailable: boolean;
  serverCounts?: OverlayTagCounts | null;
}): TagBarChips {
  const {
    pool,
    contentKind,
    aiTaggingEnabled,
    showRowA,
    textAvailable,
    imagesAvailable,
    serverCounts,
  } = options;

  if (serverCounts) {
    return chipsFromServerCounts(serverCounts, contentKind, aiTaggingEnabled, showRowA);
  }

  const kindFilterActive = aiTaggingEnabled && showRowA;
  const kindPool = filterKindPool(pool, kindFilterActive, contentKind);

  let formatChips: TagChip[] = [];
  let semanticChips: TagChip[] = [];
  let resetLabel = "All tags";

  if (!aiTaggingEnabled) {
    formatChips = countFormatTags(pool);
    resetLabel = "All formats";
  } else if (!showRowA) {
    if (imagesAvailable && !textAvailable) {
      formatChips = countFormatTags(pool);
      resetLabel = "All formats";
    } else if (textAvailable) {
      semanticChips = countSemanticTags(pool, true);
      resetLabel = "All tags";
    }
  } else if (contentKind === "text") {
    semanticChips = countSemanticTags(kindPool, true);
    resetLabel = "All tags";
  } else if (contentKind === "image") {
    formatChips = countFormatTags(kindPool);
    resetLabel = "All formats";
  } else {
    formatChips = countFormatTags(kindPool);
    semanticChips = countSemanticTags(kindPool, true);
    resetLabel = "All tags";
  }

  return {
    formatChips,
    semanticChips,
    resetLabel,
    hasChips: formatChips.length > 0 || semanticChips.length > 0,
  };
}

export function buildTagBarModel(options: {
  entries: ClipboardEntry[];
  /** Unfiltered pool for row visibility when search narrows `entries` to zero. */
  layoutEntries?: ClipboardEntry[];
  contentKind: ContentKind;
  aiTaggingEnabled: boolean;
  activeTag?: string | null;
  /** DB-wide tag totals for the visible entry scope (includes search when active). */
  displayTagCounts?: OverlayTagCounts | null;
  /** DB-wide tag totals for catalog scope (no search), used for row layout. */
  layoutTagCounts?: OverlayTagCounts | null;
  /** Active search query — when set and `entries` is empty, hide the tag row. */
  searchQuery?: string;
  /** True while the search result list is still loading (keep tags visible). */
  searchPending?: boolean;
}): TagBarModel {
  const { entries, contentKind, aiTaggingEnabled, activeTag = null } = options;
  const layoutPool = options.layoutEntries ?? entries;

  const textAvailable = options.layoutTagCounts?.has_text ?? hasTextEntries(layoutPool);
  const imagesAvailable = options.layoutTagCounts?.has_images ?? hasImageEntries(layoutPool);
  const showRowA = aiTaggingEnabled && textAvailable && imagesAvailable;

  const chipOptions = {
    contentKind,
    aiTaggingEnabled,
    showRowA,
    textAvailable,
    imagesAvailable,
  };
  const display = tagBarChipsForPool({
    pool: entries,
    ...chipOptions,
    serverCounts: options.displayTagCounts,
  });
  const layout = tagBarChipsForPool({
    pool: layoutPool,
    ...chipOptions,
    serverCounts: options.layoutTagCounts,
  });

  const showDivider =
    showRowA &&
    contentKind === "all" &&
    display.formatChips.length > 0 &&
    display.semanticChips.length > 0;

  const stickyActiveTag =
    activeTag !== null &&
    (isFormatTag(activeTag) ? imagesAvailable : aiTaggingEnabled && textAvailable);
  const stickySegment = showRowA && contentKind !== "all";

  const hideForEmptySearch =
    (options.searchQuery?.trim() ?? "") !== "" &&
    entries.length === 0 &&
    !options.searchPending &&
    activeTag === null;

  const showRowB =
    !hideForEmptySearch &&
    (display.hasChips || layout.hasChips || stickyActiveTag || stickySegment);

  return {
    showRowA,
    showRowB,
    resetLabel: display.resetLabel,
    formatChips: display.formatChips,
    semanticChips: display.semanticChips,
    showDivider,
  };
}

export function cardDisplayTags(entry: ClipboardEntry, aiTaggingEnabled: boolean): string[] {
  if (!aiTaggingEnabled) return [];
  let tags = entry.tags ?? [];
  if (entry.content_type === "image") {
    tags = tags.filter((tag) => !isFormatTag(tag));
  }
  const seenLabels = new Set<string>();
  const unique: string[] = [];
  for (const tag of tags) {
    const label = cardTagDisplayLabel(tag);
    if (seenLabels.has(label)) continue;
    seenLabels.add(label);
    unique.push(tag);
  }
  return unique;
}

export function isSemanticTag(tag: string): boolean {
  return !isFormatTag(tag);
}

/** Clear activeTag when switching content kind if incompatible. */
export function activeTagCompatibleWithKind(
  activeTag: string | null,
  contentKind: ContentKind,
): boolean {
  if (!activeTag) return true;
  if (isFormatTag(activeTag)) {
    return contentKind === "all" || contentKind === "image";
  }
  return contentKind === "all" || contentKind === "text";
}

/** Clear semantic activeTag when AI tagging is turned off. */
export function activeTagCompatibleWithAi(
  activeTag: string | null,
  aiTaggingEnabled: boolean,
): boolean {
  if (!activeTag) return true;
  if (aiTaggingEnabled) return true;
  return isFormatTag(activeTag);
}

export function formatTagEmptyLabel(tag: string): string {
  return `No ${tag.toUpperCase()} images`;
}

export function contentKindEmptyLabel(kind: ContentKind): string | null {
  if (kind === "image") return "No images in clipboard history";
  if (kind === "text") return "No text entries in history";
  return null;
}

/** True when history has entries beyond an empty first catalog page. */
export function catalogHasHistory(
  catalogEntries: ClipboardEntry[],
  catalogTagCounts: OverlayTagCounts | null | undefined,
): boolean {
  if (catalogEntries.length > 0) return true;
  if (!catalogTagCounts) return false;
  return (
    catalogTagCounts.has_text ||
    catalogTagCounts.has_images ||
    catalogTagCounts.semantic.length > 0 ||
    catalogTagCounts.format.length > 0
  );
}

export interface ReconcileOverlayFiltersOptions {
  catalogHasHistory: boolean;
  filteredEntries: ClipboardEntry[];
  activeTag: string | null;
  contentKind: ContentKind;
  kindFilterActive: boolean;
  hasMore?: boolean;
  /** When set, empty grid may be a legitimate search miss — keep sticky tag/kind filters. */
  searchQuery?: string;
}

/**
 * Drop stale overlay filters when the grid is empty but the catalog still has
 * entries.
 *
 * With server-side filtering, an empty first page means there are no DB matches,
 * so callers normally pass `hasMore: false`. The `hasMore` guard remains for
 * client-side pagination: do not clear an active filter while unloaded pages
 * may still contain matches.
 *
 * Product choice: when the grid is empty and every page is loaded, auto-clear
 * stale tag/kind filters instead of showing a persistent "no results" state.
 * Chip counts come from the server; chips with count 0 are hidden, so users
 * cannot select a tag that reconcile would immediately clear under normal use.
 */
export function reconcileOverlayFilters(
  options: ReconcileOverlayFiltersOptions,
): { activeTag: string | null; contentKind: ContentKind } | null {
  const {
    catalogHasHistory: catalogHasData,
    filteredEntries,
    activeTag,
    contentKind,
    kindFilterActive,
    hasMore = false,
    searchQuery = "",
  } = options;
  if (!catalogHasData || filteredEntries.length > 0) return null;
  if (hasMore) return null;
  if (searchQuery.trim()) return null;

  if (activeTag) {
    return { activeTag: null, contentKind };
  }

  if (kindFilterActive && contentKind !== "all") {
    return { activeTag: null, contentKind: "all" };
  }

  return null;
}

export interface OverlayFilterAdjustment {
  contentKind: ContentKind;
  activeTag: string | null;
  clearContentKindSession: boolean;
  needsReload: boolean;
}

export interface ReconcileOverlayFilterStateOptions {
  isRevealing: boolean;
  showContentKindRow: boolean;
  contentKind: ContentKind;
  activeTag: string | null;
  catalogEntries: ClipboardEntry[];
  catalogTagCounts: OverlayTagCounts | null | undefined;
  displayEntries: ClipboardEntry[];
  hasMore: boolean;
  searchQuery?: string;
}

/**
 * Single pass for overlay filter hygiene: hide Row A when only one kind remains,
 * then drop stale tag/kind filters when the grid is empty. Returns one adjustment
 * so the UI can apply state and reload at most once per reactive cycle.
 */
export function reconcileOverlayFilterState(
  options: ReconcileOverlayFilterStateOptions,
): OverlayFilterAdjustment | null {
  if (options.isRevealing) return null;

  let contentKind = options.contentKind;
  let activeTag = options.activeTag;
  let needsReload = false;
  let clearContentKindSession = false;

  if (!options.showContentKindRow && contentKind !== "all") {
    contentKind = "all";
    clearContentKindSession = true;
    if (activeTag && !activeTagCompatibleWithKind(activeTag, "all")) {
      activeTag = null;
    }
    needsReload = true;
  }

  const patch = reconcileOverlayFilters({
    catalogHasHistory: catalogHasHistory(options.catalogEntries, options.catalogTagCounts),
    filteredEntries: options.displayEntries,
    activeTag,
    contentKind,
    kindFilterActive: options.showContentKindRow,
    hasMore: options.hasMore,
    searchQuery: options.searchQuery ?? "",
  });

  if (patch) {
    if (patch.activeTag !== activeTag) {
      activeTag = patch.activeTag;
      needsReload = true;
    }
    if (patch.contentKind !== contentKind) {
      contentKind = patch.contentKind;
      if (patch.contentKind === "all") {
        clearContentKindSession = true;
      }
      needsReload = true;
    }
  }

  if (!needsReload && contentKind === options.contentKind && activeTag === options.activeTag) {
    return null;
  }

  return { contentKind, activeTag, clearContentKindSession, needsReload };
}
