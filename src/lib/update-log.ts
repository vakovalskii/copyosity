// Persistent, user-visible updater log. Every updater step (check / download /
// install / relaunch) and any error is appended here — including the silent
// on-launch auto-update attempt — so Settings → Updates can show exactly what
// happened and the user can copy it for a bug report.

const KEY = "copyosity.updateLog";
const MAX_LINES = 300;

/** Turn any thrown value into a full, human-readable string (message + stack). */
export function errorToString(e: unknown): string {
  if (e == null) return "unknown error";
  if (typeof e === "string") return e;
  if (e instanceof Error) {
    return e.stack ? `${e.name}: ${e.message}\n${e.stack}` : `${e.name}: ${e.message}`;
  }
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

export function appendUpdateLog(line: string): void {
  try {
    const ts = new Date().toISOString().replace("T", " ").slice(0, 19);
    const next = [...readUpdateLog(), `[${ts}] ${line}`].slice(-MAX_LINES);
    localStorage.setItem(KEY, JSON.stringify(next));
  } catch {
    // logging must never throw
  }
}

export function readUpdateLog(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) || "[]");
    return Array.isArray(v) ? v : [];
  } catch {
    return [];
  }
}

export function clearUpdateLog(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    // ignore
  }
}
