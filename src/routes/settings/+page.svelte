<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type {
    AppSettings,
    AudioInputDevice,
    ExcludedApp,
    ExcludableAppCandidate,
    ExcludeAppResult,
    ModelCatalog,
    ModelOption,
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
    rebindPaletteShortcut,
    getQuickMenuShortcut,
    setQuickMenuShortcut,
    listMicrophones,
    checkAccessibility,
    openAccessibilitySettings,
    checkOllamaStatus,
    unloadOllamaModel,
    startOllamaServer,
    pullOllamaModel,
    testOllamaTagging,
    hubTestConnection,
    hubListModels,
    resetOverlayBoardSizes,
    type OllamaStatus,
  } from "$lib/api";
  import ActionMenu from "$lib/components/ActionMenu.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import SectionIcon, { type SectionIconName } from "$lib/components/SectionIcon.svelte";
  import HotkeySettingsSection from "$lib/components/HotkeySettingsSection.svelte";
  import SnippetsEditor from "$lib/components/SnippetsEditor.svelte";
  import { confirmDestructive } from "$lib/confirm";
  import {
    coerceSettingsPane,
    isSettingsPane,
    parseSettingsPaneFromQuery,
    type SettingsPane,
  } from "$lib/settings-pane";
  import {
    clearAllConfirmBody,
    clearUnpinnedConfirmBody,
    type ClearHistoryAction,
  } from "$lib/destructive-actions";
  import {
    allowInClipboardHistoryAriaLabel,
    allowedInHistoryNotice,
    alreadyExcludedFromHistoryNotice,
    alreadyExcludedListMetaLabel,
    appNotFoundNotice,
    chooseApplicationActionLabel,
    couldNotAddExcludedAppNotice,
    couldNotAddSelectedAppNotice,
    excludeFromClipboardHistoryAriaLabel,
    excludeListAddLabel,
    excludeListRemoveLabel,
    excludableCandidateMetaLabel,
    excludedFromHistoryNotice,
    invokeErrorMessage,
    isAppNotFoundError,
  } from "$lib/exclusion-label";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { initPlatform, platformIsMacOS } from "$lib/platform.svelte";
  import { checkForUpdate, currentVersion, relaunch, type Update } from "$lib/updater";

  let settings = $state<AppSettings>({
    ollama_model: "qwen3:4b-instruct-2507-q4_K_M",
    retention_days: 30,
    whisper_server_url: "",
    whisper_server_token: "",
    whisper_server_model: "whisper-1",
    voice_shortcut: "option+space",
    selected_microphone: "",
    hub_enabled: false,
    hub_url: "https://api.neuraldeep.ru",
    hub_token: "",
    hub_chat_model: "qwen3.6-35b-a3b",
    hub_tagging_enabled: false,
    hub_transcribe_enabled: false,
    voice_polish_enabled: false,
    voice_polish_model: "qwen3.6-35b-a3b",
    voice_polish_screenshot: true,
    voice_polish_prompt: "",
    voice_translate_lang: "",
    voice_dictionary: "",
    voice_selected_text: false,
    board_vertical: false,
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
  let clearHistoryNotice = $state("");
  let historyCounts = $state({ total: 0, unpinned: 0, pinned: 0 });
  let clearHistoryActionInFlight = false;
  let savingSettings = $state(false);
  let settingsNotice = $state("");
  let savedModel = $state("");

  const A11Y_NOTICE_ENABLE = "Enable Copyosity in the list.";
  const A11Y_NOTICE_VERIFIED = "Accessibility verified — paste automation is ready.";

  let accessibilityGranted = $state<boolean | null>(null);
  let accessibilityNotice = $state("");
  /** User was sent to System Settings; keep enable hint until access is granted. */
  let a11yEnablePending = $state(false);
  /** macOS trust prompt already shown this settings-window visit. */
  let a11yPromptedThisVisit = false;

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

  const translateLangs = [
    { code: "en", label: "English" },
    { code: "ru", label: "Русский" },
    { code: "zh", label: "中文" },
    { code: "ja", label: "日本語" },
    { code: "ko", label: "한국어" },
    { code: "fr", label: "Français" },
    { code: "de", label: "Deutsch" },
    { code: "es", label: "Español" },
    { code: "pt", label: "Português" },
    { code: "it", label: "Italiano" },
    { code: "tr", label: "Türkçe" },
    { code: "uk", label: "Українська" },
  ];

  const retentionOptions = [
    { label: "1 day", value: 1 },
    { label: "1 week", value: 7 },
    { label: "1 month", value: 30 },
    { label: "6 months", value: 180 },
  ];

  /** Text fields and selects — saved via the footer bar. Booleans apply immediately. */
  const DEFERRED_SETTING_KEYS = [
    "ollama_model",
    "retention_days",
    "whisper_server_url",
    "whisper_server_token",
    "whisper_server_model",
    "selected_microphone",
    "hub_url",
    "hub_token",
    "hub_chat_model",
    "voice_polish_model",
    "voice_polish_prompt",
    "voice_translate_lang",
    "voice_dictionary",
  ] as const satisfies readonly (keyof AppSettings)[];

  type DeferredSettingKey = (typeof DEFERRED_SETTING_KEYS)[number];

  function pickDeferred(s: AppSettings): Pick<AppSettings, DeferredSettingKey> {
    return Object.fromEntries(
      DEFERRED_SETTING_KEYS.map((key) => [key, s[key]]),
    ) as Pick<AppSettings, DeferredSettingKey>;
  }

  async function loadSettings() {
    settings = await getAppSettings();
    selectedModelPreset = settings.ollama_model;
    savedModel = settings.ollama_model;
    snapshotDeferred();
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
      setExcludedAppsNotice(alreadyExcludedFromHistoryNotice(result.displayName), "warn");
      return;
    }
    setExcludedAppsNotice(excludedFromHistoryNotice(result.displayName));
  }

  async function refreshExcludedAppsSection() {
    await Promise.all([loadExcludedApps(), loadExcludableCandidate()]);
  }

  async function loadHistoryCounts() {
    historyCounts = await getHistoryCounts();
  }

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
    void initPlatform().then(() => {
      if (!platformIsMacOS() && activePane === "permissions") {
        activePane = "hub";
      }
      if (platformIsMacOS()) {
        void refreshExcludedAppsSection();
      }
      return undefined;
    });
    void loadSettings().then(() => {
      if (settings.hub_token?.trim()) {
        void handleHubTest();
        void loadHubModels();
      }
      return undefined;
    });
    loadModelCatalog();
    void loadHistoryCounts();
    void loadQuickMenuShortcut();
    refreshOllamaStatus();
    void currentVersion().then((v) => {
      appVersion = v;
      return undefined;
    });
    void checkUpdates();
    listMicrophones().then((m) => {
      microphones = m;
      return undefined;
    });

    const win = getCurrentWindow();
    void promptAccessibilityIfNeeded();

    const unlistenPull = listen<string>("ollama-pull-progress", (event) => {
      pullProgress = event.payload;
    });

    const unlistenPullDone = listen<boolean>("ollama-pull-done", async () => {
      ollamaLoading = false;
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

    const unlistenNavigatePane = listen<string>("navigate-settings-pane", (event) => {
      applySettingsPane(event.payload);
    });

    const unlistenFocus = win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        void updateAccessibilityStatus();
        void refreshExcludedAppsSection();
        void refreshHistoryCounts();
      }
    });

    return () => {
      unlistenPull.then((fn) => fn());
      unlistenPullDone.then((fn) => fn());
      unlistenShown.then((fn) => fn());
      unlistenClipboard.then((fn) => fn());
      unlistenHistory.then((fn) => fn());
      unlistenNavigatePane.then((fn) => fn());
      unlistenFocus.then((fn) => fn());
    };
  });

  async function saveSettings() {
    savingSettings = true;
    settingsNotice = "";
    try {
      const deferred = pickDeferred(settings);
      settings = await updateAppSettings(deferred);
      savedModel = settings.ollama_model;
      snapshotDeferred();
      settingsNotice = "Saved";
      taggingResult = undefined;
      await Promise.all([
        rebindVoiceShortcut(),
        rebindPaletteShortcut(),
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
      label: "Clear unpinned history…",
      disabled: historyCounts.unpinned === 0,
    },
    {
      id: "all",
      label: "Clear all history…",
      disabled: historyCounts.total === 0,
      destructive: true,
    },
  ]);

  let selectedModelMeta = $derived.by<ModelOption | null>(() => {
    return modelCatalog.options.find((o) => o.value === settings.ollama_model) ?? null;
  });

  let modelDirty = $derived(settings.ollama_model !== savedModel);

  type ImmediateSettingPatch = Partial<
    Pick<
      AppSettings,
      | "hub_enabled"
      | "hub_tagging_enabled"
      | "hub_transcribe_enabled"
      | "voice_polish_enabled"
      | "voice_polish_screenshot"
      | "voice_selected_text"
      | "voice_transcription_enabled"
      | "ai_tagging_enabled"
      | "overlay_shortcut_hints_enabled"
      | "board_vertical"
    >
  >;

  async function persistImmediate(
    patch: ImmediateSettingPatch,
    afterSave?: () => void | Promise<void>,
  ) {
    settingsNotice = "";
    const previous = settings;
    settings = { ...settings, ...patch };
    const deferredDraft = pickDeferred(settings);
    try {
      const saved = await updateAppSettings(patch);
      settings = { ...saved, ...deferredDraft };
      settingsNotice = "";
      await afterSave?.();
    } catch (e) {
      settings = previous;
      settingsNotice = String(e);
    }
  }

  function handleHubToggle(enabled: boolean) {
    void persistImmediate({ hub_enabled: enabled });
  }

  function handleVoiceToggle(enabled: boolean) {
    void persistImmediate({ voice_transcription_enabled: enabled }, async () => {
      await rebindVoiceShortcut();
    });
  }

  function handleAiTaggingToggle(enabled: boolean) {
    void persistImmediate({ ai_tagging_enabled: enabled }, async () => {
      if (enabled) {
        taggingResult = undefined;
        await refreshOllamaStatus();
      }
    });
  }

  function handleOverlayShortcutHintsToggle(enabled: boolean) {
    void persistImmediate({ overlay_shortcut_hints_enabled: enabled });
  }

  function handleBoardVerticalToggle(enabled: boolean) {
    void persistImmediate({ board_vertical: enabled });
  }

  async function handleRestoreOverlayBoardSizes() {
    try {
      await resetOverlayBoardSizes();
      settingsNotice = "Panel size restored to default";
    } catch (e) {
      settingsNotice = String(e);
    }
  }

  function handleHubTaggingToggle(enabled: boolean) {
    void persistImmediate({ hub_tagging_enabled: enabled });
  }

  function handleHubTranscribeToggle(enabled: boolean) {
    void persistImmediate({ hub_transcribe_enabled: enabled });
  }

  function handleVoicePolishToggle(enabled: boolean) {
    void persistImmediate({ voice_polish_enabled: enabled });
  }

  function handleVoicePolishScreenshotToggle(enabled: boolean) {
    void persistImmediate({ voice_polish_screenshot: enabled });
  }

  function handleVoiceSelectedTextToggle(enabled: boolean) {
    void persistImmediate({ voice_selected_text: enabled });
  }

  // ---- Sidebar navigation ----
  function paneFromUrl(): SettingsPane | null {
    if (typeof window === "undefined") return null;
    return parseSettingsPaneFromQuery(window.location.search);
  }

  function applySettingsPane(pane: string | null | undefined) {
    if (!pane || !isSettingsPane(pane)) return;
    activePane = pane;
  }

  let activePane = $state<SettingsPane>(coerceSettingsPane(paneFromUrl()));
  const panes: { id: SettingsPane; label: string; icon: SectionIconName }[] = [
    { id: "hub", label: "NeuralDeep", icon: "hub" },
    { id: "voice", label: "Voice", icon: "voice" },
    { id: "quickmenu", label: "Quick Menu", icon: "clipboard-panel" },
    { id: "ai", label: "Local AI", icon: "ai-tagging" },
    { id: "history", label: "History", icon: "clipboard-panel" },
    { id: "permissions", label: "Permissions", icon: "permissions" },
    { id: "updates", label: "Updates", icon: "setup" },
  ];

  // ---- Quick menu (Clipy-style native menu) ----
  let quickMenuShortcut = $state("cmd+shift+c");
  let quickMenuNotice = $state("");
  async function loadQuickMenuShortcut() {
    try {
      quickMenuShortcut = await getQuickMenuShortcut();
    } catch {
      // keep default
    }
  }
  async function saveQuickMenuShortcut() {
    quickMenuNotice = "";
    try {
      quickMenuShortcut = await setQuickMenuShortcut(quickMenuShortcut);
      quickMenuNotice = "Saved";
    } catch (e) {
      quickMenuNotice = `${e}`;
    }
  }

  let voiceShortcutNotice = $state("");
  async function saveVoiceShortcut() {
    voiceShortcutNotice = "";
    try {
      const deferred = pickDeferred(settings);
      const saved = await updateAppSettings({ voice_shortcut: settings.voice_shortcut });
      settings = { ...saved, ...deferred };
      await rebindVoiceShortcut();
      snapshotDeferred();
      voiceShortcutNotice = "Saved";
    } catch (e) {
      voiceShortcutNotice = `${e}`;
    }
  }

  // ---- Auto-updates ----
  let appVersion = $state("");
  let updateChecking = $state(false);
  let update = $state<Update | null>(null);
  let updateMessage = $state("");
  let updateInstalling = $state(false);
  let updateProgress = $state(0); // 0..100, -1 = indeterminate

  async function checkUpdates() {
    updateChecking = true;
    updateMessage = "";
    update = null;
    try {
      const u = await checkForUpdate();
      if (u) {
        update = u;
        updateMessage = `Update available: ${u.version}`;
      } else {
        updateMessage = "You're on the latest version.";
      }
    } catch (e) {
      updateMessage = `Check failed: ${e}`;
    } finally {
      updateChecking = false;
    }
  }

  async function installUpdate() {
    if (!update) return;
    updateInstalling = true;
    updateProgress = -1;
    let total = 0;
    let downloaded = 0;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
          updateProgress = total ? 0 : -1;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          updateProgress = total ? Math.round((downloaded / total) * 100) : -1;
        } else if (event.event === "Finished") {
          updateProgress = 100;
        }
      });
      updateMessage = "Installed — restarting…";
      await relaunch();
    } catch (e) {
      updateMessage = `Install failed: ${e}`;
      updateInstalling = false;
    }
  }
  const visiblePanes = $derived(
    platformIsMacOS() ? panes : panes.filter((p) => p.id !== "permissions"),
  );

  $effect(() => {
    if (activePane === "permissions" && platformIsMacOS()) {
      void updateAccessibilityStatus();
    }
  });

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

  // ---- Dirty tracking (unsaved deferred fields only) ----
  let savedDeferredSnapshot = $state("");
  let isDirty = $derived(
    savedDeferredSnapshot !== "" &&
      JSON.stringify(pickDeferred(settings)) !== savedDeferredSnapshot,
  );
  function snapshotDeferred() {
    savedDeferredSnapshot = JSON.stringify(pickDeferred(settings));
  }

  // Transient success/error bar — dismiss when navigating away or editing deferred fields.
  $effect(() => {
    void activePane;
    settingsNotice = "";
  });
  $effect(() => {
    if (isDirty) settingsNotice = "";
  });

  async function resetSettings() {
    await loadSettings();
    settingsNotice = "";
  }
</script>

{#snippet busySpinner()}
  <span class="app-btn-spinner" aria-hidden="true">
    <span class="app-btn-spinner-icon"></span>
  </span>
{/snippet}

<div class="settings-shell ui-no-select">
  <aside class="settings-sidebar">
    <div class="sidebar-title">Settings</div>
    <nav class="sidebar-nav">
      {#each visiblePanes as p}
        <button
          class="nav-item"
          class:active={activePane === p.id}
          type="button"
          onclick={() => (activePane = p.id)}
        >
          <SectionIcon name={p.icon} class="nav-section-icon" />
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

      <section class="form-section">
        <div class="form-section-header">
          <div class="form-section-title form-section-title--with-icon"><SectionIcon name="hub" />Hub</div>
          <label class="toggle">
            <input
              type="checkbox"
              role="switch"
              aria-label="Enable NeuralDeep hub"
              checked={settings.hub_enabled}
              onchange={(e) => handleHubToggle((e.currentTarget as HTMLInputElement).checked)}
            />
            <span class="toggle-slider" aria-hidden="true"></span>
          </label>
        </div>
        <fieldset
          class="form-section-body form-section-body--subsections toggle-section-body"
          class:is-disabled={!settings.hub_enabled}
          disabled={!settings.hub_enabled}
        >
          <div class="form-hint">
            When off, hub tagging, transcription, web search, and voice polishing are disabled.
          </div>

          <div class="form-subsection">
            <div class="form-subsection-title form-subsection-title--with-icon"><SectionIcon name="setup" />Connection</div>
            <div class="inset-list">
              <div class="form-field">
                <span class="form-label">3 steps</span>
                <ol class="form-instruction-list">
                  <li><button class="link-btn app-btn" type="button" onclick={() => openUrl("https://hub.neuraldeep.ru/app")}>Open the hub</button> → copy your <code>sk-…</code> key</li>
                  <li>Paste it into <strong>API Token</strong> below</li>
                  <li>Click <strong>Test connection</strong> → then <strong>Save</strong></li>
                </ol>
              </div>

              <label class="form-field">
                <span class="form-label">API base URL</span>
                <input
                  class="form-input"
                  type="text"
                  bind:value={settings.hub_url}
                  placeholder="https://api.neuraldeep.ru"
                />
              </label>
              <label class="form-field">
                <span class="form-label">API Token</span>
                <input
                  class="form-input"
                  type="password"
                  bind:value={settings.hub_token}
                  placeholder="sk-... Bearer token"
                />
                <button class="form-btn form-btn-secondary app-btn" type="button" disabled={hubTesting} onclick={handleHubTest}>
                  {hubTesting ? "Testing..." : "Test connection"}
                </button>
                {#if hubTestResult}
                  <div class="form-hint" class:ok={hubTestResult.ok} class:fail={!hubTestResult.ok}>
                    {hubTestResult.message}
                  </div>
                {/if}
              </label>
            </div>
          </div>

          <div class="form-subsection-rule" role="separator"></div>

          <div class="form-subsection">
            <div class="form-subsection-title form-subsection-title--with-icon"><SectionIcon name="ollama-model" />Model</div>
            <div class="inset-list">
              <label class="form-field">
                <span class="form-label">Chat / agent model (tagging & search)</span>
                <div class="form-inline">
                  <select class="form-select" bind:value={settings.hub_chat_model}>
                    {#each withCurrent(hubModels.length ? hubModels : ["gpt-oss-120b", "qwen3.6-35b-a3b", "gemma-4-31b"], settings.hub_chat_model) as m}
                      <option value={m}>{m}</option>
                    {/each}
                  </select>
                  <button class="form-btn form-btn-secondary app-btn" type="button" disabled={modelsLoading} onclick={loadHubModels} title="Load live model list from the hub">
                    {modelsLoading ? "…" : "↻"}
                  </button>
                </div>
                <div class="form-hint">
                  {hubModels.length ? `${hubModels.length} models loaded from the hub` : "Test connection or press ↻ to load the live model list."}
                </div>
              </label>
            </div>
          </div>

          <div class="form-subsection-rule" role="separator"></div>

          <div class="form-subsection">
            <div class="form-subsection-title form-subsection-title--with-icon"><SectionIcon name="setup" />Features</div>
            <div class="inset-list">
              <label class="form-checkbox">
                <span class="form-checkbox-leading">
                  <input
                    type="checkbox"
                    checked={settings.hub_tagging_enabled}
                    onchange={(e) =>
                      handleHubTaggingToggle((e.currentTarget as HTMLInputElement).checked)}
                  />
                  <span class="form-checkbox-box" aria-hidden="true"></span>
                </span>
                <span class="form-checkbox-label ui-selectable-text">Use hub for tagging (falls back to Ollama on error)</span>
              </label>
              <label class="form-checkbox">
                <span class="form-checkbox-leading">
                  <input
                    type="checkbox"
                    checked={settings.hub_transcribe_enabled}
                    onchange={(e) =>
                      handleHubTranscribeToggle((e.currentTarget as HTMLInputElement).checked)}
                  />
                  <span class="form-checkbox-box" aria-hidden="true"></span>
                </span>
                <span class="form-checkbox-label ui-selectable-text">Use hub for voice transcription</span>
              </label>
            </div>
            <div class="form-hint">
              <strong>Web search:</strong> press <code>⌘⇧Space</code> anywhere (or tray → Agent Search,
              or the search button in the main window) to query the web via the hub.
            </div>
          </div>
        </fieldset>
      </section>
    {:else if activePane === "voice"}
      <div class="pane-head">
        <div class="pane-title">Voice</div>
        <div class="pane-subtitle">Hold the shortcut to record, release to transcribe and paste at the cursor.</div>
      </div>

      <section class="form-section">
        <div class="form-section-header">
          <div class="form-section-title form-section-title--with-icon"><SectionIcon name="recording" />Recording</div>
          <label class="toggle">
            <input
              type="checkbox"
              role="switch"
              aria-label="Enable voice transcription"
              checked={settings.voice_transcription_enabled}
              onchange={(e) => handleVoiceToggle((e.currentTarget as HTMLInputElement).checked)}
            />
            <span class="toggle-slider" aria-hidden="true"></span>
          </label>
        </div>
        <fieldset
          class="form-section-body form-section-body--subsections toggle-section-body"
          class:is-disabled={!settings.voice_transcription_enabled}
          disabled={!settings.voice_transcription_enabled}
        >
          <div class="form-hint">
            When off, the global hold-to-record shortcut is not registered.
          </div>

          <HotkeySettingsSection
            bind:value={settings.voice_shortcut}
            placeholder="option+space"
            examples={["option+space", "cmd+shift+r", "ctrl+alt+space"]}
            detail="Hold to record, release to transcribe and paste at the cursor."
            notice={voiceShortcutNotice || undefined}
            onSave={saveVoiceShortcut}
          />

          <div class="form-subsection-rule" role="separator"></div>

          <div class="form-subsection">
            <div class="form-subsection-title form-subsection-title--with-icon">
              <SectionIcon name="hub" class="form-subsection-icon" />Transcription
            </div>
            <div class="inset-list">
              <label class="form-checkbox">
                <span class="form-checkbox-leading">
                  <input
                    type="checkbox"
                    checked={settings.hub_transcribe_enabled}
                    disabled={!settings.hub_enabled}
                    onchange={(e) =>
                      handleHubTranscribeToggle((e.currentTarget as HTMLInputElement).checked)}
                  />
                  <span class="form-checkbox-box" aria-hidden="true"></span>
                </span>
                <span class="form-checkbox-label ui-selectable-text">Transcribe with NeuralDeep Hub</span>
              </label>
            </div>
            <div class="form-hint">
              {#if settings.hub_enabled}
                On → hub <code>/v1/audio/transcriptions</code>; off → your local Whisper server.
              {:else}
                Enable the <strong>NeuralDeep Hub</strong> (Hub tab) to transcribe via the hub.
              {/if}
            </div>
          </div>

          <div class="form-subsection-rule" role="separator"></div>

          <div class="form-subsection">
            <div class="form-subsection-title form-subsection-title--with-icon">
              <SectionIcon name="voice" class="form-subsection-icon" />Microphone
            </div>
            <div class="inset-list">
              <div class="form-field">
                <select
                  class="form-select"
                  aria-label="Microphone"
                  bind:value={settings.selected_microphone}
                >
                  <option value="">System default</option>
                  {#each microphones as mic}
                    <option value={mic.name}>{mic.name}{mic.is_default ? " (default)" : ""}</option>
                  {/each}
                </select>
              </div>
            </div>
          </div>

          <div class="form-subsection-rule" role="separator"></div>

          <div class="form-subsection">
            <div class="form-subsection-title form-subsection-title--with-icon"><SectionIcon name="ollama-model" />Transcription model</div>
            <div class="form-hint">
              With <strong>hub transcription</strong> on (NeuralDeep tab), audio goes to the hub model below.
              Otherwise it uses the custom Whisper server.
            </div>
            <div class="inset-list">
              <label class="form-field">
                <span class="form-label">Model</span>
                <div class="form-inline">
                  <select class="form-select" bind:value={settings.whisper_server_model}>
                    {#each withCurrent(hubModels.length ? hubModels : ["whisper-1"], settings.whisper_server_model) as m}
                      <option value={m}>{m}</option>
                    {/each}
                  </select>
                  <button class="form-btn form-btn-secondary app-btn" type="button" disabled={modelsLoading} onclick={loadHubModels} title="Load live model list from the hub">
                    {modelsLoading ? "…" : "↻"}
                  </button>
                </div>
                <div class="form-hint">
                  Pick a transcription model — e.g. <code>whisper-1</code> or <code>подлодка</code>
                </div>
              </label>
              <label class="form-field">
                <span class="form-label">Custom Whisper server URL (optional)</span>
                <input
                  class="form-input"
                  type="text"
                  bind:value={settings.whisper_server_url}
                  placeholder="http://localhost:8000/v1/audio/transcriptions"
                />
              </label>
              <label class="form-field">
                <span class="form-label">Custom server token (optional)</span>
                <input
                  class="form-input"
                  type="password"
                  bind:value={settings.whisper_server_token}
                  placeholder="Bearer token (optional)"
                />
              </label>
            </div>
          </div>

          <div class="form-subsection-rule" role="separator"></div>

          <div class="form-subsection">
            <div class="form-subsection-title form-subsection-title--with-icon"><SectionIcon name="text-polish" />AI text polishing</div>
            <div class="form-hint">
              After transcription, run the text through the hub LLM to add punctuation,
              remove filler, format lists, and match the app you're pasting into.
              Returns only the cleaned text. Requires the NeuralDeep hub.
            </div>
            <div class="inset-list">
              <label class="form-checkbox">
                <span class="form-checkbox-leading">
                  <input
                    type="checkbox"
                    checked={settings.voice_polish_enabled}
                    disabled={!settings.hub_enabled}
                    onchange={(e) =>
                      handleVoicePolishToggle((e.currentTarget as HTMLInputElement).checked)}
                  />
                  <span class="form-checkbox-box" aria-hidden="true"></span>
                </span>
                <span class="form-checkbox-label ui-selectable-text">Polish transcription with the LLM before pasting</span>
              </label>
              {#if settings.voice_polish_enabled && settings.hub_enabled}
                <label class="form-checkbox form-checkbox--indented">
                  <span class="form-checkbox-leading">
                    <input
                      type="checkbox"
                      checked={settings.voice_polish_screenshot}
                      onchange={(e) =>
                        handleVoicePolishScreenshotToggle((e.currentTarget as HTMLInputElement).checked)}
                    />
                    <span class="form-checkbox-box" aria-hidden="true"></span>
                  </span>
                  <span class="form-checkbox-label ui-selectable-text">Send a screenshot of the target window for context (needs Screen Recording permission)</span>
                </label>
                <label class="form-checkbox form-checkbox--indented">
                  <span class="form-checkbox-leading">
                    <input
                      type="checkbox"
                      checked={settings.voice_selected_text}
                      onchange={(e) =>
                        handleVoiceSelectedTextToggle((e.currentTarget as HTMLInputElement).checked)}
                    />
                    <span class="form-checkbox-box" aria-hidden="true"></span>
                  </span>
                  <span class="form-checkbox-label ui-selectable-text">Selected-text mode: if text is selected, your voice becomes an instruction (summarize / fix / translate / rewrite it)</span>
                </label>

                <label class="form-field">
                  <span class="form-label">Polish model (multimodal for screenshots)</span>
                  <div class="form-inline">
                    <select class="form-select" bind:value={settings.voice_polish_model}>
                      {#each withCurrent(hubModels.length ? hubModels : ["qwen3.6-35b-a3b", "gemma-4-31b"], settings.voice_polish_model) as m}
                        <option value={m}>{m}</option>
                      {/each}
                    </select>
                    <button class="form-btn form-btn-secondary app-btn" type="button" disabled={modelsLoading} onclick={loadHubModels} title="Load live model list from the hub">
                      {modelsLoading ? "…" : "↻"}
                    </button>
                  </div>
                  <div class="form-hint">Use a multimodal model (e.g. <code>qwen3.6-35b-a3b</code>) when screenshots are on.</div>
                </label>

                <label class="form-field">
                  <span class="form-label">Translate result to</span>
                  <select class="form-select" bind:value={settings.voice_translate_lang}>
                    <option value="">Don't translate</option>
                    {#each translateLangs as l}
                      <option value={l.code}>{l.label}</option>
                    {/each}
                  </select>
                </label>

                <label class="form-field">
                  <span class="form-label">Custom polishing instructions (optional)</span>
                  <textarea
                    class="form-textarea"
                    rows="3"
                    bind:value={settings.voice_polish_prompt}
                    placeholder="e.g. Always sign off with 'Спасибо!'; keep it formal."
                  ></textarea>
                </label>

                <label class="form-field">
                  <span class="form-label">Dictionary — exact spellings (one per line)</span>
                  <textarea
                    class="form-textarea"
                    rows="3"
                    bind:value={settings.voice_dictionary}
                    placeholder={"NeuralDeep\nCopyosity\nKubernetes"}
                  ></textarea>
                  <div class="form-hint">The model will keep these terms spelled exactly as written.</div>
                </label>
              {/if}
            </div>
          </div>
        </fieldset>
      </section>
    {:else if activePane === "quickmenu"}
      <div class="pane-head">
        <div class="pane-title">Quick Menu</div>
        <div class="pane-subtitle">A native pop-up menu for recent history and saved snippets — paste in two clicks, no overlay browsing.</div>
      </div>

      <section class="form-section">
        <div class="form-section-body form-section-body--subsections">
          <HotkeySettingsSection
            bind:value={quickMenuShortcut}
            placeholder="cmd+shift+c"
            examples={["cmd+shift+c"]}
            detail="Press anywhere to pop the menu at your cursor with history and snippets; items 1–9 have number keys."
            notice={quickMenuNotice || undefined}
            onSave={saveQuickMenuShortcut}
          />

          <div class="form-subsection-rule" role="separator"></div>

          <div class="form-subsection">
            <div class="form-subsection-title form-subsection-title--with-icon">
              <SectionIcon name="snippets" class="form-subsection-icon" />Snippets
            </div>
            <div class="form-hint">
              Reusable text templates grouped in folders — they show up in the quick menu for two-click paste.
            </div>
            <SnippetsEditor />
          </div>
        </div>
      </section>
    {:else if activePane === "permissions"}
      <div class="pane-head">
        <div class="pane-title">Permissions</div>
        <div class="pane-subtitle">Accessibility access is required for paste automation and the global shortcut.</div>
      </div>

      <section class="form-section">
        <div class="form-section-title form-section-title--with-icon"><SectionIcon name="permissions" />Accessibility</div>
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
                  class="status-hint"
                  class:ok={accessibilityNotice === A11Y_NOTICE_VERIFIED}
                  class:warn={accessibilityNotice === A11Y_NOTICE_ENABLE}
                >
                  {accessibilityNotice}
                </div>
              {/if}
              <div class="status-hint">
                After a new build or reinstall, remove Copyosity from Accessibility and add it again if paste stops working.
              </div>
            </div>
          </div>
        </div>
      </section>
    {:else if activePane === "updates"}
      <div class="pane-head">
        <div class="pane-title">Updates</div>
        <div class="pane-subtitle">Copyosity updates itself from GitHub Releases — signed and verified.</div>
      </div>

      <section class="form-section">
        <div class="form-section-title form-section-title--with-icon"><SectionIcon name="setup" />Version</div>
        <div class="form-section-body">
          <div class="inset-list">
            <div class="status-step">
              <div class="status-row">
                <span class="status-dot" class:ok={!update && !!updateMessage} class:checking={updateChecking}></span>
                <span class="status-text">
                  Current: <code>{appVersion || "…"}</code>
                  {#if update}— new version <code>{update.version}</code> available{/if}
                </span>
                <button class="status-action app-btn" type="button" disabled={updateChecking || updateInstalling} onclick={checkUpdates}>
                  {updateChecking ? "Checking…" : "Check now"}
                </button>
              </div>
              {#if updateMessage && !update}
                <div class="status-hint">{updateMessage}</div>
              {/if}
              {#if update}
                <div class="status-hint">
                  {#if update.body}{update.body}{/if}
                </div>
                <button class="app-btn" type="button" disabled={updateInstalling} onclick={installUpdate}>
                  {#if updateInstalling}
                    {updateProgress >= 0 ? `Installing… ${updateProgress}%` : "Installing…"}
                  {:else}
                    Download &amp; install, then restart
                  {/if}
                </button>
                {#if updateInstalling && updateProgress >= 0}
                  <div class="update-progress"><div class="update-progress-fill" style="width: {updateProgress}%"></div></div>
                {/if}
              {/if}
            </div>
          </div>
        </div>
      </section>
    {:else if activePane === "ai"}
      <div class="pane-head">
        <div class="pane-title">Local AI</div>
        <div class="pane-subtitle">Run tagging on-device with Ollama — used when the hub is off or unavailable.</div>
      </div>

      <section class="form-section">
        <div class="form-section-header">
          <div class="form-section-title form-section-title--with-icon"><SectionIcon name="ai-tagging" />AI Tagging</div>
          <label class="toggle">
            <input
              type="checkbox"
              role="switch"
              aria-label="Enable AI tagging"
              checked={settings.ai_tagging_enabled}
              onchange={(e) => handleAiTaggingToggle((e.currentTarget as HTMLInputElement).checked)}
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
            When off, new entries are not auto-tagged (manual retag still works).
          </div>

          <div class="form-subsection">
            <div class="form-subsection-title form-subsection-title--with-icon"><SectionIcon name="setup" />Setup</div>
            <div class="inset-list">
              {#if ollamaStatus === null}
                <div class="status-step">
                  <div class="status-row">
                    <span class="status-dot checking"></span>
                    <span class="status-text">Checking...</span>
                  </div>
                </div>
              {:else}
                <div class="status-step">
                  <div class="status-row">
                    <span class="status-dot" class:ok={ollamaStatus.cli_installed} class:fail={!ollamaStatus.cli_installed}></span>
                    <span class="status-text">
                      {ollamaStatus.cli_installed ? "Ollama installed" : "Ollama not installed"}
                    </span>
                    {#if !ollamaStatus.cli_installed}
                      <button class="status-action app-btn" type="button" onclick={() => openUrl("https://ollama.com/download")}>
                        Open ollama.com
                      </button>
                    {/if}
                  </div>
                  {#if !ollamaStatus.cli_installed}
                    <div class="status-hint">
                      Ollama runs AI models locally on your machine. Download it from
                      <button class="link-btn app-btn" type="button" onclick={() => openUrl("https://ollama.com/download")}>ollama.com</button>,
                      install the app, and click "Check again".
                    </div>
                  {/if}
                </div>

                <div class="status-step">
                  <div class="status-row">
                    <span class="status-dot" class:ok={ollamaStatus.server_running} class:fail={ollamaStatus.cli_installed && !ollamaStatus.server_running} class:disabled={!ollamaStatus.cli_installed}></span>
                    <span class="status-text" class:dimmed={!ollamaStatus.cli_installed}>
                      {ollamaStatus.server_running ? "Server running" : "Server not running"}
                    </span>
                    {#if ollamaStatus.cli_installed && !ollamaStatus.server_running}
                      <button class="status-action app-btn" type="button" disabled={ollamaLoading} onclick={handleStartServer}>
                        {#if ollamaLoading}<span class="app-btn-spinner-icon app-btn-spinner-icon--sm app-btn-spinner-icon--success is-inline"></span> Starting...{:else}Start{/if}
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

                <div class="status-step">
                  <div class="status-row">
                    <span class="status-dot" class:ok={ollamaStatus.model_installed} class:fail={ollamaStatus.server_running && !ollamaStatus.model_installed} class:disabled={!ollamaStatus.server_running}></span>
                    <span class="status-text" class:dimmed={!ollamaStatus.server_running}>
                      {ollamaStatus.model_installed ? "Model ready" : "Model not installed"}
                    </span>
                    {#if ollamaStatus.server_running && !ollamaStatus.model_installed}
                      <button class="status-action app-btn" type="button" disabled={ollamaLoading} onclick={handlePullModel}>
                        {#if ollamaLoading}<span class="app-btn-spinner-icon app-btn-spinner-icon--sm app-btn-spinner-icon--success is-inline"></span> Pulling...{:else}Download{/if}
                      </button>
                    {/if}
                    {#if ollamaStatus.model_installed}
                      <button class="status-action app-btn" type="button" onclick={async () => { await unloadOllamaModel(); settingsNotice = "Model unloaded from memory"; }}>
                        Unload
                      </button>
                    {/if}
                  </div>
                  {#if pullProgress}
                    <div class="status-hint pull-progress">
                      <span class="app-btn-spinner-icon app-btn-spinner-icon--sm app-btn-spinner-icon--success is-inline"></span> {pullProgress}
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
                      <button class="status-action app-btn" type="button" disabled={taggingLoading || modelDirty} onclick={handleTestTagging} title={modelDirty ? "Save settings first" : ""}>
                        {#if taggingLoading}
                          <span class="app-btn-spinner-icon app-btn-spinner-icon--sm app-btn-spinner-icon--success is-inline"></span> Testing...
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
              {/if}
            </div>
            {#if ollamaStatus !== null}
              <div class="status-list-footer">
                <button class="form-btn form-btn-ghost app-btn" type="button" disabled={ollamaLoading} onclick={refreshOllamaStatus}>
                  Check again
                </button>
              </div>
            {/if}
          </div>

          <div class="form-subsection-rule" role="separator"></div>

          <div class="form-subsection">
            <div class="form-subsection-title form-subsection-title--with-icon"><SectionIcon name="ollama-model" />Ollama Model</div>
            <div class="inset-list">
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
          </div>

          <div class="form-subsection-rule" role="separator"></div>

          <div class="form-subsection">
            <div class="form-subsection-title form-subsection-title--with-icon"><SectionIcon name="this-mac" />This Mac</div>
            <dl class="form-meta inset-list" aria-label="Machine memory details">
              <div class="form-meta-item">
                <dt>Machine RAM</dt>
                <dd>{modelCatalog.total_memory_gb.toFixed(1)} GB</dd>
              </div>
              <div class="form-meta-item">
                <dt>Recommended Ollama budget</dt>
                <dd>{modelCatalog.recommended_memory_gb.toFixed(1)} GB</dd>
              </div>
              {#if selectedModelMeta}
                <div class="form-meta-item">
                  <dt>{selectedModelMeta.label}</dt>
                  <dd class:fits={selectedModelMeta.fits} class:tight={!selectedModelMeta.fits}>
                    ~{selectedModelMeta.memory_gb.toFixed(1)} GB · {selectedModelMeta.fits ? "Fits" : "Too large"}
                  </dd>
                </div>
              {/if}
            </dl>
          </div>
        </fieldset>
      </section>
    {:else if activePane === "history"}
      <div class="pane-head">
        <div class="pane-title">History</div>
        <div class="pane-subtitle">Layout, how long clips are kept, and which apps are ignored.</div>
      </div>

      <section class="form-section">
        <div class="form-section-title form-section-title--with-icon"><SectionIcon name="clipboard-panel" />Board layout</div>
        <div class="form-section-body">
          <div class="inset-list">
            <label class="form-checkbox">
              <span class="form-checkbox-leading">
                <input
                  type="checkbox"
                  checked={settings.board_vertical}
                  onchange={(e) =>
                    handleBoardVerticalToggle((e.currentTarget as HTMLInputElement).checked)}
                />
                <span class="form-checkbox-box" aria-hidden="true"></span>
              </span>
              <span class="form-checkbox-label ui-selectable-text">
                <span class="form-pref-label">Vertical board</span>
                <span class="form-pref-hint">Tall panel docked to the screen edge instead of the horizontal bottom bar. Opens on the screen where your cursor is (⌘⇧V).</span>
              </span>
            </label>
            <label class="form-checkbox">
              <span class="form-checkbox-leading">
                <input
                  type="checkbox"
                  checked={settings.overlay_shortcut_hints_enabled}
                  onchange={(e) =>
                    handleOverlayShortcutHintsToggle((e.currentTarget as HTMLInputElement).checked)}
                />
                <span class="form-checkbox-box" aria-hidden="true"></span>
              </span>
              <span class="form-checkbox-label ui-selectable-text">
                <span class="form-pref-label">Keyboard shortcuts</span>
                <span class="form-pref-hint">Show hint strips along the bottom of the clipboard panel and Quick Look preview.</span>
              </span>
            </label>
            <div class="form-pref-row">
              <div class="form-pref-copy">
                <span class="form-pref-label">Restore default size</span>
                <span class="form-pref-hint">Resets the panel's width and height to their original values.</span>
              </div>
              <button
                class="form-btn form-btn-secondary app-btn"
                type="button"
                onclick={() => void handleRestoreOverlayBoardSizes()}
              >
                Restore defaults
              </button>
            </div>
          </div>
        </div>
      </section>

      <section class="form-section">
        <div class="form-section-title form-section-title--with-icon"><SectionIcon name="privacy" />Privacy</div>
        <div class="form-section-body">
      {#if platformIsMacOS()}
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
                <rect x="3" y="5.5" width="12" height="12" rx="2.5" />
                <path d="M17 10.5v8" />
                <path d="M13 14.5h8" />
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
      {:else}
        <div class="form-hint">App exclusion is available on macOS only.</div>
      {/if}
    </div>
      </section>

      <section class="form-section">
        <div class="form-section-title form-section-title--with-icon"><SectionIcon name="storage" />Storage</div>
        <div class="form-section-body">
          <div class="inset-list">
            <label class="form-field">
              <span class="form-label">History retention</span>
              <select class="form-select" bind:value={settings.retention_days}>
                {#each retentionOptions as option}
                  <option value={option.value}>{option.label}</option>
                {/each}
              </select>
            </label>
            <div class="form-field">
              <ActionMenu
                block
                disabled={clearHistoryMenuDisabled}
                label="Clear history"
                items={clearHistoryMenuItems}
                onselect={(id) => void handleClearHistoryMenu(id as ClearHistoryAction)}
              />
            </div>
          </div>
          <div
            class="form-note form-note-neutral storage-clear-notice"
            class:visible={!!clearHistoryNotice}
            aria-live="polite"
          >
            {clearHistoryNotice}
          </div>
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

<ConfirmDialog />

<style>
  :global(body) {
    background: var(--surface-page);
    font-family: var(--font-family-system);
    color: var(--color-text-body);
  }

  .settings-shell {
    display: grid;
    grid-template-columns: 184px minmax(0, 1fr);
    height: 100%;
    min-height: 0;
    overflow: hidden;
    overflow-x: clip;
  }

  .settings-sidebar {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 16px 12px;
    background: var(--surface-sidebar);
    border-right: 1px solid var(--border-muted);
    min-height: 0;
    height: 100%;
  }

  .sidebar-title {
    padding: 4px 10px 12px;
    font-size: var(--font-size-lg);
    font-weight: 700;
    color: var(--color-text-primary);
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
    border-radius: var(--radius-control);
    color: var(--color-text-secondary);
    font: inherit;
    font-size: var(--font-size-sm);
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    transition: background var(--duration-fast) var(--ease-interactive), color var(--duration-fast) var(--ease-interactive);
  }

  .nav-item:hover {
    background: var(--surface-5);
    color: var(--color-text-primary);
  }

  .nav-item.active {
    background: var(--surface-accent-muted);
    border-color: var(--border-accent-soft);
    color: var(--color-text-primary);
  }

  .nav-item :global(.nav-section-icon) {
    width: var(--icon-size-section);
    height: var(--icon-size-section);
    flex-shrink: 0;
    color: var(--color-accent-icon);
  }

  .sidebar-foot {
    margin-top: auto;
    display: flex;
    align-items: center;
    gap: var(--space-stack);
    padding: 8px 10px;
    font-size: var(--font-size-xs);
    color: var(--color-text-tertiary);
  }

  .sidebar-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--surface-10);
    flex-shrink: 0;
  }

  .sidebar-dot.ok {
    background: var(--color-success-soft);
    box-shadow: 0 0 8px var(--shadow-success);
  }

  .sidebar-dot.fail {
    background: var(--color-danger-soft);
  }

  .settings-content {
    padding: 36px 20px 88px;
    max-width: 540px;
    width: 100%;
    min-width: 0;
    min-height: 0;
    overflow: hidden auto;
  }

  .pane-head {
    margin-bottom: var(--space-island-gap);
  }

  .pane-title {
    font-size: var(--font-size-2xl);
    font-weight: 700;
    color: var(--color-text-primary);
    letter-spacing: -0.02em;
  }

  .pane-subtitle {
    margin-top: 4px;
    font-size: var(--font-size-md);
    color: var(--color-text-tertiary);
    line-height: var(--line-height-body);
  }

  .form-hint.ok {
    color: var(--color-success-text);
  }

  .form-hint.fail {
    color: var(--color-danger-text);
  }

  .form-meta-item dd.fits {
    color: var(--color-success-text);
  }

  .form-meta-item dd.tight {
    color: var(--color-warning-text);
  }

  .excluded-apps-notice {
    margin: 0;
  }

  .dirty-bar {
    position: fixed;
    left: 184px;
    right: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--control-chevron-inset);
    padding: var(--overlay-grid-pad-y) var(--overlay-header-pad-block);
    background: var(--surface-bar);
    backdrop-filter: blur(12px);
    border-top: 1px solid var(--border-muted);
  }

  .dirty-bar.saved {
    justify-content: flex-start;
  }

  .dirty-label {
    font-size: var(--font-size-sm);
    font-weight: 600;
    color: var(--color-warning-text);
  }

  .dirty-bar.saved .dirty-label {
    color: var(--color-success-text);
  }

  .dirty-actions {
    display: flex;
    gap: var(--space-stack);
  }

  .dirty-reset {
    min-height: 36px;
    padding: 0 14px;
    background: var(--surface-5);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-control);
    color: var(--color-text-body);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }

  .dirty-save {
    min-height: 36px;
    padding: 0 18px;
    background: var(--surface-accent);
    border: 1px solid var(--border-accent-soft);
    border-radius: var(--radius-control);
    color: var(--color-text-primary);
    font: inherit;
    font-weight: 700;
    cursor: pointer;
  }

  .dirty-save:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .update-progress {
    margin-top: 10px;
    height: 6px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    overflow: hidden;
  }

  .update-progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #10b981, #34d399);
    border-radius: 999px;
    transition: width 0.2s ease;
  }
</style>
