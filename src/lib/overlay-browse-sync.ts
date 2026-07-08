export interface ShouldSyncLeadingCardOptions {
  keyboardBrowseUntil: number;
  now: number;
}

/** Whether trackpad idle / scrollend should clear a stale selection. */
export function shouldSyncLeadingCardAfterScroll(options: ShouldSyncLeadingCardOptions): boolean {
  // Time guard only — not a persistent browse mode. After expiry, trackpad may clear the
  // selection again even if it still reflects pre-arrow state.
  if (options.now < options.keyboardBrowseUntil) return false;
  return true;
}

/** Debounced trackpad sync must not run during keyboard arrow scrolling. */
export function shouldScheduleTrackpadLeadingSync(options: ShouldSyncLeadingCardOptions): boolean {
  return shouldSyncLeadingCardAfterScroll(options);
}

/** Clear suppress stuck from open/reveal only on user trackpad scroll, not keyboard scroll. */
export function shouldClearStuckSuppressOnUserScroll(options: {
  suppressSelectionSyncCount: number;
  keyboardBrowseUntil: number;
  now: number;
  isTrusted: boolean;
}): number {
  if (
    options.isTrusted &&
    options.suppressSelectionSyncCount > 0 &&
    options.now >= options.keyboardBrowseUntil
  ) {
    return 0;
  }
  return options.suppressSelectionSyncCount;
}

export interface ScrollEndBrowseSyncResult {
  nextSuppressCount: number;
  shouldSyncLeading: boolean;
}

/** Shared by scrollend and idle debounce — drains suppress, then maybe clears the selection. */
export function handleScrollEndBrowseSync(options: {
  suppressSelectionSyncCount: number;
  keyboardBrowseUntil: number;
  now: number;
}): ScrollEndBrowseSyncResult {
  // Counter (not a boolean): each programmatic scroll may emit scrollend; drain one per event.
  if (options.suppressSelectionSyncCount > 0) {
    return {
      nextSuppressCount: options.suppressSelectionSyncCount - 1,
      shouldSyncLeading: false,
    };
  }

  return {
    nextSuppressCount: 0,
    shouldSyncLeading: shouldSyncLeadingCardAfterScroll(options),
  };
}

/** Stale-async guard for coalesced scrollToSelected (see +page scrollToSelectedGeneration). */
export function shouldRunScrollToSelectedGeneration(
  requestedGeneration: number,
  currentGeneration: number,
): boolean {
  return requestedGeneration === currentGeneration;
}
