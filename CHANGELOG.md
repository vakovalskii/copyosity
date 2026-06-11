# Changelog

All notable changes to Copyosity are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-06-12

### Added

- **macOS Intel (x86_64) builds** — separate DMG artifacts for Apple Silicon and Intel Macs (for example `Copyosity_0.4.0_x86_64.dmg`); `Makefile` targets and `scripts/build-macos.sh` / `release-macos.sh` for arch-specific release builds.
- **macOS clipboard stack** — `clipboard_macos/` (pasteboard `changeCount`, concealed-pasteboard detection, paste-target remember/restore, AX tree walk, synthetic Cmd+V, Accessibility trust checks) on **objc2** (replaces legacy `objc`/`cocoa`).
- **Unified clipboard writes** (`clipboard_write.rs`) — copy, activate, paste, and voice flows share one code path with explicit **Copy** and **Paste** modes.
- **Image capture from Finder** — PNG, JPG/JPEG, and GIF files copied in Finder are stored in history (up to ~20 MB); the app ignores its own pasteboard writes.
- **Animated GIF support** — capture animated GIFs from the pasteboard or Finder (including Telegram and browsers), show thumbnails in history, and paste back as animated GIF instead of a static raster (file-URL pasteboard path with automatic temp-file cleanup).
- **Image format metadata** — `image_format` DB column; history cards show the format next to the Image badge (`Image PNG`, `Image GIF`, `Image JPG`); automatic format tags (`png`, `gif`, `jpg`) in the overlay filter bar when matching entries exist; shared detection in `image_format.rs`.
- **Overlay two-row filters** — Row A Content Kind segment (`All` / `Text` / `Images`) when AI tagging is on; Row B tag chip bar with format chips (muted + photo icon), semantic AI chips, scroll fade, and divider; shared logic in `overlay-filters.ts`, `ContentKindSegment.svelte`, and `TagFilterBar.svelte`.
- **AI tagging off — collapsed overlay filters** — Row A hidden; Row B shows format chips only when image entries exist; card footers hide all tags (including stale DB tags); filter zone shrinks to format-only chips or hides when nothing applies.
- **Image dimensions and file size** — `image_width`, `image_height`, and `image_byte_size` DB columns with capture-time persistence, startup backfill, and card meta line (`1920 × 1080 · 245 KB`) via `image-meta.ts`.
- **Dynamic overlay height** — `resize_main_window` IPC plus `overlay-layout.ts` / `overlay-resize.ts`; compact (420px) / medium (440px) / full (480px) tiers from filter layout; animated bottom-anchored resize while open (instant when Reduce Motion is on).
- **Overlay settings sync on reveal** — main window reads `get_app_settings` each time the panel opens so AI tagging on/off matches Settings before filters render.
- **Overlay filter plan** — `docs/plans/03-overlay-content-and-tag-filters.md` (content-kind + tag chip UX, progressive disclosure, height tiers).
- **Per-window Tauri capabilities** — separate permission sets for `main`, `settings`, and `voice_overlay` instead of a single default capability.
- **Ollama model name validation** before `ollama pull`.
- **Release CI checks** — `cargo audit`, `npm run check`, and `cargo test` on tagged releases.
- **README** — Apple Silicon vs Intel install table and dual-architecture DMG guidance.
- **Voice transcription toggle** — Settings switch (off by default) to enable or disable hold-to-record transcription and its global shortcut without clearing Whisper configuration.
- **Shared button interaction** (`app-btn`, `button-interaction.css`) — macOS-like press, focus, disabled, and busy states for buttons across Settings, the main window, clipboard cards, and collection tabs; overlay spinner on busy buttons without layout shift; `prepareBusyUi()` yields before blocking IPC so spinners paint reliably.
- **Shared form controls** (`form-controls.css`) — compact macOS-style inputs, selects, section layout, and form buttons reused in Settings.
- **Settings section icons** (`SectionIcon.svelte`) — semantic line icons to the left of each Settings block title: Permissions (shield), AI Tagging (tag), Setup (checklist), Ollama Model (package), This Mac (chip), Storage (database), Privacy (lock), Voice Transcription (microphone).
- **AI tagging toggle** — Settings switch (off by default) to enable or disable automatic Ollama tagging; when off, the clipboard monitor skips tag requests and startup backfill does not run.
- **`is_tagging_ready` IPC** — main window queries whether retag is available (tagging on + Ollama CLI, server, and model installed; unloaded model still counts).
- **Shared status-list layout** — compact checklist styling in `form-controls.css` with spacing tokens from `tokens.css` for consistent Settings rhythm.
- **Design tokens** (`tokens.css`) — single source for spacing, surfaces, borders, semantic colors, shadows, and focus rings; imported globally from `+layout.svelte`.
- **`form-btn-restrict`** — warning-styled button for privacy/restrict actions (panel exclude button; pairs with `form-link-restrict` in Settings).
- **Excluded apps (Privacy)** — list-first layout with Choose Application via native picker, Add by name, contextual Add row using remembered paste-target app (shows **Active app** or **Recent app**), inline section notices, and overlay header **Exclude [App]** action.
- **macOS paste pipeline doc** — `docs/architecture/macos-paste-pipeline.md` (mermaid flow, file map, Messages/session-tap decisions, `KEYBOARD_PASTE_BUNDLE_IDS`, `COPYOSITY_DEBUG_PASTE`); linked from README Development and `AGENTS.md`.
- **HIG audit** — `docs/plans/02-hig-audit.md` with prioritized accessibility, native-feel, and discoverability roadmap across overlay, settings, voice HUD, and shared tokens.
- **Overlay search keyboard shortcuts** — `⌘F` and `/` focus the search field (capture-phase listener, before WebView Find); `overlayEscapeAction` in `overlay-search.ts` for two-step Escape (clear query, then dismiss panel).
- **Unicode case-insensitive clipboard search** — `text_content_search` DB column stores lowercase text; legacy rows backfill on startup; queries match Cyrillic and Latin regardless of case.
- **Text selection tokens** (`--selection-bg`, `--selection-text`) — accent wash for search input and shared form controls.
- **Motion system** (`motion.ts`, motion tokens in `tokens.css`) — shared panel/HUD durations and Apple-style easings; `prefers-reduced-motion` token overrides; helpers `panelOpenMs`, `panelCloseFallbackMs`, `scrollBehavior`, and `subscribeReducedMotion`.
- **Unit tests** — **108 tests** in `copyosity_lib` for 0.4.0, with emphasis on clipboard monitor dedup/hash-poisoning, image format and animated GIF paste paths, image meta backfill, DB migration and tag backfill, case-insensitive/Cyrillic search and `text_content_search` backfill, settings partial updates (Whisper, voice transcription, AI tagging toggles), `tagging_ready` / `is_ai_tagging_enabled`, Ollama model validation plus `/api/ps` load-unload matching, `open_accessibility_settings` IPC, `macos_app` bundle ID resolution, app-exclusion candidate resolution, and macOS paste helpers (`bundle_prefers_keyboard_paste`, `cmd_v_uses_session_tap`, AX editable-role priority).

### Changed

- **Clipboard monitor** — reads the pasteboard only when content actually changes; identical payloads are not re-captured or re-emitted to the UI.
- **Paste pipeline** — Enter in the main window activates an entry the same way as double-click (text and images); panel plays a close animation before native hide; automated paste is deferred until hide completes (`PENDING_PASTE_AFTER_HIDE`) so focus returns to the target app first; voice transcription uses the same automated paste path; `try_ax_paste_for_pid` centralizes per-app paste strategy; native apps that ignore `CGEventPostToPid` (Messages) receive session-tap Cmd+V when frontmost; AX focus search ranks text fields above scroll areas.
- **Overlay keyboard selection** — opening the panel or changing search, collection, or tag filters selects the first (newest) entry; `Cmd+Shift+V` then `Enter` pastes the latest item without an extra arrow key; mouse hover highlighting stays separate from keyboard selection.
- **Overlay search field** — in Tab order (no `tabindex="-1"`); clear button; `:focus-within` ring aligned with Settings; `role="search"` and `aria-label`; `focus()`, `blur()`, and `isFocused()` exported for panel shortcuts.
- **Overlay arrow keys** — `←/→` always navigate cards, including when the search field is focused (Spotlight-style; left/right do not move the text cursor).
- **Overlay empty states** — contextual messages for search, tag, content-kind, and format filters (including combined filters) with secondary hints and `role="status"`.
- **Overlay panel motion** — Raycast-style asymmetric open/close via motion tokens; slide with `translate3d` (no scale); native hide waits for frontend close animation (`window-hide-request` → `hide_main_window` after `transitionend` or fallback timeout).
- **Overlay reveal layout** — before the open animation, the panel reloads entries, syncs AI tagging from Settings, and sizes the native window to the correct height tier.
- **Overlay tag filter bar** — two-row HIG layout replaces the single-row top-8 tag list; progressive disclosure (content-kind segments only when AI tagging is on and history mixes text and images; tag bar stays visible on empty filter results); 12px chips; format vs semantic styling; unified accent active state; vertical divider between chip groups; content-kind-aware chip sets.
- **Overlay card preview typography** — SF Pro for plain text previews; SF Mono only for code-like entries (`textKind`).
- **Image history cards** — footer format tags removed; redundant «Image preview» label replaced with dimensions and file size; native `title` tooltip removed from cards.
- **HIG audit (overlay)** — tag bar scroll affordances, preview typography, image meta labels, and card tooltip removal marked complete in `docs/plans/02-hig-audit.md`.
- **Reduce Motion** — panel transitions, card hover lift, copied-feedback scale, voice HUD mic pulse and EQ wobble, status-dot pulse, button spinner, and Settings toggle slider respect `prefers-reduced-motion`.
- **Clipboard card actions** — Copy / Retag / Pin / Delete visible when the card is keyboard-selected or `:focus-within`, not only on hover; `aria-label` replaces `title` tooltips on action buttons.
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
- **Settings section headers** — all block and subsection titles use a shared icon + label layout (`form-section-title--with-icon`, `form-title-icon`) aligned with the existing This Mac chip icon style.
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
- **Privacy excluded apps** — list-first Apple-style layout; feedback stays inline near actions (footer notice is Save-only); remembered app name used when Settings or the panel is frontmost; Remove uses accent link style, Add uses warning restrict style.
- **App exclusion identity** — excluded apps, clipboard skip checks, and picker/frontmost detection now use macOS bundle IDs (`com.apple.Safari`) with display names in the UI; legacy display-name rows migrate on startup; `lsappinfo` shell lookup removed.
- **Settings clear-history feedback** — confirmation sits in the Storage action row (button left, status right) instead of the Save footer; neutral `--color-text-tertiary` for destructive completion, success green reserved for Save; `form-note-success` / `form-note-neutral` variants; status copy stays on one line (`white-space: nowrap`).
- **Accessibility enable hint** — pending “enable in the list” copy uses macOS `systemOrange` (`--color-warning-text`); verified state stays success green.
- **Settings model-dirty hint** — “Model changed — save settings first, then test.” uses warning orange instead of error red.
- **Voice overlay layout** — mic icon and level meter share a centered row with tighter side padding, 12pt gap between icon and meter, and larger 22/24px proportions in a compact 96×44 HUD window.
- **Voice overlay mic color** — live recording indicator uses `--color-recording` so it reads as active capture, not an error state.
- **Clipboard card layout and spacing** — fixed 168px preview slot with a uniform 8-line text clamp (preview height no longer shrinks when tags are present); footer shows tags above the source-app and character-count row, with metadata pinned to the card bottom; card height 288px (+8px) with 8px breathing room between preview and footer, 8px between tags and metadata, and slightly roomier tag chips (`4×8` padding).

### Fixed

- **Paste into Cursor and other Electron apps** — auto-paste runs on a background thread so the panel can hide and restore focus first; tries Accessibility paste, then synthetic Cmd+V via the session event tap when the target app is frontmost.
- **Paste into Messages** — text and images paste into the compose field; `com.apple.MobileSMS` / `com.apple.iChat` skip unreliable `AXPaste` and use keyboard simulation; AX tree walk no longer targets the conversation scroll area ahead of the compose field.
- **Double paste on activate** — synthetic Cmd+V posts to the session event tap only; posting to session and HID taps together inserted duplicate text/images in Cursor, Messages, and other targets.
- **Tray menu on first launch** — the hidden main panel no longer competes with the status-bar menu popup on the first click.
- **Image history backfill** — re-copying the same image updates legacy rows missing full-size `image_data`, `image_format`, or image dimensions/size; existing `jpeg` format labels and tags are normalized to `jpg`; startup `backfill_missing_image_meta` fills width/height/byte size for older rows.
- **Settings partial updates** — changing one field no longer wipes Whisper/voice/microphone settings.
- **Clipboard card action icons** — copy, retag, pin, and delete use uniform 16×16 SVG icons instead of mismatched Unicode glyphs; pinned star is filled and highlighted.
- **Clipboard card text preview** — long text no longer bleeds into the inner border or bottom padding; preview uses a fixed-height slot with grid clipping and a CSS ellipsis (`line-clamp`: 8 lines).
- **Clipboard card footer** — tags sit on a separate row above the source app / character-count line so tag chips no longer wrap into the count label.
- **Settings Save layout jump** — saving no longer resizes the button or shifts the “Saved” label when loading state appears.
- **Settings Unload** — works on the first click with a visible busy spinner; uses the documented Ollama unload request (`prompt: ""`, `keep_alive: 0`), verifies unload via `/api/ps` (with `ollama stop` fallback), and clears the tagging test result only after the model is confirmed unloaded.
- **Settings accessibility hint** — “Enable Copyosity in the list.” no longer disappears when the Settings window regains focus; it stays until Accessibility is actually granted (then switches to the verified success message); the verified message clears when access is revoked.
- **Settings tagging test** — repeatable after success; busy spinner and “Testing…” state render on every run; successful test display requires the model to be loaded in memory.
- **Settings Ollama status dot** — “Model unloaded” uses a static yellow indicator instead of the pulsing animation reserved for in-progress checks.
- **Settings status hints** — symmetric vertical padding within checklist rows; hint copy split across lines where it improves readability (model unloaded, tagging test).
- **Tag filter bar** — format tags (`jpg`, `gif`, `png`) always appear when matching entries exist (previously only the top 8 tags by count were shown); filtering matches `image_format` for legacy rows; superseded by the two-row Content Kind + tag chip bar in this release.
- **Clipboard self-capture** — clipboard monitor skips capture when Copyosity is frontmost, even when the source bundle ID is unavailable in the pasteboard read path.
- **Tag-filter empty state** — filtering by tag or format without a search query shows filter-specific copy instead of a misleading search message.
- **Paste focus race on activate** — Enter and double-click no longer hide the panel from the frontend before paste; automated paste runs only after the close animation and native hide complete.
- **Invisible cards on panel open** — removed per-card stagger enter animation that could leave cards at `opacity: 0` in WebKit; panel slide now carries open/close motion.

### Dependencies

- **Frontend** — `@sveltejs/kit` 2.63, `svelte` 5.56.2, `svelte-check` 4.6.
- **Tauri** — synced npm (`@tauri-apps/api` 2.11, `@tauri-apps/cli` 2.11) and Rust (`tauri` 2.11); `tauri-plugin-opener` 2.5.4, `global-shortcut` 2.3.2, `sql` 2.4.0.
- **macOS** — `objc2`, `objc2-app-kit`, `objc2-foundation` (`NSData` for GIF pasteboard writes).

### Security

- Sensitive IPC commands scoped per window (`settings` cannot call paste commands; `voice_overlay` cannot call `clear_history` or `start_ollama_server`; main panel may only read/add the current excludable-app candidate, not open the picker or edit the full exclusion list; main panel may call `get_app_settings` and `resize_main_window` for overlay layout only).
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
