import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { detectTextKind, usesMonoPreview } from "./text-kind.ts";

describe("detectTextKind", () => {
  it("returns Text for empty input", () => {
    assert.equal(detectTextKind(null), "Text");
    assert.equal(detectTextKind(""), "Text");
  });

  it("detects URLs", () => {
    assert.equal(detectTextKind("https://example.com"), "URL");
    assert.equal(detectTextKind("www.example.com"), "URL");
  });

  it("detects JSON", () => {
    assert.equal(detectTextKind('{"a":1}'), "JSON");
    assert.equal(detectTextKind("[1,2]"), "JSON");
  });

  it("detects shell and bash", () => {
    assert.equal(detectTextKind("#!/bin/bash\necho hi"), "Shell");
    assert.equal(detectTextKind("$ npm install"), "Bash");
  });

  it("detects SQL, HTML, and languages", () => {
    assert.equal(detectTextKind("SELECT * FROM users"), "SQL");
    assert.equal(detectTextKind("<div>hi</div>"), "HTML");
    assert.equal(detectTextKind("const x = 1"), "JavaScript");
    assert.equal(detectTextKind("interface Foo {}"), "TypeScript");
    assert.equal(detectTextKind("def main():\n  pass"), "Python");
    assert.equal(detectTextKind("fn main() {}"), "Rust");
  });

  it("falls back to Text for prose", () => {
    assert.equal(detectTextKind("Hello clipboard"), "Text");
  });
});

describe("usesMonoPreview", () => {
  it("returns true for code-like kinds", () => {
    assert.equal(usesMonoPreview("JSON"), true);
    assert.equal(usesMonoPreview("Rust"), true);
  });

  it("returns false for plain text and URLs", () => {
    assert.equal(usesMonoPreview("Text"), false);
    assert.equal(usesMonoPreview("URL"), false);
  });
});
