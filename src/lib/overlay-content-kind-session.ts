import type { ContentKind } from "$lib/overlay-filters";

const CONTENT_KIND_SESSION_KEY = "copyosity.overlay.contentKind";

export function readContentKindSession(): ContentKind | null {
  if (typeof sessionStorage === "undefined") return null;
  const raw = sessionStorage.getItem(CONTENT_KIND_SESSION_KEY);
  if (raw === "all" || raw === "text" || raw === "image") return raw;
  return null;
}

export function writeContentKindSession(kind: ContentKind) {
  if (typeof sessionStorage === "undefined") return;
  if (kind === "all") {
    sessionStorage.removeItem(CONTENT_KIND_SESSION_KEY);
    return;
  }
  sessionStorage.setItem(CONTENT_KIND_SESSION_KEY, kind);
}

export function clearContentKindSession() {
  if (typeof sessionStorage === "undefined") return;
  sessionStorage.removeItem(CONTENT_KIND_SESSION_KEY);
}
