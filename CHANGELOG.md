# Changelog

All notable changes to Copyosity are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-06-09

### Added

- **macOS Intel (x86_64) builds** — separate DMG artifacts for Apple Silicon and Intel Macs (for example `Copyosity_0.4.0_x86_64.dmg`); `Makefile` targets and `scripts/build-macos.sh` / `release-macos.sh` for arch-specific release builds.
- **macOS clipboard stack** — `clipboard_macos/` (pasteboard `changeCount`, concealed-pasteboard detection, paste-target remember/restore, AX tree walk, synthetic Cmd+V, Accessibility trust checks) on **objc2** (replaces legacy `objc`/`cocoa`).
- **Unified clipboard writes** (`clipboard_write.rs`) — copy, activate, paste, and voice flows share one code path with explicit **Copy** and **Paste** modes.
- **Image capture from Finder** — PNG, JPG/JPEG, and GIF files copied in Finder are stored in history (up to ~20 MB); the app ignores its own pasteboard writes.
- **Animated GIF support** — capture animated GIFs from the pasteboard or Finder (including Telegram and browsers), show thumbnails in history, and paste back as animated GIF instead of a static raster (file-URL pasteboard path with automatic temp-file cleanup).
- **Image format metadata** — `image_format` DB column; history cards show the format next to the Image badge (`Image PNG`, `Image GIF`, `Image JPG`); automatic format tags (`png`, `gif`, `jpg`) for the tag bar; shared detection in `image_format.rs`.
- **Per-window Tauri capabilities** — separate permission sets for `main`, `settings`, and `voice_overlay` instead of a single default capability.
- **Ollama model name validation** before `ollama pull`.
- **Release CI checks** — `cargo audit`, `npm run check`, and `cargo test` on tagged releases.
- **README** — Apple Silicon vs Intel install table and dual-architecture DMG guidance.
- **Voice transcription toggle** — Settings switch (off by default) to enable or disable hold-to-record transcription and its global shortcut without clearing Whisper configuration.
- **Shared button interaction** (`app-btn`, `button-interaction.css`) — macOS-like press, focus, disabled, and busy states for buttons across Settings, the main window, clipboard cards, and collection tabs; overlay spinner on busy buttons without layout shift; `prepareBusyUi()` yields before blocking IPC so spinners paint reliably.
- **Shared form controls** (`form-controls.css`) — compact macOS-style inputs, selects, section layout, and form buttons reused in Settings.
- **AI tagging toggle** — Settings switch (off by default) to enable or disable automatic Ollama tagging; when off, the clipboard monitor skips tag requests and startup backfill does not run.
- **`is_tagging_ready` IPC** — main window queries whether retag is available (tagging on + Ollama CLI, server, and model installed; unloaded model still counts).
- **Shared status-list layout** — compact checklist styling in `form-controls.css` with spacing tokens from `tokens.css` for consistent Settings rhythm.
- **Design tokens** (`tokens.css`) — single source for spacing, surfaces, borders, semantic colors, shadows, and focus rings; imported globally from `+layout.svelte`.
- **`form-link-danger`** — inline destructive text button style for list remove actions (Privacy excluded apps).
- **Unit tests** — **82 tests** in `copyosity_lib` for 0.4.0, with emphasis on clipboard monitor dedup/hash-poisoning, image format and animated GIF paste paths, DB migration and tag backfill, settings partial updates (Whisper, voice transcription, AI tagging toggles), `tagging_ready` / `is_ai_tagging_enabled`, Ollama model validation plus `/api/ps` load-unload matching, and `open_accessibility_settings` IPC.

### Changed

- **Clipboard monitor** — reads the pasteboard only when content actually changes; identical payloads are not re-captured or re-emitted to the UI.
- **Paste pipeline** — Enter in the main window activates an entry the same way as double-click (text and images); paste returns focus to the previous target app; voice transcription uses the same automated paste path.
- **Accessibility in Settings** — silent checks vs macOS trust dialog are separated; one prompt per Settings visit; live AX probe; **Recheck** confirms when access is still valid; guidance after rebuild or reinstall; `open_accessibility_settings` IPC from Settings.
- **Settings window** — native title bar (draggable again) with a custom header drag region.
- **Voice overlay** — pre-created NSPanel with non-activating behavior so showing the overlay no longer steals focus from the target app; audio level meter uses a logarithmic dB scale for quiet laptop mics.
- **Tray click** — opens the menu only; use **Open Copyosity** or `Cmd+Shift+V` to show the clipboard panel.
- **Makefile** — portable `APP_DIR` (`CURDIR`); `make check` runs `cargo test`.
- **Settings unload feedback** — Ollama step 3 shows three distinct states: model ready (green), model on disk but unloaded (static yellow), model not installed (red); `model_loaded` comes from Ollama `/api/ps`.
- **Settings Ollama actions** — Unload, Test, Start, and Download run on a background thread (`spawn_blocking`) so the WebView stays responsive; busy buttons use `is-busy` (not `disabled`) so WebKit keeps spinner animations running.
- **Voice transcription Settings UI** — compact on/off toggle; Whisper fields sit in a disabled fieldset when off; active toggle uses muted `--color-success-control` (no glow).
- **Settings Save button** — muted blue aligned with other Settings accents; stable width during save (overlay spinner, reserved “Saved” slot); macOS-style press/focus feedback without hover lift.
- **Settings form controls** — compact macOS-style inputs (32px, 13px type), consistent section spacing, shared `form-controls.css` reused across form blocks; clear-history action moved into Storage section, footer reserved for Save only.
- **Button hover/press** — removed `translateY` lift on hover; press uses inset darken/brightness instead of scale; async Settings actions expose `aria-busy` while loading.
- **Settings AI section** — merged “Local AI Status” and “AI Model” into a single **AI Tagging** block with on/off toggle; Setup checklist uses `status-list` with symmetric step padding; Ollama model picker lives in the same section.
- **Settings Ollama model UI** — preset options show memory estimate plus **Fits** / **Too large** and **Installed** in the picker; **This Mac** subsection (inset memory panel with RAM and recommended Ollama budget) is separated from the model field; custom models show a hint that memory use cannot be estimated.
- **Settings Permissions** — accessibility status uses the shared `status-list` pattern.
- **Voice transcription fieldset** — reuses `toggle-section-body` instead of a duplicate disabled-state class.
- **Retag button** — shown on text cards only when AI tagging is ready; hidden when tagging is off or Ollama is not set up.
- **Enabling AI tagging** — triggers tag backfill for existing untagged entries (on Save or toggle); `ensure_runtime` runs only while tagging is enabled.
- **UI color system** — main window, Settings, clipboard cards, search bar, voice overlay, and shared form/button styles use CSS variables from `tokens.css` instead of scattered hex/rgba literals.
- **Success palette** — cool sage green scale (`--color-success` for dots and overlays, `--color-success-text` for hints and save feedback, `--color-success-control` for toggles) tuned to dark-theme accent/danger weight.
- **Settings toggles** — accent focus ring when off, `--ring-success-control` when on; no neon fill or outer glow.
- **Privacy excluded apps** — Remove uses red `form-link-danger` instead of amber text.

### Fixed

- **Paste into Cursor and other Electron apps** — auto-paste runs on a background thread so the panel can hide and restore focus first; tries Accessibility paste, then session-wide Cmd+V.
- **Tray menu on first launch** — the hidden main panel no longer competes with the status-bar menu popup on the first click.
- **Image history backfill** — re-copying the same image updates legacy rows missing full-size `image_data` or `image_format`; existing `jpeg` format labels and tags are normalized to `jpg`.
- **Settings partial updates** — changing one field no longer wipes Whisper/voice/microphone settings.
- **Clipboard card action icons** — copy, retag, pin, and delete use uniform 16×16 SVG icons instead of mismatched Unicode glyphs; pinned star is filled and highlighted.
- **Clipboard card text preview** — long text no longer bleeds into the inner border or bottom padding; preview uses a padded outer box with grid clipping, and truncated text shows a CSS ellipsis (`line-clamp`: 9 lines without tags, 8 when tags are shown).
- **Clipboard card footer** — character count sits on its own line below tags instead of sharing a row, so many tags no longer wrap into the count label.
- **Settings Save layout jump** — saving no longer resizes the button or shifts the “Saved” label when loading state appears.
- **Settings Unload** — works on the first click with a visible busy spinner; uses the documented Ollama unload request (`prompt: ""`, `keep_alive: 0`), verifies unload via `/api/ps` (with `ollama stop` fallback), and clears the tagging test result only after the model is confirmed unloaded.
- **Settings tagging test** — repeatable after success; busy spinner and “Testing…” state render on every run; successful test display requires the model to be loaded in memory.
- **Settings Ollama status dot** — “Model unloaded” uses a static yellow indicator instead of the pulsing animation reserved for in-progress checks.
- **Settings status hints** — symmetric vertical padding within checklist rows; hint copy split across lines where it improves readability (model unloaded, tagging test).

### Dependencies

- **Frontend** — `@sveltejs/kit` 2.63, `svelte` 5.56.2, `svelte-check` 4.6.
- **Tauri** — synced npm (`@tauri-apps/api` 2.11, `@tauri-apps/cli` 2.11) and Rust (`tauri` 2.11); `tauri-plugin-opener` 2.5.4, `global-shortcut` 2.3.2, `sql` 2.4.0.
- **macOS** — `objc2`, `objc2-app-kit`, `objc2-foundation` (`NSData` for GIF pasteboard writes).

### Security

- Sensitive IPC commands scoped per window (`settings` cannot call paste commands; `voice_overlay` cannot call `clear_history` or `start_ollama_server`).
- `cargo audit` in the release workflow.
- **Tauri 2.11** — upstream IPC ACL hardening for custom commands from remote origins.

## [0.3.0] - 2026-04-10

### Added

- **Voice transcription** — dictate from the voice overlay into the active app.
- **Model pull progress** — non-blocking Ollama download via REST API with live progress in Settings.
- **Accessibility permission check** in Settings with explicit request action.
- Panel visible over fullscreen apps on all Spaces.

### Changed

- Wider Settings window; auto-save model before tagging test; spinner and loading hints in Settings.
- Ollama lookup searches common install paths when running from a `.app` bundle.

### Fixed

- Image copy used thumbnail instead of original resolution.
- Model presets, pull error handling, and unload-model button behavior.
- Tagging test timeout (60 s for cold model load); status refresh after save.
- Quit button uses `std::process::exit` to bypass `prevent_exit`.
- Settings window opener permission via Tauri capabilities.

## [0.2.1]

### Fixed

- Paste reliability, copy button behavior, scroll position reset, and ghost windows after hide.

## [0.2.0]

### Added

- NSPanel-based main window (no focus stealing).
- Dedicated Settings window.
- Security hardening and updated app icons.
