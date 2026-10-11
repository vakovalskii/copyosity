import { invoke } from "@tauri-apps/api/core";

import { formatKeys } from "./key-label";

let platform = $state<"macos" | "windows" | "linux" | "unknown" | null>(null);

export async function initPlatform(): Promise<void> {
  if (platform !== null) return;
  try {
    platform = await invoke<"macos" | "windows" | "linux">("get_platform");
  } catch {
    platform = "unknown";
  }
}

export function platformIsMacOS(): boolean {
  return platform === "macos";
}

/** Excluded apps: bundle ids on macOS, exe names on Windows (win_apps.rs). */
export function platformSupportsAppExclusion(): boolean {
  return platform === "macos" || platform === "windows";
}

export function platformLoaded(): boolean {
  return platform !== null;
}

/** Shortcut label for the current platform, see `key-label.ts`. Reactive: reads `platform`. */
export function keyLabel(mac: string): string {
  return formatKeys(mac, platform);
}
