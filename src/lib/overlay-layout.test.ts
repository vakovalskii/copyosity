import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  OVERLAY_HEIGHT_BASE,
  OVERLAY_HINTS_EXTRA_HEIGHT,
  overlayHeightForLayout,
} from "./overlay-layout.ts";

describe("overlayHeightForLayout", () => {
  it("returns base height when hints are off", () => {
    assert.equal(overlayHeightForLayout({ showShortcutHints: false }), OVERLAY_HEIGHT_BASE);
  });

  it("adds hints extra height when hints are on", () => {
    assert.equal(
      overlayHeightForLayout({ showShortcutHints: true }),
      OVERLAY_HEIGHT_BASE + OVERLAY_HINTS_EXTRA_HEIGHT,
    );
  });

  it("defaults showShortcutHints to true", () => {
    assert.equal(overlayHeightForLayout({}), OVERLAY_HEIGHT_BASE + OVERLAY_HINTS_EXTRA_HEIGHT);
  });

  it("with-hints height matches Rust default pre-show", () => {
    assert.equal(OVERLAY_HEIGHT_BASE + OVERLAY_HINTS_EXTRA_HEIGHT, 450);
  });
});
