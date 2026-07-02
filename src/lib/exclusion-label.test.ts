import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  alreadyExcludedFromHistoryLabel,
  alreadyExcludedFromHistoryNotice,
  appNotFoundNotice,
  excludeFromClipboardHistoryAriaLabel,
  excludeFromHistoryLabel,
  excludableCandidateMetaLabel,
  invokeErrorMessage,
  isAppNotFoundError,
} from "./exclusion-label.ts";

describe("excludeFromHistoryLabel", () => {
  it("capitalizes the app name in visible copy", () => {
    assert.equal(excludeFromHistoryLabel("safari"), "Exclude Safari from history");
    assert.equal(excludeFromHistoryLabel("  notes  "), "Exclude Notes from history");
  });
});

describe("excludableCandidateMetaLabel", () => {
  it("distinguishes frontmost vs remembered sources", () => {
    assert.equal(excludableCandidateMetaLabel("frontmost"), "Active app");
    assert.equal(excludableCandidateMetaLabel("remembered"), "Recent app");
  });
});

describe("aria labels", () => {
  it("uses clipboard history context in full labels", () => {
    assert.equal(
      excludeFromClipboardHistoryAriaLabel("chrome"),
      "Exclude Chrome from clipboard history",
    );
  });
});

describe("status notices", () => {
  it("formats already-excluded copy consistently", () => {
    assert.equal(alreadyExcludedFromHistoryLabel("mail"), "Mail excluded from history");
    assert.equal(alreadyExcludedFromHistoryNotice("mail"), "Mail is already excluded");
  });
});

describe("invokeErrorMessage", () => {
  it("extracts messages from common invoke error shapes", () => {
    assert.equal(invokeErrorMessage("app_not_found: Safari"), "app_not_found: Safari");
    assert.equal(invokeErrorMessage(new Error("boom")), "boom");
    assert.equal(invokeErrorMessage({ message: "nested" }), "nested");
    assert.equal(invokeErrorMessage({ error: "legacy" }), "legacy");
    assert.equal(invokeErrorMessage(null), "");
  });
});

describe("isAppNotFoundError", () => {
  it("detects app_not_found invoke failures", () => {
    assert.equal(isAppNotFoundError("app_not_found: Foo"), true);
    assert.equal(isAppNotFoundError(new Error("app_not_found: Foo")), true);
    assert.equal(isAppNotFoundError(new Error("permission denied")), false);
  });
});

describe("appNotFoundNotice", () => {
  it("includes the entered app name", () => {
    assert.match(appNotFoundNotice("Safari"), /Safari/);
    assert.match(appNotFoundNotice("Safari"), /Choose Application/);
  });
});
