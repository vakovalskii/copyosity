import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { isSelectAllKey } from "./text-field-shortcuts.ts";

describe("isSelectAllKey", () => {
  it("matches Cmd/Ctrl+A without Alt", () => {
    assert.equal(isSelectAllKey({ metaKey: true, ctrlKey: false, altKey: false, key: "a" }), true);
    assert.equal(isSelectAllKey({ metaKey: false, ctrlKey: true, altKey: false, key: "A" }), true);
  });

  it("rejects plain A and Alt+A", () => {
    assert.equal(
      isSelectAllKey({ metaKey: false, ctrlKey: false, altKey: false, key: "a" }),
      false,
    );
    assert.equal(isSelectAllKey({ metaKey: true, ctrlKey: false, altKey: true, key: "a" }), false);
  });
});
