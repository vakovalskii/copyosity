import { overlayEscapeAction } from "./overlay-search.ts";

export type QuickLookToggleContext = {
  displayListPending: boolean;
  displayFetchFailed: boolean;
  selectedIndex: number;
  entryCount: number;
};

export type OverlayEscapeContext = {
  cardContextMenuOpen: boolean;
  quickLookOpen: boolean;
  hasSearchQuery: boolean;
};

export type QuickLookSpaceContext = QuickLookToggleContext & {
  searchFocused: boolean;
  typingInField: boolean;
};

export type SearchToGridContext = {
  boardVertical: boolean;
  searchFocused: boolean;
};

function isButtonTarget(target: EventTarget | null): boolean {
  return target !== null && "tagName" in target && (target as Element).tagName === "BUTTON";
}

export function canToggleQuickLook(ctx: QuickLookToggleContext): boolean {
  if (ctx.displayListPending || ctx.displayFetchFailed) return false;
  return ctx.selectedIndex >= 0 && ctx.selectedIndex < ctx.entryCount;
}

export function resolveOverlayEscapeAction(
  ctx: OverlayEscapeContext,
): "close-context-menu" | "close-quick-look" | "clear-search" | "dismiss-overlay" {
  if (ctx.cardContextMenuOpen) return "close-context-menu";
  if (ctx.quickLookOpen) return "close-quick-look";
  const action = overlayEscapeAction(ctx.hasSearchQuery);
  return action === "clear-search" ? "clear-search" : "dismiss-overlay";
}

export function shouldHandleQuickLookSpace(
  key: string,
  modifiers: { altKey: boolean; metaKey: boolean; ctrlKey: boolean },
  target: EventTarget | null,
  ctx: QuickLookSpaceContext,
): boolean {
  if (key !== " ") return false;
  if (modifiers.altKey || modifiers.metaKey || modifiers.ctrlKey) return false;
  if (isButtonTarget(target)) return false;
  if (ctx.searchFocused || ctx.typingInField) return false;
  return canToggleQuickLook(ctx);
}

export function shouldHandleQuickLookCmdY(
  key: string,
  modifiers: { altKey: boolean; metaKey: boolean; ctrlKey: boolean; shiftKey: boolean },
  target: EventTarget | null,
  ctx: QuickLookToggleContext,
): boolean {
  if (!modifiers.metaKey && !modifiers.ctrlKey) return false;
  if (modifiers.altKey || modifiers.shiftKey) return false;
  if (key.toLowerCase() !== "y") return false;
  if (isButtonTarget(target)) return false;
  return canToggleQuickLook(ctx);
}

export function shouldBlockOverlayActionWhileQuickLookOpen(quickLookOpen: boolean): boolean {
  return quickLookOpen;
}

export function shouldExitSearchToGrid(
  key: string,
  modifiers: { metaKey: boolean; ctrlKey: boolean; altKey: boolean },
  ctx: SearchToGridContext,
): boolean {
  if (!ctx.searchFocused) return false;
  if (modifiers.metaKey || modifiers.ctrlKey || modifiers.altKey) return false;
  const enterGridKey = ctx.boardVertical
    ? key === "ArrowDown"
    : key === "ArrowRight" || key === "ArrowDown";
  return enterGridKey;
}
