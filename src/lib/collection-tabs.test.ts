import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  isCollectionsScrollable,
  isCustomCollectionActive,
  isHistoryCollectionActive,
} from "./collection-tabs.ts";

describe("isCollectionsScrollable", () => {
  it("scrolls when collections exist", () => {
    assert.equal(isCollectionsScrollable(2, false), true);
  });

  it("scrolls while the add form is open with no collections", () => {
    assert.equal(isCollectionsScrollable(0, true), true);
  });

  it("does not scroll with only the add button and no collections", () => {
    assert.equal(isCollectionsScrollable(0, false), false);
  });
});

describe("isHistoryCollectionActive", () => {
  it("is true only for default history scope", () => {
    assert.equal(isHistoryCollectionActive(null, false), true);
    assert.equal(isHistoryCollectionActive(null, true), false);
    assert.equal(isHistoryCollectionActive(3, false), false);
  });
});

describe("isCustomCollectionActive", () => {
  it("is true when the collection is selected and Starred is off", () => {
    assert.equal(isCustomCollectionActive(3, 3, false), true);
    assert.equal(isCustomCollectionActive(3, 3, true), false);
    assert.equal(isCustomCollectionActive(3, null, false), false);
  });
});
