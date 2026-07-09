import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { displayQueryKey, tagCountsQueryKey } from "./overlay-display-query.ts";

const base = {
  searchQuery: "",
  activeTag: null as string | null,
  contentKind: "all" as const,
  showContentKindRow: true,
  collectionId: null as number | null,
  pinnedOnly: false,
};

describe("displayQueryKey", () => {
  it("is stable for identical inputs", () => {
    assert.equal(displayQueryKey(base), displayQueryKey({ ...base }));
  });

  it("changes when the search query changes", () => {
    assert.notEqual(displayQueryKey(base), displayQueryKey({ ...base, searchQuery: "hi" }));
  });

  it("changes when the active tag changes", () => {
    assert.notEqual(displayQueryKey(base), displayQueryKey({ ...base, activeTag: "code" }));
  });

  it("changes across collections and pinned scope", () => {
    assert.notEqual(displayQueryKey(base), displayQueryKey({ ...base, collectionId: 3 }));
    assert.notEqual(displayQueryKey(base), displayQueryKey({ ...base, pinnedOnly: true }));
  });

  it("ignores contentKind while the kind row is hidden", () => {
    const hidden = { ...base, showContentKindRow: false };
    assert.equal(
      displayQueryKey({ ...hidden, contentKind: "image" }),
      displayQueryKey({ ...hidden, contentKind: "text" }),
    );
  });

  it("distinguishes contentKind only when the kind row is shown", () => {
    assert.notEqual(
      displayQueryKey({ ...base, contentKind: "image" }),
      displayQueryKey({ ...base, contentKind: "text" }),
    );
  });

  it("treats a non-'all' kind the same as 'all' when the row is hidden", () => {
    assert.equal(
      displayQueryKey({ ...base, showContentKindRow: false, contentKind: "image" }),
      displayQueryKey({ ...base, showContentKindRow: false, contentKind: "all" }),
    );
  });
});

describe("tagCountsQueryKey", () => {
  it("depends on collection, pinned scope, and search only", () => {
    const a = tagCountsQueryKey({ collectionId: null, pinnedOnly: false, searchQuery: "" });
    assert.equal(a, tagCountsQueryKey({ collectionId: null, pinnedOnly: false, searchQuery: "" }));
    assert.notEqual(a, tagCountsQueryKey({ collectionId: 1, pinnedOnly: false, searchQuery: "" }));
    assert.notEqual(
      a,
      tagCountsQueryKey({ collectionId: null, pinnedOnly: true, searchQuery: "" }),
    );
    assert.notEqual(
      a,
      tagCountsQueryKey({ collectionId: null, pinnedOnly: false, searchQuery: "x" }),
    );
  });
});
