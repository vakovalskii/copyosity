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
    margin: 0;
    padding: 0;
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
    padding: 0 14px;
    background: rgba(18, 18, 24, 0.9);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.4);
  }

  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex-shrink: 0;
    background: #6b7280;
    transition: background 0.2s ease;
  }

  .dot.live {
    background: #f8534b;
    box-shadow: 0 0 0 0 rgba(248, 83, 75, 0.6);
    animation: pulse-dot 1.3s ease-out infinite;
  }

  @keyframes pulse-dot {
    0% { box-shadow: 0 0 0 0 rgba(248, 83, 75, 0.55); }
    70% { box-shadow: 0 0 0 7px rgba(248, 83, 75, 0); }
    100% { box-shadow: 0 0 0 0 rgba(248, 83, 75, 0); }
  }

  .wave {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: 1;
    height: 26px;
    min-width: 0;
    overflow: hidden;
  }

  .bar {
    flex: 1;
    min-width: 2px;
    min-height: 8%;
    border-radius: 999px;
    background: linear-gradient(180deg, #ff7a6b, #f8534b 55%, #e0463f);
    transition: height 90ms cubic-bezier(0.3, 0.8, 0.3, 1);
  }

  .time {
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
    font-family: "SF Mono", ui-monospace, Menlo, monospace;
    font-size: 12px;
    font-weight: 600;
    color: #e8eaf0;
    letter-spacing: 0.02em;
  }
</style>
