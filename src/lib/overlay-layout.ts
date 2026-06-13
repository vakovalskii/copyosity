import type { TagBarModel } from "$lib/overlay-filters";

/** Logical overlay height tiers (CSS px, scaled in Rust). */
export type OverlayHeightTier = "compact" | "medium" | "full";

export const OVERLAY_HEIGHT_BY_TIER: Record<OverlayHeightTier, number> = {
  compact: 420,
  medium: 440,
  full: 480,
};

export function computeOverlayHeightTier(options: {
  showRowA: boolean;
  showRowB: boolean;
  hasSettingsNotice: boolean;
}): OverlayHeightTier {
  const filterRows = (options.showRowA ? 1 : 0) + (options.showRowB ? 1 : 0);

  if (filterRows >= 2) return "full";
  if (filterRows >= 1 || options.hasSettingsNotice) return "medium";
  return "compact";
}

export function overlayHeightForLayout(options: {
  tagBar: Pick<TagBarModel, "showRowA" | "showRowB">;
  hasSettingsNotice: boolean;
}): number {
  const tier = computeOverlayHeightTier({
    showRowA: options.tagBar.showRowA,
    showRowB: options.tagBar.showRowB,
    hasSettingsNotice: options.hasSettingsNotice,
  });
  return OVERLAY_HEIGHT_BY_TIER[tier];
}
