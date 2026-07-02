import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { createPanelTransitionEpoch, planInstantNativeHide } from "./overlay-motion.ts";

describe("planInstantNativeHide", () => {
  it("keeps animate mode when the panel was already hidden", () => {
    let bumps = 0;
    const plan = planInstantNativeHide(false, () => {
      bumps += 1;
      return bumps;
    });
    assert.equal(plan.motionMode, "animate");
    assert.equal(plan.releaseEpoch, null);
    assert.equal(bumps, 0);
  });

  it("snaps with instant mode when the panel was still visible", () => {
    let bumps = 0;
    const plan = planInstantNativeHide(true, () => {
      bumps += 1;
      return bumps;
    });
    assert.equal(plan.motionMode, "instant");
    assert.equal(plan.releaseEpoch, 1);
    assert.equal(bumps, 1);
  });
});

describe("createPanelTransitionEpoch", () => {
  it("invalidates a scheduled release after a later bump", () => {
    const epoch = createPanelTransitionEpoch();
    const scheduled = epoch.bump();
    epoch.bump();
    assert.equal(epoch.isCurrent(scheduled), false);
  });

  it("accepts the current scheduled release", () => {
    const epoch = createPanelTransitionEpoch();
    const scheduled = epoch.bump();
    assert.equal(epoch.isCurrent(scheduled), true);
  });
});
