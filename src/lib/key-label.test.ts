import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { formatKeys } from "./key-label.ts";

describe("formatKeys", () => {
  it("keeps macOS glyphs on macOS and before the platform is known", () => {
    assert.equal(formatKeys("⌘⇧Space", "macos"), "⌘⇧Space");
    assert.equal(formatKeys("⌘Y", null), "⌘Y");
  });

  it("spells modifiers out on Windows", () => {
    assert.equal(formatKeys("⌘⇧Space", "windows"), "Ctrl+Shift+Space");
    assert.equal(formatKeys("⌘Y", "windows"), "Ctrl+Y");
    assert.equal(formatKeys("⌘↵", "windows"), "Ctrl+↵");
    assert.equal(formatKeys("⇧↵", "windows"), "Shift+↵");
    assert.equal(formatKeys("⌥Space", "windows"), "Alt+Space");
  });

  it("does not repeat Ctrl when both ⌘ and ⌃ are present", () => {
    assert.equal(formatKeys("⌃⌘V", "windows"), "Ctrl+V");
  });

  it("leaves labels without modifiers alone", () => {
    assert.equal(formatKeys("↵", "windows"), "↵");
    assert.equal(formatKeys("Esc", "windows"), "Esc");
  });
});
