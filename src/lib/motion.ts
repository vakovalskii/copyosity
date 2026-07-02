const REDUCED_MOTION_QUERY = "(prefers-reduced-motion: reduce)";
const PANEL_CLOSE_FALLBACK_BUFFER_MS = 64;

function readCssDurationMs(variable: string, fallbackMs: number): number {
  if (typeof document === "undefined") return fallbackMs;
  const raw = getComputedStyle(document.documentElement).getPropertyValue(variable).trim();
  if (!raw) return fallbackMs;
  const value = parseFloat(raw);
  if (Number.isNaN(value)) return fallbackMs;
  if (raw.endsWith("ms")) return value;
  return value * 1000;
}

/** Reads --duration-panel-open from tokens.css (fallback 300ms). */
export function panelOpenMs(): number {
  return readCssDurationMs("--duration-panel-open", 300);
}

/** Max panel close duration + buffer; reads tokens.css close vars. */
export function panelCloseFallbackMs(): number {
  const transform = readCssDurationMs("--duration-panel-close", 180);
  const opacity = readCssDurationMs("--duration-panel-opacity-close", 120);
  return Math.max(transform, opacity) + PANEL_CLOSE_FALLBACK_BUFFER_MS;
}

export function prefersReducedMotion(): boolean {
  if (typeof window === "undefined") return false;
  return window.matchMedia(REDUCED_MOTION_QUERY).matches;
}

/** Launcher pattern: instant scroll when Reduce Motion is on, smooth otherwise. */
export function scrollBehavior(): ScrollBehavior {
  return prefersReducedMotion() ? "auto" : "smooth";
}

/** Subscribe to system Reduce Motion changes; returns unsubscribe. */
export function subscribeReducedMotion(onChange: (reduced: boolean) => void): () => void {
  if (typeof window === "undefined") return () => {};
  const mq = window.matchMedia(REDUCED_MOTION_QUERY);
  const handler = () => onChange(mq.matches);
  mq.addEventListener("change", handler);
  return () => mq.removeEventListener("change", handler);
}

/** Run after two animation frames so DOM/class changes are committed before transitions resume. */
export function afterLayoutFlush(): Promise<void> {
  return new Promise((resolve) => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => resolve());
    });
  });
}
