/**
 * Static overlay heights (CSS px, scaled in Rust).
 *
 * Measured from layout chrome — header, filter row, card grid, optional hints footer.
 * Grid uses fixed padding (--overlay-grid-pad-y); it does not flex-grow.
 */
export const OVERLAY_HEIGHT_BASE = 415;

/** Footer shortcut hints strip (Settings → Clipboard Panel → Keyboard shortcuts). */
export const OVERLAY_HINTS_EXTRA_HEIGHT = 35;

export function overlayHeightForLayout(options: { showShortcutHints?: boolean }): number {
  const showHints = options.showShortcutHints ?? true;
  return OVERLAY_HEIGHT_BASE + (showHints ? OVERLAY_HINTS_EXTRA_HEIGHT : 0);
}
