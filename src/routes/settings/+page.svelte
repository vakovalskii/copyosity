<script lang="ts">
  import { onMount } from "svelte";
  import type { AppSettings, ExcludedApp, ModelCatalog, ModelOption } from "$lib/types";
  import {
    addExcludedApp,
    addFrontmostAppToExcluded,
    clearHistory,
    getAppSettings,
    getExcludedApps,
    getModelCatalog,
    removeExcludedApp,
    updateAppSettings,
    checkOllamaStatus,
    startOllamaServer,
    pullOllamaModel,
    testOllamaTagging,
    type OllamaStatus,
  } from "$lib/api";
  import { openUrl } from "@tauri-apps/plugin-opener";

  let settings = $state<AppSettings>({
    ollama_model: "qwen3:4b-instruct-2507-q4_K_M",
    retention_days: 30,
  });
  let modelCatalog = $state<ModelCatalog>({
    total_memory_gb: 0,
    recommended_memory_gb: 0,
    options: [],
  });
  let selectedModelPreset = $state("__custom__");
  let excludedApps: ExcludedApp[] = $state([]);
  let excludedAppInput = $state("");
  let savingSettings = $state(false);
  let settingsNotice = $state("");

  let ollamaStatus = $state<OllamaStatus | null>(null);
  let ollamaLoading = $state(false);
  let taggingResult = $state<string[] | null | undefined>(undefined);
  let taggingLoading = $state(false);

  const retentionOptions = [
    { label: "1 day", value: 1 },
    { label: "1 week", value: 7 },
    { label: "1 month", value: 30 },
    { label: "6 months", value: 180 },
  ];

  async function loadSettings() {
    settings = await getAppSettings();
    selectedModelPreset = settings.ollama_model;
  }

  async function loadModelCatalog() {
    modelCatalog = await getModelCatalog();
    if (!modelCatalog.options.some((o) => o.value === settings.ollama_model)) {
      selectedModelPreset = "__custom__";
    }
  }

  async function loadExcludedApps() {
    excludedApps = await getExcludedApps();
  }

  async function refreshOllamaStatus() {
    ollamaLoading = true;
    taggingResult = undefined;
    try {
      ollamaStatus = await checkOllamaStatus();
    } finally {
      ollamaLoading = false;
    }
  }

  async function handleStartServer() {
    ollamaLoading = true;
    try {
      await startOllamaServer();
      await refreshOllamaStatus();
    } finally {
      ollamaLoading = false;
    }
  }

  async function handlePullModel() {
    ollamaLoading = true;
    try {
      await pullOllamaModel();
      await refreshOllamaStatus();
    } finally {
      ollamaLoading = false;
    }
  }

  async function handleTestTagging() {
    taggingLoading = true;
    try {
      taggingResult = await testOllamaTagging();
    } finally {
      taggingLoading = false;
    }
  }

  onMount(() => {
    loadSettings().then(() => loadModelCatalog());
    loadExcludedApps();
    refreshOllamaStatus();
  });

  async function saveSettings() {
    savingSettings = true;
    settingsNotice = "";
    try {
      settings = await updateAppSettings(settings);
      await loadModelCatalog();
      settingsNotice = "Saved";
    } finally {
      savingSettings = false;
    }
  }

  function handleModelPresetChange(value: string) {
    selectedModelPreset = value;
    if (value !== "__custom__") {
      settings.ollama_model = value;
    }
  }

  async function handleAddExcludedApp() {
    const value = excludedAppInput.trim();
    if (!value) return;
    await addExcludedApp(value);
    excludedAppInput = "";
    await loadExcludedApps();
  }

  async function handleAddFrontmostApp() {
    const added = await addFrontmostAppToExcluded();
    settingsNotice = added ? `Excluded ${added}` : "No active app detected";
    await loadExcludedApps();
  }

  async function handleRemoveExcludedApp(id: number) {
    await removeExcludedApp(id);
    await loadExcludedApps();
  }

  async function handleClearHistory() {
    await clearHistory();
    settingsNotice = "History cleared";
  }

  let selectedModelMeta = $derived.by<ModelOption | null>(() => {
    return modelCatalog.options.find((o) => o.value === settings.ollama_model) ?? null;
  });
</script>

<div class="settings-page">
  <div class="settings-head">
    <div class="settings-title">Settings</div>
    <div class="settings-subtitle">Local AI and history behavior</div>
  </div>

  <section class="settings-section">
    <div class="settings-section-title">Local AI Status</div>

    {#if ollamaStatus === null}
      <div class="status-row">
        <span class="status-dot checking"></span>
        <span class="status-text">Checking...</span>
      </div>
    {:else}
      <!-- Step 1: Ollama installed -->
      <div class="status-step">
        <div class="status-row">
          <span class="status-dot" class:ok={ollamaStatus.cli_installed} class:fail={!ollamaStatus.cli_installed}></span>
          <span class="status-text">
            {ollamaStatus.cli_installed ? "Ollama installed" : "Ollama not installed"}
          </span>
          {#if !ollamaStatus.cli_installed}
            <button class="status-action" type="button" onclick={() => openUrl("https://ollama.com/download")}>
              Download
            </button>
          {/if}
        </div>
        {#if !ollamaStatus.cli_installed}
          <div class="status-hint">
            Ollama runs AI models locally on your machine. Download it from
            <button class="link-btn" type="button" onclick={() => openUrl("https://ollama.com/download")}>ollama.com</button>,
            install the app, and click "Check again".
          </div>
        {/if}
      </div>

      <!-- Step 2: Server running -->
      <div class="status-step">
        <div class="status-row">
          <span class="status-dot" class:ok={ollamaStatus.server_running} class:fail={ollamaStatus.cli_installed && !ollamaStatus.server_running} class:disabled={!ollamaStatus.cli_installed}></span>
          <span class="status-text" class:dimmed={!ollamaStatus.cli_installed}>
            {ollamaStatus.server_running ? "Server running" : "Server not running"}
          </span>
          {#if ollamaStatus.cli_installed && !ollamaStatus.server_running}
            <button class="status-action" type="button" disabled={ollamaLoading} onclick={handleStartServer}>
              {ollamaLoading ? "Starting..." : "Start"}
            </button>
          {/if}
        </div>
        {#if ollamaStatus.cli_installed && !ollamaStatus.server_running}
          <div class="status-hint">
            Ollama server is not running. Click "Start" to launch it, or run
            <code>ollama serve</code> in your terminal.
          </div>
        {/if}
      </div>

      <!-- Step 3: Model installed -->
      <div class="status-step">
        <div class="status-row">
          <span class="status-dot" class:ok={ollamaStatus.model_installed} class:fail={ollamaStatus.server_running && !ollamaStatus.model_installed} class:disabled={!ollamaStatus.server_running}></span>
          <span class="status-text" class:dimmed={!ollamaStatus.server_running}>
            {ollamaStatus.model_installed ? `Model ready` : `Model not installed`}
          </span>
          {#if ollamaStatus.server_running && !ollamaStatus.model_installed}
            <button class="status-action" type="button" disabled={ollamaLoading} onclick={handlePullModel}>
              {ollamaLoading ? "Pulling..." : "Download"}
            </button>
          {/if}
        </div>
        {#if ollamaStatus.server_running && !ollamaStatus.model_installed}
          <div class="status-hint">
            Model <code>{ollamaStatus.model_name}</code> needs to be downloaded.
            Click "Download" or run <code>ollama pull {ollamaStatus.model_name}</code> in terminal.
            This may take a few minutes depending on your connection.
          </div>
        {:else if ollamaStatus.model_installed}
          <div class="status-hint ok">
            Using <code>{ollamaStatus.model_name}</code>
          </div>
        {/if}
      </div>

      <!-- Step 4: Tagging test -->
      <div class="status-step">
        <div class="status-row">
          <span class="status-dot" class:ok={taggingResult !== undefined && taggingResult !== null} class:fail={taggingResult === null} class:disabled={!ollamaStatus.model_installed}></span>
          <span class="status-text" class:dimmed={!ollamaStatus.model_installed}>
            {#if taggingResult === undefined}
              Tagging not tested
            {:else if taggingResult !== null}
              Tagging works
            {:else}
              Tagging failed
            {/if}
          </span>
          {#if ollamaStatus.model_installed}
            <button class="status-action" type="button" disabled={taggingLoading} onclick={handleTestTagging}>
              {taggingLoading ? "Testing..." : "Test"}
            </button>
          {/if}
        </div>
        {#if taggingResult !== undefined && taggingResult !== null}
          <div class="status-hint ok">
            Test result: {taggingResult.join(", ")}
          </div>
        {:else if taggingResult === null}
          <div class="status-hint fail">
            The model did not return tags. Try a different model or check Ollama logs.
          </div>
        {:else if ollamaStatus.model_installed}
          <div class="status-hint">
            Click "Test" to verify that the model can tag clipboard content.
          </div>
        {/if}
      </div>

      <button class="settings-ghost-btn refresh-btn" type="button" disabled={ollamaLoading} onclick={refreshOllamaStatus}>
        Check again
      </button>
    {/if}
  </section>

  <section class="settings-section">
    <div class="settings-section-title">AI Model</div>
    <label class="settings-field">
      <span class="settings-label">Ollama model</span>
      <select
        class="settings-select"
        bind:value={selectedModelPreset}
        onchange={(e) => handleModelPresetChange((e.currentTarget as HTMLSelectElement).value)}
      >
        {#each modelCatalog.options as option}
          <option value={option.value}>
            {option.label} · ~{option.memory_gb.toFixed(1)} GB · {option.fits ? "fits" : "tight"}{option.installed ? " · installed" : ""}
          </option>
        {/each}
        <option value="__custom__">Custom model</option>
      </select>
      {#if selectedModelPreset === "__custom__"}
        <input
          class="settings-input"
          type="text"
          bind:value={settings.ollama_model}
          placeholder="qwen3:4b-instruct-2507-q4_K_M"
        />
      {/if}
      <div class="settings-info-card">
        <div class="settings-hint">
          Machine RAM: {modelCatalog.total_memory_gb.toFixed(1)} GB
        </div>
        <div class="settings-hint">
          Recommended Ollama budget: {modelCatalog.recommended_memory_gb.toFixed(1)} GB
        </div>
        {#if selectedModelMeta}
          <div class="settings-hint" class:fits={selectedModelMeta.fits} class:tight={!selectedModelMeta.fits}>
            {selectedModelMeta.label} needs about {selectedModelMeta.memory_gb.toFixed(1)} GB and
            {selectedModelMeta.fits ? " should fit this machine." : " may be too heavy for this machine."}
          </div>
        {/if}
      </div>
    </label>
  </section>

  <section class="settings-section">
    <div class="settings-section-title">Storage</div>
    <label class="settings-field">
      <span class="settings-label">History retention</span>
      <select class="settings-select" bind:value={settings.retention_days}>
        {#each retentionOptions as option}
          <option value={option.value}>{option.label}</option>
        {/each}
      </select>
    </label>
  </section>

  <section class="settings-section">
    <div class="settings-section-title">Privacy</div>
    <div class="settings-field">
      <span class="settings-label">Excluded apps</span>
      <div class="settings-inline">
        <input
          class="settings-input"
          type="text"
          bind:value={excludedAppInput}
          placeholder="App name, for example Telegram"
        />
        <button class="settings-small-btn" type="button" onclick={handleAddExcludedApp}>
          Add
        </button>
      </div>
      <button class="settings-ghost-btn" type="button" onclick={handleAddFrontmostApp}>
        Exclude current app
      </button>
      {#if excludedApps.length > 0}
        <div class="excluded-apps">
          {#each excludedApps as app}
            <div class="excluded-app-row">
              <span class="excluded-app-name">{app.bundle_id}</span>
              <button
                class="excluded-remove-btn"
                type="button"
                onclick={() => handleRemoveExcludedApp(app.id)}
              >
                Remove
              </button>
            </div>
          {/each}
        </div>
      {:else}
        <div class="settings-hint">Clipboard from excluded apps will not be stored or tagged.</div>
      {/if}
    </div>
  </section>

  <div class="settings-actions">
    <button class="settings-save-btn" type="button" disabled={savingSettings} onclick={saveSettings}>
      {savingSettings ? "Saving..." : "Save settings"}
    </button>
    {#if settingsNotice}
      <div class="settings-note">{settingsNotice}</div>
    {/if}
  </div>

  <div class="settings-divider"></div>

  <div class="settings-secondary">
    <button class="settings-item danger" type="button" onclick={handleClearHistory}>Clear unpinned history</button>
  </div>
</div>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    background: rgba(30, 30, 36, 0.96);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", sans-serif;
    color: #e0e0e0;
    user-select: none;
    -webkit-user-select: none;
  }

  :global(*) {
    box-sizing: border-box;
  }

  .settings-page {
    padding: 20px;
    max-width: 460px;
    margin: 0 auto;
  }

  .settings-head {
    margin-bottom: 16px;
  }

  .settings-title {
    font-size: 20px;
    font-weight: 700;
    color: #f2f5fb;
    letter-spacing: -0.02em;
  }

  .settings-subtitle {
    margin-top: 4px;
    font-size: 13px;
    color: #9097aa;
  }

  .settings-section {
    padding: 14px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 14px;
  }

  .settings-section + .settings-section {
    margin-top: 10px;
  }

  .settings-section-title {
    margin-bottom: 10px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #8f97aa;
  }

  .settings-field {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .settings-inline {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .settings-label {
    font-size: 12px;
    font-weight: 600;
    color: #c6cada;
  }

  .settings-input,
  .settings-select {
    width: 100%;
    min-height: 42px;
    padding: 10px 12px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.11);
    border-radius: 11px;
    color: #edf1f8;
    font: inherit;
    outline: none;
    transition: border-color 0.15s ease, background 0.15s ease, box-shadow 0.15s ease;
  }

  .settings-select {
    appearance: none;
    -webkit-appearance: none;
    -moz-appearance: none;
    padding-right: 42px;
    background-image:
      linear-gradient(45deg, transparent 50%, rgba(237, 241, 248, 0.9) 50%),
      linear-gradient(135deg, rgba(237, 241, 248, 0.9) 50%, transparent 50%),
      linear-gradient(180deg, rgba(255, 255, 255, 0.04), rgba(255, 255, 255, 0.01));
    background-position:
      calc(100% - 18px) calc(50% - 2px),
      calc(100% - 12px) calc(50% - 2px),
      0 0;
    background-size:
      6px 6px,
      6px 6px,
      100% 100%;
    background-repeat: no-repeat;
    cursor: pointer;
  }

  .settings-select:hover {
    background-color: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.16);
  }

  .settings-select option {
    color: #edf1f8;
    background: #23252c;
  }

  .settings-input:focus,
  .settings-select:focus {
    border-color: rgba(120, 160, 255, 0.4);
    box-shadow: 0 0 0 3px rgba(94, 140, 255, 0.15);
    background: rgba(255, 255, 255, 0.08);
  }

  .settings-input::placeholder {
    color: rgba(237, 240, 248, 0.35);
  }

  .settings-info-card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 11px;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 11px;
  }

  .settings-hint {
    font-size: 11px;
    line-height: 1.35;
    color: #97a0b4;
  }

  .settings-hint.fits {
    color: #8fd1a1;
  }

  .settings-hint.tight {
    color: #e3b370;
  }

  .settings-small-btn,
  .settings-ghost-btn {
    min-height: 40px;
    border-radius: 11px;
    font: inherit;
    cursor: pointer;
    transition: background 0.15s ease, border-color 0.15s ease, color 0.15s ease, transform 0.15s ease;
  }

  .settings-small-btn {
    padding: 0 14px;
    background: rgba(96, 134, 230, 0.16);
    border: 1px solid rgba(120, 160, 255, 0.22);
    color: #edf1f8;
    white-space: nowrap;
  }

  .settings-small-btn:hover,
  .settings-ghost-btn:hover {
    transform: translateY(-1px);
  }

  .settings-ghost-btn {
    padding: 0 12px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #d8dce6;
    width: fit-content;
  }

  .settings-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 14px;
  }

  .settings-save-btn {
    min-height: 42px;
    padding: 0 16px;
    background: linear-gradient(180deg, rgba(103, 145, 255, 0.95), rgba(75, 121, 244, 0.92));
    border: 1px solid rgba(144, 177, 255, 0.35);
    border-radius: 12px;
    color: #f7f9ff;
    font: inherit;
    font-weight: 700;
    cursor: pointer;
    transition: transform 0.15s ease, filter 0.15s ease, opacity 0.15s ease;
  }

  .settings-save-btn:hover {
    transform: translateY(-1px);
    filter: brightness(1.04);
  }

  .settings-save-btn:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .settings-note {
    padding: 0 2px;
    font-size: 11px;
    color: #91d6a6;
  }

  .settings-divider {
    height: 1px;
    margin: 14px 0 12px;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.08), transparent);
  }

  .settings-secondary {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .excluded-apps {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 160px;
    overflow-y: auto;
    padding-right: 2px;
  }

  .excluded-app-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 9px 10px;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 10px;
  }

  .excluded-app-name {
    font-size: 12px;
    color: #e7ebf3;
    min-width: 0;
    word-break: break-word;
  }

  .excluded-remove-btn {
    border: none;
    background: transparent;
    color: #e3b370;
    cursor: pointer;
    font: inherit;
    font-size: 11px;
    padding: 0;
    white-space: nowrap;
  }

  .settings-item {
    width: 100%;
    min-height: 40px;
    padding: 10px 12px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 10px;
    color: #dfe3ec;
    text-align: left;
    cursor: pointer;
    font: inherit;
    transition: background 0.15s ease, border-color 0.15s ease, color 0.15s ease;
  }

  .settings-item:hover {
    background: rgba(255, 255, 255, 0.07);
    border-color: rgba(255, 255, 255, 0.1);
  }

  .settings-item.danger {
    color: #f0c8c8;
  }

  .settings-item.danger:hover {
    background: rgba(255, 107, 107, 0.08);
    border-color: rgba(255, 107, 107, 0.14);
  }

  .status-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 0;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    background: rgba(255, 255, 255, 0.15);
  }

  .status-dot.ok {
    background: #4ade80;
    box-shadow: 0 0 6px rgba(74, 222, 128, 0.4);
  }

  .status-dot.fail {
    background: #f87171;
    box-shadow: 0 0 6px rgba(248, 113, 113, 0.4);
  }

  .status-dot.checking {
    background: #fbbf24;
    animation: pulse 1s infinite;
  }

  .status-dot.disabled {
    background: rgba(255, 255, 255, 0.08);
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .status-text {
    flex: 1;
    font-size: 12px;
    color: #d8dce6;
  }

  .status-text.dimmed {
    color: #6b7280;
  }

  .status-action {
    padding: 4px 12px;
    min-height: 28px;
    border-radius: 8px;
    background: rgba(96, 134, 230, 0.16);
    border: 1px solid rgba(120, 160, 255, 0.22);
    color: #c4d4ff;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.15s ease, transform 0.15s ease;
  }

  .status-action:hover:not(:disabled) {
    background: rgba(96, 134, 230, 0.28);
    transform: translateY(-1px);
  }

  .status-action:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .refresh-btn {
    margin-top: 8px;
    min-height: 32px;
    font-size: 12px;
  }

  .status-step {
    padding: 6px 0;
  }

  .status-step + .status-step {
    border-top: 1px solid rgba(255, 255, 255, 0.04);
  }

  .status-hint {
    margin: 6px 0 2px 18px;
    font-size: 11px;
    line-height: 1.5;
    color: #8a90a0;
  }

  .status-hint.ok {
    color: #6ecf8a;
  }

  .status-hint.fail {
    color: #f0a0a0;
  }

  .status-hint code {
    padding: 1px 5px;
    background: rgba(255, 255, 255, 0.07);
    border-radius: 4px;
    font-family: "SF Mono", Menlo, monospace;
    font-size: 10.5px;
    color: #c8cee0;
  }

  .link-btn {
    background: none;
    border: none;
    padding: 0;
    color: #7da4ff;
    cursor: pointer;
    font: inherit;
    font-size: 11px;
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .link-btn:hover {
    color: #a8c4ff;
  }
</style>
