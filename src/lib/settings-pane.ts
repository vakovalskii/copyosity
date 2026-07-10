export const SETTINGS_PANE_IDS = [
  "hub",
  "voice",
  "quickmenu",
  "ai",
  "history",
  "permissions",
  "updates",
  "language",
] as const;

export type SettingsPane = (typeof SETTINGS_PANE_IDS)[number];

const paneIds = new Set<string>(SETTINGS_PANE_IDS);

export function isSettingsPane(value: string): value is SettingsPane {
  return paneIds.has(value);
}

export function parseSettingsPaneFromQuery(search: string): SettingsPane | null {
  const pane = new URLSearchParams(search).get("pane");
  return pane && isSettingsPane(pane) ? pane : null;
}

export function resolveSettingsPaneUpdate(
  current: SettingsPane,
  incoming: string | null | undefined,
): SettingsPane {
  if (!incoming || !isSettingsPane(incoming)) return current;
  return incoming;
}

export function coerceSettingsPane(
  incoming: SettingsPane | null | undefined,
  fallback: SettingsPane = "hub",
): SettingsPane {
  return incoming ?? fallback;
}
