<script lang="ts">
  import { onMount } from "svelte";
  import { prepareBusyUi } from "$lib/run-with-busy-ui";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    parseEntryTaggedEvent,
    type AppSettings,
    type AudioInputDevice,
    type ExcludedApp,
    type ExcludableAppCandidate,
    type ExcludeAppResult,
    type ModelCatalog,
  } from "$lib/types";
  import {
    addExcludedApp,
    addExcludableAppCandidate,
    clearAllHistory,
    clearHistory,
    getAppSettings,
    getExcludedApps,
    getExcludableAppCandidate,
    getHistoryCounts,
    getModelCatalog,
    pickAppToExclude,
    removeExcludedApp,
    updateAppSettings,
    rebindVoiceShortcut,
    listMicrophones,
    checkAccessibility,
    openAccessibilitySettings,
    checkOllamaStatus,
    unloadOllamaModel,
    startOllamaServer,
    pullOllamaModel,
    testOllamaTagging,
    type OllamaStatus,
  } from "$lib/api";
  import ActionMenu from "$lib/components/ActionMenu.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import SectionIcon from "$lib/components/SectionIcon.svelte";
  import { confirmDestructive } from "$lib/confirm";
  import {
    clearAllConfirmBody,
    clearUnpinnedConfirmBody,
    type ClearHistoryAction,
  } from "$lib/destructive-actions";
  import {
    allowInClipboardHistoryAriaLabel,
    allowedInHistoryNotice,
    alreadyExcludedListMetaLabel,
    appNotFoundNotice,
    chooseApplicationActionLabel,
    excludeFromClipboardHistoryAriaLabel,
    invokeErrorMessage,
    isAppNotFoundError,
    excludableCandidateMetaLabel,
    excludeListAddLabel,
    excludeListRemoveLabel,
    alreadyExcludedFromHistoryNotice,
    couldNotAddExcludedAppNotice,
    couldNotAddSelectedAppNotice,
    excludedFromHistoryNotice,
  } from "$lib/exclusion-label";
  import { openUrl } from "@tauri-apps/plugin-opener";

  function openOllamaDownload() {
    void openUrl("https://ollama.com/download");
  }

  let settings = $state<AppSettings>({
    ollama_model: "qwen3:4b-instruct-2507-q4_K_M",
    retention_days: 30,
    whisper_server_url: "",
    whisper_server_token: "",
    whisper_server_model: "whisper-1",
    voice_shortcut: "option+space",
    selected_microphone: "",
    voice_transcription_enabled: false,
    ai_tagging_enabled: false,
    overlay_shortcut_hints_enabled: true,
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
  let excludableCandidate: ExcludableAppCandidate | null = $state(null);
  let excludedAppsNotice = $state("");
  let excludedAppsNoticeTone = $state<"neutral" | "warn">("neutral");
  let excludeActionBusy = $state(false);
  let savingSettings = $state(false);
  let settingsNotice = $state("");
  let clearHistoryNotice = $state("");
  let historyCounts = $state({ total: 0, unpinned: 0, pinned: 0 });
  /** Suppress stale-notice reset while Settings-initiated clear is finishing. */
  let clearHistoryActionInFlight = false;
  let savedModel = $state("");

  const A11Y_NOTICE_ENABLE = "Enable Copyosity in the list.";
  const A11Y_NOTICE_VERIFIED = "Accessibility verified — paste automation is ready.";

  let accessibilityGranted = $state<boolean | null>(null);
  let accessibilityNotice = $state("");
  /** User was sent to System Settings; keep enable hint until access is granted. */
  let a11yEnablePending = $state(false);

  let ollamaStatus = $state<OllamaStatus | null>(null);
  let ollamaBusy = $state<"refresh" | "start" | "pull" | "unload" | null>(null);
  let pullProgress = $state("");
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
    savedModel = settings.ollama_model;
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

  async function loadExcludableCandidate() {
    excludableCandidate = await getExcludableAppCandidate();
  }

  function isActiveApp(bundleId: string): boolean {
    return excludableCandidate?.bundleId === bundleId;
  }

  const listedExcludedApps = $derived(
    excludedApps.filter((app) => !isActiveApp(app.bundleId)),
  );

  const activeExcludedEntry = $derived.by(() => {
    const candidate = excludableCandidate;
    if (!candidate?.alreadyExcluded) return undefined;
    return excludedApps.find((app) => app.bundleId === candidate.bundleId);
  });

  const excludedAppsPanelHasRows = $derived(
    !!excludableCandidate || listedExcludedApps.length > 0,
  );

  function setExcludedAppsNotice(message: string, tone: "neutral" | "warn" = "neutral") {
    excludedAppsNotice = message;
    excludedAppsNoticeTone = tone;
  }

  function showExcludeAppNotice(result: ExcludeAppResult) {
    if (result.alreadyExcluded) {
      setExcludedAppsNotice(
        alreadyExcludedFromHistoryNotice(result.displayName),
        "warn",
      );
      return;
    }
    setExcludedAppsNotice(excludedFromHistoryNotice(result.displayName));
  }

  async function refreshExcludedAppsSection() {
    await Promise.all([loadExcludedApps(), loadExcludableCandidate()]);
  }

  async function syncOllamaStatus() {
    ollamaStatus = await checkOllamaStatus();
  }

  async function refreshOllamaStatus(showBusy = false) {
    if (ollamaBusy || taggingLoading) return;
    if (showBusy) {
      ollamaBusy = "refresh";
      taggingResult = undefined;
      await prepareBusyUi();
    }
    try {
      await syncOllamaStatus();
    } finally {
      if (showBusy && ollamaBusy === "refresh") ollamaBusy = null;
    }
  }

  async function handleUnloadModel() {
    if (ollamaBusy || taggingLoading) return;
    ollamaBusy = "unload";
    await prepareBusyUi();
    try {
      const ok = await unloadOllamaModel();
      await syncOllamaStatus();
      if (ok && ollamaStatus && !ollamaStatus.model_loaded) {
        taggingResult = undefined;
      }
    } finally {
      if (ollamaBusy === "unload") ollamaBusy = null;
    }
  }

  async function handleStartServer() {
    if (ollamaBusy || taggingLoading) return;
    ollamaBusy = "start";
    await prepareBusyUi();
    try {
      await startOllamaServer();
      await refreshOllamaStatus();
    } finally {
      if (ollamaBusy === "start") ollamaBusy = null;
    }
  }

  async function handlePullModel() {
    if (ollamaBusy || taggingLoading) return;
    ollamaBusy = "pull";
    pullProgress = "Starting download...";
    await prepareBusyUi();
    await pullOllamaModel();
    // Command returns immediately, progress comes via events
    // ollama-pull-done will reset the state
  }

  async function handleTestTagging() {
    if (taggingLoading || modelDirty || ollamaBusy) return;
    taggingLoading = true;
    await prepareBusyUi();
    try {
      taggingResult = await testOllamaTagging();
    } finally {
      taggingLoading = false;
    }
    await syncOllamaStatus();
  }

  /** macOS trust prompt already shown this settings-window visit. */
  let a11yPromptedThisVisit = false;

  function syncA11yNotice(granted: boolean, showVerified = false) {
    if (granted) {
      a11yEnablePending = false;
      if (showVerified || accessibilityNotice === A11Y_NOTICE_ENABLE) {
        accessibilityNotice = A11Y_NOTICE_VERIFIED;
      }
      return;
    }
    if (a11yEnablePending) {
      accessibilityNotice = A11Y_NOTICE_ENABLE;
    } else {
      accessibilityNotice = "";
    }
  }

  async function updateAccessibilityStatus() {
    const granted = await checkAccessibility(false);
    const wasPending = a11yEnablePending;
    accessibilityGranted = granted;
    syncA11yNotice(granted, wasPending);
    return granted;
  }

  /** One macOS prompt per settings visit when access is still missing. */
  async function promptAccessibilityIfNeeded() {
    let granted = await checkAccessibility(false);
    if (!granted && !a11yPromptedThisVisit) {
      await checkAccessibility(true);
      a11yPromptedThisVisit = true;
      a11yEnablePending = true;
      granted = await checkAccessibility(false);
    }
    accessibilityGranted = granted;
    syncA11yNotice(granted);
    return granted;
  }

  async function handleRequestAccessibility() {
    await openAccessibilitySettings();
    await checkAccessibility(true);
    a11yPromptedThisVisit = true;
    a11yEnablePending = true;
    accessibilityGranted = await checkAccessibility(false);
    syncA11yNotice(accessibilityGranted, true);
  }

  async function handleRecheckAccessibility() {
    const granted = await checkAccessibility(false);
    accessibilityGranted = granted;
    if (granted) {
      syncA11yNotice(true, true);
      return;
    }
    await checkAccessibility(true);
    a11yPromptedThisVisit = true;
    a11yEnablePending = true;
    accessibilityGranted = await checkAccessibility(false);
    syncA11yNotice(accessibilityGranted);
  }

  onMount(() => {
    // Load everything in parallel instead of sequentially
    loadSettings();
    loadModelCatalog();
    loadHistoryCounts();
    refreshExcludedAppsSection();
    refreshOllamaStatus();
    listMicrophones().then((m) => (microphones = m));

    const win = getCurrentWindow();
    void promptAccessibilityIfNeeded();

    const unlistenPull = listen<string>("ollama-pull-progress", (event) => {
      pullProgress = event.payload;
    });

    const unlistenPullDone = listen<boolean>("ollama-pull-done", async (event) => {
      ollamaBusy = null;
      pullProgress = "";
      await refreshOllamaStatus();
    });

    const unlistenShown = listen("settings-shown", () => {
      a11yPromptedThisVisit = false;
      void promptAccessibilityIfNeeded();
      void refreshExcludedAppsSection();
      void refreshHistoryCounts();
    });

    const onHistoryCountsEvent = () => void refreshHistoryCounts();
    const unlistenClipboard = listen("clipboard-changed", onHistoryCountsEvent);
    const unlistenHistory = listen("history-changed", onHistoryCountsEvent);

    const unlistenFocus = win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        void updateAccessibilityStatus();
        void refreshExcludedAppsSection();
        void refreshHistoryCounts();
      }
    });

    // Retag / auto-tag can load an unloaded model — refresh step 3 only then.
    let ollamaSyncTimer: ReturnType<typeof setTimeout>;
    const unlistenEntryTagged = listen("entry-tagged", (event) => {
      if (parseEntryTaggedEvent(event.payload) === null) return;
      if (ollamaStatus?.model_loaded) return;
      clearTimeout(ollamaSyncTimer);
      ollamaSyncTimer = setTimeout(() => void syncOllamaStatus(), 150);
    });

    return () => {
      clearTimeout(ollamaSyncTimer);
      unlistenPull.then((fn) => fn());
      unlistenPullDone.then((fn) => fn());
      unlistenShown.then((fn) => fn());
      unlistenClipboard.then((fn) => fn());
      unlistenHistory.then((fn) => fn());
      unlistenFocus.then((fn) => fn());
      unlistenEntryTagged.then((fn) => fn());
    };
  });

  async function saveSettings() {
    if (savingSettings) return;
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
        voice_transcription_enabled: settings.voice_transcription_enabled,
        ai_tagging_enabled: settings.ai_tagging_enabled,
        overlay_shortcut_hints_enabled: settings.overlay_shortcut_hints_enabled,
      });
      savedModel = settings.ollama_model;
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

  async function handleAddExcludedByName() {
    const value = excludedAppInput.trim();
    if (!value) return;
    setExcludedAppsNotice("");
    try {
      const result = await addExcludedApp(value);
      if (!result.alreadyExcluded) {
        excludedAppInput = "";
      }
      showExcludeAppNotice(result);
      await refreshExcludedAppsSection();
    } catch (err) {
      if (isAppNotFoundError(err)) {
        setExcludedAppsNotice(appNotFoundNotice(value), "warn");
        return;
      }
      setExcludedAppsNotice(couldNotAddExcludedAppNotice(value), "warn");
    }
  }

  async function handleAddCandidateApp() {
    if (excludeActionBusy) return;
    excludeActionBusy = true;
    setExcludedAppsNotice("");
    try {
      const result = await addExcludableAppCandidate();
      if (result) {
        showExcludeAppNotice(result);
      } else {
        setExcludedAppsNotice(
          "No active app detected. Switch to the app you want to exclude, or choose one below.",
          "warn",
        );
      }
      await refreshExcludedAppsSection();
    } catch (err) {
      const fallback = excludableCandidate
        ? couldNotAddExcludedAppNotice(excludableCandidate.displayName)
        : couldNotAddSelectedAppNotice();
      setExcludedAppsNotice(invokeErrorMessage(err) || fallback, "warn");
    } finally {
      excludeActionBusy = false;
    }
  }

  async function handleChooseApp() {
    if (excludeActionBusy) return;
    excludeActionBusy = true;
    setExcludedAppsNotice("");
    try {
      const result = await pickAppToExclude();
      if (result) {
        showExcludeAppNotice(result);
        await refreshExcludedAppsSection();
      }
    } catch (err) {
      const message = invokeErrorMessage(err);
      if (message.startsWith("main_thread_required:")) {
        setExcludedAppsNotice("Could not open the app picker. Try again.", "warn");
      } else {
        setExcludedAppsNotice(message || couldNotAddSelectedAppNotice(), "warn");
      }
    } finally {
      excludeActionBusy = false;
    }
  }

  async function handleRemoveExcludedApp(id: number, displayName: string) {
    if (excludeActionBusy) return;
    excludeActionBusy = true;
    try {
      await removeExcludedApp(id);
      setExcludedAppsNotice(allowedInHistoryNotice(displayName));
      await refreshExcludedAppsSection();
    } catch (err) {
      setExcludedAppsNotice(
        invokeErrorMessage(err) || "Could not update excluded apps. Try again.",
        "warn",
      );
    } finally {
      excludeActionBusy = false;
    }
  }

  async function loadHistoryCounts() {
    historyCounts = await getHistoryCounts();
  }

  /** Refresh counts from backend; clear stale clear-history feedback when counts change externally. */
  async function refreshHistoryCounts() {
    const previous = historyCounts;
    const skipNoticeReset = clearHistoryActionInFlight;
    await loadHistoryCounts();
    if (skipNoticeReset || clearHistoryActionInFlight) return;
    if (
      historyCounts.total !== previous.total ||
      historyCounts.unpinned !== previous.unpinned ||
      historyCounts.pinned !== previous.pinned
    ) {
      clearHistoryNotice = "";
    }
  }

  async function finishClearHistoryAction(notice: string) {
    await loadHistoryCounts();
    clearHistoryNotice = notice;
  }

  async function handleClearHistoryMenu(action: ClearHistoryAction) {
    clearHistoryNotice = "";
    await loadHistoryCounts();

    switch (action) {
      case "unpinned": {
        if (historyCounts.unpinned === 0) {
          clearHistoryNotice = "No unpinned history to clear";
          return;
        }
        const confirmed = await confirmDestructive({
          title: "Clear unpinned history?",
          messageBody: clearUnpinnedConfirmBody(historyCounts),
          confirmLabel: "Clear unpinned",
        });
        if (!confirmed) return;
        clearHistoryActionInFlight = true;
        try {
          await clearHistory();
          await finishClearHistoryAction("Unpinned history cleared");
        } catch (err) {
          await finishClearHistoryAction(
            invokeErrorMessage(err) || "Could not clear history. Try again.",
          );
        } finally {
          clearHistoryActionInFlight = false;
        }
        break;
      }
      case "all": {
        if (historyCounts.total === 0) {
          clearHistoryNotice = "History is already empty";
          return;
        }
        const confirmed = await confirmDestructive({
          title: "Clear all history?",
          messageBody: clearAllConfirmBody(historyCounts),
          confirmLabel: "Clear all",
        });
        if (!confirmed) return;
        clearHistoryActionInFlight = true;
        try {
          await clearAllHistory();
          await finishClearHistoryAction("All history cleared");
        } catch (err) {
          await finishClearHistoryAction(
            invokeErrorMessage(err) || "Could not clear history. Try again.",
          );
        } finally {
          clearHistoryActionInFlight = false;
        }
        break;
      }
      default: {
        const exhaustive: never = action;
        void exhaustive;
        return;
      }
    }
  }

  const clearHistoryMenuDisabled = $derived(historyCounts.total === 0);

  const clearHistoryMenuItems = $derived([
    {
      id: "unpinned",
      label: "Clear unpinned history",
      disabled: historyCounts.unpinned === 0,
    },
    {
      id: "all",
      label: "Clear all history…",
      disabled: historyCounts.total === 0,
      destructive: true,
    },
  ]);

  async function handleVoiceToggle(enabled: boolean) {
    settings.voice_transcription_enabled = enabled;
    settings = await updateAppSettings({ voice_transcription_enabled: enabled });
    await rebindVoiceShortcut();
  }

  async function handleAiTaggingToggle(enabled: boolean) {
    settings.ai_tagging_enabled = enabled;
    settings = await updateAppSettings({ ai_tagging_enabled: enabled });
    if (enabled) {
      taggingResult = undefined;
      await refreshOllamaStatus();
    }
  }

  async function handleOverlayShortcutHintsToggle(enabled: boolean) {
    const previous = settings.overlay_shortcut_hints_enabled;
    settings.overlay_shortcut_hints_enabled = enabled;
    try {
      settings = await updateAppSettings({ overlay_shortcut_hints_enabled: enabled });
    } catch (err) {
      settings.overlay_shortcut_hints_enabled = previous;
      settingsNotice = invokeErrorMessage(err) || "Could not save setting. Try again.";
    }
  }

  const modelDirty = $derived(settings.ollama_model !== savedModel);

  const ollamaBusyActive = $derived(ollamaBusy !== null);
  const taggingSucceeded = $derived(
    !!ollamaStatus?.model_loaded &&
      taggingResult !== undefined &&
      taggingResult !== null,
  );
  const taggingFailed = $derived(!!ollamaStatus?.model_loaded && taggingResult === null);
  const taggingUntested = $derived(!ollamaStatus?.model_loaded || taggingResult === undefined);
</script>

{#snippet busySpinner()}
  <span class="app-btn-spinner" aria-hidden="true">
    <span class="app-btn-spinner-icon"></span>
  </span>
{/snippet}

<div class="settings-page ui-no-select">
  <div class="settings-head" data-tauri-drag-region>
    <div class="settings-title">Settings</div>
    <div class="settings-subtitle">Local AI and history behavior</div>
  </div>

  <section class="form-section">
    <div class="form-section-title form-section-title--with-icon">
      <SectionIcon name="permissions" />
      Permissions
    </div>
    <div class="form-section-body">
    <div class="inset-list">
      <div class="status-step">
        <div class="status-row">
          <span class="status-dot" class:ok={accessibilityGranted === true} class:fail={accessibilityGranted === false} class:checking={accessibilityGranted === null}></span>
          <span class="status-text">
            {accessibilityGranted === null ? "Checking..." : accessibilityGranted ? "Accessibility granted" : "Accessibility not granted"}
          </span>
          {#if accessibilityGranted === false}
            <button class="status-action app-btn" type="button" onclick={handleRequestAccessibility}>
              Request
            </button>
          {:else if accessibilityGranted === true}
            <button class="status-action app-btn" type="button" onclick={handleRecheckAccessibility}>
              Recheck
            </button>
          {/if}
        </div>
        {#if accessibilityGranted === false}
          <div class="status-hint">
            Required for paste automation (Cmd+V) and global shortcut.<br />
            Click "Request" to open System Settings, then enable <strong>Copyosity</strong>.
          </div>
        {/if}
        {#if accessibilityNotice}
          <div
            class="status-hint a11y-notice"
            class:ok={accessibilityNotice === A11Y_NOTICE_VERIFIED}
            class:warn={accessibilityNotice === A11Y_NOTICE_ENABLE}
          >{accessibilityNotice}</div>
        {/if}
        <div class="status-hint">
          After a new build or reinstall, remove Copyosity from Accessibility and add it again if paste stops working.
        </div>
      </div>
    </div>
    </div>
  </section>

  <section class="form-section">
    <div class="form-section-header">
      <div class="form-section-title form-section-title--with-icon">
        <SectionIcon name="ai-tagging" />
        AI Tagging
      </div>
      <label class="toggle">
        <input
          type="checkbox"
          role="switch"
          aria-label="Enable AI tagging"
          checked={settings.ai_tagging_enabled}
          onchange={(e) => void handleAiTaggingToggle((e.currentTarget as HTMLInputElement).checked)}
        />
        <span class="toggle-slider" aria-hidden="true"></span>
      </label>
    </div>
    <fieldset
      class="form-section-body form-section-body--subsections toggle-section-body"
      class:is-disabled={!settings.ai_tagging_enabled}
      disabled={!settings.ai_tagging_enabled}
    >
      <div class="form-hint">
        Automatically tag clipboard text entries using a local Ollama model.
      </div>
      <div class="form-subsection">
        <div class="form-subsection-title form-subsection-title--with-icon">
          <SectionIcon name="setup" />
          Setup
        </div>
        <div class="inset-list">
          {#if ollamaStatus === null}
            <div class="status-step">
              <div class="status-row">
                <span class="status-dot checking"></span>
                <span class="status-text">Checking...</span>
              </div>
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
            <button class="status-action app-btn" type="button" onclick={openOllamaDownload}>
              Open ollama.com
            </button>
          {/if}
        </div>
        {#if !ollamaStatus.cli_installed}
          <div class="status-hint">
            Ollama runs AI models locally on your machine. Download it from
            <button class="link-btn app-btn" type="button" onclick={openOllamaDownload}>ollama.com</button>,
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
            <button
              class="status-action app-btn"
              type="button"
              class:is-busy={ollamaBusy === "start"}
              class:is-locked={(ollamaBusyActive && ollamaBusy !== "start") || taggingLoading}
              aria-busy={ollamaBusy === "start" ? "true" : undefined}
              onclick={handleStartServer}
            >
              <span class="app-btn-label">Start</span>
              {@render busySpinner()}
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
          <span
            class="status-dot"
            class:ok={ollamaStatus.model_installed && ollamaStatus.model_loaded}
            class:warn={ollamaStatus.model_installed && !ollamaStatus.model_loaded}
            class:fail={ollamaStatus.server_running && !ollamaStatus.model_installed}
            class:disabled={!ollamaStatus.server_running}
          ></span>
          <span class="status-text" class:dimmed={!ollamaStatus.server_running}>
            {#if !ollamaStatus.model_installed}
              Model not installed
            {:else if ollamaStatus.model_loaded}
              Model ready
            {:else}
              Model unloaded
            {/if}
          </span>
          {#if ollamaStatus.server_running && !ollamaStatus.model_installed}
            <button
              class="status-action app-btn"
              type="button"
              class:is-busy={ollamaBusy === "pull"}
              class:is-locked={(ollamaBusyActive && ollamaBusy !== "pull") || taggingLoading}
              aria-busy={ollamaBusy === "pull" ? "true" : undefined}
              onclick={handlePullModel}
            >
              <span class="app-btn-label">Download</span>
              {@render busySpinner()}
            </button>
          {/if}
          {#if ollamaStatus.model_installed}
            <button
              class="status-action app-btn"
              type="button"
              class:is-busy={ollamaBusy === "unload"}
              class:is-locked={(ollamaBusyActive && ollamaBusy !== "unload") || taggingLoading}
              aria-busy={ollamaBusy === "unload" ? "true" : undefined}
              onclick={handleUnloadModel}
            >
              <span class="app-btn-label">Unload</span>
              {@render busySpinner()}
            </button>
          {/if}
        </div>
        {#if pullProgress}
          <div class="status-hint pull-progress">
            <span class="app-btn-spinner-icon is-inline" aria-hidden="true"></span>
            {pullProgress}
          </div>
        {:else if ollamaStatus.server_running && !ollamaStatus.model_installed}
          <div class="status-hint">
            Model <code>{ollamaStatus.model_name}</code> needs to be downloaded.
            Click "Download" or run <code>ollama pull {ollamaStatus.model_name}</code> in terminal.
            This may take a few minutes depending on your connection.
          </div>
        {:else if ollamaStatus.model_installed && ollamaStatus.model_loaded}
          <div class="status-hint ok">
            Using <code>{ollamaStatus.model_name}</code>
          </div>
        {:else if ollamaStatus.model_installed}
          <div class="status-hint">
            Model is on disk but not loaded in memory.<br />
            Click <strong>Test</strong> or use tagging to load it again.
          </div>
        {/if}
      </div>

      <!-- Step 4: Tagging test -->
      <div class="status-step">
        <div class="status-row">
          <span
            class="status-dot"
            class:checking={taggingLoading}
            class:ok={!taggingLoading && taggingSucceeded}
            class:fail={!taggingLoading && taggingFailed}
            class:disabled={!taggingLoading && taggingUntested}
          ></span>
          <span class="status-text" class:dimmed={!ollamaStatus.model_installed}>
            {#if taggingLoading}
              Testing...
            {:else if taggingUntested}
              Tagging not tested
            {:else if taggingResult !== null}
              Tagging works
            {:else}
              Tagging failed
            {/if}
          </span>
          {#if ollamaStatus.model_installed}
            <button
              class="status-action app-btn"
              type="button"
              disabled={modelDirty}
              class:is-busy={taggingLoading}
              class:is-locked={!modelDirty && ollamaBusyActive && !taggingLoading}
              aria-busy={taggingLoading ? "true" : undefined}
              aria-label="Test tagging"
              aria-describedby={modelDirty ? "tagging-test-save-hint" : undefined}
              onclick={handleTestTagging}
            >
              <span class="app-btn-label">Test</span>
              {@render busySpinner()}
            </button>
          {/if}
        </div>
        {#if modelDirty}
          <div id="tagging-test-save-hint" class="status-hint warn">
            Model changed — save settings first, then test.
          </div>
        {:else if taggingLoading}
          <div class="status-hint">
            {#if taggingResult !== undefined}
              Re-running test... Results below update when the request finishes.
            {:else}
              Sending test request... This can take up to 60 seconds on first run while the model loads into memory.
            {/if}
          </div>
        {:else if taggingSucceeded}
          <div class="status-hint ok">
            Test result: {taggingResult!.join(", ")}
          </div>
        {:else if taggingFailed}
          <div class="status-hint fail">
            The model did not return tags. Try a different model or check Ollama logs.
          </div>
        {:else if ollamaStatus.model_installed}
          <div class="status-hint">
            Click "Test" to verify tagging. You can re-run the test anytime.<br />
            "Check again" refreshes Ollama status and clears this result.
          </div>
        {/if}
      </div>

          {/if}
        </div>
        {#if ollamaStatus !== null}
          <div class="status-list-footer">
            <button
              class="form-btn form-btn-ghost app-btn"
              type="button"
              class:is-busy={ollamaBusy === "refresh"}
              class:is-locked={(ollamaBusyActive && ollamaBusy !== "refresh") || taggingLoading}
              aria-busy={ollamaBusy === "refresh" ? "true" : undefined}
              onclick={() => refreshOllamaStatus(true)}
            >
              <span class="app-btn-label">Check again</span>
              {@render busySpinner()}
            </button>
          </div>
        {/if}
      </div>
      <div class="form-subsection-rule" role="separator"></div>
      <div class="form-subsection">
        <div class="form-subsection-title form-subsection-title--with-icon">
          <SectionIcon name="ollama-model" />
          Ollama Model
        </div>
        <div class="form-field">
          <select
            class="form-select"
            aria-label="Ollama model"
            bind:value={selectedModelPreset}
            onchange={(e) => handleModelPresetChange((e.currentTarget as HTMLSelectElement).value)}
          >
            {#each modelCatalog.options as option}
              <option value={option.value}>
                {option.label} · ~{option.memory_gb.toFixed(1)} GB · {option.fits ? "Fits" : "Too large"}{option.installed ? " · Installed" : ""}
              </option>
            {/each}
            <option value="__custom__">Custom model</option>
          </select>
        </div>
        {#if selectedModelPreset === "__custom__"}
          <div class="form-field">
            <label class="form-label" for="custom-ollama-model">Model name</label>
            <input
              id="custom-ollama-model"
              class="form-input"
              type="text"
              bind:value={settings.ollama_model}
              placeholder="qwen3:4b-instruct-2507-q4_K_M"
            />
            <div class="form-hint">Memory use cannot be estimated for custom models.</div>
          </div>
        {/if}
      </div>
      <div class="form-subsection-rule" role="separator"></div>
      <div class="form-subsection">
        <div class="form-subsection-title form-subsection-title--with-icon">
          <SectionIcon name="this-mac" />
          This Mac
        </div>
        <dl class="form-meta inset-list" aria-label="Machine memory details">
          <div class="form-meta-item">
            <dt>Machine RAM</dt>
            <dd>{modelCatalog.total_memory_gb.toFixed(1)} GB</dd>
          </div>
          <div class="form-meta-item">
            <dt>Recommended Ollama budget</dt>
            <dd>{modelCatalog.recommended_memory_gb.toFixed(1)} GB</dd>
          </div>
        </dl>
      </div>
    </fieldset>
  </section>

  <section class="form-section">
    <div class="form-section-title form-section-title--with-icon">
      <SectionIcon name="clipboard-panel" />
      Clipboard Panel
    </div>
    <div class="form-section-body">
      <div class="inset-list">
        <div class="form-pref-row">
          <div class="form-pref-copy ui-selectable-text">
            <span class="form-pref-label">Keyboard shortcuts</span>
            <span class="form-pref-hint"
              >Show hint strip along the bottom of the clipboard panel.</span
            >
          </div>
          <label class="toggle">
            <input
              type="checkbox"
              role="switch"
              aria-label="Keyboard shortcut hints in clipboard panel"
              checked={settings.overlay_shortcut_hints_enabled}
              onchange={(e) =>
                void handleOverlayShortcutHintsToggle(
                  (e.currentTarget as HTMLInputElement).checked,
                )}
            />
            <span class="toggle-slider" aria-hidden="true"></span>
          </label>
        </div>
      </div>
    </div>
  </section>

  <section class="form-section">
    <div class="form-section-title form-section-title--with-icon">
      <SectionIcon name="storage" />
      Storage
    </div>
    <div class="form-section-body">
    <div class="form-field-group">
    <label class="form-field">
      <span class="form-label">History retention</span>
      <select class="form-select" bind:value={settings.retention_days}>
        {#each retentionOptions as option}
          <option value={option.value}>{option.label}</option>
        {/each}
      </select>
    </label>
    <ActionMenu
      block
      disabled={clearHistoryMenuDisabled}
      label="Clear history"
      items={clearHistoryMenuItems}
      onselect={(id) => void handleClearHistoryMenu(id as ClearHistoryAction)}
    />
    <div
      class="form-note form-note-neutral storage-clear-notice"
      class:visible={!!clearHistoryNotice}
      aria-live="polite"
    >
      {clearHistoryNotice}
    </div>
    </div>
    </div>
  </section>

  <section class="form-section">
    <div class="form-section-title form-section-title--with-icon">
      <SectionIcon name="privacy" />
      Privacy
    </div>
    <div class="form-section-body">
    <div class="form-field excluded-apps-field">
      <span class="form-label">Excluded apps</span>
      <div class="form-hint">
        Clipboard from excluded apps will not be stored or tagged.
      </div>

      <div class="excluded-apps-stack">
      <div class="excluded-apps-panel inset-list" role="group" aria-label="Excluded applications">
        {#if excludableCandidate}
          <div class="excluded-apps-row excluded-apps-row--candidate">
            <div class="excluded-apps-row-leading">
              <span class="excluded-apps-row-label">{excludableCandidate.displayName}</span>
              {#if !excludableCandidate.alreadyExcluded}
                <span class="excluded-apps-row-sep" aria-hidden="true">·</span>
                <span class="excluded-apps-row-meta"
                  >{excludableCandidateMetaLabel(excludableCandidate.source)}</span
                >
              {/if}
            </div>
            {#if excludableCandidate.alreadyExcluded && activeExcludedEntry}
              {@const removeLabel = allowInClipboardHistoryAriaLabel(excludableCandidate.displayName)}
              <button
                class="form-link-accent excluded-list-action app-btn"
                type="button"
                aria-label={removeLabel}
                aria-busy={excludeActionBusy}
                disabled={excludeActionBusy}
                onclick={() =>
                  handleRemoveExcludedApp(activeExcludedEntry.id, activeExcludedEntry.displayName)}
              >
                <span class="excluded-list-action-icon" aria-hidden="true">−</span>
                <span>{excludeListRemoveLabel()}</span>
              </button>
            {:else if excludableCandidate.alreadyExcluded}
              <span class="excluded-apps-row-meta">{alreadyExcludedListMetaLabel()}</span>
            {:else}
              {@const addLabel = excludeFromClipboardHistoryAriaLabel(excludableCandidate.displayName)}
              <button
                class="form-link-restrict excluded-list-action app-btn"
                type="button"
                aria-label={addLabel}
                aria-busy={excludeActionBusy}
                disabled={excludeActionBusy}
                onclick={handleAddCandidateApp}
              >
                <span class="excluded-list-action-icon" aria-hidden="true">+</span>
                <span>{excludeListAddLabel()}</span>
              </button>
            {/if}
          </div>
        {/if}

        {#each listedExcludedApps as app (app.id)}
          <div class="excluded-apps-row excluded-apps-row--listed">
            <span class="excluded-apps-row-label">{app.displayName}</span>
            <button
              class="form-link-accent excluded-list-action app-btn"
              type="button"
              aria-label={allowInClipboardHistoryAriaLabel(app.displayName)}
              aria-busy={excludeActionBusy}
              disabled={excludeActionBusy}
              onclick={() => handleRemoveExcludedApp(app.id, app.displayName)}
            >
              <span class="excluded-list-action-icon" aria-hidden="true">−</span>
              <span>{excludeListRemoveLabel()}</span>
            </button>
          </div>
        {/each}

        {#if !excludedAppsPanelHasRows}
          <div class="excluded-apps-empty" role="status">No apps excluded yet</div>
        {/if}
      </div>

      <button
        class="form-btn form-btn-secondary excluded-choose-app-btn app-btn"
        type="button"
        class:is-busy={excludeActionBusy}
        class:is-locked={excludeActionBusy}
        aria-busy={excludeActionBusy ? "true" : undefined}
        disabled={excludeActionBusy}
        onclick={handleChooseApp}
      >
        <svg
          class="form-btn-icon"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <rect x="3" y="3" width="12" height="12" rx="2.5" />
          <path d="M17 8v8" />
          <path d="M13 12h8" />
        </svg>
        <span class="app-btn-label">{chooseApplicationActionLabel}</span>
        {@render busySpinner()}
      </button>

      <div class="form-inline excluded-by-name">
        <input
          class="form-input"
          type="text"
          bind:value={excludedAppInput}
          placeholder="Installed app name, e.g. Telegram"
          aria-label="Installed app name to exclude"
          onkeydown={(e) => {
            if (e.key === "Enter") void handleAddExcludedByName();
          }}
        />
        <button
          class="form-btn form-btn-secondary app-btn"
          type="button"
          onclick={handleAddExcludedByName}
        >
          Add
        </button>
      </div>
      </div>

      {#if excludedAppsNotice}
        <div
          class="status-hint excluded-apps-notice"
          class:neutral={excludedAppsNoticeTone === "neutral"}
          class:warn={excludedAppsNoticeTone === "warn"}
          aria-live="polite"
        >
          {excludedAppsNotice}
        </div>
      {/if}
    </div>
    </div>
  </section>

  <section class="form-section">
    <div class="form-section-header">
      <div class="form-section-title form-section-title--with-icon">
        <SectionIcon name="voice" />
        Voice Transcription
      </div>
      <label class="toggle">
        <input
          type="checkbox"
          role="switch"
          aria-label="Enable voice transcription"
          checked={settings.voice_transcription_enabled}
          onchange={(e) => void handleVoiceToggle((e.currentTarget as HTMLInputElement).checked)}
        />
        <span class="toggle-slider" aria-hidden="true"></span>
      </label>
    </div>
    <fieldset
      class="form-section-body toggle-section-body"
      class:is-disabled={!settings.voice_transcription_enabled}
      disabled={!settings.voice_transcription_enabled}
    >
      <div class="form-hint">
        Hold the shortcut to record, release to transcribe and paste at cursor.<br />
        Requires an OpenAI-compatible Whisper server.
      </div>
      <label class="form-field">
        <span class="form-label">Shortcut (hold to record)</span>
        <input
          class="form-input"
          type="text"
          bind:value={settings.voice_shortcut}
          placeholder="option+space"
        />
        <div class="form-hint">
          Use: <code>cmd</code>, <code>option</code>, <code>ctrl</code>, <code>shift</code> + key<br />
          Examples: <code>option+space</code>, <code>cmd+shift+r</code>, <code>ctrl+alt+space</code>
        </div>
      </label>
      <label class="form-field">
        <span class="form-label">Microphone</span>
        <select class="form-select" bind:value={settings.selected_microphone}>
          <option value="">System default</option>
          {#each microphones as mic}
            <option value={mic.name}>{mic.name}{mic.is_default ? " (default)" : ""}</option>
          {/each}
        </select>
      </label>
      <label class="form-field">
        <span class="form-label">Server URL</span>
        <input
          class="form-input"
          type="text"
          bind:value={settings.whisper_server_url}
          placeholder="http://localhost:8000/v1/audio/transcriptions"
        />
      </label>
      <label class="form-field">
        <span class="form-label">API Token</span>
        <input
          class="form-input"
          type="password"
          bind:value={settings.whisper_server_token}
          placeholder="Bearer token (optional)"
        />
      </label>
      <label class="form-field">
        <span class="form-label">Model</span>
        <input
          class="form-input"
          type="text"
          bind:value={settings.whisper_server_model}
          placeholder="whisper-1"
        />
      </label>
    </fieldset>
  </section>

  <footer class="settings-footer">
    <div class="form-actions">
      <button
        class="form-btn form-btn-primary app-btn"
        type="button"
        class:is-busy={savingSettings}
        class:is-locked={ollamaBusyActive || taggingLoading}
        aria-busy={savingSettings ? "true" : undefined}
        onclick={saveSettings}
      >
        <span class="app-btn-label">Save settings</span>
        {@render busySpinner()}
      </button>
      <div
        class="form-note form-note-success"
        class:visible={!!settingsNotice}
        aria-live="polite"
      >
        {settingsNotice}
      </div>
    </div>
  </footer>

  <ConfirmDialog />
</div>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    background: var(--surface-page);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", sans-serif;
    color: var(--color-text-body);
  }

  :global(*) {
    box-sizing: border-box;
  }

  .settings-page {
    padding: 36px 20px 20px;
    max-width: 540px;
    margin: 0 auto;
  }

  .settings-head {
    margin-bottom: 16px;
    cursor: default;
    -webkit-app-region: drag;
  }

  .settings-title {
    font-size: var(--font-size-2xl);
    font-weight: 700;
    color: var(--color-text-primary);
    letter-spacing: -0.02em;
  }

  .settings-subtitle {
    margin-top: 4px;
    font-size: var(--font-size-md);
    color: var(--color-text-tertiary);
  }

  .settings-footer {
    margin-top: var(--space-section);
  }

  .settings-footer .form-actions {
    margin-top: 0;
  }

  .excluded-apps-notice {
    margin: 0;
  }
</style>
