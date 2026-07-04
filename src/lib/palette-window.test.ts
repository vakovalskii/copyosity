import assert from "node:assert/strict";
import { describe, it, beforeEach, afterEach } from "node:test";

import {
  isPaletteDotLogicalSize,
  loadPaletteRestoreSize,
  savePaletteRestoreSize,
} from "./palette-window.ts";

describe("isPaletteDotLogicalSize", () => {
  it("matches dot footprint within tolerance", () => {
    assert.equal(isPaletteDotLogicalSize(72, 72), true);
    assert.equal(isPaletteDotLogicalSize(72.4, 72.3), true);
    assert.equal(isPaletteDotLogicalSize(72.6, 72), false);
    assert.equal(isPaletteDotLogicalSize(640, 460), false);
  });
});

describe("palette restore size persistence", () => {
  const key = "paletteRestoreSize";
  const store = new Map<string, string>();
  const original = globalThis.localStorage;

  beforeEach(() => {
    store.clear();
    globalThis.localStorage = {
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
    globalThis.localStorage = original;
  });

  it("returns default when storage is empty", () => {
    assert.deepEqual(loadPaletteRestoreSize(), { w: 640, h: 460 });
  });

  it("round-trips a valid saved size", () => {
    savePaletteRestoreSize({ w: 720, h: 520 });
    assert.deepEqual(loadPaletteRestoreSize(), { w: 720, h: 520 });
  });

  it("rejects sizes below palette minimums", () => {
    savePaletteRestoreSize({ w: 200, h: 100 });
    assert.deepEqual(loadPaletteRestoreSize(), { w: 640, h: 460 });
  });
});
