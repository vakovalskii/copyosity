import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { shouldLoadNextEntryPage } from "./overlay-pagination.ts";

describe("shouldLoadNextEntryPage", () => {
  it("loads the next page near the right edge when more entries are available", () => {
    assert.equal(
      shouldLoadNextEntryPage({
        scrollLeft: 690,
        clientWidth: 300,
        scrollWidth: 1_200,
        hasMore: true,
        loading: false,
      }),
      true,
    );
  });

  it("does not load while already loading or after the last page", () => {
    const base = {
      scrollLeft: 690,
      clientWidth: 300,
      scrollWidth: 1_200,
    };

    assert.equal(shouldLoadNextEntryPage({ ...base, hasMore: true, loading: true }), false);
    assert.equal(shouldLoadNextEntryPage({ ...base, hasMore: false, loading: false }), false);
  });

  it("does not load when the scroll position is still away from the edge", () => {
    assert.equal(
      shouldLoadNextEntryPage({
        scrollLeft: 300,
        clientWidth: 300,
        scrollWidth: 1_200,
        hasMore: true,
        loading: false,
      }),
      false,
    );
  });

  it("loads when content fits without scrolling (distance to end is zero)", () => {
    assert.equal(
      shouldLoadNextEntryPage({
        scrollLeft: 0,
        clientWidth: 600,
        scrollWidth: 600,
        hasMore: true,
        loading: false,
      }),
      true,
    );
  });

  it("respects a custom prefetch distance", () => {
    assert.equal(
      shouldLoadNextEntryPage({
        scrollLeft: 500,
        clientWidth: 300,
        scrollWidth: 1_200,
        hasMore: true,
        loading: false,
        prefetchPx: 500,
      }),
      true,
    );
    assert.equal(
      shouldLoadNextEntryPage({
        scrollLeft: 300,
        clientWidth: 300,
        scrollWidth: 1_200,
        hasMore: true,
        loading: false,
        prefetchPx: 100,
      }),
      false,
    );
  });
});
