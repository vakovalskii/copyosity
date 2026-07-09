import assert from "node:assert/strict";
import { describe, it, beforeEach, afterEach } from "node:test";

import {
  clearContentKindSession,
  readContentKindSession,
  writeContentKindSession,
} from "./overlay-content-kind-session.ts";

describe("overlay content-kind session persistence", () => {
  const store = new Map<string, string>();
  const original = globalThis.sessionStorage;

  beforeEach(() => {
    store.clear();
    globalThis.sessionStorage = {
      getItem: (k) => store.get(k) ?? null,
      setItem: (k, v) => {
        store.set(k, v);
      },
      removeItem: (k) => {
        store.delete(k);
      },
      clear: () => store.clear(),
      key: () => null,
      length: store.size,
    } as Storage;
  });

  afterEach(() => {
    globalThis.sessionStorage = original;
  });

  it("returns null when nothing is stored", () => {
    assert.equal(readContentKindSession(), null);
  });

  it("round-trips text and image kinds", () => {
    writeContentKindSession("text");
    assert.equal(readContentKindSession(), "text");
    writeContentKindSession("image");
    assert.equal(readContentKindSession(), "image");
  });

  it("does not persist the default 'all' kind", () => {
    writeContentKindSession("image");
    writeContentKindSession("all");
    assert.equal(readContentKindSession(), null);
  });

  it("clears an existing selection", () => {
    writeContentKindSession("text");
    clearContentKindSession();
    assert.equal(readContentKindSession(), null);
  });

  it("ignores unknown stored values", () => {
    store.set("copyosity.overlay.contentKind", "bogus");
    assert.equal(readContentKindSession(), null);
  });
});
