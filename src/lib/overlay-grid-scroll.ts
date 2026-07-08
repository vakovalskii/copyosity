export interface HorizontalRect {
  left: number;
  right: number;
}

export interface VerticalRect {
  top: number;
  bottom: number;
}

/** Leftmost card intersecting the padded horizontal viewport (matches scroll-snap start). */
export function indexOfLeadingVisibleCard(
  viewport: HorizontalRect,
  padLeft: number,
  padRight: number,
  cards: HorizontalRect[],
  slack = 2,
): number {
  if (cards.length === 0) return -1;

  const visibleLeft = viewport.left + padLeft;
  const visibleRight = viewport.right - padRight;

  let bestIndex = -1;
  let bestDistance = Infinity;

  for (let i = 0; i < cards.length; i++) {
    const card = cards[i];
    if (card.right <= visibleLeft - slack) continue;
    if (card.left >= visibleRight + slack) break;

    const distance = Math.abs(card.left - visibleLeft);
    if (distance < bestDistance) {
      bestDistance = distance;
      bestIndex = i;
    }
  }

  if (bestIndex >= 0) return bestIndex;

  let closestIndex = 0;
  let closestDistance = Infinity;
  for (let i = 0; i < cards.length; i++) {
    const distance = Math.abs(cards[i].left - visibleLeft);
    if (distance < closestDistance) {
      closestDistance = distance;
      closestIndex = i;
    }
  }

  return closestIndex;
}

/** True when the card does not intersect the padded horizontal viewport. */
export function isCardOffScreen(
  viewport: HorizontalRect,
  padLeft: number,
  padRight: number,
  card: HorizontalRect,
): boolean {
  const visibleLeft = viewport.left + padLeft;
  const visibleRight = viewport.right - padRight;
  return card.right <= visibleLeft || card.left >= visibleRight;
}

/** Topmost card intersecting the padded vertical viewport. */
export function indexOfLeadingVisibleCardVertical(
  viewport: VerticalRect,
  padTop: number,
  padBottom: number,
  cards: VerticalRect[],
  slack = 2,
): number {
  if (cards.length === 0) return -1;

  const visibleTop = viewport.top + padTop;
  const visibleBottom = viewport.bottom - padBottom;

  let bestIndex = -1;
  let bestDistance = Infinity;

  for (let i = 0; i < cards.length; i++) {
    const card = cards[i];
    if (card.bottom <= visibleTop - slack) continue;
    if (card.top >= visibleBottom + slack) break;

    const distance = Math.abs(card.top - visibleTop);
    if (distance < bestDistance) {
      bestDistance = distance;
      bestIndex = i;
    }
  }

  if (bestIndex >= 0) return bestIndex;

  let closestIndex = 0;
  let closestDistance = Infinity;
  for (let i = 0; i < cards.length; i++) {
    const distance = Math.abs(cards[i].top - visibleTop);
    if (distance < closestDistance) {
      closestDistance = distance;
      closestIndex = i;
    }
  }

  return closestIndex;
}

/** True when the card does not intersect the padded vertical viewport. */
export function isCardOffScreenVertical(
  viewport: VerticalRect,
  padTop: number,
  padBottom: number,
  card: VerticalRect,
): boolean {
  const visibleTop = viewport.top + padTop;
  const visibleBottom = viewport.bottom - padBottom;
  return card.bottom <= visibleTop || card.top >= visibleBottom;
}

export type VerticalCardViewportPosition = "inside" | "above" | "below";

/** Where a card sits relative to the padded vertical viewport. */
export function verticalCardViewportPosition(
  viewport: VerticalRect,
  padTop: number,
  padBottom: number,
  card: VerticalRect,
  slack = 2,
): VerticalCardViewportPosition {
  const visibleTop = viewport.top + padTop;
  const visibleBottom = viewport.bottom - padBottom;
  if (card.bottom <= visibleTop - slack) return "above";
  if (card.top >= visibleBottom + slack) return "below";
  return "inside";
}

/**
 * Pixels to add to `container.scrollTop` so the card fits the padded vertical viewport.
 * Returns 0 when the card already intersects the padded area (no scroll needed).
 */
export function verticalScrollDeltaToSnapCard(
  viewport: VerticalRect,
  padTop: number,
  padBottom: number,
  card: VerticalRect,
  slack = 2,
): number {
  const visibleTop = viewport.top + padTop;
  const visibleBottom = viewport.bottom - padBottom;

  if (card.top >= visibleTop - slack && card.bottom <= visibleBottom + slack) {
    return 0;
  }

  if (card.bottom > visibleBottom + slack) {
    return card.bottom - visibleBottom;
  }
  if (card.top < visibleTop - slack) {
    return card.top - visibleTop;
  }
  return 0;
}

/**
 * Vertical keyboard ↑/↓ scroll: align the row top when moving down and the row bottom when
 * moving up so selection advances through the full list instead of sticking on the bottom edge.
 */
export function verticalScrollDeltaForKeyboardNav(
  viewport: VerticalRect,
  padTop: number,
  padBottom: number,
  card: VerticalRect,
  direction: "up" | "down",
  slack = 2,
): number {
  const visibleTop = viewport.top + padTop;
  const visibleBottom = viewport.bottom - padBottom;

  if (card.top >= visibleTop - slack && card.bottom <= visibleBottom + slack) {
    return 0;
  }

  if (direction === "down") {
    return card.top - visibleTop;
  }
  return card.bottom - visibleBottom;
}

/**
 * Whether keyboard ←/→ should re-anchor to the leading visible card before moving.
 *
 * Product rule (not a bug): anchor only when selection is unset, missing, or fully off-screen.
 * Do NOT anchor when leading ≠ selected but the selected card is still visible — that breaks
 * rapid key repeat (each keypress would reset to leading, then +1, so the index sticks).
 * Trackpad scroll only clears a stale selection (see overlay-browse-sync.ts); it never
 * auto-selects a card. Arrows advance `selectedIndex` only.
 *
 * Vertical board: do not anchor to the topmost visible row when moving ↓ past a row below the
 * fold (or ↑ past a row above) — that snaps selection back to the first visible card and loops.
 */
export function shouldAnchorKeyboardSelectionBeforeArrow(options: {
  selectedIndex: number;
  selectedOffScreen: boolean;
  wrapperMissing?: boolean;
  boardVertical?: boolean;
  direction?: "left" | "right";
  verticalPosition?: VerticalCardViewportPosition;
}): boolean {
  if (options.selectedIndex < 0) return true;
  if (options.wrapperMissing) return true;
  if (!options.selectedOffScreen) return false;
  if (options.boardVertical && options.verticalPosition && options.direction) {
    if (options.direction === "right" && options.verticalPosition === "below") return false;
    if (options.direction === "left" && options.verticalPosition === "above") return false;
  }
  return true;
}

/** One keyboard arrow step: optional anchor to leading, then move (mirrors +page arrow handler). */
export function nextIndexAfterKeyboardArrow(options: {
  direction: "left" | "right";
  selectedIndex: number;
  leadingIndex: number;
  selectedOffScreen: boolean;
  entryCount: number;
  wrapperMissing?: boolean;
  boardVertical?: boolean;
  verticalPosition?: VerticalCardViewportPosition;
}): number {
  let index = options.selectedIndex;
  const hadNoSelection = index < 0;
  if (
    shouldAnchorKeyboardSelectionBeforeArrow({
      selectedIndex: index,
      selectedOffScreen: options.selectedOffScreen,
      wrapperMissing: options.wrapperMissing,
      boardVertical: options.boardVertical,
      direction: options.direction,
      verticalPosition: options.verticalPosition,
    }) &&
    options.leadingIndex >= 0
  ) {
    index = options.leadingIndex;
    // Nothing was selected yet and we're moving forward: land on the anchor
    // itself so the first → / ↓ press selects the leading card, instead of
    // skipping straight past it. A leading ← / ↑ press still pages backward
    // from the anchor to the previous card.
    if (hadNoSelection && options.direction === "right") return index;
  }
  if (options.direction === "right") {
    return Math.min(index + 1, options.entryCount - 1);
  }
  return Math.max(index - 1, 0);
}
