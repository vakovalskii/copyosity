import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
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

/** How often the menu-bar app re-checks while it keeps running. */
export const BACKGROUND_UPDATE_INTERVAL_MS = 6 * 60 * 60 * 1000;

let notifiedVersion: string | null = null;

/**
 * Check for an update and, if one is available, install it silently through
 * the Rust `install_update` command (temp dir on the app's own volume, one
 * install at a time) without restarting. The new version applies on the next
 * launch; the user is notified once per version. Best-effort — never throws.
 */
export async function backgroundUpdate(reason: "launch" | "periodic"): Promise<void> {
  try {
    appendUpdateLog(`${reason}: running ${await getVersion()}, checking for updates…`);
    const installed = await invoke<string | null>("install_update", { restart: false });
    if (!installed) {
      appendUpdateLog(`${reason}: already up to date`);
      return;
    }
    appendUpdateLog(`${reason}: ${installed} installed — restart to apply`);
    if (notifiedVersion !== installed) {
      notifiedVersion = installed;
      await notify(
        "Copyosity updated",
        `Version ${installed} installed — restart Copyosity to apply.`,
      );
    }
  } catch (e) {
    // Offline / transient failure — the next periodic check or the
    // Settings → Updates pane retries.
    appendUpdateLog(`${reason}: ERROR — ${errorToString(e)}`);
  }
}

/**
 * Check on launch, then every {@link BACKGROUND_UPDATE_INTERVAL_MS} while the
 * app stays in the menu bar. Returns a function that stops the periodic checks.
 */
export function startBackgroundUpdates(): () => void {
  void backgroundUpdate("launch");
  const timer = setInterval(() => void backgroundUpdate("periodic"), BACKGROUND_UPDATE_INTERVAL_MS);
  return () => clearInterval(timer);
}

/** Current running app version (from tauri.conf.json). */
export async function currentVersion(): Promise<string> {
  return await getVersion();
}

export { relaunch };
