import assert from "node:assert/strict";
import { describe, it, beforeEach, afterEach } from "node:test";

import {
  isSnippetFolderExpanded,
  loadCollapsedSnippetFolderIds,
  pruneCollapsedSnippetFolderIds,
  saveCollapsedSnippetFolderIds,
} from "./snippet-folders-ui.ts";

describe("snippet-folders-ui", () => {
  const original = globalThis.localStorage;

  beforeEach(() => {
    const store = new Map<string, string>();
    globalThis.localStorage = {
      getItem: (key) => store.get(key) ?? null,
      setItem: (key, value) => {
        store.set(key, value);
      },
      removeItem: (key) => {
        store.delete(key);
      },
      clear: () => {
        store.clear();
      },
      key: () => null,
      get length() {
        return store.size;
      },
    };
  });

  afterEach(() => {
    globalThis.localStorage = original;
  });

  it("defaults to all folders expanded", () => {
    assert.equal(isSnippetFolderExpanded(1, loadCollapsedSnippetFolderIds()), true);
  });

  it("round-trips collapsed folder ids", () => {
    saveCollapsedSnippetFolderIds(new Set([2, 5]));
    const loaded = loadCollapsedSnippetFolderIds();
    assert.equal(isSnippetFolderExpanded(1, loaded), true);
    assert.equal(isSnippetFolderExpanded(2, loaded), false);
    assert.equal(isSnippetFolderExpanded(5, loaded), false);
  });

  it("returns empty set for corrupt storage", () => {
    localStorage.setItem("snippetFolderCollapsedIds", "not-json");
    assert.equal(loadCollapsedSnippetFolderIds().size, 0);
  });

  it("returns empty set for non-array JSON", () => {
    localStorage.setItem("snippetFolderCollapsedIds", "{}");
    assert.equal(loadCollapsedSnippetFolderIds().size, 0);
  });

  it("keeps only integer folder ids from mixed arrays", () => {
    localStorage.setItem("snippetFolderCollapsedIds", JSON.stringify([1, "x", 2.5, 3]));
    const loaded = loadCollapsedSnippetFolderIds();
    assert.deepEqual(
      [...loaded].toSorted((a, b) => a - b),
      [1, 3],
    );
  });

  it("deduplicates stored folder ids", () => {
    localStorage.setItem("snippetFolderCollapsedIds", JSON.stringify([2, 2, 5]));
    assert.deepEqual(
      [...loadCollapsedSnippetFolderIds()].toSorted((a, b) => a - b),
      [2, 5],
    );
  });

  it("treats an empty stored array as expanded for all folders", () => {
    localStorage.setItem("snippetFolderCollapsedIds", "[]");
    assert.equal(isSnippetFolderExpanded(1, loadCollapsedSnippetFolderIds()), true);
  });
});

describe("pruneCollapsedSnippetFolderIds", () => {
  it("drops collapsed ids for deleted folders", () => {
    assert.deepEqual(
      [...pruneCollapsedSnippetFolderIds(new Set([2, 5]), [1, 2, 3])].toSorted((a, b) => a - b),
      [2],
    );
  });

  it("returns the same ids when all collapsed folders still exist", () => {
    const collapsed = new Set([2, 5]);
    const pruned = pruneCollapsedSnippetFolderIds(collapsed, [2, 5]);
    assert.deepEqual(
      [...pruned].toSorted((a, b) => a - b),
      [2, 5],
    );
  });

  it("clears stale ids when no active folders remain", () => {
    assert.equal(pruneCollapsedSnippetFolderIds(new Set([99]), [1, 2]).size, 0);
    assert.equal(pruneCollapsedSnippetFolderIds(new Set([1]), []).size, 0);
  });

  it("returns an empty set when nothing was collapsed", () => {
    assert.equal(pruneCollapsedSnippetFolderIds(new Set(), [1, 2]).size, 0);
  });
});
