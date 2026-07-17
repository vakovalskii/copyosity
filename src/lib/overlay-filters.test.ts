import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { ClipboardEntry } from "$lib/types";

import {
  reconcileOverlayFilters,
  reconcileOverlayFilterState,
  catalogHasHistory,
  buildTagBarModel,
  entryMatchesTag,
  isSemanticTagUiEnabled,
  type ReconcileOverlayFiltersOptions,
} from "./overlay-filters.ts";

/** Minimal, type-correct ClipboardEntry factory for tests. */
function makeEntry(id: number): ClipboardEntry {
  return {
    id,
    content_type: "text",
    text_content: `entry-${id}`,
    image_data: null,
    image_thumb: null,
    source_app: null,
    source_app_icon: null,
    content_hash: `hash-${id}`,
    char_count: null,
    created_at: "2026-01-01T00:00:00Z",
    is_pinned: false,
    collection_id: null,
    tags: [],
  };
}

const baseOptions: ReconcileOverlayFiltersOptions = {
  catalogHasHistory: true,
  filteredEntries: [],
  activeTag: null,
  contentKind: "all",
  kindFilterActive: false,
};

describe("catalogHasHistory", () => {
  it("returns true when the catalog page has entries", () => {
    assert.equal(catalogHasHistory([makeEntry(1)], null), true);
  });

  it("returns true when tag counts show history on an empty first page", () => {
    assert.equal(
      catalogHasHistory([], {
        semantic: [{ tag: "api", count: 3 }],
        format: [],
        has_text: true,
        has_images: false,
      }),
      true,
    );
  });

  it("returns false when catalog and counts are empty", () => {
    assert.equal(
      catalogHasHistory([], { semantic: [], format: [], has_text: false, has_images: false }),
      false,
    );
  });
});

describe("reconcileOverlayFilters", () => {
  it("returns null when there is no catalog history", () => {
    assert.equal(
      reconcileOverlayFilters({ ...baseOptions, catalogHasHistory: false, hasMore: false }),
      null,
    );
  });

  it("returns null when filtered entries already exist", () => {
    assert.equal(
      reconcileOverlayFilters({
        ...baseOptions,
        filteredEntries: [makeEntry(2)],
        activeTag: "api",
        hasMore: false,
      }),
      null,
    );
  });

  it("keeps active tag when more unfiltered pages may contain matches", () => {
    assert.equal(
      reconcileOverlayFilters({ ...baseOptions, activeTag: "api", hasMore: true }),
      null,
    );
  });

  it("clears active tag once all pages are loaded and still no matches", () => {
    // Server-side filtering also passes hasMore: false when page 0 is empty.
    assert.deepEqual(
      reconcileOverlayFilters({ ...baseOptions, activeTag: "api", hasMore: false }),
      { activeTag: null, contentKind: "all" },
    );
  });

  it("clears the content-kind filter when it leaves the grid empty", () => {
    assert.deepEqual(
      reconcileOverlayFilters({
        ...baseOptions,
        contentKind: "image",
        kindFilterActive: true,
        hasMore: false,
      }),
      { activeTag: null, contentKind: "all" },
    );
  });

  it("does not clear the content-kind filter when kindFilterActive is false", () => {
    assert.equal(
      reconcileOverlayFilters({
        ...baseOptions,
        contentKind: "image",
        kindFilterActive: false,
        hasMore: false,
      }),
      null,
    );
  });

  it("does not clear filters when search is active and the grid is empty", () => {
    assert.equal(
      reconcileOverlayFilters({
        ...baseOptions,
        activeTag: "api",
        searchQuery: "nomatch",
        hasMore: false,
      }),
      null,
    );
  });

  it("treats a whitespace-only search as inactive", () => {
    assert.deepEqual(
      reconcileOverlayFilters({
        ...baseOptions,
        activeTag: "api",
        searchQuery: "   ",
        hasMore: false,
      }),
      { activeTag: null, contentKind: "all" },
    );
  });

  it("leaves content-kind untouched when it is already 'all'", () => {
    assert.equal(
      reconcileOverlayFilters({
        ...baseOptions,
        contentKind: "all",
        kindFilterActive: true,
        hasMore: false,
      }),
      null,
    );
  });
});

describe("reconcileOverlayFilterState", () => {
  it("returns null while the overlay is revealing", () => {
    assert.equal(
      reconcileOverlayFilterState({
        isRevealing: true,
        showContentKindRow: false,
        contentKind: "text",
        activeTag: null,
        catalogEntries: [makeEntry(1)],
        catalogTagCounts: null,
        displayEntries: [],
        hasMore: false,
      }),
      null,
    );
  });

  it("resets content kind when Row A is hidden in one adjustment", () => {
    assert.deepEqual(
      reconcileOverlayFilterState({
        isRevealing: false,
        showContentKindRow: false,
        contentKind: "image",
        activeTag: null,
        catalogEntries: [makeEntry(1)],
        catalogTagCounts: null,
        displayEntries: [makeEntry(1)],
        hasMore: false,
      }),
      {
        contentKind: "all",
        activeTag: null,
        clearContentKindSession: true,
        needsReload: true,
      },
    );
  });

  it("merges kind reset and stale tag clear without duplicate reload flags", () => {
    assert.deepEqual(
      reconcileOverlayFilterState({
        isRevealing: false,
        showContentKindRow: false,
        contentKind: "text",
        activeTag: "api",
        catalogEntries: [makeEntry(1)],
        catalogTagCounts: null,
        displayEntries: [],
        hasMore: false,
      }),
      {
        contentKind: "all",
        activeTag: null,
        clearContentKindSession: true,
        needsReload: true,
      },
    );
  });

  it("clears stale tag when tag counts show history but the filtered page is empty", () => {
    assert.deepEqual(
      reconcileOverlayFilterState({
        isRevealing: false,
        showContentKindRow: true,
        contentKind: "all",
        activeTag: "api",
        catalogEntries: [],
        catalogTagCounts: {
          semantic: [{ tag: "api", count: 2 }],
          format: [],
          has_text: true,
          has_images: false,
        },
        displayEntries: [],
        hasMore: false,
      }),
      {
        contentKind: "all",
        activeTag: null,
        clearContentKindSession: false,
        needsReload: true,
      },
    );
  });

  it("clears only the tag first when kind segment may still have matches", () => {
    assert.deepEqual(
      reconcileOverlayFilterState({
        isRevealing: false,
        showContentKindRow: true,
        contentKind: "image",
        activeTag: "png",
        catalogEntries: [],
        catalogTagCounts: {
          semantic: [],
          format: [{ tag: "gif", count: 1 }],
          has_text: false,
          has_images: true,
        },
        displayEntries: [],
        hasMore: false,
      }),
      {
        contentKind: "image",
        activeTag: null,
        clearContentKindSession: false,
        needsReload: true,
      },
    );
  });

  it("returns null when filters are already consistent", () => {
    assert.equal(
      reconcileOverlayFilterState({
        isRevealing: false,
        showContentKindRow: true,
        contentKind: "all",
        activeTag: null,
        catalogEntries: [makeEntry(1)],
        catalogTagCounts: null,
        displayEntries: [makeEntry(1)],
        hasMore: true,
      }),
      null,
    );
  });

  it("keeps sticky filters when an active search miss leaves the grid empty", () => {
    assert.equal(
      reconcileOverlayFilterState({
        isRevealing: false,
        showContentKindRow: true,
        contentKind: "all",
        activeTag: "api",
        catalogEntries: [makeEntry(1)],
        catalogTagCounts: {
          semantic: [{ tag: "api", count: 1 }],
          format: [],
          has_text: true,
          has_images: false,
        },
        displayEntries: [],
        hasMore: false,
        searchQuery: "nomatch",
      }),
      null,
    );
  });

  it("treats whitespace-only search as inactive when reconciling", () => {
    assert.deepEqual(
      reconcileOverlayFilterState({
        isRevealing: false,
        showContentKindRow: true,
        contentKind: "all",
        activeTag: "api",
        catalogEntries: [makeEntry(1)],
        catalogTagCounts: {
          semantic: [{ tag: "api", count: 1 }],
          format: [],
          has_text: true,
          has_images: false,
        },
        displayEntries: [],
        hasMore: false,
        searchQuery: "   ",
      }),
      {
        contentKind: "all",
        activeTag: null,
        clearContentKindSession: false,
        needsReload: true,
      },
    );
  });
});

describe("buildTagBarModel with server tag counts", () => {
  it("uses DB-wide counts instead of the loaded entry page", () => {
    const model = buildTagBarModel({
      entries: [makeEntry(1)],
      contentKind: "all",
      aiTaggingEnabled: true,
      displayTagCounts: {
        semantic: [{ tag: "python", count: 125 }],
        format: [{ tag: "png", count: 24 }],
        has_text: true,
        has_images: true,
      },
      layoutTagCounts: {
        semantic: [{ tag: "python", count: 125 }],
        format: [{ tag: "png", count: 24 }],
        has_text: true,
        has_images: true,
      },
    });

    assert.deepEqual(model.semanticChips, [["python", 125]]);
    assert.deepEqual(model.formatChips, [["png", 24]]);
  });

  it("sorts format chips by count descending", () => {
    const model = buildTagBarModel({
      entries: [makeImageEntry(1, "PNG")],
      contentKind: "all",
      aiTaggingEnabled: false,
      displayTagCounts: {
        semantic: [],
        format: [
          { tag: "gif", count: 87 },
          { tag: "jpg", count: 5 },
          { tag: "png", count: 256 },
        ],
        has_text: false,
        has_images: true,
      },
      layoutTagCounts: {
        semantic: [],
        format: [
          { tag: "gif", count: 87 },
          { tag: "jpg", count: 5 },
          { tag: "png", count: 256 },
        ],
        has_text: false,
        has_images: true,
      },
    });

    assert.deepEqual(model.formatChips, [
      ["png", 256],
      ["gif", 87],
      ["jpg", 5],
    ]);
  });

  it("merges semantic chips that share a UI label", () => {
    const model = buildTagBarModel({
      entries: [makeEntry(1)],
      contentKind: "all",
      aiTaggingEnabled: true,
      displayTagCounts: {
        semantic: [
          { tag: "javascript", count: 5 },
          { tag: "js", count: 3 },
        ],
        format: [],
        has_text: true,
        has_images: false,
      },
      layoutTagCounts: {
        semantic: [
          { tag: "javascript", count: 5 },
          { tag: "js", count: 3 },
        ],
        format: [],
        has_text: true,
        has_images: false,
      },
    });

    assert.deepEqual(model.semanticChips, [["js", 8]]);
  });

  it("hides the tag row when search returns no matches", () => {
    const model = buildTagBarModel({
      entries: [],
      layoutEntries: [makeEntry(1), makeImageEntry(2, "png")],
      contentKind: "all",
      aiTaggingEnabled: true,
      searchQuery: "missing",
      searchPending: false,
      displayTagCounts: {
        semantic: [],
        format: [],
        has_text: false,
        has_images: false,
      },
      layoutTagCounts: {
        semantic: [{ tag: "api", count: 3 }],
        format: [{ tag: "png", count: 2 }],
        has_text: true,
        has_images: true,
      },
    });

    assert.equal(model.showRowB, false);
  });

  it("toggles tag row visibility with search pending state", () => {
    const base = {
      entries: [] as ReturnType<typeof makeEntry>[],
      layoutEntries: [makeEntry(1)],
      contentKind: "all" as const,
      aiTaggingEnabled: true,
      searchQuery: "api",
      layoutTagCounts: {
        semantic: [{ tag: "api", count: 1 }],
        format: [],
        has_text: true,
        has_images: false,
      },
    };

    assert.equal(buildTagBarModel({ ...base, searchPending: false }).showRowB, false);
    assert.equal(buildTagBarModel({ ...base, searchPending: true }).showRowB, true);
  });

  it("keeps the tag row when search is empty but a tag filter is active", () => {
    const model = buildTagBarModel({
      entries: [],
      layoutEntries: [makeEntry(1)],
      contentKind: "all",
      aiTaggingEnabled: true,
      activeTag: "api",
      searchQuery: "missing",
      searchPending: false,
      displayTagCounts: {
        semantic: [],
        format: [],
        has_text: false,
        has_images: false,
      },
      layoutTagCounts: {
        semantic: [{ tag: "api", count: 3 }],
        format: [],
        has_text: true,
        has_images: false,
      },
    });

    assert.equal(model.showRowB, true);
  });

  it("does not hide the tag row for whitespace-only search queries", () => {
    const model = buildTagBarModel({
      entries: [],
      layoutEntries: [makeEntry(1)],
      contentKind: "all",
      aiTaggingEnabled: true,
      searchQuery: "   ",
      searchPending: false,
      layoutTagCounts: {
        semantic: [{ tag: "api", count: 1 }],
        format: [],
        has_text: true,
        has_images: false,
      },
    });

    assert.equal(model.showRowB, true);
  });
});

function makeImageEntry(id: number, formatTag: string): ClipboardEntry {
  return {
    ...makeEntry(id),
    content_type: "image",
    text_content: null,
    tags: [formatTag],
  };
}

describe("entryMatchesTag", () => {
  it("matches format tags from entry.tags", () => {
    const entry = makeImageEntry(1, "jpg");
    assert.equal(entryMatchesTag(entry, "jpg"), true);
    assert.equal(entryMatchesTag(entry, "png"), false);
  });

  it("matches format tags from image_format when tags omit format", () => {
    const entry = {
      ...makeImageEntry(5, "jpg"),
      tags: ["screenshot"],
      image_format: "JPG",
    };
    assert.equal(entryMatchesTag(entry, "jpg"), true);
    assert.equal(entryMatchesTag(entry, "png"), false);
  });

  it("matches semantic tags from entry.tags", () => {
    const entry = { ...makeEntry(2), tags: ["api"] };
    assert.equal(entryMatchesTag(entry, "api"), true);
  });

  it("matches display labels against synonym DB tags", () => {
    const jsEntry = { ...makeEntry(3), tags: ["js"] };
    const fullEntry = { ...makeEntry(4), tags: ["javascript"] };
    assert.equal(entryMatchesTag(jsEntry, "js"), true);
    assert.equal(entryMatchesTag(fullEntry, "js"), true);
    assert.equal(entryMatchesTag(fullEntry, "javascript"), true);
  });
});

describe("isSemanticTagUiEnabled", () => {
  it("is true for hub tagging when local AI is off", () => {
    assert.equal(
      isSemanticTagUiEnabled({
        ai_tagging_enabled: false,
        hub_enabled: true,
        hub_tagging_enabled: true,
        hub_token: "sk-test",
      }),
      true,
    );
  });

  it("is false when hub master switch is off", () => {
    assert.equal(
      isSemanticTagUiEnabled({
        ai_tagging_enabled: false,
        hub_enabled: false,
        hub_tagging_enabled: true,
        hub_token: "sk-test",
      }),
      false,
    );
  });
});
