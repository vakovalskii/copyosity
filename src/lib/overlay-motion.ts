/** Panel CSS motion: animated open/close vs snap without transition. */
export type PanelMotionMode = "animate" | "instant";

export type PanelTransitionEpoch = {
  bump: () => number;
  isCurrent: (value: number) => boolean;
};

export function createPanelTransitionEpoch(): PanelTransitionEpoch {
  let epoch = 0;
  return {
    bump(): number {
      epoch += 1;
      return epoch;
    },
    isCurrent(value: number): boolean {
      return epoch === value;
    },
  };
}

export type InstantNativeHidePlan = {
  motionMode: PanelMotionMode;
  releaseEpoch: number | null;
};

/** Frontend CSS plan when native panel hides without `window-hide-request` (e.g. settings). */
export function planInstantNativeHide(
  wasVisible: boolean,
  bumpEpoch: () => number,
): InstantNativeHidePlan {
  if (!wasVisible) {
    return { motionMode: "animate", releaseEpoch: null };
  }
  return { motionMode: "instant", releaseEpoch: bumpEpoch() };
}
