<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { marked } from "marked";
  import KeyboardHints, { type KeyboardHint } from "$lib/components/KeyboardHints.svelte";
  import { invokeErrorMessage } from "$lib/exclusion-label";
  import {
    isPaletteDotLogicalSize,
    loadPaletteRestoreSize,
    savePaletteRestoreSize,
    type PaletteRestoreSize,
  } from "$lib/palette-window";
  import "$lib/styles/palette.css";

  marked.setOptions({ breaks: true, gfm: true });

  // Explicit window dragging — data-tauri-drag-region is unreliable on the
  // converted NSPanel, so start dragging on mousedown over the top bar
  // (but not when pressing one of its buttons).
  function startDrag(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    if (e.button === 0) {
      e.preventDefault();
      getCurrentWindow().startDragging();
    }
  }

  type Mode = "search" | "agent";

  let mode = $state<Mode>("agent");
  let query = $state("");
  let answer = $state("");
  let answerHtml = $derived(answer ? (marked.parse(answer) as string) : "");
  let progress = $state<string[]>([]);
  type StatusTone = "neutral" | "warn" | "fail";
  let statusNotice = $state("");
  let statusNoticeTone = $state<StatusTone>("neutral");
  let loading = $state(false);
  let recording = $state(false);
  let elapsed = $state(0);
  let inputEl: HTMLInputElement | undefined = $state();

  type Session = { q: string; a: string; mode: Mode; ts: number };
  let sessions = $state<Session[]>([]);
  let showHistory = $state(false);

  const paletteShortcutHints = $derived<KeyboardHint[]>([
    { keys: "↵", action: mode === "agent" ? "run agent" : "search" },
    { keys: "Tab", action: "switch mode (Web ⇄ Agent)" },
    { prefix: "Mic", action: "voice" },
    { keys: "⌘↵", action: "insert" },
    { keys: "Esc", action: "close" },
  ]);

  function clearStatusNotice() {
    statusNotice = "";
    statusNoticeTone = "neutral";
  }

  function setStatusNotice(message: string, tone: StatusTone = "fail") {
    statusNotice = message;
    statusNoticeTone = tone;
  }

  function setInvokeFailure(err: unknown, fallback: string) {
    setStatusNotice(invokeErrorMessage(err) || fallback, "fail");
  }

  function toggleMode() {
    clearStatusNotice();
    mode = mode === "agent" ? "search" : "agent";
  }

  function toggleHistory() {
    loadSessions();
    showHistory = !showHistory;
  }

  function loadSessions() {
    try {
      sessions = JSON.parse(localStorage.getItem("agentSessions") || "[]");
    } catch {
      sessions = [];
    }
  }
  function saveSession(q: string, a: string, m: Mode) {
    if (!q.trim() || !a.trim()) return;
    sessions = [{ q, a, mode: m, ts: Date.now() }, ...sessions].slice(0, 50);
    localStorage.setItem("agentSessions", JSON.stringify(sessions));
  }
  function openSession(s: Session) {
    query = s.q;
    answer = s.a;
    mode = s.mode;
    progress = [];
    clearStatusNotice();
    loading = false;
    showHistory = false;
    setTimeout(() => inputEl?.focus(), 30);
  }
  function timeAgo(ts: number): string {
    const s = Math.floor((Date.now() - ts) / 1000);
    if (s < 60) return "just now";
    if (s < 3600) return `${Math.floor(s / 60)} min ago`;
    if (s < 86400) return `${Math.floor(s / 3600)} hr ago`;
    return new Date(ts).toLocaleDateString();
  }

  // Drag bottom-right corner to resize (transparent rounded panel has no
  // obvious OS resize edge, so we provide an explicit grip).
  function startResize(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    getCurrentWindow().startResizeDragging("SouthEast");
  }

  // Collapse the whole panel to a small pulsing dot and back.
  let minimized = $state(false);
  let paletteReady = $state(false);
  let restoreSize = $state<PaletteRestoreSize>(loadPaletteRestoreSize());

  async function syncDotModeFromWindow() {
    try {
      minimized = await invoke<boolean>("palette_is_dot_mode");
    } catch {
      try {
        const win = getCurrentWindow();
        const sz = await win.innerSize();
        const f = await win.scaleFactor();
        minimized = isPaletteDotLogicalSize(sz.width / f, sz.height / f);
      } catch {
        /* keep current minimized flag */
      }
    }
  }

  async function initPaletteState() {
    restoreSize = loadPaletteRestoreSize();
    await syncDotModeFromWindow();
    paletteReady = true;
  }

  async function captureAndPersistRestoreSize() {
    const win = getCurrentWindow();
    try {
      const sz = await win.innerSize();
      const f = await win.scaleFactor();
      restoreSize = {
        w: Math.round(sz.width / f),
        h: Math.round(sz.height / f),
      };
      savePaletteRestoreSize(restoreSize);
    } catch {
      /* keep last persisted size */
    }
  }
  async function minimize() {
    await captureAndPersistRestoreSize();
    clearStatusNotice();
    try {
      await invoke("palette_set_dot_mode", {
        minimized: true,
        restoreWidth: restoreSize.w,
        restoreHeight: restoreSize.h,
      });
      minimized = true;
    } catch (e) {
      setInvokeFailure(e, "Could not minimize palette.");
    }
  }
  async function restoreWindow() {
    restoreSize = loadPaletteRestoreSize();
    try {
      await invoke("palette_set_dot_mode", {
        minimized: false,
        restoreWidth: restoreSize.w,
        restoreHeight: restoreSize.h,
      });
      minimized = false;
    } catch (e) {
      setInvokeFailure(e, "Could not restore palette.");
      return;
    }
    setTimeout(() => inputEl?.focus(), 40);
  }
  function dotDblClick() {
    void restoreWindow();
  }

  // Live elapsed counter while the agent is working (qwen3.6 reasoning is slow,
  // so a moving timer makes it clear it's running, not frozen).
  $effect(() => {
    if (!loading) return;
    elapsed = 0;
    const t = setInterval(() => (elapsed += 1), 1000);
    return () => clearInterval(t);
  });

  function reset() {
    query = "";
    answer = "";
    progress = [];
    clearStatusNotice();
    loading = false;
    recording = false;
  }

  async function run() {
    const q = query.trim();
    if (!q || loading) return;
    showHistory = false;
    loading = true;
    answer = "";
    clearStatusNotice();
    progress = [];
    try {
      if (mode === "agent") {
        // Streams via agent-progress / agent-final / agent-error events.
        await invoke("palette_agent", { query: q });
      } else {
        answer = await invoke<string>("palette_search", { query: q });
        loading = false;
        saveSession(q, answer, "search");
      }
    } catch (e) {
      setInvokeFailure(e, "Request failed. Try again.");
      loading = false;
    }
  }

  async function toggleMic() {
    if (recording) {
      recording = false;
      loading = true;
      clearStatusNotice();
      try {
        const text = await invoke<string>("palette_voice_stop");
        if (text.trim()) {
          query = text.trim();
          loading = false;
          run();
        } else {
          loading = false;
        }
      } catch (e) {
        setInvokeFailure(e, "Voice input failed. Try again.");
        loading = false;
      }
    } else {
      clearStatusNotice();
      try {
        await invoke("palette_voice_start");
        recording = true;
      } catch (e) {
        setInvokeFailure(e, "Voice input failed. Try again.");
      }
    }
  }

  async function insert() {
    if (!answer) return;
    await invoke("palette_insert", { text: answer });
    reset();
  }

  async function copy() {
    if (!answer) return;
    await navigator.clipboard.writeText(answer).catch(() => {});
  }

  async function close() {
    // Hide only — keep the running/finished agent so reopening shows it.
    // Use ＋ (New) to clear. Transient status hints/errors do not persist.
    clearStatusNotice();
    await invoke("palette_hide");
  }

  function onKeydown(e: KeyboardEvent) {
    if (minimized) {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        void restoreWindow();
      }
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "Tab") {
      e.preventDefault();
      toggleMode();
    } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      insert();
    } else if (e.key === "Enter") {
      e.preventDefault();
      run();
    }
  }

  onMount(() => {
    const unlistens = [
      listen("palette-show", async () => {
        clearStatusNotice();
        await syncDotModeFromWindow();
        if (!minimized) {
          setTimeout(() => {
            inputEl?.focus();
            inputEl?.select();
          }, 40);
        }
      }),
      listen<string>("agent-progress", (e) => {
        clearStatusNotice();
        progress = [...progress, e.payload];
      }),
      listen<string>("agent-final", (e) => {
        answer = e.payload;
        loading = false;
        clearStatusNotice();
        saveSession(query, e.payload, "agent");
      }),
      listen<string>("agent-error", (e) => {
        setStatusNotice(e.payload, "fail");
        loading = false;
      }),
    ];
    loadSessions();
    void initPaletteState().then(() => {
      if (!minimized) inputEl?.focus();
      return undefined;
    });
    return () => unlistens.forEach((u) => u.then((fn) => fn()));
  });
</script>

<svelte:window on:keydown={onKeydown} />

{#if paletteReady}
  {#if minimized}
  <div
    class="min-dot-shell"
    role="button"
    tabindex="0"
    data-tauri-drag-region="deep"
    title="Drag to move · double-click or Enter to expand"
    aria-label="Agent status dot. Drag to move, double-click or press Enter to expand."
    ondblclick={dotDblClick}
  >
    <span
      class="min-dot-orb"
      class:busy={loading}
      class:done={!loading && !!answer}
      aria-hidden="true"
    ></span>
  </div>
{:else}
<div class="palette">
  <div class="palette-head">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <header class="topbar" onmousedown={startDrag}>
    <button
      class="mode-badge app-btn"
      class:agent={mode === "agent"}
      type="button"
      title="Tab — switch mode"
      aria-pressed={mode === "agent"}
      onclick={toggleMode}
    >
      {mode === "agent" ? "Agent" : "Web"}
    </button>
    {#if loading}<span class="run-dot" title="Agent running"></span><span class="run-label">running… {elapsed}s</span>{/if}
    <div class="topbar-spacer"></div>
    <button
      class="bar-btn overlay-icon-btn app-btn"
      class:active={showHistory}
      type="button"
      title="Session history"
      aria-label="Session history"
      onclick={toggleHistory}
    >
      <svg class="overlay-icon-btn-icon" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
        <path d="M3 3v5h5" />
        <path d="M12 7v5l4 2" />
      </svg>
    </button>
    <button
      class="bar-btn overlay-icon-btn app-btn"
      type="button"
      title="New query"
      aria-label="New query"
      onclick={() => {
        reset();
        showHistory = false;
      }}
    >
      <svg class="overlay-icon-btn-icon" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M12 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
        <path d="M18.375 2.625a1 1 0 0 1 1.414 0l2.586 2.586a1 1 0 0 1 0 1.414L12.625 14.125 9 15l.875-3.625Z" />
      </svg>
    </button>
    <button
      class="bar-btn overlay-icon-btn app-btn"
      type="button"
      title="Compact to dot"
      aria-label="Compact to dot"
      onclick={minimize}
    >
      <svg class="overlay-icon-btn-icon" viewBox="0 0 24 24" aria-hidden="true">
        <rect x="4" y="4" width="16" height="16" rx="2" />
        <circle cx="17" cy="17" r="2.25" />
      </svg>
    </button>
    <button
      class="bar-btn overlay-icon-btn overlay-icon-btn--close app-btn"
      type="button"
      title="Hide (Esc) — state is preserved"
      aria-label="Close"
      onclick={close}
    >
      <svg class="overlay-icon-btn-icon overlay-icon-btn-icon--close" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M5 5 19 19M19 5 5 19" />
      </svg>
    </button>
  </header>
  <div class="query-field" class:recording role="search">
    <input
      bind:this={inputEl}
      bind:value={query}
      class="query-input"
      type="text"
      aria-label={mode === "agent" ? "Ask the agent" : "Search the web"}
      placeholder={mode === "agent"
        ? "Ask the agent — it will search and analyze…"
        : "Search the web via NeuralDeep…"}
      autocomplete="off"
      spellcheck="false"
      disabled={recording}
      oninput={clearStatusNotice}
    />
    {#if loading && !recording}
      <span
        class="query-spinner app-btn-spinner-icon app-btn-spinner-icon--palette is-inline"
        aria-hidden="true"
      ></span>
    {/if}
    <button
      class="mic-btn app-btn"
      class:recording
      type="button"
      title={recording ? "Stop voice input" : "Voice input"}
      aria-label={recording ? "Stop voice input" : "Start voice input"}
      aria-pressed={recording}
      disabled={loading && !recording}
      onclick={toggleMic}
    >
      <svg class="mic-btn-icon" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
        <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
        <path d="M12 19v3" />
      </svg>
    </button>
  </div>
  </div>

  {#if statusNotice}
    <p
      class="overlay-status-hint"
      class:neutral={statusNoticeTone === "neutral"}
      class:warn={statusNoticeTone === "warn"}
      class:fail={statusNoticeTone === "fail"}
      role={statusNoticeTone === "fail" ? "alert" : "status"}
      aria-live="polite"
    >
      {statusNotice}
    </p>
  {/if}

  {#if showHistory}
    <div class="history">
      {#if sessions.length === 0}
        <p class="overlay-status-hint neutral">No history yet — ask a question and it will appear here.</p>
      {:else}
        {#each sessions as s}
          <button class="history-item app-btn" type="button" onclick={() => openSession(s)}>
            <span class="history-mode" class:agent={s.mode === "agent"}>{s.mode === "agent" ? "A" : "W"}</span>
            <span class="history-q">{s.q}</span>
            <span class="history-time">{timeAgo(s.ts)}</span>
          </button>
        {/each}
      {/if}
    </div>
  {:else if answer}
    <!-- eslint-disable-next-line svelte/no-at-html-tags -->
    <div class="result markdown">{@html answerHtml}</div>
    <div class="actions">
      <button class="app-btn" onclick={insert}>Insert ⌘↵</button>
      <button class="app-btn" onclick={copy}>Copy</button>
      <button class="ghost app-btn" onclick={close}>Close Esc</button>
    </div>
  {:else if loading && progress.length}
    <div class="progress">
      {#each progress as line}<div class="progress-line">{line}</div>{/each}
    </div>
  {/if}

  {#if !answer}
    <footer class="overlay-shortcuts overlay-footer-strip">
      <KeyboardHints hints={paletteShortcutHints} />
    </footer>
  {/if}

  <button
    class="resize-grip"
    type="button"
    tabindex="-1"
    aria-label="Resize"
    title="Drag to resize"
    onmousedown={startResize}
  ></button>
</div>
{/if}
{/if}
