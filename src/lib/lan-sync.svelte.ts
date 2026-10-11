/**
 * Settings → Sync (experimental LAN sync). Mirrors `lan_sync_status` from the backend; builds
 * without the `lan-sync` cargo feature answer `lan_sync_unavailable` and the pane stays hidden.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type LanSyncDevice = {
  deviceId: string;
  name: string;
  os: string;
  addr?: string;
  lastSeen?: string | null;
  online?: boolean;
  visible?: boolean;
};

export type LanSyncStatus = {
  enabled: boolean;
  deviceId: string;
  name: string;
  os: string;
  port: number;
  pairingCode: string | null;
  pairingSecondsLeft: number | null;
  discovered: LanSyncDevice[];
  peers: LanSyncDevice[];
  lastError: string | null;
};

export const lanSync = $state<{
  available: boolean;
  status: LanSyncStatus | null;
  busy: boolean;
  error: string;
  codeFor: string | null;
  codeInput: string;
}>({ available: false, status: null, busy: false, error: "", codeFor: null, codeInput: "" });

export async function refreshLanSync(): Promise<void> {
  try {
    lanSync.status = await invoke<LanSyncStatus>("lan_sync_status");
    lanSync.available = true;
  } catch {
    lanSync.available = false;
    lanSync.status = null;
  }
}

/** Keep the pane live: backend emits `lan-sync-status` on every change. */
export async function watchLanSync(): Promise<UnlistenFn> {
  await refreshLanSync();
  return listen("lan-sync-status", () => void refreshLanSync());
}

async function run(fn: () => Promise<unknown>): Promise<void> {
  lanSync.busy = true;
  lanSync.error = "";
  try {
    await fn();
  } catch (e) {
    lanSync.error = String(e);
  } finally {
    lanSync.busy = false;
    await refreshLanSync();
  }
}

export const setLanSyncEnabled = (enabled: boolean) =>
  run(() => invoke("lan_sync_set_enabled", { enabled }));
export const showPairingCode = () => run(() => invoke("lan_sync_start_pairing"));
export const hidePairingCode = () => run(() => invoke("lan_sync_cancel_pairing"));
export const unpairDevice = (deviceId: string) =>
  run(() => invoke("lan_sync_unpair", { deviceId }));

export function startEnteringCode(deviceId: string): void {
  lanSync.codeFor = deviceId;
  lanSync.codeInput = "";
  lanSync.error = "";
}

export const pairWithCode = () =>
  run(async () => {
    if (!lanSync.codeFor) return;
    await invoke("lan_sync_pair", { deviceId: lanSync.codeFor, code: lanSync.codeInput.trim() });
    lanSync.codeFor = null;
    lanSync.codeInput = "";
  });

/** "2026-10-11T10:00:00.000Z" → "10:00" today, otherwise "11.10 10:00". */
export function formatSeen(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const hm = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  return d.toDateString() === new Date().toDateString()
    ? hm
    : `${d.toLocaleDateString([], { day: "2-digit", month: "2-digit" })} ${hm}`;
}
