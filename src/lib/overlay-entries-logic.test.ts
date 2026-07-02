import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  isReconcileDepthExhausted,
  MAX_RECONCILE_ADJUSTMENT_DEPTH,
  shouldBackfillEntriesAfterShrink,
  shouldRefetchTagCounts,
} from "./overlay-entries-logic.ts";

describe("isReconcileDepthExhausted", () => {
  it("returns false below the depth cap", () => {
    assert.equal(
      isReconcileDepthExhausted(
        MAX_RECONCILE_ADJUSTMENT_DEPTH - 1,
        { needsReload: true, contentKind: "all", activeTag: null },
        { contentKind: "image", activeTag: "png" },
      ),
      false,
    );
  });

  it("returns true at the cap when another adjustment is still pending", () => {
    assert.equal(
      isReconcileDepthExhausted(
        MAX_RECONCILE_ADJUSTMENT_DEPTH,
        { needsReload: true, contentKind: "all", activeTag: null },
        { contentKind: "image", activeTag: "png" },
      ),
      true,
    );
  });

  it("returns false at the cap when no adjustment is pending", () => {
    assert.equal(
      isReconcileDepthExhausted(MAX_RECONCILE_ADJUSTMENT_DEPTH, null, {
        contentKind: "image",
        activeTag: "png",
      }),
      false,
    );
  });

  it("returns false at the cap when pending does not need reload", () => {
    assert.equal(
      isReconcileDepthExhausted(
        MAX_RECONCILE_ADJUSTMENT_DEPTH,
        { needsReload: false, contentKind: "all", activeTag: null },
        { contentKind: "image", activeTag: "png" },
      ),
      false,
    );
  });
});

describe("shouldBackfillEntriesAfterShrink", () => {
  it("backfills when the grid is empty but more DB rows may exist", () => {
    assert.equal(shouldBackfillEntriesAfterShrink(0, true), true);
  });

  it("does not backfill when entries remain on screen", () => {
    assert.equal(shouldBackfillEntriesAfterShrink(49, true), false);
  });

  it("does not backfill when the catalog is exhausted", () => {
    assert.equal(shouldBackfillEntriesAfterShrink(0, false), false);
  });
});

describe("shouldRefetchTagCounts", () => {
  const counts = {
    semantic: [],
    format: [],
    has_text: true,
    has_images: false,
  };

  it("refetches when the scope key changes", () => {
    assert.equal(shouldRefetchTagCounts("a", "b", counts), true);
  });

  it("refetches when cached counts are missing", () => {
    assert.equal(shouldRefetchTagCounts("a", "a", null), true);
  });

  it("skips refetch when key and counts match", () => {
    assert.equal(shouldRefetchTagCounts("a", "a", counts), false);
  });
});
