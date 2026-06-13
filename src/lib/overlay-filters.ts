import type { ClipboardEntry } from "$lib/types";

export type ContentKind = "all" | "text" | "image";

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
  if ((entry.tags ?? []).includes(tag)) return true;
  if (!isFormatTag(tag) || entry.content_type !== "image") return false;
  return entry.image_format?.toLowerCase() === tag;
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
    for (const tag of entry.tags ?? []) {
      if (HIDDEN_TOP_TAGS.has(tag) || isFormatTag(tag)) continue;
      counts.set(tag, (counts.get(tag) ?? 0) + 1);
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
}): TagBarChips {
  const { pool, contentKind, aiTaggingEnabled, showRowA, textAvailable, imagesAvailable } = options;
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
}): TagBarModel {
  const { entries, contentKind, aiTaggingEnabled, activeTag = null } = options;
  const layoutPool = options.layoutEntries ?? entries;

  const textAvailable = hasTextEntries(layoutPool);
  const imagesAvailable = hasImageEntries(layoutPool);
  const showRowA = aiTaggingEnabled && textAvailable && imagesAvailable;

  const chipOptions = {
    contentKind,
    aiTaggingEnabled,
    showRowA,
    textAvailable,
    imagesAvailable,
  };
  const display = tagBarChipsForPool({ pool: entries, ...chipOptions });
  const layout = tagBarChipsForPool({ pool: layoutPool, ...chipOptions });

  const showDivider =
    showRowA &&
    contentKind === "all" &&
    display.formatChips.length > 0 &&
    display.semanticChips.length > 0;

  const stickyActiveTag =
    activeTag !== null &&
    (isFormatTag(activeTag) ? imagesAvailable : aiTaggingEnabled && textAvailable);
  const stickySegment = showRowA && contentKind !== "all";

  const showRowB = display.hasChips || layout.hasChips || stickyActiveTag || stickySegment;

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
  const tags = entry.tags ?? [];
  if (entry.content_type === "image") {
    return tags.filter((tag) => !isFormatTag(tag));
  }
  return tags;
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
