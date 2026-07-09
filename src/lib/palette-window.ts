/** Logical min-dot size — keep in sync with `palette_window::PALETTE_DOT_SIZE`. */
export const PALETTE_DOT_SIZE = 72;
export const PALETTE_DOT_SIZE_TOLERANCE = 0.5;
export const PALETTE_MIN_WIDTH = 500;
export const PALETTE_MIN_HEIGHT = 220;

const RESTORE_SIZE_KEY = "paletteRestoreSize";
const DEFAULT_RESTORE = { w: 640, h: 460 };

export type PaletteRestoreSize = { w: number; h: number };

export function isPaletteDotLogicalSize(logicalWidth: number, logicalHeight: number): boolean {
  return (
    logicalWidth <= PALETTE_DOT_SIZE + PALETTE_DOT_SIZE_TOLERANCE &&
    logicalHeight <= PALETTE_DOT_SIZE + PALETTE_DOT_SIZE_TOLERANCE
  );
}

export function loadPaletteRestoreSize(): PaletteRestoreSize {
  try {
    const raw = localStorage.getItem(RESTORE_SIZE_KEY);
    if (!raw) return { ...DEFAULT_RESTORE };
    const parsed = JSON.parse(raw) as { w?: unknown; h?: unknown };
    const w = parsed.w;
    const h = parsed.h;
    if (
      typeof w === "number" &&
      typeof h === "number" &&
      w >= PALETTE_MIN_WIDTH &&
      h >= PALETTE_MIN_HEIGHT
    ) {
      return { w: Math.round(w), h: Math.round(h) };
    }
  } catch {
    /* ignore corrupt storage */
  }
  return { ...DEFAULT_RESTORE };
}

export function savePaletteRestoreSize(size: PaletteRestoreSize): void {
  localStorage.setItem(RESTORE_SIZE_KEY, JSON.stringify(size));
}
