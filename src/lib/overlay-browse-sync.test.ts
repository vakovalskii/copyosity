import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  handleScrollEndBrowseSync,
  shouldClearStuckSuppressOnUserScroll,
  shouldIncrementSuppressOnProgrammaticScroll,
  shouldRunScrollToSelectedGeneration,
  shouldScheduleTrackpadLeadingSync,
  shouldSyncLeadingCardAfterScroll,
} from "./overlay-browse-sync.ts";

describe("shouldSyncLeadingCardAfterScroll", () => {
  it("syncs when no keyboard arrow browsing is in flight", () => {
    assert.equal(
      shouldSyncLeadingCardAfterScroll({
        keyboardBrowseUntil: 0,
        now: 1_000,
      }),
      true,
    );
  });

  it("skips during recent keyboard arrow browsing", () => {
    assert.equal(
      shouldSyncLeadingCardAfterScroll({
        keyboardBrowseUntil: 1_200,
        now: 1_000,
      }),
      false,
    );
  });
});

describe("shouldScheduleTrackpadLeadingSync", () => {
  it("schedules when trackpad browsing is idle", () => {
    assert.equal(shouldScheduleTrackpadLeadingSync({ keyboardBrowseUntil: 0, now: 1_000 }), true);
  });

  it("does not schedule while keyboard arrow scroll is in flight", () => {
    assert.equal(
      shouldScheduleTrackpadLeadingSync({ keyboardBrowseUntil: 1_500, now: 1_000 }),
      false,
    );
  });
});

describe("shouldClearStuckSuppressOnUserScroll", () => {
  it("clears stuck suppress on trusted trackpad scroll", () => {
    assert.equal(
      shouldClearStuckSuppressOnUserScroll({
        suppressSelectionSyncCount: 1,
        keyboardBrowseUntil: 0,
        now: 1_000,
        isTrusted: true,
      }),
      0,
    );
  });

  it("keeps suppress during keyboard arrow browsing even when scroll is trusted", () => {
    assert.equal(
      shouldClearStuckSuppressOnUserScroll({
        suppressSelectionSyncCount: 1,
        keyboardBrowseUntil: 1_500,
        now: 1_000,
        isTrusted: true,
      }),
      1,
    );
  });

  it("does not clear suppress for untrusted programmatic scroll", () => {
    assert.equal(
      shouldClearStuckSuppressOnUserScroll({
        suppressSelectionSyncCount: 1,
        keyboardBrowseUntil: 0,
        now: 1_000,
        isTrusted: false,
      }),
      1,
    );
  });
});

describe("handleScrollEndBrowseSync", () => {
  it("consumes suppress without syncing after keyboard scroll", () => {
    assert.deepEqual(
      handleScrollEndBrowseSync({
        suppressSelectionSyncCount: 1,
        keyboardBrowseUntil: 1_500,
        now: 1_000,
      }),
      { nextSuppressCount: 0, shouldSyncLeading: false },
    );
  });

  it("syncs leading card after trackpad scroll when idle", () => {
    assert.deepEqual(
      handleScrollEndBrowseSync({
        suppressSelectionSyncCount: 0,
        keyboardBrowseUntil: 0,
        now: 1_000,
      }),
      { nextSuppressCount: 0, shouldSyncLeading: true },
    );
  });

  it("does not sync leading card during keyboard browse even without suppress", () => {
    assert.deepEqual(
      handleScrollEndBrowseSync({
        suppressSelectionSyncCount: 0,
        keyboardBrowseUntil: 1_500,
        now: 1_000,
      }),
      { nextSuppressCount: 0, shouldSyncLeading: false },
    );
  });
});

describe("shouldIncrementSuppressOnProgrammaticScroll", () => {
  it("suppresses leading sync for reveal/clamp programmatic scroll by default", () => {
    assert.equal(shouldIncrementSuppressOnProgrammaticScroll({ didScroll: true }), true);
  });

  it("suppresses for explicit keyboard arrow scroll", () => {
    assert.equal(
      shouldIncrementSuppressOnProgrammaticScroll({ didScroll: true, suppressLeadingSync: true }),
      true,
    );
  });

  it("does not suppress for trackpad leading-card follow-up", () => {
    assert.equal(
      shouldIncrementSuppressOnProgrammaticScroll({ didScroll: true, suppressLeadingSync: false }),
      false,
    );
  });

  it("does not suppress when the programmatic scroll did not move the viewport", () => {
    assert.equal(shouldIncrementSuppressOnProgrammaticScroll({ didScroll: false }), false);
  });
});

describe("shouldRunScrollToSelectedGeneration", () => {
  it("runs only the latest generation after rapid arrow input", () => {
    assert.equal(shouldRunScrollToSelectedGeneration(2, 2), true);
    assert.equal(shouldRunScrollToSelectedGeneration(1, 2), false);
  });
});

describe("keyboard arrow then trackpad", () => {
  it("does not steal focus via leading sync while keyboard browse guard is active", () => {
    const now = 1_000;
    const keyboardBrowseUntil = 1_400;

    assert.equal(shouldScheduleTrackpadLeadingSync({ keyboardBrowseUntil, now }), false);

    const suppress = shouldClearStuckSuppressOnUserScroll({
      suppressSelectionSyncCount: 1,
      keyboardBrowseUntil,
      now,
      isTrusted: true,
    });
    assert.equal(suppress, 1);

    assert.deepEqual(
      handleScrollEndBrowseSync({
        suppressSelectionSyncCount: suppress,
        keyboardBrowseUntil,
        now,
      }),
      { nextSuppressCount: 0, shouldSyncLeading: false },
    );
  });
});

/** Intentional: time-based guard, not a persistent keyboard mode flag. */
describe("trackpad leading sync after keyboard (intentional)", () => {
  it("may sync leading again after keyboardBrowseUntil expires", () => {
    assert.equal(shouldSyncLeadingCardAfterScroll({ keyboardBrowseUntil: 500, now: 1_000 }), true);
  });

  it("drains suppress one count per scrollend; sync only after counter reaches zero", () => {
    const first = handleScrollEndBrowseSync({
      suppressSelectionSyncCount: 1,
      keyboardBrowseUntil: 0,
      now: 1_000,
    });
    assert.deepEqual(first, { nextSuppressCount: 0, shouldSyncLeading: false });

    const second = handleScrollEndBrowseSync({
      suppressSelectionSyncCount: first.nextSuppressCount,
      keyboardBrowseUntil: 0,
      now: 1_001,
    });
    assert.deepEqual(second, { nextSuppressCount: 0, shouldSyncLeading: true });
  });

  it("idle debounce uses the same policy as scrollend (shared handleScrollEndBrowseSync)", () => {
    assert.deepEqual(
      handleScrollEndBrowseSync({
        suppressSelectionSyncCount: 1,
        keyboardBrowseUntil: 0,
        now: 1_000,
      }),
      handleScrollEndBrowseSync({
        suppressSelectionSyncCount: 1,
        keyboardBrowseUntil: 0,
        now: 1_000,
      }),
    );
  });
});
