export interface HorizontalRect {
  left: number;
  right: number;
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

/**
 * Whether keyboard ←/→ should re-anchor to the leading visible card before moving.
 *
 * Product rule (not a bug): anchor only when selection is unset, missing, or fully off-screen.
 * Do NOT anchor when leading ≠ selected but the selected card is still visible — that breaks
 * rapid key repeat (each keypress would reset to leading, then +1, so the index sticks).
 * Trackpad scroll uses `selectLeadingVisibleCard` separately; arrows advance `selectedIndex` only.
 */
export function shouldAnchorKeyboardSelectionBeforeArrow(options: {
  selectedIndex: number;
  selectedOffScreen: boolean;
  wrapperMissing?: boolean;
}): boolean {
  if (options.selectedIndex < 0) return true;
  if (options.wrapperMissing) return true;
  return options.selectedOffScreen;
}

/** One keyboard arrow step: optional anchor to leading, then move (mirrors +page arrow handler). */
export function nextIndexAfterKeyboardArrow(options: {
  direction: "left" | "right";
  selectedIndex: number;
  leadingIndex: number;
  selectedOffScreen: boolean;
  entryCount: number;
  wrapperMissing?: boolean;
}): number {
  let index = options.selectedIndex;
  if (
    shouldAnchorKeyboardSelectionBeforeArrow({
      selectedIndex: index,
      selectedOffScreen: options.selectedOffScreen,
      wrapperMissing: options.wrapperMissing,
    }) &&
    options.leadingIndex >= 0
  ) {
    index = options.leadingIndex;
  }
  if (options.direction === "right") {
    return Math.min(index + 1, options.entryCount - 1);
  }
  return Math.max(index - 1, 0);
}
