import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { isEntryTaggedPayload, parseEntryTaggedEvent } from "./types.ts";

describe("isEntryTaggedPayload", () => {
  it("accepts a valid entry-tagged payload", () => {
    assert.equal(isEntryTaggedPayload({ entryId: 42, tags: ["api", "rust"] }), true);
  });

  it("rejects missing or malformed fields", () => {
    assert.equal(isEntryTaggedPayload(null), false);
    assert.equal(isEntryTaggedPayload({ entryId: "42", tags: [] }), false);
    assert.equal(isEntryTaggedPayload({ entryId: 1, tags: ["ok", 2] }), false);
    assert.equal(isEntryTaggedPayload({ tags: ["api"] }), false);
  });
});

describe("parseEntryTaggedEvent", () => {
  it("parses the current payload shape", () => {
    assert.deepEqual(parseEntryTaggedEvent({ entryId: 7, tags: ["a"] }), {
      kind: "payload",
      payload: { entryId: 7, tags: ["a"] },
    });
  });

  it("accepts legacy bare entry id payloads", () => {
    assert.deepEqual(parseEntryTaggedEvent(99), { kind: "legacy-id", entryId: 99 });
  });

  it("rejects invalid legacy ids", () => {
    assert.equal(parseEntryTaggedEvent(0), null);
    assert.equal(parseEntryTaggedEvent(-1), null);
    assert.equal(parseEntryTaggedEvent(1.5), null);
  });
});
