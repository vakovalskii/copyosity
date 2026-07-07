import type { ContentKind } from "$lib/overlay-filters";
import type { OverlayTagCounts } from "$lib/types";

/** Max reconcile adjustment fetches inside one reload (tag → kind → row hide). */
export const MAX_RECONCILE_ADJUSTMENT_DEPTH = 4;

export interface ReconcileAdjustmentSnapshot {
  needsReload: boolean;
  contentKind: ContentKind;
  activeTag: string | null;
}

export function isReconcileDepthExhausted(
  depth: number,
  pending: ReconcileAdjustmentSnapshot | null,
  current: Pick<ReconcileAdjustmentSnapshot, "contentKind" | "activeTag">,
): boolean {
  if (depth < MAX_RECONCILE_ADJUSTMENT_DEPTH) return false;
  if (!pending?.needsReload) return false;
  return pending.contentKind !== current.contentKind || pending.activeTag !== current.activeTag;
}

export function shouldRefetchTagCounts(
  cachedKey: string,
  queryKey: string,
  cachedCounts: OverlayTagCounts | null,
): boolean {
  return cachedKey !== queryKey || cachedCounts === null;
}

/**
 * Backfill when local eviction emptied the grid but the last fetch indicated more
 * rows in the DB. Same path as scroll prefetch (offset 0 when the list is empty).
 */
export function shouldBackfillEntriesAfterShrink(
  entryCount: number,
  entriesHasMore: boolean,
): boolean {
  return entryCount === 0 && entriesHasMore;
}

/** Repopulate an empty unfiltered grid from the warm catalog (no network). */
export function shouldSyncDisplayFromCatalog(
  entryCount: number,
  catalogCount: number,
  filteredDisplay: boolean,
): boolean {
  return !filteredDisplay && entryCount === 0 && catalogCount > 0;
}

/** Refresh an unfiltered grid from the warm catalog (e.g. after clearing a tag filter). */
export function shouldRefreshUnfilteredDisplayFromCatalog(
  catalogCount: number,
  filteredDisplay: boolean,
): boolean {
  return !filteredDisplay && catalogCount > 0;
}
