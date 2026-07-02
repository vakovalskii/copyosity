import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  formatBytes,
  formatImageDimensions,
  formatImageFooterLabel,
  formatImageFormatBadge,
  imageOcrPreviewText,
  resolveImageFooterMetaParts,
  resolveImageFormatBadge,
} from "./image-meta.ts";

describe("imageOcrPreviewText", () => {
  it("returns trimmed OCR for image entries", () => {
    assert.equal(imageOcrPreviewText("image", "  Hello world  "), "Hello world");
  });

  it("returns empty for non-image content", () => {
    assert.equal(imageOcrPreviewText("text", "Hello"), "");
    assert.equal(imageOcrPreviewText("file", "Hello"), "");
  });

  it("returns empty when OCR is missing or whitespace-only", () => {
    assert.equal(imageOcrPreviewText("image", null), "");
    assert.equal(imageOcrPreviewText("image", undefined), "");
    assert.equal(imageOcrPreviewText("image", "   "), "");
    assert.equal(imageOcrPreviewText("image", ""), "");
  });
});

describe("formatBytes", () => {
  it("formats byte ranges", () => {
    assert.equal(formatBytes(512), "512 B");
    assert.equal(formatBytes(1023), "1023 B");
    assert.equal(formatBytes(1024), "1.0 KB");
    assert.equal(formatBytes(1536), "1.5 KB");
    assert.equal(formatBytes(10 * 1024 - 1), "10.0 KB");
    assert.equal(formatBytes(10 * 1024), "10 KB");
    assert.equal(formatBytes(1024 * 1024), "1.0 MB");
    assert.equal(formatBytes(1_572_864), "1.5 MB");
  });
});

describe("formatImageDimensions", () => {
  it("formats positive dimensions with locale grouping", () => {
    const formatted = formatImageDimensions(1920, 1080);
    assert.ok(formatted);
    const [width, height] = formatted!.split(" × ");
    assert.equal(Number(width.replaceAll(",", "")), 1920);
    assert.equal(Number(height.replaceAll(",", "")), 1080);
  });

  it("returns null for missing values", () => {
    assert.equal(formatImageDimensions(null, 1080), null);
    assert.equal(formatImageDimensions(0, 0), null);
  });
});

describe("formatImageFormatBadge", () => {
  it("normalizes JPEG to JPG", () => {
    assert.equal(formatImageFormatBadge("jpeg"), "JPG");
    assert.equal(formatImageFormatBadge("PNG"), "PNG");
  });
});

describe("resolveImageFooterMetaParts", () => {
  it("returns dimensions and byte size parts", () => {
    assert.deepEqual(resolveImageFooterMetaParts(1920, 1080, 1_572_864), {
      dimensions: "1,920 × 1,080",
      byteSize: "1.5 MB",
    });
  });

  it("returns partial meta when only one field is present", () => {
    assert.deepEqual(resolveImageFooterMetaParts(null, null, 2048), {
      dimensions: null,
      byteSize: "2.0 KB",
    });
    assert.deepEqual(resolveImageFooterMetaParts(800, 600, null), {
      dimensions: "800 × 600",
      byteSize: null,
    });
  });

  it("returns null when nothing is available", () => {
    assert.equal(resolveImageFooterMetaParts(null, null, null), null);
    assert.equal(resolveImageFooterMetaParts(0, 0, 0), null);
  });
});

describe("resolveImageFormatBadge", () => {
  it("prefers stored format over thumb magic", () => {
    assert.equal(resolveImageFormatBadge("GIF", "iVBORw0KGgo"), "GIF");
  });

  it("detects format from thumbnail bytes", () => {
    assert.equal(resolveImageFormatBadge(null, "R0lGODlh"), "GIF");
    assert.equal(resolveImageFormatBadge(null, "/9j/4AAQ"), "JPG");
    assert.equal(resolveImageFormatBadge(null, "iVBORw0KGgo"), "PNG");
  });

  it("returns null without stored format or thumbnail", () => {
    assert.equal(resolveImageFormatBadge(null, null), null);
    assert.equal(resolveImageFormatBadge("", undefined), null);
  });
});

describe("formatImageFooterLabel", () => {
  it("joins dimensions and size without format", () => {
    assert.equal(formatImageFooterLabel(1920, 1080, 1_572_864), "1,920 × 1,080 · 1.5 MB");
  });

  it("returns null when nothing is available", () => {
    assert.equal(formatImageFooterLabel(null, null, null), null);
  });

  it("supports size only", () => {
    assert.equal(formatImageFooterLabel(null, null, 2048), "2.0 KB");
  });
});
