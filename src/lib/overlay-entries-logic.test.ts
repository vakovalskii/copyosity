import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  isReconcileDepthExhausted,
  isSearchPageReadyBeforeFetch,
  MAX_RECONCILE_ADJUSTMENT_DEPTH,
  shouldBackfillEntriesAfterShrink,
  shouldRefetchTagCounts,
  shouldRefreshUnfilteredDisplayFromCatalog,
  shouldShowOverlayEntryGrid,
  shouldSyncDisplayFromCatalog,
} from "./overlay-entries-logic.ts";

describe("isSearchPageReadyBeforeFetch", () => {
  it("marks a non-empty query pending before its debounced fetch starts", () => {
    assert.equal(isSearchPageReadyBeforeFetch("api"), false);
  });

  it("treats an empty or whitespace-only query as ready", () => {
    assert.equal(isSearchPageReadyBeforeFetch(""), true);
    assert.equal(isSearchPageReadyBeforeFetch("   "), true);
  });
});

describe("shouldShowOverlayEntryGrid", () => {
  it("hides catalog cards while a search page is pending", () => {
    assert.equal(shouldShowOverlayEntryGrid(12, true, "api"), false);
  });

  it("keeps cards visible when search is idle", () => {
    assert.equal(shouldShowOverlayEntryGrid(12, false, "api"), true);
    assert.equal(shouldShowOverlayEntryGrid(12, true, ""), true);
    assert.equal(shouldShowOverlayEntryGrid(12, true, "   "), true);
  });

  it("stays empty when there are no entries", () => {
    assert.equal(shouldShowOverlayEntryGrid(0, false, ""), false);
  });
});

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

describe("shouldRefreshUnfilteredDisplayFromCatalog", () => {
  it("refreshes when the catalog has rows and no filter is active", () => {
    assert.equal(shouldRefreshUnfilteredDisplayFromCatalog(12, false), true);
  });

  it("does not refresh filtered lists or an empty catalog", () => {
    assert.equal(shouldRefreshUnfilteredDisplayFromCatalog(12, true), false);
    assert.equal(shouldRefreshUnfilteredDisplayFromCatalog(0, false), false);
  });
});

describe("shouldSyncDisplayFromCatalog", () => {
  it("syncs when the grid is empty but the warm catalog has rows", () => {
    assert.equal(shouldSyncDisplayFromCatalog(0, 12, false), true);
  });

  it("does not sync filtered lists or non-empty grids", () => {
    assert.equal(shouldSyncDisplayFromCatalog(0, 12, true), false);
    assert.equal(shouldSyncDisplayFromCatalog(5, 12, false), false);
    assert.equal(shouldSyncDisplayFromCatalog(0, 0, false), false);
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
