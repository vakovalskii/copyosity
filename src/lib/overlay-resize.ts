import { panelOpenMs } from "$lib/motion";
import { invoke } from "@tauri-apps/api/core";

let appliedHeight: number | null = null;
let animFrame: number | null = null;
let lastThrottledHeight: number | null = null;

const RESIZE_THROTTLE_MS = 32;

async function invokeResizeMainWindow(height: number, rememberHeight: boolean): Promise<void> {
  await invoke("resize_main_window", { height, rememberHeight });
}

/** Authoritative resize; Rust remembers height for the next pre-show placement. */
export async function resizeMainWindow(height: number): Promise<void> {
  await invokeResizeMainWindow(height, true);
  appliedHeight = height;
}

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

function cancelAnimation() {
  if (animFrame !== null) {
    cancelAnimationFrame(animFrame);
    animFrame = null;
  }
}

/** Smooth bottom-anchored resize; instant when Reduce Motion is on. */
export async function animateOverlayResize(targetHeight: number): Promise<void> {
  const start = appliedHeight ?? targetHeight;
  if (start === targetHeight) {
    appliedHeight = targetHeight;
    return;
  }

  cancelAnimation();

  if (prefersReducedMotion()) {
    await resizeMainWindow(targetHeight);
    return;
  }

  const duration = Math.min(panelOpenMs(), 240);
  const t0 = performance.now();
  lastThrottledHeight = null;
  let lastInvokeAt = 0;

  return new Promise<void>((resolve) => {
    const step = (now: number) => {
      const t = Math.min(1, (now - t0) / duration);
      const eased = 1 - (1 - t) ** 3;
      const h = Math.round(start + (targetHeight - start) * eased);
      if (t >= 1) {
        animFrame = null;
        void resizeMainWindow(targetHeight).then(resolve);
        return;
      }
      if (h !== lastThrottledHeight && now - lastInvokeAt >= RESIZE_THROTTLE_MS) {
        lastThrottledHeight = h;
        lastInvokeAt = now;
        void invokeResizeMainWindow(h, false);
      }
      animFrame = requestAnimationFrame(step);
    };
    animFrame = requestAnimationFrame(step);
  });
}

export function resetOverlayResizeState() {
  cancelAnimation();
  appliedHeight = null;
  lastThrottledHeight = null;
}
