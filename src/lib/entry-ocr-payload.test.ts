import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { isEntryOcrPayload, parseEntryOcrEvent } from "./types.ts";

describe("isEntryOcrPayload", () => {
  it("accepts a valid entry-ocr payload", () => {
    assert.equal(isEntryOcrPayload({ entryId: 42, ocrText: "Hello world" }), true);
  });

  it("rejects missing or malformed fields", () => {
    assert.equal(isEntryOcrPayload(null), false);
    assert.equal(isEntryOcrPayload({ entryId: "42", ocrText: "x" }), false);
    assert.equal(isEntryOcrPayload({ entryId: 1, ocrText: "" }), false);
    assert.equal(isEntryOcrPayload({ ocrText: "x" }), false);
  });
});

describe("parseEntryOcrEvent", () => {
  it("parses the current payload shape", () => {
    assert.deepEqual(parseEntryOcrEvent({ entryId: 7, ocrText: "line one" }), {
      kind: "payload",
      payload: { entryId: 7, ocrText: "line one" },
    });
  });

  it("accepts legacy bare entry id payloads", () => {
    assert.deepEqual(parseEntryOcrEvent(99), { kind: "legacy-id", entryId: 99 });
  });

  it("rejects invalid legacy ids", () => {
    assert.equal(parseEntryOcrEvent(0), null);
    assert.equal(parseEntryOcrEvent(-1), null);
    assert.equal(parseEntryOcrEvent(1.5), null);
  });
});
