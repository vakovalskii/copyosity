<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { marked } from "marked";

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
    if (s < 60) return "только что";
    if (s < 3600) return `${Math.floor(s / 60)} мин назад`;
    if (s < 86400) return `${Math.floor(s / 3600)} ч назад`;
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
    title="Перетащи — двигать · двойной клик — развернуть"
    onmousedown={dotMouseDown}
    aria-label="Move or expand"
  ></button>
{:else}
<div class="palette">
  <div class="topbar" role="toolbar" tabindex="-1" onmousedown={startDrag}>
    <button
      class="mode-badge"
      class:agent={mode === "agent"}
      type="button"
      title="Tab — сменить режим"
      onclick={() => (mode = mode === "agent" ? "search" : "agent")}
    >
      {mode === "agent" ? "Agent" : "Web"}
    </button>
    {#if loading}<span class="run-dot" title="Агент работает"></span><span class="run-label">работает… {elapsed}s</span>{/if}
    <div class="topbar-spacer"></div>
    <button class="bar-btn" class:active={showHistory} type="button" title="История сессий" onclick={() => { loadSessions(); showHistory = !showHistory; }}>🕘</button>
    <button class="bar-btn" type="button" title="Новый запрос" onclick={() => { reset(); showHistory = false; }}>＋</button>
    <button class="bar-btn" type="button" title="Свернуть в точку" onclick={minimize}>–</button>
    <button class="bar-btn" type="button" title="Скрыть (Esc) — состояние сохранится" onclick={close}>✕</button>
  </div>
  <div class="search-row">
    <input
      bind:this={inputEl}
      bind:value={query}
      class="search-input"
      type="text"
      placeholder={mode === "agent"
        ? "Спроси агента — он поищет и проанализирует…"
        : "Поиск в вебе через NeuralDeep…"}
      autocomplete="off"
      spellcheck="false"
    />
    <button class="mic-btn" class:recording type="button" title="Голос" onclick={toggleMic} aria-label="Voice">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/>
        <path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" y1="19" x2="12" y2="23"/>
      </svg>
    </button>
    {#if loading}<span class="spinner"></span>{/if}
  </div>

  {#if showHistory}
    <div class="history">
      {#if sessions.length === 0}
        <div class="hint" style="border:none;padding:0;">История пуста — задай вопрос, и он сохранится здесь.</div>
      {:else}
        {#each sessions as s}
          <button class="history-item" type="button" onclick={() => openSession(s)}>
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
      <button onclick={insert}>Insert ⌘↵</button>
      <button onclick={copy}>Copy</button>
      <button class="ghost" onclick={close}>Close Esc</button>
    </div>
  {:else if loading && progress.length}
    <div class="progress">
      {#each progress as line}<div class="progress-line">{line}</div>{/each}
    </div>
  {:else if !loading}
    <div class="hint">
      Enter — {mode === "agent" ? "запустить агента" : "искать"} · Tab — сменить режим (Web ⇄ Agent) ·
      🎤 — голос · ⌘↵ — вставить · Esc — закрыть
    </div>
  {/if}

  <button
    class="resize-grip"
    type="button"
    tabindex="-1"
    aria-label="Resize"
    title="Потяни, чтобы изменить размер"
    onmousedown={startResize}
  ></button>
</div>
{/if}

<style>
  :global(body) {
    margin: 0;
    background: transparent;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }

  .palette {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    box-sizing: border-box;
    background: rgba(22, 22, 28, 0.92);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    border-radius: 16px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #f2f2f5;
    padding: 14px 16px;
    gap: 12px;
  }

  .topbar {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
    margin: -4px -4px 0;
    padding: 2px 4px 6px;
  }
  .topbar-spacer { flex: 1; align-self: stretch; }

  .bar-btn {
    flex-shrink: 0;
    width: 24px;
    height: 22px;
    border-radius: 7px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: transparent;
    color: #b8b8c0;
    font-size: 13px;
    line-height: 1;
    cursor: pointer;
  }
  .bar-btn:hover { background: rgba(255, 255, 255, 0.08); color: #fff; }
  .bar-btn.active { background: rgba(155, 120, 255, 0.25); color: #fff; border-color: rgba(155,120,255,0.4); }

  .history {
    flex: 1;
    overflow-y: auto;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    padding-top: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .history-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    color: #e6e6ea;
    font: inherit;
    font-size: 13px;
    padding: 7px 8px;
    border-radius: 8px;
    cursor: pointer;
  }
  .history-item:hover { background: rgba(255, 255, 255, 0.06); }
  .history-mode {
    flex-shrink: 0;
    width: 18px; height: 18px;
    border-radius: 5px;
    font-size: 10px; font-weight: 700;
    display: flex; align-items: center; justify-content: center;
    background: rgba(120, 160, 255, 0.2); color: #acc4ff;
  }
  .history-mode.agent { background: rgba(155, 120, 255, 0.25); color: #d2bcff; }
  .history-q { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .history-time { flex-shrink: 0; font-size: 11px; color: #76767f; }

  .resize-grip {
    position: fixed;
    right: 2px;
    bottom: 2px;
    width: 16px;
    height: 16px;
    padding: 0;
    border: none;
    cursor: nwse-resize;
    background:
      linear-gradient(135deg, transparent 0 50%, rgba(255,255,255,0.35) 50% 60%, transparent 60% 70%, rgba(255,255,255,0.35) 70% 80%, transparent 80%);
    z-index: 10;
  }

  .run-dot {
    width: 8px; height: 8px; border-radius: 50%;
    background: #9b78ff;
    animation: pulse 1.1s ease-in-out infinite;
  }
  .run-label { font-size: 12px; color: #b9b9c2; }

  .search-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }

  .mode-badge {
    flex-shrink: 0;
    font-size: 11px;
    font-weight: 700;
    padding: 4px 9px;
    border-radius: 7px;
    border: none;
    background: rgba(120, 160, 255, 0.18);
    color: #acc4ff;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    cursor: pointer;
  }
  .mode-badge.agent {
    background: rgba(155, 120, 255, 0.22);
    color: #d2bcff;
  }

  .search-input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: #f2f2f5;
    font-size: 18px;
  }
  .search-input::placeholder { color: #76767f; }

  .mic-btn {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: transparent;
    color: #b8b8c0;
    cursor: pointer;
  }
  .mic-btn:hover { background: rgba(255, 255, 255, 0.06); }
  .mic-btn.recording {
    color: #fff;
    background: #e5534b;
    border-color: #e5534b;
    animation: pulse 1.1s ease-in-out infinite;
  }
  @keyframes pulse { 0%,100% { opacity: 1; } 50% { opacity: 0.55; } }

  .result {
    flex: 1;
    overflow-y: auto;
    font-size: 14px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
    -webkit-user-select: text;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    padding-top: 12px;
  }
  .result.error { color: #ff8a80; }

  /* Markdown rendering of the agent answer */
  .markdown { white-space: normal; }
  .markdown :global(h1),
  .markdown :global(h2),
  .markdown :global(h3) { font-size: 15px; margin: 10px 0 4px; font-weight: 700; }
  .markdown :global(p) { margin: 6px 0; }
  .markdown :global(ul),
  .markdown :global(ol) { margin: 6px 0; padding-left: 20px; }
  .markdown :global(li) { margin: 2px 0; }
  .markdown :global(a) { color: #8aa0ff; text-decoration: underline; }
  .markdown :global(code) {
    background: rgba(255, 255, 255, 0.08);
    padding: 1px 5px;
    border-radius: 5px;
    font-size: 12px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .markdown :global(pre) {
    background: rgba(0, 0, 0, 0.3);
    padding: 10px;
    border-radius: 8px;
    overflow-x: auto;
  }
  .markdown :global(pre code) { background: none; padding: 0; }
  .markdown :global(strong) { font-weight: 700; color: #fff; }
  .markdown :global(blockquote) {
    border-left: 3px solid rgba(255, 255, 255, 0.2);
    margin: 6px 0;
    padding-left: 10px;
    color: #b9b9c2;
  }

  .progress {
    flex: 1;
    overflow-y: auto;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    padding-top: 12px;
    font-size: 13px;
    color: #b9b9c2;
  }
  .progress-line { padding: 3px 0; }

  .hint {
    font-size: 12px;
    color: #76767f;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    padding-top: 12px;
  }

  .actions { display: flex; gap: 8px; flex-shrink: 0; }
  .actions button {
    font: inherit;
    font-size: 12px;
    padding: 6px 12px;
    border-radius: 8px;
    border: 1px solid rgba(120, 160, 255, 0.25);
    background: rgba(96, 134, 230, 0.18);
    color: #dce4ff;
    cursor: pointer;
  }
  .actions button.ghost {
    background: transparent;
    border-color: rgba(255, 255, 255, 0.12);
    color: #b8b8c0;
  }
  .actions button:hover { filter: brightness(1.15); }

  .min-dot {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 44px;
    height: 44px;
    border-radius: 50%;
    border: none;
    cursor: pointer;
    background: radial-gradient(circle at 35% 30%, #b69bff, #7b5cff 70%);
    box-shadow: 0 4px 16px rgba(123, 92, 255, 0.5);
    animation: dotpulse 1.6s ease-in-out infinite;
  }
  .min-dot.busy {
    background: radial-gradient(circle at 35% 30%, #b69bff, #7b5cff 70%);
    animation: dotpulse 0.9s ease-in-out infinite;
  }
  .min-dot.done {
    background: radial-gradient(circle at 35% 30%, #6ee7a0, #2ecc71 70%);
    box-shadow: 0 4px 16px rgba(46, 204, 113, 0.5);
  }
  @keyframes dotpulse {
    0%, 100% { transform: translate(-50%, -50%) scale(1); opacity: 1; }
    50% { transform: translate(-50%, -50%) scale(0.82); opacity: 0.7; }
  }

  .spinner {
    width: 14px; height: 14px;
    border: 2px solid rgba(255, 255, 255, 0.25);
    border-top-color: #8aa0ff;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
