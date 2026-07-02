import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { displayQueryKey, tagCountsQueryKey } from "./overlay-display-query.ts";

describe("displayQueryKey", () => {
  const base = {
    searchQuery: "",
    activeTag: null as string | null,
    contentKind: "all" as const,
    showContentKindRow: true,
    collectionId: null as number | null,
    pinnedOnly: false,
  };

  it("changes when search, tag, or kind filter changes", () => {
    const baseline = displayQueryKey(base);
    assert.notEqual(displayQueryKey({ ...base, searchQuery: "api" }), baseline);
    assert.notEqual(displayQueryKey({ ...base, activeTag: "python" }), baseline);
    assert.notEqual(
      displayQueryKey({ ...base, contentKind: "text", showContentKindRow: true }),
      baseline,
    );
  });

  it("ignores content kind when the kind row is hidden", () => {
    const hiddenRow = displayQueryKey({
      ...base,
      contentKind: "image",
      showContentKindRow: false,
    });
    const allKind = displayQueryKey({
      ...base,
      contentKind: "all",
      showContentKindRow: false,
    });
    assert.equal(hiddenRow, allKind);
    assert.notEqual(
      displayQueryKey({ ...base, contentKind: "image", showContentKindRow: true }),
      hiddenRow,
    );
  });

  it("includes collection and pinned scope", () => {
    assert.notEqual(
      displayQueryKey({ ...base, collectionId: 3 }),
      displayQueryKey({ ...base, collectionId: null }),
    );
    assert.notEqual(
      displayQueryKey({ ...base, pinnedOnly: true }),
      displayQueryKey({ ...base, pinnedOnly: false }),
    );
  });
});

describe("tagCountsQueryKey", () => {
  const base = {
    collectionId: null as number | null,
    pinnedOnly: false,
    searchQuery: "",
  };

  it("ignores active tag and content kind (counts are scope-only)", () => {
    const key = tagCountsQueryKey({ ...base, searchQuery: "api" });
    assert.equal(key, tagCountsQueryKey({ ...base, searchQuery: "api" }));
    assert.notEqual(key, tagCountsQueryKey({ ...base, searchQuery: "" }));
  });
});
