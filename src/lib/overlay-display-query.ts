import type { ContentKind } from "./overlay-filters";

/** Stable key for whether the current display query still matches a fetch that started earlier. */
export function displayQueryKey(input: {
  searchQuery: string;
  activeTag: string | null;
  contentKind: ContentKind;
  showContentKindRow: boolean;
  collectionId: number | null;
  pinnedOnly: boolean;
}): string {
  const kindFilter =
    input.showContentKindRow && input.contentKind !== "all" ? input.contentKind : "all";
  return [
    input.collectionId ?? "null",
    input.pinnedOnly,
    input.searchQuery,
    input.activeTag ?? "",
    kindFilter,
  ].join("\0");
}

/** Tag bar counts depend on collection/search scope only — not activeTag or contentKind. */
export function tagCountsQueryKey(input: {
  collectionId: number | null;
  pinnedOnly: boolean;
  searchQuery: string;
}): string {
  return [input.collectionId ?? "null", input.pinnedOnly, input.searchQuery].join("\0");
}
