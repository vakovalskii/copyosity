<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { marked } from "marked";
  import KeyboardHints, { type KeyboardHint } from "$lib/components/KeyboardHints.svelte";
  import "$lib/styles/palette.css";

  marked.setOptions({ breaks: true, gfm: true });

  // Explicit window dragging — data-tauri-drag-region is unreliable on the
  // converted NSPanel, so start dragging on mousedown over the top bar
  // (but not when pressing one of its buttons).
  function startDrag(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    if (e.button === 0) getCurrentWindow().startDragging();
  }

  type Mode = "search" | "agent";

  let mode = $state<Mode>("agent");
  let query = $state("");
  let answer = $state("");
  let answerHtml = $derived(answer ? (marked.parse(answer) as string) : "");
  let progress = $state<string[]>([]);
  let error = $state("");
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
    error = "";
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
  let restoreSize = { w: 640, h: 460 };
  async function minimize() {
    const win = getCurrentWindow();
    try {
      const sz = await win.innerSize();
      const f = await win.scaleFactor();
      restoreSize = { w: Math.round(sz.width / f), h: Math.round(sz.height / f) };
    } catch {}
    minimized = true;
    await win.setSize(new LogicalSize(72, 72));
  }
  async function restoreWindow() {
    minimized = false;
    await getCurrentWindow().setSize(new LogicalSize(restoreSize.w, restoreSize.h));
    setTimeout(() => inputEl?.focus(), 40);
  }
  // On the dot: single press drags (moves the window), double press restores.
  // e.detail === 2 on the second mousedown of a double-click.
  function dotMouseDown(e: MouseEvent) {
    if (e.detail >= 2) {
      restoreWindow();
      return;
    }
    getCurrentWindow().startDragging();
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
    error = "";
    loading = false;
  }

  async function run() {
    const q = query.trim();
    if (!q || loading) return;
    loading = true;
    answer = "";
    error = "";
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
      error = String(e);
      loading = false;
    }
  }

  async function toggleMic() {
    if (recording) {
      recording = false;
      loading = true;
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
        error = String(e);
        loading = false;
      }
    } else {
      try {
        await invoke("palette_voice_start");
        recording = true;
      } catch (e) {
        error = String(e);
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
    // Use ＋ (New) to clear.
    await invoke("palette_hide");
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "Tab") {
      e.preventDefault();
      mode = mode === "agent" ? "search" : "agent";
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
      listen("palette-show", () => {
        // Keep prior state (a running/finished agent stays visible on reopen);
        // just refocus and select the query for quick replacement.
        setTimeout(() => {
          inputEl?.focus();
          inputEl?.select();
        }, 40);
      }),
      listen<string>("agent-progress", (e) => {
        progress = [...progress, e.payload];
      }),
      listen<string>("agent-final", (e) => {
        answer = e.payload;
        loading = false;
        saveSession(query, e.payload, "agent");
      }),
      listen<string>("agent-error", (e) => {
        error = e.payload;
        loading = false;
      }),
    ];
    loadSessions();
    inputEl?.focus();
    return () => unlistens.forEach((u) => u.then((fn) => fn()));
  });
</script>

<svelte:window on:keydown={onKeydown} />

{#if minimized}
  <button
    class="min-dot"
    class:busy={loading}
    class:done={!loading && !!answer}
    type="button"
    title="Drag to move · double-click to expand"
    onmousedown={dotMouseDown}
    aria-label="Move or expand"
  ></button>
{:else}
<div class="palette">
  <div class="topbar" role="toolbar" tabindex="-1" onmousedown={startDrag}>
    <button
      class="mode-badge app-btn"
      class:agent={mode === "agent"}
      type="button"
      title="Tab — switch mode"
      aria-pressed={mode === "agent"}
      onclick={() => (mode = mode === "agent" ? "search" : "agent")}
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
      onclick={() => {
        loadSessions();
        showHistory = !showHistory;
      }}
    >
      <svg class="overlay-icon-btn-icon" viewBox="0 0 24 24" aria-hidden="true">
        <circle cx="12" cy="12" r="10" />
        <polyline points="12 6 12 12 16 14" />
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
        <line x1="12" y1="5" x2="12" y2="19" />
        <line x1="5" y1="12" x2="19" y2="12" />
      </svg>
    </button>
    <button class="bar-btn overlay-icon-btn app-btn" type="button" title="Minimize to dot" aria-label="Minimize" onclick={minimize}>
      <svg class="overlay-icon-btn-icon" viewBox="0 0 24 24" aria-hidden="true">
        <line x1="5" y1="12" x2="19" y2="12" />
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
        <path d="M6.4 6.4 17.6 17.6M17.6 6.4 6.4 17.6" />
      </svg>
    </button>
  </div>
  <div class="search-row">
    <input
      bind:this={inputEl}
      bind:value={query}
      class="search-input"
      type="text"
      placeholder={mode === "agent"
        ? "Ask the agent — it will search and analyze…"
        : "Search the web via NeuralDeep…"}
      autocomplete="off"
      spellcheck="false"
    />
    <button class="mic-btn overlay-icon-btn app-btn" class:recording type="button" title="Voice input" onclick={toggleMic} aria-label={recording ? "Stop voice input" : "Start voice input"}>
      <svg class="mic-btn-icon" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
        <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
        <path d="M12 19v3" />
      </svg>
    </button>
    {#if loading}<span class="app-btn-spinner-icon app-btn-spinner-icon--palette is-inline"></span>{/if}
  </div>

  {#if showHistory}
    <div class="history">
      {#if sessions.length === 0}
        <div class="hint history-empty-hint">No history yet — ask a question and it will appear here.</div>
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
  {:else if error}
    <div class="result error">{error}</div>
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
  {:else if !loading}
    <footer class="palette-shortcuts">
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
