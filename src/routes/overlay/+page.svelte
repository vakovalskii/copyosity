<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { prefersReducedMotion, subscribeReducedMotion } from "$lib/motion";

  let bars = $state([12, 20, 8, 15, 10]);
  let reducedMotion = $state(prefersReducedMotion());

  function levelToBarHeight(level: number, index: number): number {
    if (reducedMotion) {
      return Math.max(20, Math.min(80, level));
    }
    const offset = Math.sin(Date.now() / 120 + index * 1.8) * 18;
    return Math.max(6, Math.min(100, level + offset + Math.random() * 12));
  }

  onMount(() => {
    const unsubscribeMotion = subscribeReducedMotion((value) => {
      reducedMotion = value;
    });

    const unlisten = listen<number>("audio-level", (event) => {
      const level = event.payload;
      bars = bars.map((_, i) => levelToBarHeight(level, i));
    });

    return () => {
      unsubscribeMotion();
      unlisten.then((fn) => fn());
    };
  });
</script>

<div class="overlay" role="status" aria-live="polite" aria-atomic="true">
  <span class="sr-only">Recording voice</span>
  <div class="content" aria-hidden="true">
    <div class="mic-icon">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/>
        <path d="M19 10v2a7 7 0 0 1-14 0v-2"/>
        <line x1="12" y1="19" x2="12" y2="23"/>
        <line x1="8" y1="23" x2="16" y2="23"/>
      </svg>
    </div>
    <div class="eq">
      {#each bars as h, i}
        <div
          class="bar"
          class:bar-static={reducedMotion}
          style="height: {h}%;"
          style:transition-delay={reducedMotion ? undefined : `calc(${i} * var(--duration-stagger-step))`}
        ></div>
      {/each}
    </div>
  </div>
</div>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    padding: 0;
    width: 100%;
    height: 100%;
    background: transparent;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
  }

  .overlay {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    padding: 10px 12px;
    background: var(--surface-voice);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border-radius: 22px;
    border: 1px solid var(--border-soft);
  }

  .content {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-shrink: 0;
  }

  .mic-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    flex-shrink: 0;
    color: var(--color-recording);
    animation: pulse-mic var(--duration-pulse-mic) ease-in-out infinite;
  }

  @media (prefers-reduced-transparency: reduce) {
    .overlay {
      backdrop-filter: none;
      -webkit-backdrop-filter: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .mic-icon {
      animation: none;
      opacity: 1;
    }
  }

  .mic-icon svg {
    width: 22px;
    height: 22px;
    display: block;
  }

  @keyframes pulse-mic {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.55; }
  }

  .eq {
    display: flex;
    align-items: center;
    gap: 3px;
    height: 24px;
    flex-shrink: 0;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
    border: 0;
  }

  .bar {
    width: 4px;
    min-height: 4px;
    background: var(--gradient-voice-bar);
    border-radius: 2px;
    transition: height var(--duration-fast) var(--ease-interactive);
  }

  @media (prefers-reduced-motion: reduce) {
    .bar,
    .bar.bar-static {
      transition: none;
      transition-delay: 0ms;
    }
  }
</style>
