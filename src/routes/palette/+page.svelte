<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  let query = $state("");
  let answer = $state("");
  let error = $state("");
  let loading = $state(false);
  let inputEl: HTMLInputElement | undefined = $state();

  function reset() {
    query = "";
    answer = "";
    error = "";
    loading = false;
  }

  async function runSearch() {
    const q = query.trim();
    if (!q || loading) return;
    loading = true;
    answer = "";
    error = "";
    try {
      answer = await invoke<string>("palette_search", { query: q });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
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
    await invoke("palette_hide");
    reset();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      insert();
    } else if (e.key === "Enter") {
      e.preventDefault();
      runSearch();
    }
  }

  onMount(() => {
    const unlisten = listen("palette-show", () => {
      reset();
      // focus shortly after the panel becomes key
      setTimeout(() => inputEl?.focus(), 40);
    });
    inputEl?.focus();
    return () => {
      unlisten.then((fn) => fn());
    };
  });
</script>

<svelte:window on:keydown={onKeydown} />

<div class="palette">
  <div class="search-row">
    <svg class="search-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="11" cy="11" r="7" /><line x1="21" y1="21" x2="16.65" y2="16.65" />
    </svg>
    <input
      bind:this={inputEl}
      bind:value={query}
      class="search-input"
      type="text"
      placeholder="Search the web via NeuralDeep…"
      autocomplete="off"
      spellcheck="false"
    />
    {#if loading}
      <span class="spinner"></span>
    {/if}
  </div>

  {#if error}
    <div class="result error">{error}</div>
  {:else if answer}
    <div class="result">{answer}</div>
    <div class="actions">
      <button onclick={insert}>Insert ⌘↵</button>
      <button onclick={copy}>Copy</button>
      <button class="ghost" onclick={close}>Close Esc</button>
    </div>
  {:else if !loading}
    <div class="hint">Enter to ask · ⌘Enter to insert into the active app · Esc to close</div>
  {/if}
</div>

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

  .search-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }

  .search-icon {
    color: #9a9aa6;
    flex-shrink: 0;
  }

  .search-input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: #f2f2f5;
    font-size: 18px;
  }

  .search-input::placeholder {
    color: #76767f;
  }

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

  .result.error {
    color: #ff8a80;
  }

  .hint {
    font-size: 12px;
    color: #76767f;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    padding-top: 12px;
  }

  .actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

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

  .actions button:hover {
    filter: brightness(1.15);
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(255, 255, 255, 0.25);
    border-top-color: #8aa0ff;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
