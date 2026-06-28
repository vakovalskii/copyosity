<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import type { AppSettings, AudioInputDevice, ExcludedApp, ModelCatalog, ModelOption } from "$lib/types";
  import {
    addExcludedApp,
    addFrontmostAppToExcluded,
    clearHistory,
    getAppSettings,
    getExcludedApps,
    getModelCatalog,
    removeExcludedApp,
    updateAppSettings,
    rebindVoiceShortcut,
    listMicrophones,
    checkAccessibility,
    checkOllamaStatus,
    unloadOllamaModel,
    startOllamaServer,
    pullOllamaModel,
    testOllamaTagging,
    hubTestConnection,
    hubListModels,
    type OllamaStatus,
  } from "$lib/api";
  import { openUrl } from "@tauri-apps/plugin-opener";

  let settings = $state<AppSettings>({
    ollama_model: "qwen3:4b-instruct-2507-q4_K_M",
    retention_days: 30,
    whisper_server_url: "",
    whisper_server_token: "",
    whisper_server_model: "whisper-1",
    voice_shortcut: "option+space",
    selected_microphone: "",
    hub_url: "https://api.neuraldeep.ru",
    hub_token: "",
    hub_chat_model: "gpt-oss-120b",
    hub_tagging_enabled: false,
    hub_transcribe_enabled: false,
    hub_search_enabled: false,
  });
  let microphones: AudioInputDevice[] = $state([]);
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
  let savedModel = $state("");

  let accessibilityGranted = $state<boolean | null>(null);

  let ollamaStatus = $state<OllamaStatus | null>(null);
  let ollamaLoading = $state(false);
  let pullProgress = $state("");
  let taggingResult = $state<string[] | null | undefined>(undefined);
  let taggingLoading = $state(false);

  let hubTesting = $state(false);
  let hubTestResult = $state<{ ok: boolean; message: string } | null>(null);

  async function handleHubTest() {
    hubTesting = true;
    hubTestResult = null;
    try {
      const count = await hubTestConnection(settings.hub_url, settings.hub_token);
      hubTestResult = { ok: true, message: `Connected — ${count} model(s) available` };
      loadHubModels();
    } catch (e) {
      hubTestResult = { ok: false, message: String(e) };
    } finally {
      hubTesting = false;
    }
  }

  const retentionOptions = [
    { label: "1 day", value: 1 },
    { label: "1 week", value: 7 },
    { label: "1 month", value: 30 },
    { label: "6 months", value: 180 },
  ];

  async function loadSettings() {
    settings = await getAppSettings();
    selectedModelPreset = settings.ollama_model;
    savedModel = settings.ollama_model;
    snapshot();
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
    pullProgress = "Starting download...";
    await pullOllamaModel();
    // Command returns immediately, progress comes via events
    // ollama-pull-done will reset the state
  }

  async function handleTestTagging() {
    taggingLoading = true;
    taggingResult = undefined;
    try {
      taggingResult = await testOllamaTagging();
    } finally {
      taggingLoading = false;
    }
  }

  onMount(() => {
    // Load everything in parallel instead of sequentially
    loadSettings().then(() => {
      // Auto-check hub connection + load the model list so the auth status
      // and available models are visible on open.
      if (settings.hub_token?.trim()) {
        handleHubTest();
        loadHubModels();
      }
    });
    loadModelCatalog();
    loadExcludedApps();
    refreshOllamaStatus();
    listMicrophones().then((m) => (microphones = m));
    checkAccessibility().then((v) => (accessibilityGranted = v));

    const unlistenPull = listen<string>("ollama-pull-progress", (event) => {
      pullProgress = event.payload;
    });

    const unlistenPullDone = listen<boolean>("ollama-pull-done", async (event) => {
      ollamaLoading = false;
      pullProgress = "";
      await refreshOllamaStatus();
    });

    return () => {
      unlistenPull.then((fn) => fn());
      unlistenPullDone.then((fn) => fn());
    };
  });

  async function saveSettings() {
    savingSettings = true;
    settingsNotice = "";
    try {
      settings = await updateAppSettings({
        ollama_model: settings.ollama_model,
        retention_days: settings.retention_days,
        whisper_server_url: settings.whisper_server_url,
        whisper_server_token: settings.whisper_server_token,
        whisper_server_model: settings.whisper_server_model,
        voice_shortcut: settings.voice_shortcut,
        selected_microphone: settings.selected_microphone,
        hub_url: settings.hub_url,
        hub_token: settings.hub_token,
        hub_chat_model: settings.hub_chat_model,
        hub_tagging_enabled: settings.hub_tagging_enabled,
        hub_transcribe_enabled: settings.hub_transcribe_enabled,
        hub_search_enabled: settings.hub_search_enabled,
      });
      savedModel = settings.ollama_model;
      snapshot();
      settingsNotice = "Saved";
      taggingResult = undefined;
      // Run post-save tasks in parallel
      await Promise.all([
        rebindVoiceShortcut(),
        loadModelCatalog(),
        refreshOllamaStatus(),
      ]);
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

  let modelDirty = $derived(settings.ollama_model !== savedModel);

  // ---- Sidebar navigation ----
  type Pane = "hub" | "voice" | "ai" | "history" | "permissions";
  let activePane = $state<Pane>("hub");
  const panes: { id: Pane; label: string; icon: string }[] = [
    { id: "hub", label: "NeuralDeep", icon: "✦" },
    { id: "voice", label: "Voice", icon: "🎙" },
    { id: "ai", label: "Local AI", icon: "🧠" },
    { id: "history", label: "History", icon: "🗂" },
    { id: "permissions", label: "Permissions", icon: "🔑" },
  ];

  // ---- Hub model list (from /v1/models) ----
  let hubModels = $state<string[]>([]);
  let modelsLoading = $state(false);
  async function loadHubModels() {
    if (!settings.hub_token?.trim()) return;
    modelsLoading = true;
    try {
      hubModels = await hubListModels(settings.hub_url, settings.hub_token);
    } catch {
      // leave hubModels as-is; the field still allows manual entry
    } finally {
      modelsLoading = false;
    }
  }
  /** Ensure the currently-selected value is always present in the dropdown. */
  function withCurrent(list: string[], current: string): string[] {
    return current && !list.includes(current) ? [current, ...list] : list;
  }

  // ---- Dirty tracking (unsaved-changes bar) ----
  let savedSnapshot = $state("");
  let isDirty = $derived(savedSnapshot !== "" && JSON.stringify(settings) !== savedSnapshot);
  function snapshot() {
    savedSnapshot = JSON.stringify(settings);
  }
  async function resetSettings() {
    await loadSettings();
    snapshot();
    settingsNotice = "";
  }
</script>

<div class="settings-shell">
  <aside class="settings-sidebar">
    <div class="sidebar-title">Settings</div>
    <nav class="sidebar-nav">
      {#each panes as p}
        <button
          class="nav-item"
          class:active={activePane === p.id}
          type="button"
          onclick={() => (activePane = p.id)}
        >
          <span class="nav-icon">{p.icon}</span>
          <span>{p.label}</span>
        </button>
      {/each}
    </nav>
    <div class="sidebar-foot">
      <span class="sidebar-dot" class:ok={hubTestResult?.ok} class:fail={hubTestResult && !hubTestResult.ok}></span>
      <span>{hubTestResult ? (hubTestResult.ok ? "Hub connected" : "Hub error") : settings.hub_token?.trim() ? "Hub configured" : "Hub not set"}</span>
    </div>
  </aside>

  <main class="settings-content">
    {#if activePane === "hub"}
      <div class="pane-head">
        <div class="pane-title">NeuralDeep Hub</div>
        <div class="pane-subtitle">Cloud models for tagging, transcription, web search and the research agent.</div>
      </div>

      <section class="settings-section">
        <div class="settings-hint" style="margin-bottom: 10px;">
          <strong>3 steps:</strong>
          <ol style="margin: 6px 0 0; padding-left: 18px;">
            <li><button class="link-btn" type="button" onclick={() => openUrl("https://hub.neuraldeep.ru/app")}>Open the hub</button> → copy your <code>sk-…</code> key</li>
            <li>Paste it into <strong>API Token</strong> below</li>
            <li>Click <strong>Test connection</strong> → then <strong>Save</strong></li>
          </ol>
        </div>

        <label class="settings-field">
          <span class="settings-label">API base URL</span>
          <input
            class="settings-input"
            type="text"
            bind:value={settings.hub_url}
            placeholder="https://api.neuraldeep.ru"
          />
        </label>
        <label class="settings-field" style="margin-top: 8px;">
          <span class="settings-label">API Token</span>
          <input
            class="settings-input"
            type="password"
            bind:value={settings.hub_token}
            placeholder="sk-... Bearer token"
          />
        </label>

        <div class="settings-field" style="margin-top: 10px;">
          <button class="settings-ghost-btn" type="button" disabled={hubTesting} onclick={handleHubTest}>
            {hubTesting ? "Testing..." : "Test connection"}
          </button>
          {#if hubTestResult}
            <div class="settings-hint" style="margin-top: 6px; color: {hubTestResult.ok ? '#3bbf6a' : '#e5534b'};">
              {hubTestResult.message}
            </div>
          {/if}
        </div>
      </section>

      <section class="settings-section">
        <div class="settings-section-title">Model</div>
        <label class="settings-field">
          <span class="settings-label">Chat / agent model (tagging & search)</span>
          <div class="settings-inline">
            <select class="settings-select" bind:value={settings.hub_chat_model}>
              {#each withCurrent(hubModels.length ? hubModels : ["gpt-oss-120b", "qwen3.6-35b-a3b", "gemma-4-31b"], settings.hub_chat_model) as m}
                <option value={m}>{m}</option>
              {/each}
            </select>
            <button class="settings-small-btn" type="button" disabled={modelsLoading} onclick={loadHubModels} title="Load live model list from the hub">
              {modelsLoading ? "…" : "↻"}
            </button>
          </div>
          <div class="settings-hint">
            {hubModels.length ? `${hubModels.length} models loaded from the hub` : "Test connection or press ↻ to load the live model list."}
          </div>
        </label>
      </section>

      <section class="settings-section">
        <div class="settings-section-title">Features</div>
        <label class="settings-toggle">
          <input type="checkbox" bind:checked={settings.hub_tagging_enabled} />
          <span>Use hub for tagging (falls back to Ollama on error)</span>
        </label>
        <label class="settings-toggle" style="margin-top: 10px;">
          <input type="checkbox" bind:checked={settings.hub_transcribe_enabled} />
          <span>Use hub for voice transcription</span>
        </label>
        <div class="settings-hint" style="margin-top: 12px;">
          <strong>Web search:</strong> press <code>⌘⇧Space</code> anywhere (or tray → Agent Search,
          or the search button in the main window) to query the web via the hub.
        </div>
      </section>
    {:else if activePane === "voice"}
      <div class="pane-head">
        <div class="pane-title">Voice</div>
        <div class="pane-subtitle">Hold the shortcut to record, release to transcribe and paste at the cursor.</div>
      </div>

      <section class="settings-section">
        <div class="settings-section-title">Recording</div>
        <label class="settings-field">
          <span class="settings-label">Shortcut (hold to record)</span>
          <input
            class="settings-input"
            type="text"
            bind:value={settings.voice_shortcut}
            placeholder="option+space"
          />
          <div class="settings-hint">
            Use: <code>cmd</code>, <code>option</code>, <code>ctrl</code>, <code>shift</code> + key.
            Examples: <code>option+space</code>, <code>cmd+shift+r</code>, <code>ctrl+alt+space</code>
          </div>
        </label>
        <label class="settings-field" style="margin-top: 8px;">
          <span class="settings-label">Microphone</span>
          <select class="settings-select" bind:value={settings.selected_microphone}>
            <option value="">System default</option>
            {#each microphones as mic}
              <option value={mic.name}>{mic.name}{mic.is_default ? " (default)" : ""}</option>
            {/each}
          </select>
        </label>
      </section>

      <section class="settings-section">
        <div class="settings-section-title">Transcription model</div>
        <div class="settings-hint" style="margin-bottom: 10px;">
          With <strong>hub transcription</strong> on (NeuralDeep tab), audio goes to the hub model below.
          Otherwise it uses the custom Whisper server.
        </div>
        <label class="settings-field">
          <span class="settings-label">Model</span>
          <div class="settings-inline">
            <select class="settings-select" bind:value={settings.whisper_server_model}>
              {#each withCurrent(hubModels.length ? hubModels : ["whisper-1"], settings.whisper_server_model) as m}
                <option value={m}>{m}</option>
              {/each}
            </select>
            <button class="settings-small-btn" type="button" disabled={modelsLoading} onclick={loadHubModels} title="Load live model list from the hub">
              {modelsLoading ? "…" : "↻"}
            </button>
          </div>
          <div class="settings-hint">
            Pick a transcription model — e.g. <code>whisper-1</code> or <code>подлодка</code>.
          </div>
        </label>
        <label class="settings-field" style="margin-top: 8px;">
          <span class="settings-label">Custom Whisper server URL (optional)</span>
          <input
            class="settings-input"
            type="text"
            bind:value={settings.whisper_server_url}
            placeholder="http://localhost:8000/v1/audio/transcriptions"
          />
        </label>
        <label class="settings-field" style="margin-top: 8px;">
          <span class="settings-label">Custom server token (optional)</span>
          <input
            class="settings-input"
            type="password"
            bind:value={settings.whisper_server_token}
            placeholder="Bearer token (optional)"
          />
        </label>
      </section>
    {:else if activePane === "permissions"}
      <div class="pane-head">
        <div class="pane-title">Permissions</div>
        <div class="pane-subtitle">Accessibility access is required for paste automation and the global shortcut.</div>
      </div>

      <section class="settings-section">
        <div class="settings-section-title">Accessibility</div>
    <div class="status-step">
      <div class="status-row">
        <span class="status-dot" class:ok={accessibilityGranted === true} class:fail={accessibilityGranted === false} class:checking={accessibilityGranted === null}></span>
        <span class="status-text">
          {accessibilityGranted === null ? "Checking..." : accessibilityGranted ? "Accessibility granted" : "Accessibility not granted"}
        </span>
        {#if accessibilityGranted === false}
          <button class="status-action" type="button" onclick={async () => { accessibilityGranted = await checkAccessibility(); }}>
            Request
          </button>
        {:else if accessibilityGranted === true}
          <button class="status-action" type="button" onclick={async () => { accessibilityGranted = await checkAccessibility(); }}>
            Recheck
          </button>
        {/if}
      </div>
      {#if accessibilityGranted === false}
        <div class="status-hint">
          Required for paste automation (Cmd+V) and global shortcut.
          Click "Request" to open System Settings, then enable Copyosity under Privacy → Accessibility.
        </div>
      {/if}
    </div>
  </section>
    {:else if activePane === "ai"}
      <div class="pane-head">
        <div class="pane-title">Local AI</div>
        <div class="pane-subtitle">Run tagging on-device with Ollama — used when the hub is off or unavailable.</div>
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
              Open ollama.com
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
              {#if ollamaLoading}<span class="spinner"></span> Starting...{:else}Start{/if}
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
              {#if ollamaLoading}<span class="spinner"></span> Pulling...{:else}Download{/if}
            </button>
          {/if}
          {#if ollamaStatus.model_installed}
            <button class="status-action" type="button" onclick={async () => { await unloadOllamaModel(); settingsNotice = "Model unloaded from memory"; }}>
              Unload
            </button>
          {/if}
        </div>
        {#if pullProgress}
          <div class="status-hint pull-progress">
            <span class="spinner"></span> {pullProgress}
          </div>
        {:else if ollamaStatus.server_running && !ollamaStatus.model_installed}
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
            <button class="status-action" type="button" disabled={taggingLoading || modelDirty} onclick={handleTestTagging} title={modelDirty ? "Save settings first" : ""}>
              {#if taggingLoading}
                <span class="spinner"></span> Testing...
              {:else}
                Test
              {/if}
            </button>
          {/if}
        </div>
        {#if modelDirty}
          <div class="status-hint fail">
            Model changed — save settings first, then test.
          </div>
        {:else if taggingLoading}
          <div class="status-hint">
            Sending test request... This can take up to 60 seconds on first run while the model loads into memory.
          </div>
        {:else if taggingResult !== undefined && taggingResult !== null}
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
    {:else if activePane === "history"}
      <div class="pane-head">
        <div class="pane-title">History</div>
        <div class="pane-subtitle">How long clips are kept and which apps are ignored.</div>
      </div>

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

  <section class="settings-section">
    <div class="settings-section-title">Danger zone</div>
    <div class="settings-secondary">
      <button class="settings-item danger" type="button" onclick={handleClearHistory}>Clear unpinned history</button>
    </div>
  </section>
    {/if}
  </main>
</div>

{#if isDirty}
  <div class="dirty-bar">
    <span class="dirty-label">Unsaved changes</span>
    <div class="dirty-actions">
      <button class="dirty-reset" type="button" onclick={resetSettings}>Reset</button>
      <button class="dirty-save" type="button" disabled={savingSettings} onclick={saveSettings}>
        {savingSettings ? "Saving…" : "Save"}
      </button>
    </div>
  </div>
{:else if settingsNotice}
  <div class="dirty-bar saved">
    <span class="dirty-label">{settingsNotice}</span>
  </div>
{/if}

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

  /* ---- Shell: sidebar + content ---- */
  .settings-shell {
    display: grid;
    grid-template-columns: 184px 1fr;
    min-height: 100vh;
  }

  .settings-sidebar {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 16px 12px;
    background: rgba(0, 0, 0, 0.22);
    border-right: 1px solid rgba(255, 255, 255, 0.06);
    position: sticky;
    top: 0;
    height: 100vh;
  }

  .sidebar-title {
    padding: 4px 10px 12px;
    font-size: 16px;
    font-weight: 700;
    color: #f2f5fb;
    letter-spacing: -0.02em;
  }

  .sidebar-nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 10px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 10px;
    color: #c0c5d4;
    font: inherit;
    font-size: 13px;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    transition: background 0.15s ease, color 0.15s ease;
  }

  .nav-item:hover {
    background: rgba(255, 255, 255, 0.05);
    color: #eef1f8;
  }

  .nav-item.active {
    background: rgba(96, 134, 230, 0.18);
    border-color: rgba(120, 160, 255, 0.25);
    color: #eef2ff;
  }

  .nav-icon {
    width: 18px;
    text-align: center;
    font-size: 14px;
  }

  .sidebar-foot {
    margin-top: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    font-size: 11px;
    color: #8b93a6;
  }

  .sidebar-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.2);
    flex-shrink: 0;
  }

  .sidebar-dot.ok {
    background: #4ade80;
    box-shadow: 0 0 6px rgba(74, 222, 128, 0.4);
  }

  .sidebar-dot.fail {
    background: #f87171;
  }

  .settings-content {
    padding: 22px 24px 88px;
    max-width: 620px;
    width: 100%;
  }

  .pane-head {
    margin-bottom: 16px;
  }

  .pane-title {
    font-size: 20px;
    font-weight: 700;
    color: #f2f5fb;
    letter-spacing: -0.02em;
  }

  .pane-subtitle {
    margin-top: 4px;
    font-size: 13px;
    color: #9097aa;
    line-height: 1.4;
  }

  /* ---- Dirty bar ---- */
  .dirty-bar {
    position: fixed;
    left: 184px;
    right: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 24px;
    background: rgba(24, 25, 31, 0.92);
    backdrop-filter: blur(12px);
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }

  .dirty-bar.saved {
    justify-content: flex-start;
  }

  .dirty-label {
    font-size: 12px;
    font-weight: 600;
    color: #e3b370;
  }

  .dirty-bar.saved .dirty-label {
    color: #91d6a6;
  }

  .dirty-actions {
    display: flex;
    gap: 8px;
  }

  .dirty-reset {
    min-height: 36px;
    padding: 0 14px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    color: #d8dce6;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .dirty-reset:hover {
    background: rgba(255, 255, 255, 0.09);
  }

  .dirty-save {
    min-height: 36px;
    padding: 0 18px;
    background: linear-gradient(180deg, rgba(103, 145, 255, 0.95), rgba(75, 121, 244, 0.92));
    border: 1px solid rgba(144, 177, 255, 0.35);
    border-radius: 10px;
    color: #f7f9ff;
    font: inherit;
    font-weight: 700;
    cursor: pointer;
    transition: transform 0.15s ease, filter 0.15s ease, opacity 0.15s ease;
  }

  .dirty-save:hover {
    transform: translateY(-1px);
    filter: brightness(1.04);
  }

  .dirty-save:disabled {
    opacity: 0.6;
    cursor: default;
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

  .settings-toggle {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 13px;
    cursor: pointer;
  }

  .settings-toggle input {
    width: 16px;
    height: 16px;
    cursor: pointer;
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

  .spinner {
    display: inline-block;
    width: 10px;
    height: 10px;
    border: 2px solid rgba(255, 255, 255, 0.2);
    border-top-color: #c4d4ff;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
    vertical-align: middle;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .pull-progress {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #c4d4ff;
    font-family: "SF Mono", Menlo, monospace;
    font-size: 10.5px;
    word-break: break-all;
  }
</style>
