/**
 * Shortcut labels are written once in macOS glyphs (`⌘⇧Space`, `⌘Y`) and shown per
 * platform: unchanged on macOS, `Ctrl+Shift+Space` / `Ctrl+Y` on Windows and Linux.
 * Only the modifier glyphs change; key glyphs such as `↵` or `←` are understood everywhere.
 */

const MODIFIERS: Record<string, string> = { "⌘": "Ctrl", "⌃": "Ctrl", "⌥": "Alt", "⇧": "Shift" };

export function formatKeys(mac: string, platform: string | null): string {
  if (platform !== "windows" && platform !== "linux") return mac;
  const mods: string[] = [];
  let rest = mac;
  while (rest.length > 0 && MODIFIERS[rest[0]] !== undefined) {
    const name = MODIFIERS[rest[0]];
    if (!mods.includes(name)) mods.push(name);
    rest = rest.slice(1);
  }
  if (mods.length === 0) return mac;
  return [...mods, rest].filter(Boolean).join("+");
}
