const DEFAULT_PREFETCH_PX = 240;

export interface ShouldLoadNextEntryPageOptions {
  scrollLeft: number;
  clientWidth: number;
  scrollWidth: number;
  hasMore: boolean;
  loading: boolean;
  prefetchPx?: number;
}

/** Whether the grid is scrolled close enough to the right edge to fetch the next page. */
export function shouldLoadNextEntryPage(options: ShouldLoadNextEntryPageOptions): boolean {
  if (options.loading || !options.hasMore) return false;
  const prefetchPx = options.prefetchPx ?? DEFAULT_PREFETCH_PX;
  const distanceToEnd = options.scrollWidth - (options.scrollLeft + options.clientWidth);
  return distanceToEnd <= prefetchPx;
}
