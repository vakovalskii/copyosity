import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

export type { Update };

/** Check GitHub Releases for a newer version. Returns null when up to date. */
export async function checkForUpdate(): Promise<Update | null> {
  return await check();
}

/** Current running app version (from tauri.conf.json). */
export async function currentVersion(): Promise<string> {
  return await getVersion();
}

export { relaunch };
