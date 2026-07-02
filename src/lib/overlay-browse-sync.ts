export interface ShouldSyncLeadingCardOptions {
  keyboardBrowseUntil: number;
  now: number;
}

/** Whether trackpad idle / scrollend should select the leading visible card. */
export function shouldSyncLeadingCardAfterScroll(options: ShouldSyncLeadingCardOptions): boolean {
  // Time guard only — not a persistent browse mode. After expiry, trackpad may sync leading again
  // even if selection still reflects pre-arrow state (product tradeoff; do not re-anchor on leading≠selected).
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

/** Shared by scrollend and idle debounce — drains suppress, then maybe syncs leading. */
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

/** Any programmatic scroll that moves the viewport blocks one leading sync on scrollend. */
export function shouldIncrementSuppressOnProgrammaticScroll(options: {
  didScroll: boolean;
  /** False only for trackpad leading-card follow-up (`selectLeadingVisibleCard`). Default true. */
  suppressLeadingSync?: boolean;
}): boolean {
  const suppressLeadingSync = options.suppressLeadingSync ?? true;
  return options.didScroll && suppressLeadingSync;
}

/** Stale-async guard for coalesced scrollToSelected (see +page scrollToSelectedGeneration). */
export function shouldRunScrollToSelectedGeneration(
  requestedGeneration: number,
  currentGeneration: number,
): boolean {
  return requestedGeneration === currentGeneration;
}
