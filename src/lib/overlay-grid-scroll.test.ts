import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  indexOfLeadingVisibleCard,
  indexOfLeadingVisibleCardVertical,
  isCardOffScreen,
  isCardOffScreenVertical,
  nextIndexAfterKeyboardArrow,
  shouldAnchorKeyboardSelectionBeforeArrow,
  verticalCardViewportPosition,
  verticalScrollDeltaForKeyboardNav,
  verticalScrollDeltaToSnapCard,
} from "./overlay-grid-scroll.ts";

const viewport = { left: 0, right: 400 };
const viewportVertical = { top: 0, bottom: 600 };

/** Documents the rejected anchor policy (leading≠selected) — freezes rapid → at card 1. */
function antiPatternNextAfterArrowRight(selected: number): number {
  const leading = 0;
  let index = selected;
  if (leading >= 0 && leading !== index) index = leading;
  return Math.min(index + 1, 9);
}

describe("indexOfLeadingVisibleCard", () => {
  it("returns the card aligned nearest the padded viewport start", () => {
    const cards = [
      { left: -120, right: 100 },
      { left: 112, right: 332 },
      { left: 344, right: 564 },
    ];

    assert.equal(indexOfLeadingVisibleCard(viewport, 16, 16, cards), 1);
  });

  it("returns the first card when only one fits", () => {
    const cards = [{ left: 16, right: 236 }];

    assert.equal(indexOfLeadingVisibleCard(viewport, 16, 16, cards), 0);
  });

  it("returns -1 for an empty list", () => {
    assert.equal(indexOfLeadingVisibleCard(viewport, 16, 16, []), -1);
  });

  it("falls back to the card nearest the padded start when none intersect", () => {
    const cards = [
      { left: -500, right: -280 },
      { left: -260, right: -40 },
    ];

    assert.equal(indexOfLeadingVisibleCard(viewport, 16, 16, cards), 1);
  });

  it("stops scanning after cards past the padded viewport", () => {
    const cards = [
      { left: 16, right: 236 },
      { left: 248, right: 468 },
      { left: 900, right: 1_120 },
    ];

    assert.equal(indexOfLeadingVisibleCard(viewport, 16, 16, cards), 0);
  });
});

describe("isCardOffScreen", () => {
  it("returns true when the card is fully left of the padded viewport", () => {
    assert.equal(isCardOffScreen(viewport, 16, 16, { left: -300, right: -80 }), true);
  });

  it("returns true when the card is fully right of the padded viewport", () => {
    assert.equal(isCardOffScreen(viewport, 16, 16, { left: 420, right: 640 }), true);
  });

  it("returns false when the card intersects the padded viewport", () => {
    assert.equal(isCardOffScreen(viewport, 16, 16, { left: 112, right: 332 }), false);
  });
});

describe("indexOfLeadingVisibleCardVertical", () => {
  it("returns the card aligned nearest the padded viewport top", () => {
    const cards = [
      { top: -120, bottom: 100 },
      { top: 112, bottom: 332 },
      { top: 344, bottom: 564 },
    ];

    assert.equal(indexOfLeadingVisibleCardVertical(viewportVertical, 16, 16, cards), 1);
  });

  it("returns -1 for an empty list", () => {
    assert.equal(indexOfLeadingVisibleCardVertical(viewportVertical, 16, 16, []), -1);
  });

  it("falls back to the card nearest the padded top when none intersect", () => {
    const cards = [
      { top: -500, bottom: -280 },
      { top: -260, bottom: -40 },
    ];

    assert.equal(indexOfLeadingVisibleCardVertical(viewportVertical, 16, 16, cards), 1);
  });
});

describe("verticalScrollDeltaForKeyboardNav", () => {
  it("aligns the row top to the padded viewport top when moving down", () => {
    assert.equal(
      verticalScrollDeltaForKeyboardNav(
        viewportVertical,
        16,
        16,
        { top: 500, bottom: 620 },
        "down",
      ),
      484,
    );
  });

  it("aligns the row bottom to the padded viewport bottom when moving up", () => {
    assert.equal(
      verticalScrollDeltaForKeyboardNav(viewportVertical, 16, 16, { top: -80, bottom: 80 }, "up"),
      -504,
    );
  });

  it("returns 0 when the row is already fully visible", () => {
    assert.equal(
      verticalScrollDeltaForKeyboardNav(viewportVertical, 16, 16, { top: 32, bottom: 200 }, "down"),
      0,
    );
  });
});

describe("verticalCardViewportPosition", () => {
  it("classifies above, inside, and below the padded viewport", () => {
    assert.equal(
      verticalCardViewportPosition(viewportVertical, 16, 16, { top: -80, bottom: 10 }),
      "above",
    );
    assert.equal(
      verticalCardViewportPosition(viewportVertical, 16, 16, { top: 32, bottom: 200 }),
      "inside",
    );
    assert.equal(
      verticalCardViewportPosition(viewportVertical, 16, 16, { top: 620, bottom: 740 }),
      "below",
    );
  });
});

describe("verticalScrollDeltaToSnapCard", () => {
  it("returns 0 when the card is fully inside the padded viewport", () => {
    assert.equal(
      verticalScrollDeltaToSnapCard(viewportVertical, 16, 16, { top: 32, bottom: 200 }),
      0,
    );
  });

  it("scrolls down when the card extends below the padded viewport", () => {
    assert.equal(
      verticalScrollDeltaToSnapCard(viewportVertical, 16, 16, { top: 500, bottom: 700 }),
      116,
    );
  });

  it("scrolls up when the card extends above the padded viewport", () => {
    assert.equal(
      verticalScrollDeltaToSnapCard(viewportVertical, 16, 16, { top: -80, bottom: 80 }),
      -96,
    );
  });
});

describe("isCardOffScreenVertical", () => {
  it("returns true when the card is fully above the padded viewport", () => {
    assert.equal(
      isCardOffScreenVertical(viewportVertical, 16, 16, { top: -300, bottom: -80 }),
      true,
    );
  });

  it("returns true when the card is fully below the padded viewport", () => {
    assert.equal(
      isCardOffScreenVertical(viewportVertical, 16, 16, { top: 620, bottom: 840 }),
      true,
    );
  });

  it("returns false when the card intersects the padded viewport", () => {
    assert.equal(
      isCardOffScreenVertical(viewportVertical, 16, 16, { top: 112, bottom: 332 }),
      false,
    );
  });
});

/**
 * Intentional product behavior — not bugs. Do not "fix" by anchoring when leading ≠ selected.
 * See shouldAnchorKeyboardSelectionBeforeArrow in overlay-grid-scroll.ts.
 */
describe("keyboard arrow anchor policy (intentional)", () => {
  it("does not anchor when selected card is on-screen even if leading index differs", () => {
    assert.equal(
      shouldAnchorKeyboardSelectionBeforeArrow({
        selectedIndex: 1,
        selectedOffScreen: false,
      }),
      false,
    );
  });

  it("anchors when selection is unset (first arrow after trackpad)", () => {
    assert.equal(
      shouldAnchorKeyboardSelectionBeforeArrow({ selectedIndex: -1, selectedOffScreen: false }),
      true,
    );
  });

  it("anchors when the selected card is fully off-screen", () => {
    assert.equal(
      shouldAnchorKeyboardSelectionBeforeArrow({ selectedIndex: 0, selectedOffScreen: true }),
      true,
    );
  });

  it("anchors when the selected wrapper is missing from the DOM", () => {
    assert.equal(
      shouldAnchorKeyboardSelectionBeforeArrow({
        selectedIndex: 2,
        selectedOffScreen: false,
        wrapperMissing: true,
      }),
      true,
    );
  });

  it("does not anchor vertical ↓ when the selection is below the fold", () => {
    assert.equal(
      shouldAnchorKeyboardSelectionBeforeArrow({
        selectedIndex: 8,
        selectedOffScreen: true,
        boardVertical: true,
        direction: "right",
        verticalPosition: "below",
      }),
      false,
    );
  });

  it("does not anchor vertical ↑ when the selection is above the fold", () => {
    assert.equal(
      shouldAnchorKeyboardSelectionBeforeArrow({
        selectedIndex: 2,
        selectedOffScreen: true,
        boardVertical: true,
        direction: "left",
        verticalPosition: "above",
      }),
      false,
    );
  });

  it("vertical ↓ below the fold advances without snapping back to leading", () => {
    const next = nextIndexAfterKeyboardArrow({
      direction: "right",
      selectedIndex: 8,
      leadingIndex: 0,
      selectedOffScreen: true,
      entryCount: 20,
      boardVertical: true,
      verticalPosition: "below",
    });
    assert.equal(next, 9);
  });

  it("vertical loop regression: off-screen below then ↓ never resets to leading+1", () => {
    let selected = 5;
    const leading = 0;
    for (let step = 0; step < 6; step++) {
      selected = nextIndexAfterKeyboardArrow({
        direction: "right",
        selectedIndex: selected,
        leadingIndex: leading,
        selectedOffScreen: true,
        entryCount: 20,
        boardVertical: true,
        verticalPosition: "below",
      });
    }
    assert.equal(selected, 11);
  });

  it("rapid → advances 0→1→2→3 while leading stays 0 and cards stay on-screen", () => {
    const context = {
      leadingIndex: 0,
      selectedOffScreen: false,
      entryCount: 10,
    };
    let selected = 0;
    for (let step = 0; step < 3; step++) {
      selected = nextIndexAfterKeyboardArrow({
        direction: "right",
        selectedIndex: selected,
        ...context,
      });
    }
    assert.equal(selected, 3);
  });

  it("rapid ← advances 3→2→1→0 while leading stays 0 and cards stay on-screen", () => {
    const context = {
      leadingIndex: 0,
      selectedOffScreen: false,
      entryCount: 10,
    };
    let selected = 3;
    for (let step = 0; step < 3; step++) {
      selected = nextIndexAfterKeyboardArrow({
        direction: "left",
        selectedIndex: selected,
        ...context,
      });
    }
    assert.equal(selected, 0);
  });

  it("anti-pattern: anchoring on leading≠selected sticks index on rapid repeat", () => {
    assert.equal(antiPatternNextAfterArrowRight(0), 1);
    assert.equal(antiPatternNextAfterArrowRight(1), 1, "wrong policy freezes at card 1");
  });
});
