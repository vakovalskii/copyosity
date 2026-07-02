<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";

  const N = 28;
  let bars = $state<number[]>(Array(N).fill(8));
  let elapsed = $state(0); // seconds
  let active = $state(false);

  let startMs = 0;
  let lastMs = 0;

  function fmt(s: number): string {
    const m = Math.floor(s / 60);
    const ss = Math.floor(s % 60);
    return `${m}:${ss.toString().padStart(2, "0")}`;
  }

  onMount(() => {
    const unlisten = listen<number>("audio-level", (event) => {
      const now = Date.now();
      // A gap means a fresh recording session — reset the timer + waveform.
      if (now - lastMs > 800) {
        startMs = now;
        bars = Array(N).fill(8);
      }
      lastMs = now;
      active = true;

      const lvl = Math.max(8, Math.min(100, event.payload));
      // Scroll the new sample in from the right.
      bars = [...bars.slice(1), lvl];
    });

    const tick = setInterval(() => {
      if (lastMs && Date.now() - lastMs < 800) {
        elapsed = (Date.now() - startMs) / 1000;
      } else {
        active = false;
      }
    }, 100);

    return () => {
      unlisten.then((fn) => fn());
      clearInterval(tick);
    };
  });
</script>

<div class="capsule">
  <span class="dot" class:live={active}></span>
  <div class="wave">
    {#each bars as h, i}
      <div class="bar" style="height: {Math.max(8, h)}%" data-i={i}></div>
    {/each}
  </div>
  <span class="time">{fmt(elapsed)}</span>
</div>

<style>
  :global(body) {
    background: transparent;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
  }

  .capsule {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100vw;
    height: 100vh;
    padding: 0 var(--space-panel-inline);
    background: var(--surface-voice);
    backdrop-filter: blur(var(--blur-voice-capsule));
    -webkit-backdrop-filter: blur(var(--blur-voice-capsule));
    border-radius: var(--radius-pill);
    border: 1px solid var(--border-soft);
    box-shadow: var(--shadow-voice-capsule);
  }

  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--color-text-disabled);
    transition: background var(--duration-standard) ease;
  }

  .dot.live {
    background: var(--color-recording);
    box-shadow: 0 0 0 0 var(--color-recording-pulse);
    animation: pulse-dot var(--duration-pulse-mic) ease-out infinite;
  }

  @keyframes pulse-dot {
    0% {
      box-shadow: 0 0 0 0 rgb(var(--rgb-recording) / 55%);
    }

    70% {
      box-shadow: var(--shadow-recording-pulse);
    }

    100% {
      box-shadow: 0 0 0 0 rgb(var(--rgb-recording) / 0%);
    }
  }

  .wave {
    display: flex;
    align-items: center;
    gap: var(--space-segment-inset);
    flex: 1;
    height: 26px;
    min-width: 0;
    overflow: hidden;
  }

  .bar {
    flex: 1;
    min-width: var(--space-segment-inset);
    min-height: 8%;
    border-radius: var(--radius-pill);
    background: var(--gradient-voice-bar);
    transition: height 90ms cubic-bezier(0.3, 0.8, 0.3, 1);
  }

  .time {
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    font-weight: 600;
    color: var(--color-text-primary);
    letter-spacing: 0.02em;
  }
</style>
