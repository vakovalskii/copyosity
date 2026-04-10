<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";

  let bars = $state([12, 20, 8, 15, 10]);

  onMount(() => {
    const unlisten = listen<number>("audio-level", (event) => {
      const level = event.payload;
      // Generate 5 bars with some variation around the level
      bars = bars.map((_, i) => {
        const offset = Math.sin(Date.now() / 120 + i * 1.8) * 18;
        return Math.max(6, Math.min(100, level + offset + Math.random() * 12));
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  });
</script>

<div class="overlay">
  <div class="mic-icon">
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
      <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/>
      <path d="M19 10v2a7 7 0 0 1-14 0v-2"/>
      <line x1="12" y1="19" x2="12" y2="23"/>
      <line x1="8" y1="23" x2="16" y2="23"/>
    </svg>
  </div>
  <div class="eq">
    {#each bars as h, i}
      <div class="bar" style="height: {h}%; transition-delay: {i * 15}ms;"></div>
    {/each}
  </div>
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

  .overlay {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    width: 100vw;
    height: 100vh;
    background: rgba(18, 18, 24, 0.88);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border-radius: 16px;
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .mic-icon {
    color: #f87171;
    animation: pulse-mic 1.2s ease-in-out infinite;
    flex-shrink: 0;
  }

  @keyframes pulse-mic {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.6; transform: scale(0.92); }
  }

  .eq {
    display: flex;
    align-items: flex-end;
    gap: 3px;
    height: 36px;
    padding: 0 2px;
  }

  .bar {
    width: 5px;
    min-height: 4px;
    background: linear-gradient(180deg, #f87171, #fb923c);
    border-radius: 3px;
    transition: height 80ms ease-out;
  }
</style>
