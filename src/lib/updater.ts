import { getVersion } from "@tauri-apps/api/app";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

import { appendUpdateLog, errorToString } from "./update-log";

export type { Update };

/** Check GitHub Releases for a newer version. Returns null when up to date. */
export async function checkForUpdate(): Promise<Update | null> {
  return await check();
}

/** Post a native OS notification (requesting permission once if needed). */
export async function notify(title: string, body: string): Promise<void> {
  try {
    let granted = await isPermissionGranted();
    if (!granted) {
      granted = (await requestPermission()) === "granted";
    }
    if (granted) {
      sendNotification({ title, body });
    }
  } catch {
    // Notifications are best-effort; never block the app on them.
  }
}

/**
 * On launch: check for an update and, if one is available, download + install it
 * silently, then notify the user. The new version applies on the next launch
 * (no forced restart mid-session). Best-effort — never throws.
 */
export async function autoUpdateOnLaunch(): Promise<void> {
  try {
    appendUpdateLog("launch: checking for updates…");
    const update = await check();
    if (!update) {
      appendUpdateLog("launch: already up to date");
      return;
    }
    appendUpdateLog(`launch: update ${update.version} found — downloading + installing…`);
    await notify("Copyosity update available", `Downloading version ${update.version}…`);
    await update.downloadAndInstall();
    appendUpdateLog(`launch: installed ${update.version} — restart to apply`);
    await notify(
      "Copyosity updated",
      `Version ${update.version} installed — restart Copyosity to apply.`,
    );
  } catch (e) {
    // Offline / transient failure — the Settings → Updates pane can retry.
    appendUpdateLog(`launch: ERROR — ${errorToString(e)}`);
  }
}

/** Current running app version (from tauri.conf.json). */
export async function currentVersion(): Promise<string> {
  return await getVersion();
}

export { relaunch };
