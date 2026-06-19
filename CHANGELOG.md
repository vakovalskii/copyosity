# Changelog

All notable changes to Copyosity are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - Unreleased

### Added

- **Per-window IPC command scoping** — sensitive commands (`paste_entry`, `clear_history`, `start_ollama_server`, exclusion list editing) limited to the appropriate window via per-window Tauri capabilities (`main`, `settings`, `voice_overlay`).
- **`cargo audit` in release workflow** — dependency audit step in GitHub Actions before release artifacts ship.
- **macOS Intel (x86_64) builds** — separate DMG artifacts for Apple Silicon and Intel Macs; `Makefile` targets and release scripts for arch-specific builds.
- **macOS clipboard stack** — `clipboard_macos/` on **objc2** (pasteboard monitoring, concealed-pasteboard detection, paste-target remember/restore, AX tree walk, synthetic Cmd+V, Accessibility trust checks); replaces legacy `objc`/`cocoa`.
- **Unified clipboard writes** (`clipboard_write.rs`) — copy, activate, paste, and voice flows share one path with explicit **Copy** and **Paste** modes.
- **Image capture** — PNG, JPG/JPEG, and animated GIF from the pasteboard or Finder (up to ~20 MB); paste back animated GIFs; format badges and filter chips (`png`, `gif`, `jpg`); dimensions and file size on cards (`image_format`, `image_width`, `image_height`, `image_byte_size`).
- **Overlay filters** — two-row layout when AI tagging is on (Content Kind segment + tag chip bar); format-only row when tagging is off; dynamic panel height (420 / 440 / 480 px base; +28 px when keyboard hints are on); settings sync on reveal; header close button and outside-click dismiss; search, content kind, and tag filters run in the database with DB-wide chip counts for the active scope.
- **Overlay keyboard hints** — footer shortcut strip (`Click copy` · `↵ paste` · `Double-click paste` · `← → browse` · `Esc clear search / dismiss`); optional via **Settings → Clipboard Panel → Keyboard shortcuts** (default on).
- **Overlay infinite scroll** — horizontal scroll loads history in pages (50 entries); deleting or filtering away the visible page fetches the next matches; **Try again** when a page fails to load.
- **Overlay scroll-snap** — horizontal snap so trackpad scroll stops on whole cards; after scroll, selection follows the leading visible card; keyboard `←/→` re-anchors only when selection is off-screen or unset.
- **Overlay search** — `⌘F` and `/` focus search; two-step Escape (clear query, then dismiss); Unicode case-insensitive search (Cyrillic and Latin); denser `--surface-search` field for vibrancy readability; clear button 28×28 pt hit target (HIG) with a 20×20 pt visual circle, without growing the search row.
- **Settings — AI tagging toggle** — off by default; `is_tagging_ready` IPC; merged Setup + Ollama model section; model presets with memory fit and install status; tag backfill when enabled.
- **Settings — voice transcription toggle** — off by default; Whisper fields in a disabled fieldset when off.
- **Settings — Privacy excluded apps** — list with native picker, add by name, contextual candidate row (**Active app** / **Recent app**), overlay **Exclude [App]** action; bundle IDs as stable keys with legacy display-name migration.
- **Settings — clear history** — `get_history_counts` and `clear_all_history` IPC; menu for unpinned or all with confirm dialog; live count sync.
- **Design system** — `tokens.css` (spacing, surfaces, typography, motion, selection); shared `form-controls.css`, `button-interaction.css`, and `.inset-list` grouped rows; section icons; input modality tracking for pointer vs keyboard focus rings.
- **Accessibility** — `:focus-visible` rings, reduced-motion and reduced-transparency support, voice HUD live region baseline, primary **Paste** on cards with keyboard selection and roving `tabindex`; card selection chrome separated from keyboard focus ring via `data-input-modality`; overlay History / Starred `tablist` / `tab` / `aria-selected`; custom collections `role="group"` / `aria-pressed`.
- **Release and dev toolchain** — Oxlint, Oxfmt, Lefthook, `make fix` / `make lint` / `make check`; Vite 8 (Rolldown); optional `sccache` for Rust dev builds.

### Changed

- **Paste pipeline** — Enter and double-click paste the keyboard-selected entry; panel closes before native paste so focus returns to the target app; Messages and similar apps use session-tap Cmd+V; voice transcription uses the same automated paste path.
- **Overlay** — Raycast-style open/close motion; `←/→` browse cards (arrows in search change selection, not cursor); empty states per filter type; SF Pro / SF Mono preview typography; filter chips visually distinct from card metadata tags; stale tag or Content Kind filters auto-clear when the grid is empty but history still has entries (`reconcileOverlayFilters`); History / Starred as a macOS segmented control (short labels); custom collections as scrollable pills; unified 32 pt header row (search, tabs, exclude, settings, close); **Exclude [App]** de-emphasized secondary chrome.
- **Clipboard cards** — fixed-height preview and footer layout; Paste / Retag / Pin / Delete on hover or keyboard focus (not bare selection); pin star always visible with stronger warning card border and resting warning chrome on the star; distinct warning hover for pinned vs unpinned star; Delete hover/active danger chrome; uniform action icons; success feedback only after a confirmed copy; pointer toolbar actions blur the card so focus does not stick.
- **Clipboard monitor** — captures only on real pasteboard changes; ignores Copyosity's own writes and when Copyosity is frontmost; snapshots hash on history clear so stale clipboard is not re-inserted.
- **Settings** — native title bar; compact macOS-style forms on an 8 pt grid with subsections; Save-only footer; accessibility check with one prompt per visit and **Recheck**; Ollama actions on background threads with visible busy states; app display names from installed bundle metadata (for example **Visual Studio Code** instead of plist **Code**).
- **Voice overlay** — non-activating panel (no focus steal); logarithmic mic meter; compact 96×44 HUD layout.
- **Tray** — click opens menu only; use **Open Copyosity** or `Cmd+Shift+V` for the clipboard panel.

### Fixed

- Paste into Cursor, Messages, and other Electron/native targets (no double paste, compose field focus, background-thread timing).
- Finder image capture (file-URL pasteboard type, no filename-only text cards, re-copy after clear/delete).
- History clear and last-card delete no longer loop stale clipboard content back into history.
- Image backfill for legacy rows (full-size data, format, dimensions, `jpg` normalization).
- Settings partial updates no longer wipe Whisper or voice settings; accessibility hint persists until granted; Ollama unload and tagging test reliability; Save button layout jump.
- Overlay dismissal on Space switch; invisible cards on panel open; selection/focus desync during search; card scroll clipping at grid edges; mouse pin no longer leaves the action toolbar stuck open after selection; pinned star hover no longer matches the inactive star hover style.
- Excluded-app add-by-name errors and bundle-ID resolution; clear-history menu, counts, notices, and confirm-dialog keyboard leak.
- Settings Storage/Privacy spacing and clear-notice phantom gap.

### Dependencies

- **Frontend** — Vite 8, `@sveltejs/kit` 2.65, `@sveltejs/vite-plugin-svelte` 7, `svelte` 5.56, `svelte-check` 4.6.
- **Tauri** — `@tauri-apps/api` / `tauri` 2.11; `tauri-plugin-opener` 2.5, `global-shortcut` 2.3, `sql` 2.4.
- **macOS** — `objc2`, `objc2-app-kit`, `objc2-foundation`.

### Security

- Tauri 2.11 upstream IPC ACL hardening.

## [0.3.0] - 2026-04-10

### Added

- **Explicit Tauri capabilities for settings window** — `src-tauri/capabilities/settings.json`; scoped permissions for settings-only IPC (including window opener).
- **Ollama model name validation** — `ollama::validate_model_name` rejects invalid names before `ollama pull`.
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

## [0.2.1]

### Fixed

- Paste reliability, copy button behavior, scroll position reset, and ghost windows after hide.

## [0.2.0]

### Added

- NSPanel-based main window (no focus stealing).
- Dedicated Settings window.
- Security hardening and updated app icons.
