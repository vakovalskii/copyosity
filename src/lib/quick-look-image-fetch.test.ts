import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  initialQuickLookImageTab,
  resolveFullImagePayload,
  shouldApplyFullImageResult,
} from "./quick-look-image-fetch.ts";

describe("shouldApplyFullImageResult", () => {
  it("accepts matching in-flight requests", () => {
    assert.equal(shouldApplyFullImageResult(2, 2, false), true);
  });

  it("rejects stale or cancelled requests", () => {
    assert.equal(shouldApplyFullImageResult(1, 2, false), false);
    assert.equal(shouldApplyFullImageResult(2, 2, true), false);
  });
});

describe("resolveFullImagePayload", () => {
  it("returns image data when present", () => {
    assert.equal(resolveFullImagePayload("abc123"), "abc123");
  });

  it("returns null when image data is missing", () => {
    assert.equal(resolveFullImagePayload(null), null);
    assert.equal(resolveFullImagePayload(undefined), null);
  });
});

describe("initialQuickLookImageTab", () => {
  it("defaults to the image tab", () => {
    assert.equal(initialQuickLookImageTab(), "image");
  });
});
