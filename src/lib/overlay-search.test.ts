import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { overlayEscapeAction } from "./overlay-search.ts";

describe("overlayEscapeAction", () => {
  it("clears the search first when a query is present", () => {
    assert.equal(overlayEscapeAction(true), "clear-search");
  });

  it("dismisses the overlay when there is no query", () => {
    assert.equal(overlayEscapeAction(false), "dismiss");
  });
});
