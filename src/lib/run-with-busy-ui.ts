import { tick } from "svelte";

/** Yield until Svelte has flushed and the browser has painted. */
export async function prepareBusyUi(): Promise<void> {
  await tick();
  await new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  });
}
