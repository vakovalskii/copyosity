import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  OVERLAY_BASE_HEIGHT_BY_TIER,
  OVERLAY_HINTS_EXTRA_HEIGHT,
  overlayHeightForLayout,
} from "./overlay-layout.ts";

const compactBar = { showRowA: false, showRowB: false };
const fullBar = { showRowA: true, showRowB: true };

describe("overlayHeightForLayout", () => {
  it("returns base compact height when hints are off", () => {
    assert.equal(
      overlayHeightForLayout({
        tagBar: compactBar,
        hasSettingsNotice: false,
        showShortcutHints: false,
      }),
      OVERLAY_BASE_HEIGHT_BY_TIER.compact,
    );
  });

  it("adds hints extra height when hints are on", () => {
    assert.equal(
      overlayHeightForLayout({
        tagBar: compactBar,
        hasSettingsNotice: false,
        showShortcutHints: true,
      }),
      OVERLAY_BASE_HEIGHT_BY_TIER.compact + OVERLAY_HINTS_EXTRA_HEIGHT,
    );
  });

  it("defaults showShortcutHints to true", () => {
    assert.equal(
      overlayHeightForLayout({
        tagBar: compactBar,
        hasSettingsNotice: false,
      }),
      OVERLAY_BASE_HEIGHT_BY_TIER.compact + OVERLAY_HINTS_EXTRA_HEIGHT,
    );
  });

  it("uses medium tier when one filter row is visible", () => {
    assert.equal(
      overlayHeightForLayout({
        tagBar: { showRowA: false, showRowB: true },
        hasSettingsNotice: false,
        showShortcutHints: false,
      }),
      OVERLAY_BASE_HEIGHT_BY_TIER.medium,
    );
  });

  it("uses full tier with hints enabled", () => {
    assert.equal(
      overlayHeightForLayout({
        tagBar: fullBar,
        hasSettingsNotice: false,
        showShortcutHints: true,
      }),
      OVERLAY_BASE_HEIGHT_BY_TIER.full + OVERLAY_HINTS_EXTRA_HEIGHT,
    );
  });
});
