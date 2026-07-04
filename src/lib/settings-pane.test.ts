import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  coerceSettingsPane,
  parseSettingsPaneFromQuery,
  resolveSettingsPaneUpdate,
  SETTINGS_PANE_IDS,
} from "./settings-pane.ts";

describe("parseSettingsPaneFromQuery", () => {
  it("returns null for an empty query", () => {
    assert.equal(parseSettingsPaneFromQuery(""), null);
  });

  it("parses each valid pane id", () => {
    for (const pane of SETTINGS_PANE_IDS) {
      assert.equal(parseSettingsPaneFromQuery(`?pane=${pane}`), pane);
    }
  });

  it("rejects unknown, empty, and case-mismatched panes", () => {
    assert.equal(parseSettingsPaneFromQuery("?pane=invalid"), null);
    assert.equal(parseSettingsPaneFromQuery("?pane="), null);
    assert.equal(parseSettingsPaneFromQuery("?pane=QuickMenu"), null);
  });

  it("ignores unrelated query params", () => {
    assert.equal(parseSettingsPaneFromQuery("?pane=voice&foo=bar"), "voice");
  });
});

describe("resolveSettingsPaneUpdate", () => {
  it("switches to a valid pane", () => {
    assert.equal(resolveSettingsPaneUpdate("hub", "quickmenu"), "quickmenu");
  });

  it("keeps the current pane for missing or invalid values", () => {
    assert.equal(resolveSettingsPaneUpdate("hub", null), "hub");
    assert.equal(resolveSettingsPaneUpdate("hub", undefined), "hub");
    assert.equal(resolveSettingsPaneUpdate("hub", ""), "hub");
    assert.equal(resolveSettingsPaneUpdate("hub", "not-a-pane"), "hub");
  });
});

describe("coerceSettingsPane", () => {
  it("defaults missing values to hub", () => {
    assert.equal(coerceSettingsPane(null), "hub");
    assert.equal(coerceSettingsPane(undefined), "hub");
  });

  it("returns a provided pane", () => {
    assert.equal(coerceSettingsPane("quickmenu"), "quickmenu");
  });

  it("uses a custom fallback", () => {
    assert.equal(coerceSettingsPane(null, "voice"), "voice");
  });
});
