export type OverlayEscapeAction = "clear-search" | "dismiss";

export function overlayEscapeAction(hasSearchQuery: boolean): OverlayEscapeAction {
  return hasSearchQuery ? "clear-search" : "dismiss";
}
