/** Tracks open card context menus so overlay-level Esc dismisses them first. */

let openCount = 0;

export const OVERLAY_CLOSE_CARD_CONTEXT_MENUS = "overlay:close-card-context-menus";

export function notifyCardContextMenuOpened(): void {
  openCount += 1;
}

export function notifyCardContextMenuClosed(): void {
  openCount = Math.max(0, openCount - 1);
}

export function isCardContextMenuOpen(): boolean {
  return openCount > 0;
}

export function closeAllCardContextMenus(): void {
  if (openCount === 0) return;
  window.dispatchEvent(new CustomEvent(OVERLAY_CLOSE_CARD_CONTEXT_MENUS));
}
