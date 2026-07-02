import { invoke } from "@tauri-apps/api/core";

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

export function platformLoaded(): boolean {
  return platform !== null;
}
