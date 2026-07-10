/**
 * Data URL for a base64-encoded image payload — thumbnail or full-size, card or Quick Look.
 * GIF bytes (thumb or full) are stored raw so `<img>` plays the animation natively;
 * everything else is stored as PNG. Magic-byte prefix must stay aligned with
 * `src-tauri/src/image_format.rs` (`detect_from_b64`).
 */
export function imageDataUrl(b64: string): string {
  const mime = b64.startsWith("R0lGOD") ? "image/gif" : "image/png";
  return `data:${mime};base64,${b64}`;
}

/** Recognized text shown under an image thumbnail on overlay cards. Empty when not applicable. */
export function imageOcrPreviewText(
  contentType: string,
  ocrText: string | null | undefined,
): string {
  if (contentType !== "image") return "";
  const trimmed = ocrText?.trim();
  return trimmed ?? "";
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 10 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function formatImageDimensions(
  width: number | null | undefined,
  height: number | null | undefined,
): string | null {
  if (width != null && height != null && width > 0 && height > 0) {
    return `${width.toLocaleString()} × ${height.toLocaleString()}`;
  }
  return null;
}

/** Compact footer line for image cards: `1,920 × 1,080 · 1.2 MB` (format lives in the header). */
export type ImageFooterMetaParts = {
  dimensions: string | null;
  byteSize: string | null;
};

export function resolveImageFooterMetaParts(
  width: number | null | undefined,
  height: number | null | undefined,
  byteSize: number | null | undefined,
): ImageFooterMetaParts | null {
  const dimensions = formatImageDimensions(width, height);
  const size = byteSize != null && byteSize > 0 ? formatBytes(byteSize) : null;
  if (!dimensions && !size) return null;
  return { dimensions, byteSize: size };
}

export function formatImageFooterLabel(
  width: number | null | undefined,
  height: number | null | undefined,
  byteSize: number | null | undefined,
): string | null {
  const parts = resolveImageFooterMetaParts(width, height, byteSize);
  if (!parts) return null;
  const labels = [parts.dimensions, parts.byteSize].filter(
    (value): value is string => value != null,
  );
  return labels.length > 0 ? labels.join(" · ") : null;
}

/** Header badge label: PNG, GIF, JPG, … */
export function formatImageFormatBadge(format: string | null | undefined): string | null {
  const normalized = format?.trim();
  if (!normalized) return null;
  return normalized.toUpperCase() === "JPEG" ? "JPG" : normalized.toUpperCase();
}

/** Resolve format badge from stored meta or thumbnail magic bytes.
 *  Magic-byte prefixes must stay aligned with `src-tauri/src/image_format.rs` (`detect_from_b64`). */
export function resolveImageFormatBadge(
  format: string | null | undefined,
  thumbB64: string | null | undefined,
): string | null {
  const fromMeta = formatImageFormatBadge(format);
  if (fromMeta) return fromMeta;
  if (!thumbB64) return null;
  if (thumbB64.startsWith("R0lGOD")) return "GIF";
  if (thumbB64.startsWith("/9j/")) return "JPG";
  return "PNG";
}
