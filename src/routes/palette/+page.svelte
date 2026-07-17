<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { marked } from "marked";
  import { Chat } from "@ai-sdk/svelte";
  import type { UIMessage } from "ai";
  import KeyboardHints, { type KeyboardHint } from "$lib/components/KeyboardHints.svelte";
  import { getAppSettings, hubListModels } from "$lib/api";
  import {
    createAgentTransport,
    captureActiveWindowDataUrl,
    hubAgentBlockReason,
    sessionCanPersist,
    trimOrphanedUserMessages,
  } from "$lib/agent";
  import { t } from "$lib/i18n";
  import { invokeErrorMessage } from "$lib/exclusion-label";
  import { resetFocusState } from "$lib/input-modality";
  import {
    isPaletteDotLogicalSize,
    loadPaletteRestoreSize,
    savePaletteRestoreSize,
    type PaletteRestoreSize,
  } from "$lib/palette-window";
  import "$lib/styles/palette.css";

  marked.setOptions({ breaks: true, gfm: true });
  const md = (s: string) => marked.parse(s) as string;

  // Explicit window dragging — data-tauri-drag-region is unreliable on the
  // converted NSPanel, so start dragging on mousedown over the top bar.
  // Skip interactive controls (button, select, input) — otherwise the drag
  // preventDefault() swallows their clicks (e.g. the model <select> never opens).
  function startDrag(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("button, select, input, textarea, a, [role='button']"))
      return;
    if (e.button === 0) {
      e.preventDefault();
      getCurrentWindow().startDragging();
    }
  }

  type Mode = "search" | "agent";

  let mode = $state<Mode>("agent");
  let input = $state("");
  let inputEl: HTMLTextAreaElement | undefined = $state();
  let transcriptEl: HTMLDivElement | undefined = $state();

  // Web-search mode is still single-shot (one query -> one answer).
  let searchAnswer = $state("");
  let searchLoading = $state(false);
  let searchAnswerHtml = $derived(searchAnswer ? md(searchAnswer) : "");

  type StatusTone = "neutral" | "warn" | "fail";
  let statusNotice = $state("");
  let statusNoticeTone = $state<StatusTone>("neutral");
  let recording = $state(false);
  let elapsed = $state(0);

  // Agent model selection (persisted; defaults to the hub chat model).
  const AGENT_MODEL_KEY = "paletteAgentModel";
  const TRANSLUCENT_KEY = "paletteTranslucent";
  let agentModel = $state<string>(localStorage.getItem(AGENT_MODEL_KEY) || "");
  let models = $state<string[]>([]);
  let hubEnabled = $state(false);
  let hubUrl = $state("");
  let hubToken = $state("");
  let hubChatModel = $state("");
  const MODEL_FALLBACKS = ["qwen3.6-35b-a3b", "gpt-oss-120b", "gemma-4-31b"] as const;
  let attachScreenshot = $state(false);
  let translucent = $state(localStorage.getItem(TRANSLUCENT_KEY) === "1");

  function agentHubBlockReason(): string | null {
    return hubAgentBlockReason({
      hub_enabled: hubEnabled,
      hub_url: hubUrl,
      hub_token: hubToken,
    });
  }

  // Client-side ReAct chat (Vercel AI SDK). Recreated on "New" / history load.
  function makeChat(initial: UIMessage[] = []) {
    return new Chat({
      messages: initial,
      transport: createAgentTransport(() => agentModel),
      onError: (e) => {
        revertFailedUserTurn();
        setStatusNotice(e?.message || "Agent request failed", "fail");
      },
    });
  }
  let chat = $state(makeChat());
  let sessionId = $state<string>(crypto.randomUUID());

  const agentBusy = $derived(chat.status === "submitted" || chat.status === "streaming");
  const loading = $derived(mode === "agent" ? agentBusy : searchLoading);
  const agentMessages = $derived(chat.messages);
  const hasConversation = $derived(mode === "agent" ? agentMessages.length > 0 : !!searchAnswer);

  function revertFailedUserTurn() {
    const trimmed = trimOrphanedUserMessages(chat.messages);
    if (trimmed.length === chat.messages.length) return;
    chat = makeChat(trimmed);
  }

  function syncAgentMessagesFromHubState() {
    if (!agentHubBlockReason()) return;
    const trimmed = trimOrphanedUserMessages(chat.messages);
    if (trimmed.length !== chat.messages.length) chat = makeChat(trimmed);
  }

  function lastAssistantText(): string {
    for (let i = chat.messages.length - 1; i >= 0; i--) {
      const m = chat.messages[i];
      if (m.role !== "assistant") continue;
      const text = m.parts
        .filter((p) => p.type === "text")
        .map((p) => (p as { text: string }).text)
        .join("\n")
        .trim();
      if (text) return text;
    }
    return "";
  }
  const answerForActions = $derived(mode === "agent" ? lastAssistantText() : searchAnswer);

  function toolLabel(name: string): { icon: string; label: string } {
    switch (name) {
      case "web_search":
        return { icon: "🔎", label: "Web search" };
      case "create_note":
        return { icon: "📝", label: "Note" };
      case "create_reminder":
        return { icon: "⏰", label: "Reminder" };
      case "list_reminders":
        return { icon: "⏰", label: "Reminders" };
      case "read_calendar":
        return { icon: "📅", label: "Calendar" };
      default:
        return { icon: "🛠", label: name };
    }
  }

  // --- session history (persists full AI SDK message arrays) ------------------
  type Session = { id: string; title: string; ts: number; messages: UIMessage[] };
  let sessions = $state<Session[]>([]);
  let showHistory = $state(false);

  function loadSessions() {
    try {
      sessions = JSON.parse(localStorage.getItem("agentSessions") || "[]");
    } catch {
      sessions = [];
    }
  }
  function persistSessions() {
    localStorage.setItem("agentSessions", JSON.stringify(sessions.slice(0, 50)));
  }
  function firstUserText(msgs: UIMessage[]): string {
    const u = msgs.find((m) => m.role === "user");
    const text = u?.parts
      .filter((p) => p.type === "text")
      .map((p) => (p as { text: string }).text)
      .join(" ")
      .trim();
    return text || "Conversation";
  }
  function upsertSession() {
    const msgs = chat.messages;
    if (!sessionCanPersist(msgs)) return;
    const entry: Session = {
      id: sessionId,
      title: firstUserText(msgs),
      ts: Date.now(),
      messages: msgs as UIMessage[],
    };
    const rest = sessions.filter((s) => s.id !== sessionId);
    sessions = [entry, ...rest].slice(0, 50);
    persistSessions();
  }
  // Persist whenever a turn completes.
  $effect(() => {
    if (chat.status === "ready" && chat.messages.length > 0) upsertSession();
  });

  function clearHistory() {
    sessions = [];
    persistSessions();
  }
  function openSession(s: Session) {
    mode = "agent";
    sessionId = s.id;
    chat = makeChat(s.messages);
    searchAnswer = "";
    clearStatusNotice();
    showHistory = false;
    setTimeout(() => inputEl?.focus(), 30);
  }
  function newConversation() {
    sessionId = crypto.randomUUID();
    chat = makeChat();
    searchAnswer = "";
    input = "";
    clearStatusNotice();
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

  // The hub /v1/models list mixes chat models with embedding/reranker models;
  // only chat models make sense for the agent, so drop the non-chat ones.
  const NON_CHAT_MODEL = /embed|rerank|^bge|^e5-|jina|frida|giga/i;
  function isChatModel(id: string): boolean {
    return !NON_CHAT_MODEL.test(id);
  }
  function modelOptions(): string[] {
    if (!hubEnabled) return [];
    const chat = models.filter(isChatModel);
    const base = chat.length ? chat : [...MODEL_FALLBACKS];
    return agentModel && !base.includes(agentModel) ? [agentModel, ...base] : base;
  }
  function syncAgentModelSelection() {
    if (!hubEnabled) return;
    const options = modelOptions();
    if (options.length === 0) return;
    const preferred = hubChatModel && options.includes(hubChatModel) ? hubChatModel : options[0];
    if (!agentModel || !options.includes(agentModel)) {
      agentModel = preferred;
      localStorage.setItem(AGENT_MODEL_KEY, agentModel);
    }
  }
  function onModelChange(e: Event) {
    agentModel = (e.currentTarget as HTMLSelectElement).value;
    localStorage.setItem(AGENT_MODEL_KEY, agentModel);
  }
  function toggleTranslucent() {
    translucent = !translucent;
    localStorage.setItem(TRANSLUCENT_KEY, translucent ? "1" : "0");
  }
  let hubSettingsLoadGeneration = 0;
  let lastHubModelsCredsKey = "";

  function hubModelsCredsKey(s: {
    hub_enabled: boolean;
    hub_url: string;
    hub_token: string;
  }): string {
    return `${s.hub_enabled}\0${s.hub_url.trim()}\0${s.hub_token.trim()}`;
  }

  async function loadAgentModelDefaults(loadModels = true) {
    const generation = ++hubSettingsLoadGeneration;
    try {
      const s = await getAppSettings();
      const credsKey = hubModelsCredsKey(s);
      const shouldFetchModels =
        loadModels && s.hub_enabled && (models.length === 0 || credsKey !== lastHubModelsCredsKey);
      const nextModels = shouldFetchModels
        ? await hubListModels(s.hub_url, s.hub_token)
        : models;
      if (generation !== hubSettingsLoadGeneration) return;
      hubEnabled = s.hub_enabled;
      hubUrl = s.hub_url;
      hubToken = s.hub_token;
      hubChatModel = s.hub_chat_model;
      if (!agentModel) agentModel = s.hub_chat_model;
      if (loadModels) {
        if (!s.hub_enabled) {
          models = [];
          lastHubModelsCredsKey = "";
        } else if (shouldFetchModels) {
          models = nextModels;
          lastHubModelsCredsKey = credsKey;
        }
      }
    } catch {
      if (generation !== hubSettingsLoadGeneration) return;
      hubEnabled = false;
      hubUrl = "";
      hubToken = "";
      models = [];
      lastHubModelsCredsKey = "";
    }
    syncAgentModelSelection();
    syncAgentMessagesFromHubState();
  }

  const paletteShortcutHints = $derived<KeyboardHint[]>([
    { keys: "↵", action: mode === "agent" ? $t("palette.run.agent") : $t("palette.run.search") },
    { keys: "⇧↵", action: "New line" },
    { keys: "Tab", action: $t("palette.switchMode") },
    { keys: "⌘↵", action: $t("palette.insert") },
    { keys: "Esc", action: $t("palette.close") },
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
  /** Center placeholder vs status rail — never show both. */
  const showCenterPlaceholder = $derived(!statusNotice);
  // Drop failed user-only tails when the hub is unavailable (not just hide in UI).
  $effect(() => {
    void hubEnabled;
    void hubUrl;
    void hubToken;
    if (agentHubBlockReason()) syncAgentMessagesFromHubState();
  });

  function toggleMode() {
    clearStatusNotice();
    mode = mode === "agent" ? "search" : "agent";
    if (mode === "agent") syncAgentMessagesFromHubState();
  }
  function toggleHistory() {
    loadSessions();
    const opening = !showHistory;
    if (opening) clearStatusNotice();
    showHistory = opening;
  }

  // Drag bottom-right corner to resize.
  function startResize(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    getCurrentWindow().startResizeDragging("SouthEast");
  }

  // Collapse the whole panel to an amorphous blob and back.
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
      restoreSize = { w: Math.round(sz.width / f), h: Math.round(sz.height / f) };
      savePaletteRestoreSize(restoreSize);
    } catch {
      /* keep last persisted size */
    }
  }
  async function minimize() {
    await captureAndPersistRestoreSize();
    clearStatusNotice();
    resetFocusState();
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

  // Live elapsed counter while the agent is working.
  $effect(() => {
    if (!loading) return;
    elapsed = 0;
    const timer = setInterval(() => (elapsed += 1), 1000);
    return () => clearInterval(timer);
  });

  // Auto-scroll the transcript to the newest content.
  $effect(() => {
    // touch reactive deps
    void chat.messages.length;
    void chat.status;
    void searchAnswer;
    const el = transcriptEl;
    if (!el) return;
    void tick().then(() => {
      el.scrollTop = el.scrollHeight;
      return undefined;
    });
  });

  // Refocus the composer once the agent finishes a turn, so a follow-up can be
  // typed immediately without clicking back into the input.
  $effect(() => {
    if (mode === "agent" && chat.status === "ready" && chat.messages.length > 0 && !minimized) {
      void tick().then(() => {
        inputEl?.focus();
        return undefined;
      });
    }
  });

  function autoGrow() {
    if (!inputEl) return;
    inputEl.style.height = "auto";
    inputEl.style.height = `${Math.min(inputEl.scrollHeight, 140)}px`;
  }

  async function submit() {
    const text = input.trim();
    if (!text || loading) return;
    clearStatusNotice();
    showHistory = false;
    if (mode === "agent") {
      await loadAgentModelDefaults(false);
      const hubBlock = agentHubBlockReason();
      if (hubBlock) {
        setStatusNotice(hubBlock, "fail");
        return;
      }
      let files: { type: "file"; mediaType: string; url: string }[] | undefined;
      if (attachScreenshot) {
        const url = await captureActiveWindowDataUrl();
        if (url) files = [{ type: "file", mediaType: "image/png", url }];
        attachScreenshot = false;
      }
      input = "";
      autoGrow();
      try {
        await chat.sendMessage(files ? { text, files } : { text });
      } catch (e) {
        revertFailedUserTurn();
        input = text;
        autoGrow();
        setInvokeFailure(e, "Request failed. Try again.");
      }
    } else {
      input = "";
      autoGrow();
      searchLoading = true;
      searchAnswer = "";
      try {
        searchAnswer = await invoke<string>("palette_search", { query: text });
      } catch (e) {
        setInvokeFailure(e, "Request failed. Try again.");
      }
      searchLoading = false;
    }
  }

  function stopAgent() {
    void chat.stop();
  }

  async function toggleMic() {
    if (recording) {
      recording = false;
      clearStatusNotice();
      try {
        const text = await invoke<string>("palette_voice_stop");
        if (text.trim()) {
          input = text.trim();
          autoGrow();
          void submit();
        }
      } catch (e) {
        setInvokeFailure(e, "Voice input failed. Try again.");
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
    if (!answerForActions) return;
    await invoke("palette_insert", { text: answerForActions });
  }
  async function copy() {
    if (!answerForActions) return;
    await navigator.clipboard.writeText(answerForActions).catch(() => {});
  }
  async function close() {
    // Hide only — keep the conversation so reopening shows it. Use ＋ to clear.
    clearStatusNotice();
    resetFocusState();
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
    } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      insert();
    }
  }

  // Enter on the composer sends; Shift+Enter inserts a newline; Tab switches mode.
  function onComposerKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && !e.metaKey && !e.ctrlKey) {
      e.preventDefault();
      void submit();
    } else if (e.key === "Tab" && !input.trim()) {
      e.preventDefault();
      toggleMode();
    }
  }

  onMount(() => {
    const unlistens = [
      listen("palette-show", async () => {
        clearStatusNotice();
        await syncDotModeFromWindow();
        await loadAgentModelDefaults();
        if (!minimized) {
          setTimeout(() => {
            inputEl?.focus();
            inputEl?.select();
          }, 40);
        }
      }),
      listen("palette-hide", () => {
        resetFocusState();
      }),
      listen("hub-settings-changed", () => {
        void loadAgentModelDefaults();
      }),
    ];
    loadSessions();
    void loadAgentModelDefaults();
    void initPaletteState().then(() => {
      if (!minimized) inputEl?.focus();
      return undefined;
    });

    // Sticky-to-edges: snap to a nearby edge after the window stops moving.
    let moveTimer: ReturnType<typeof setTimeout> | undefined;
    const movedPromise = getCurrentWindow().onMoved(() => {
      clearTimeout(moveTimer);
      moveTimer = setTimeout(() => {
        void invoke("palette_snap_to_edges").catch(() => {});
      }, 180);
    });

    return () => {
      clearTimeout(moveTimer);
      void movedPromise.then((fn) => fn());
      unlistens.forEach((u) => u.then((fn) => fn()));
    };
  });
</script>

<svelte:window on:keydown={onKeydown} />

{#if paletteReady}
  {#if minimized}
    <div
      class="min-blob-shell"
      role="button"
      tabindex="0"
      data-tauri-drag-region="deep"
      title="Drag to move · double-click or Enter to expand"
      aria-label="Agent status. Drag to move, double-click or press Enter to expand."
      ondblclick={dotDblClick}
    >
      <span class="min-blob" class:busy={loading} class:done={!loading && hasConversation} aria-hidden="true"></span>
    </div>
  {:else}
    <div class="palette" class:translucent class:mode-web={mode === "search"} class:mode-agent={mode === "agent"}>
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
            {mode === "agent" ? $t("palette.agent") : $t("palette.web")}
          </button>
          {#if mode === "agent"}
            <select
              class="form-select model-select"
              class:is-unavailable={!hubEnabled}
              title={hubEnabled ? "Agent model" : "Enable NeuralDeep hub in Settings to choose a model"}
              aria-label="Agent model"
              disabled={!hubEnabled}
              value={hubEnabled ? agentModel : ""}
              onchange={onModelChange}
            >
              {#if !hubEnabled}
                <option value="">Hub disabled</option>
              {:else}
                {#each modelOptions() as m (m)}
                  <option value={m}>{m}</option>
                {/each}
              {/if}
            </select>
          {/if}
          {#if loading}
            <span class="run-dot" title="Agent running"></span>
            <span class="run-label">running… {elapsed}s</span>
          {/if}
          <div class="topbar-spacer"></div>
          <button
            class="bar-btn overlay-icon-btn app-btn"
            class:active={translucent}
            type="button"
            title="Transparency (glass) mode"
            aria-label="Toggle transparency"
            aria-pressed={translucent}
            onclick={toggleTranslucent}
          >
            <svg class="overlay-icon-btn-icon" viewBox="0 0 24 24" aria-hidden="true">
              <rect x="3" y="3" width="18" height="18" rx="3" />
              <path d="M3 9h18M9 3v18" />
            </svg>
          </button>
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
            title="New conversation"
            aria-label="New conversation"
            onclick={newConversation}
          >
            <svg class="overlay-icon-btn-icon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M12 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
              <path d="M18.375 2.625a1 1 0 0 1 1.414 0l2.586 2.586a1 1 0 0 1 0 1.414L12.625 14.125 9 15l.875-3.625Z" />
            </svg>
          </button>
          <button
            class="bar-btn overlay-icon-btn app-btn"
            type="button"
            title="Compact to blob"
            aria-label="Compact to blob"
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
            title="Hide (Esc) — conversation is preserved"
            aria-label="Close"
            onclick={close}
          >
            <svg class="overlay-icon-btn-icon overlay-icon-btn-icon--close" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M5 5 19 19M19 5 5 19" />
            </svg>
          </button>
        </header>
      </div>

      <!-- Transcript / history / search result -->
      <div class="transcript" bind:this={transcriptEl}>
        {#if showHistory}
          {#if sessions.length === 0}
            {#if showCenterPlaceholder}
              <div class="empty-hint">
                <svg class="empty-icon" viewBox="0 0 24 24" aria-hidden="true">
                  <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
                  <path d="M3 3v5h5" />
                  <path d="M12 7v5l4 2" />
                </svg>
                <p>{$t("palette.noHistory")}</p>
              </div>
            {/if}
          {:else}
            <div class="history-head">
              <span class="history-count">{$t("palette.recent", { n: sessions.length })}</span>
              <button class="history-clear app-btn" type="button" onclick={clearHistory}>{$t("common.clear")}</button>
            </div>
            {#each sessions as s (s.id)}
              <button class="history-item app-btn" type="button" onclick={() => openSession(s)}>
                <span class="history-mode agent">A</span>
                <span class="history-q">{s.title}</span>
                <span class="history-time">{timeAgo(s.ts)}</span>
              </button>
            {/each}
          {/if}
        {:else if mode === "search"}
          {#if searchAnswerHtml}
            <div class="msg msg-assistant">
              <!-- eslint-disable-next-line svelte/no-at-html-tags -->
              <div class="bubble markdown">{@html searchAnswerHtml}</div>
            </div>
          {:else if !searchLoading && showCenterPlaceholder}
            <div class="empty-hint">
              <svg class="empty-icon empty-icon--web" viewBox="0 0 24 24" aria-hidden="true">
                <circle cx="11" cy="11" r="8" />
                <line x1="21" y1="21" x2="16.65" y2="16.65" />
              </svg>
              <p>{$t("palette.searchWeb")}</p>
            </div>
          {/if}
        {:else}
          {#if agentMessages.length === 0 && !agentBusy && showCenterPlaceholder}
            <div class="empty-hint">
              <svg class="empty-icon empty-icon--agent" viewBox="0 0 24 24" aria-hidden="true">
                <path
                  d="m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275L12 3Z"
                />
                <path d="M5 3v4M3 5h4" />
                <path d="M19 17v4M17 19h4" />
              </svg>
              <p>{$t("palette.askAgent")}</p>
            </div>
          {/if}
          {#each agentMessages as message (message.id)}
            {#if message.role === "user"}
              <div class="msg msg-user">
                <div class="bubble">
                  {#each message.parts as part, i (i)}
                    {#if part.type === "text"}
                      <span>{part.text}</span>
                    {:else if part.type === "file" && (part as { mediaType?: string }).mediaType?.startsWith("image/")}
                      <img class="msg-image" src={(part as { url: string }).url} alt="attached screenshot" />
                    {/if}
                  {/each}
                </div>
              </div>
            {:else if message.role === "assistant"}
              <div class="msg msg-assistant">
                <div class="msg-parts">
                  {#each message.parts as part, i (i)}
                    {#if part.type === "reasoning"}
                      {#if (part as { text: string }).text.trim()}
                        <details class="reasoning" open={agentBusy && i === message.parts.length - 1}>
                          <summary>💭 Reasoning</summary>
                          <div class="reasoning-body">{(part as { text: string }).text}</div>
                        </details>
                      {/if}
                    {:else if part.type === "text"}
                      {#if part.text.trim()}
                        <!-- eslint-disable-next-line svelte/no-at-html-tags -->
                        <div class="bubble markdown">{@html md(part.text)}</div>
                      {/if}
                    {:else if part.type.startsWith("tool-")}
                      {@const tp = part as {
                        type: string;
                        state?: string;
                        input?: unknown;
                        output?: unknown;
                        errorText?: string;
                      }}
                      {@const meta = toolLabel(tp.type.slice(5))}
                      <details class="tool-chip" class:running={tp.state === "input-available" || tp.state === "input-streaming"} class:failed={tp.state === "output-error"}>
                        <summary>
                          <span class="tool-icon">{meta.icon}</span>
                          <span class="tool-name">{meta.label}</span>
                          {#if tp.state === "input-available" || tp.state === "input-streaming"}
                            <span class="tool-spinner" aria-hidden="true"></span>
                          {:else if tp.state === "output-error"}
                            <span class="tool-badge fail">error</span>
                          {:else}
                            <span class="tool-badge ok">done</span>
                          {/if}
                        </summary>
                        <div class="tool-body">
                          {#if tp.input}
                            <div class="tool-io-label">input</div>
                            <pre class="tool-io">{JSON.stringify(tp.input, null, 2)}</pre>
                          {/if}
                          {#if tp.state === "output-error"}
                            <div class="tool-io-label">error</div>
                            <pre class="tool-io">{tp.errorText}</pre>
                          {:else if tp.output != null}
                            <div class="tool-io-label">output</div>
                            <pre class="tool-io">{typeof tp.output === "string" ? tp.output : JSON.stringify(tp.output, null, 2)}</pre>
                          {/if}
                        </div>
                      </details>
                    {/if}
                  {/each}
                </div>
              </div>
            {/if}
          {/each}
          {#if agentBusy && (chat.lastMessage?.role !== "assistant" || chat.lastMessage.parts.every((p) => (p.type === "text" || p.type === "reasoning") && !(p as { text: string }).text.trim()))}
            <div class="msg msg-assistant">
              <div class="thinking"><span></span><span></span><span></span></div>
            </div>
          {/if}
        {/if}
      </div>

      <!-- Action strip for the latest answer -->
      {#if hasConversation && !loading && !showHistory && answerForActions}
        <div class="actions">
          <button class="app-btn" onclick={insert}>{$t("common.insert")} ⌘↵</button>
          <button class="app-btn" onclick={copy}>{$t("common.copy")}</button>
        </div>
      {/if}

      <!-- Transient status/errors — fixed rail above composer (HIG: feedback near input). -->
      <div class="palette-status-rail" aria-live="polite" aria-atomic="true">
        {#if statusNotice}
          <p
            class="palette-status-msg overlay-status-hint"
            class:neutral={statusNoticeTone === "neutral"}
            class:warn={statusNoticeTone === "warn"}
            class:fail={statusNoticeTone === "fail"}
            role={statusNoticeTone === "fail" ? "alert" : "status"}
          >
            {statusNotice}
          </p>
        {/if}
      </div>

      <!-- Composer (input at the bottom) -->
      <div class="composer query-field" class:recording role="search">
        <textarea
          bind:this={inputEl}
          bind:value={input}
          class="query-input composer-input"
          rows="1"
          aria-label={mode === "agent" ? "Ask the agent" : "Search the web"}
          placeholder={mode === "agent" ? $t("palette.askAgent") : $t("palette.searchWeb")}
          autocomplete="off"
          spellcheck="false"
          disabled={recording}
          oninput={() => {
            clearStatusNotice();
            autoGrow();
          }}
          onkeydown={onComposerKeydown}
        ></textarea>
        <div class="composer-trailing">
        {#if mode === "agent"}
          <button
            class="shot-btn app-btn"
            class:active={attachScreenshot}
            type="button"
            title={attachScreenshot
              ? "Screenshot attached — the agent will see the active window"
              : "Attach a screenshot of the active window for the agent"}
            aria-label="Attach screenshot of the active window"
            aria-pressed={attachScreenshot}
            disabled={loading}
            onclick={() => (attachScreenshot = !attachScreenshot)}
          >
            <svg class="shot-btn-icon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z" />
              <circle cx="12" cy="13" r="4" />
            </svg>
          </button>
        {/if}
        <button
          class="mic-btn app-btn"
          class:recording
          type="button"
          title={recording ? "Stop voice input" : "Voice input"}
          aria-label={recording ? "Stop voice input" : "Start voice input"}
          aria-pressed={recording}
          onclick={toggleMic}
        >
          <svg class="mic-btn-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
            <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
            <path d="M12 19v3" />
          </svg>
        </button>
        {#if agentBusy}
          <button class="send-btn stop app-btn" type="button" title="Stop" aria-label="Stop" onclick={stopAgent}>
            <svg viewBox="0 0 24 24" aria-hidden="true"><rect x="6" y="6" width="12" height="12" rx="2" /></svg>
          </button>
        {:else}
          <button
            class="send-btn app-btn"
            type="button"
            title="Send (↵)"
            aria-label="Send"
            disabled={!input.trim() || loading}
            onclick={submit}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 12h14M12 5l7 7-7 7" /></svg>
          </button>
        {/if}
        </div>
      </div>

      <footer class="overlay-shortcuts overlay-footer-strip">
        <KeyboardHints hints={paletteShortcutHints} />
      </footer>

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

<style>
  /* Glass / transparency mode. */
  .palette.translucent {
    background: color-mix(in oklab, var(--surface-overlay) 52%, transparent);
    backdrop-filter: blur(calc(var(--blur-palette-panel) * 1.4));
    -webkit-backdrop-filter: blur(calc(var(--blur-palette-panel) * 1.4));
  }

  /* Transcript — the scrollable chat log. */
  .transcript {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 6px 2px 2px;
    user-select: text;
    -webkit-user-select: text;
  }

  .empty-hint {
    margin: auto;
    text-align: center;
    color: var(--color-text-tertiary);
    font-size: var(--font-size-md);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }
  .empty-icon {
    width: 1.75rem;
    height: 1.75rem;
    fill: none;
    stroke: currentcolor;
    stroke-width: 1.75;
    stroke-linecap: round;
    stroke-linejoin: round;
    opacity: 0.9;
  }
  .empty-icon--agent {
    color: var(--palette-mode-accent);
  }
  .empty-icon--web {
    color: var(--palette-mode-accent);
  }

  .msg {
    display: flex;
    flex-direction: column;
    max-width: 100%;
  }
  .msg-user {
    align-items: flex-end;
  }
  .msg-assistant {
    align-items: flex-start;
  }
  .msg-parts {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
  }

  .bubble {
    font-size: var(--font-size-base);
    line-height: var(--line-height-body);
    overflow-wrap: break-word;
    word-break: normal;
  }
  .msg-user .bubble {
    background: var(--surface-agent-muted);
    color: var(--color-text-primary);
    padding: 8px 12px;
    border-radius: 14px 14px 4px 14px;
    max-width: 85%;
    white-space: pre-wrap;
  }
  .msg-assistant .bubble {
    color: var(--color-text-primary);
    max-width: 100%;
  }
  .msg-image {
    display: block;
    max-width: 180px;
    max-height: 120px;
    border-radius: 10px;
    margin-top: 6px;
  }

  /* Reasoning — collapsible "thinking" block. */
  .reasoning {
    border: 1px solid var(--border-soft);
    border-radius: 10px;
    background: var(--surface-4);
    font-size: var(--font-size-sm);
  }
  .reasoning > summary {
    cursor: pointer;
    padding: 6px 10px;
    color: var(--color-text-tertiary);
    list-style: none;
    user-select: none;
  }
  .reasoning > summary::-webkit-details-marker {
    display: none;
  }
  .reasoning-body {
    padding: 0 10px 8px;
    color: var(--color-text-subtle);
    white-space: pre-wrap;
    line-height: var(--line-height-body);
    border-top: 1px solid var(--border-soft);
    margin-top: 2px;
    padding-top: 8px;
  }

  /* Tool call chip. */
  .tool-chip {
    border: 1px solid var(--border-soft);
    border-radius: 10px;
    background: var(--surface-4);
    font-size: var(--font-size-sm);
    align-self: flex-start;
    max-width: 100%;
  }
  .tool-chip.running {
    border-color: var(--border-agent);
    background: var(--surface-agent-muted);
  }
  .tool-chip.failed {
    border-color: rgb(var(--rgb-recording) / 40%);
  }
  .tool-chip > summary {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    padding: 6px 10px;
    list-style: none;
    user-select: none;
    color: var(--color-text-secondary);
  }
  .tool-chip > summary::-webkit-details-marker {
    display: none;
  }
  .tool-name {
    font-weight: 600;
  }
  .tool-badge {
    margin-left: auto;
    font-size: var(--font-size-xs);
    padding: 1px 6px;
    border-radius: 6px;
  }
  .tool-badge.ok {
    background: var(--surface-8);
    color: var(--color-text-tertiary);
  }
  .tool-badge.fail {
    background: rgb(var(--rgb-recording) / 20%);
    color: var(--color-recording);
  }
  .tool-spinner {
    margin-left: auto;
    width: 12px;
    height: 12px;
    border: 2px solid var(--border-emphasis);
    border-top-color: transparent;
    border-radius: 50%;
    animation: palette-spin 0.7s linear infinite;
  }
  .tool-body {
    padding: 0 10px 8px;
    border-top: 1px solid var(--border-soft);
  }
  .tool-io-label {
    font-size: var(--font-size-xs);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--color-text-faint);
    margin: 8px 0 2px;
  }
  .tool-io {
    margin: 0;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    line-height: 1.4;
    color: var(--color-text-subtle);
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 180px;
    overflow-y: auto;
  }

  @keyframes palette-spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Typing indicator while the model warms up. */
  .thinking {
    display: inline-flex;
    gap: 5px;
    padding: 8px 4px;
  }
  .thinking span {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--color-agent);
    opacity: 0.5;
    animation: palette-typing 1.2s ease-in-out infinite;
  }
  .thinking span:nth-child(2) {
    animation-delay: 0.2s;
  }
  .thinking span:nth-child(3) {
    animation-delay: 0.4s;
  }
  @keyframes palette-typing {
    0%,
    100% {
      opacity: 0.3;
      transform: translateY(0);
    }
    50% {
      opacity: 1;
      transform: translateY(-3px);
    }
  }

  /* Composer — textarea input at the bottom. */
  .composer {
    height: auto;
    min-height: 44px;
    align-items: flex-end;
    padding-top: 8px;
    padding-bottom: 8px;
  }
  .composer-input {
    resize: none;
    max-height: 140px;
    overflow-y: auto;
    padding: 3px 0;
    line-height: 1.4;
  }
  .composer-trailing {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .send-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    flex-shrink: 0;
    border: none;
    border-radius: 50%;
    background: var(--palette-mode-accent);
    color: var(--color-on-accent, #fff);
    cursor: pointer;
    transition: filter var(--duration-fast) var(--ease-interactive);
  }
  .send-btn:disabled {
    opacity: var(--opacity-disabled);
    cursor: default;
  }
  .send-btn:hover:not(:disabled) {
    filter: brightness(1.1);
  }
  .send-btn.stop {
    background: var(--color-recording);
  }
  .send-btn svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentcolor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .send-btn.stop svg {
    fill: currentcolor;
    stroke: none;
  }

  .history-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.1rem 0.2rem 0.35rem;
  }
  .history-count {
    font-size: 0.72rem;
    opacity: 0.6;
  }
  .history-clear {
    font-size: 0.72rem;
    padding: 0.15rem 0.5rem;
    border-radius: 6px;
    border: 1px solid var(--surface-10);
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
  .history-clear:hover {
    background: var(--surface-6);
  }

  /* --- Amorphous "blob" for the collapsed state ---------------------------- */
  .min-blob-shell {
    position: fixed;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: grab;
    touch-action: none;
    background: rgb(0 0 0 / 0.001%);
    outline: none;
  }
  .min-blob-shell:active {
    cursor: grabbing;
  }
  .min-blob {
    width: 72%;
    height: 72%;
    max-width: 120px;
    max-height: 120px;
    pointer-events: none;
    background: var(--gradient-agent-orb);
    box-shadow: var(--shadow-agent-orb);
    filter: blur(0.2px) saturate(1.15);
    border-radius: 42% 58% 63% 37% / 41% 44% 56% 59%;
    animation:
      blob-morph 8s ease-in-out infinite,
      blob-breathe 3.2s ease-in-out infinite;
  }
  .min-blob.busy {
    animation:
      blob-morph 3.5s ease-in-out infinite,
      blob-breathe 1s ease-in-out infinite;
  }
  .min-blob.done {
    background: var(--gradient-palette-success-orb);
    box-shadow: var(--shadow-palette-success-orb);
  }
  @keyframes blob-morph {
    0%,
    100% {
      border-radius: 42% 58% 63% 37% / 41% 44% 56% 59%;
    }
    33% {
      border-radius: 63% 37% 44% 56% / 56% 63% 37% 44%;
    }
    66% {
      border-radius: 37% 63% 51% 49% / 62% 44% 56% 38%;
    }
  }
  @keyframes blob-breathe {
    0%,
    100% {
      transform: scale(1) rotate(0deg);
      opacity: 1;
    }
    50% {
      transform: scale(0.9) rotate(8deg);
      opacity: 0.82;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .min-blob {
      animation: none;
    }
  }
</style>
