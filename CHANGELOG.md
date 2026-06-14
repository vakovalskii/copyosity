# Changelog

All notable changes to Copyosity are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - Unreleased

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
- **Release CI checks** — `cargo audit`, `npm run check`, Oxlint, Oxfmt (`--check`), `cargo clippy --all-targets -D warnings`, `cargo fmt --check`, and `cargo test` on tagged releases.
- **Lint and format toolchain** — Oxlint + Oxfmt (`.oxlintrc.json`, `.oxfmtrc.json`) for JS/TS, Svelte `<script>` blocks, and CSS; `cargo clippy` and `rustfmt` for Rust (`src-tauri/clippy.toml`, `rustfmt.toml`, `[lints.clippy]` in `Cargo.toml`); Husky + lint-staged with `--fix` on pre-commit; `make fix`, `make lint`, and expanded `make check`; `.editorconfig` and Oxc VS Code extension (`oxc.oxc-vscode`) for format-on-save; `svelte-check` kept for Svelte/TypeScript types.
- **README** — Apple Silicon vs Intel install table and dual-architecture DMG guidance.
- **Voice transcription toggle** — Settings switch (off by default) to enable or disable hold-to-record transcription and its global shortcut without clearing Whisper configuration.
- **Shared button interaction** (`app-btn`, `button-interaction.css`) — macOS-like press, focus, disabled, and busy states for buttons across Settings, the main window, clipboard cards, and collection tabs; overlay spinner on busy buttons without layout shift; `prepareBusyUi()` yields before blocking IPC so spinners paint reliably.
- **Shared form controls** (`form-controls.css`) — compact macOS-style inputs, selects, section layout, and form buttons reused in Settings.
- **Input modality tracking** (`input-modality.ts`) — `data-input-modality` on `<html>` distinguishes pointer vs keyboard focus so form controls show a tight ring on click and the 3px keyboard halo only after Tab or other navigation keys (WebKit/Tauri `:focus-visible` quirk).
- **Settings section icons** (`SectionIcon.svelte`) — semantic line icons to the left of each Settings block title: Permissions (shield), AI Tagging (tag), Setup (checklist), Ollama Model (package), This Mac (chip), Storage (database), Privacy (lock), Voice Transcription (microphone).
- **AI tagging toggle** — Settings switch (off by default) to enable or disable automatic Ollama tagging; when off, the clipboard monitor skips tag requests and startup backfill does not run.
- **`is_tagging_ready` IPC** — main window queries whether retag is available (tagging on + Ollama CLI, server, and model installed; unloaded model still counts).
- **Shared status-list layout** — compact checklist styling in `form-controls.css` with spacing tokens from `tokens.css` for consistent Settings rhythm.
- **Design tokens** (`tokens.css`) — single source for spacing, surfaces, borders, semantic colors, shadows, focus rings, rem-based typography (`--font-size-*`), concentric corner radii (`--radius-inset` / `--radius-surface` / `--radius-panel`), and card chrome (`--card-width`, `--card-height`); imported globally from `+layout.svelte`.
- **Entry tag tokens** — `--color-entry-tag`, `--surface-entry-tag`, and `--border-entry-tag` for neutral card metadata badges (separate from toolbar filter chips).
- **`form-btn-restrict`** — warning-styled button for privacy/restrict actions (panel exclude button; pairs with `form-link-restrict` in Settings).
- **Excluded apps (Privacy)** — list-first layout with Choose Application via native picker, Add by name, contextual Add row using remembered paste-target app (shows **Active app** or **Recent app**), inline section notices, and overlay header **Exclude [App]** action.
- **Settings clear history** — `get_history_counts` and `clear_all_history` IPC; `ActionMenu` dropdown (unpinned / all…) with ↑/↓ wrap (skip disabled), Home/End, type-ahead, open on ↓/Enter/Space, Escape to trigger, Tab loop inside open menu (close via Escape or focus leaving the control); `ConfirmDialog` + `confirmDestructive` queued service, modal Tab trap (Cancel ↔ action), dialog shell focus then Tab to first button, scroll-key trap so arrows do not scroll Settings behind the modal, return focus to the menu trigger; `destructive-actions.ts` HIG body copy (counts, non-breaking count lines, neutral confirm styling); `history-changed` after delete, pin, clear, retention cleanup on save, and startup retention purge when rows are removed.
- **Shared chevron** — `ChevronDown.svelte` for menus; form `<select>` uses the same SVG chevron token (`--icon-chevron-down`).
- **macOS paste pipeline doc** — `docs/architecture/macos-paste-pipeline.md` (mermaid flow, file map, Messages/session-tap decisions, `KEYBOARD_PASTE_BUNDLE_IDS`, `COPYOSITY_DEBUG_PASTE`); linked from README Development and `AGENTS.md`.
- **HIG audit** — `docs/plans/02-hig-audit.md` with prioritized accessibility, native-feel, and discoverability roadmap across overlay, settings, voice HUD, and shared tokens.
- **Voice HUD accessibility plan** — `docs/plans/04-voice-hud-accessibility-full-cycle.md` (full screen-reader lifecycle: recording → processing → terminal states; baseline live region shipped in 0.4.0).
- **Overlay search keyboard shortcuts** — `⌘F` and `/` focus the search field (capture-phase listener, before WebView Find); `overlayEscapeAction` in `overlay-search.ts` for two-step Escape (clear query, then dismiss panel).
- **Overlay close controls** — explicit header close button plus macOS outside-click dismiss guard (`overlay_dismiss.rs`) using a global mouse monitor with a focus-loss fallback.
- **Unicode case-insensitive clipboard search** — `text_content_search` DB column stores lowercase text; legacy rows backfill on startup; queries match Cyrillic and Latin regardless of case.
- **Text selection tokens** (`--selection-bg`, `--selection-text`) — accent wash for search input and shared form controls.
- **Motion system** (`motion.ts`, motion tokens in `tokens.css`) — shared panel/HUD durations and Apple-style easings; `prefers-reduced-motion` token overrides; helpers `panelOpenMs`, `panelCloseFallbackMs`, `scrollBehavior`, and `subscribeReducedMotion`.
- **Unit tests** — **124 tests** in `copyosity_lib` for 0.4.0, with emphasis on clipboard monitor dedup/hash-poisoning, history-clear snapshot vs re-copy, `clear_all_history` / `get_history_counts`, and Finder file-path capture (`public.file-url` and legacy filenames), image format and animated GIF paste paths, image meta backfill, DB migration and tag backfill, case-insensitive/Cyrillic search and `text_content_search` backfill, settings partial updates (Whisper, voice transcription, AI tagging toggles), `tagging_ready` / `is_ai_tagging_enabled`, Ollama model validation plus `/api/ps` load-unload matching, `open_accessibility_settings` IPC, `macos_app` bundle ID resolution and case-insensitive display-name lookup, app-exclusion candidate resolution, and macOS paste helpers (`bundle_prefers_keyboard_paste`, `cmd_v_uses_session_tap`, AX editable-role priority).

### Changed

- **Clipboard monitor** — reads the pasteboard only when content actually changes; identical payloads are not re-captured or re-emitted to the UI; `notify_history_cleared` on `clear_history` or deleting the last unpinned entry snapshots the current pasteboard hash (no instant re-insert of stale clipboard content) while still allowing re-capture after a new copy; dedup uses the stored entry hash, not probe-only; Finder file paths prefer native `public.file-url` (`NSPasteboardItem` / `NSURL`) with legacy `NSFilenamesPboardType` and arboard fallbacks.
- **Paste pipeline** — Enter pastes the keyboard-selected entry (from the search field or the panel, same as double-click for text and images); panel plays a close animation before native hide; automated paste is deferred until hide completes (`PENDING_PASTE_AFTER_HIDE`) so focus returns to the target app first; voice transcription uses the same automated paste path; `try_ax_paste_for_pid` centralizes per-app paste strategy; native apps that ignore `CGEventPostToPid` (Messages) receive session-tap Cmd+V when frontmost; AX focus search ranks text fields above scroll areas; `finalize_panel_hide` shared by `hide_main_window` and opening Settings while the overlay is visible.
- **Overlay keyboard selection** — opening the panel or changing search, collection, or tag filters selects the first (newest) entry; `Cmd+Shift+V` then `Enter` pastes the latest item without an extra arrow key; mouse hover highlighting stays separate from keyboard selection.
- **Overlay search field** — in Tab order (no `tabindex="-1"`); clear button; `:focus-within` ring aligned with Settings; `role="search"` and `aria-label`; `focus()`, `blur()`, and `isFocused()` exported for panel shortcuts; search fetches use a generation token so overlapping reloads cannot show stale results.
- **Overlay search catalog** — `catalogEntries` keeps the filter bar and panel height stable while search narrows the card list (`layoutEntries` in `buildTagBarModel`).
- **Overlay arrow keys** — `←/→` browse cards everywhere except other text inputs (for example collection name); in the search field, arrows change the selected result instead of moving the text cursor; focus stays in search while results scroll into view.
- **Overlay Enter** — pastes the keyboard-selected entry when one exists; works from the search field (first matching result after each query) and from the panel when no text field is focused; blocked in other inputs (for example collection name).
- **Overlay search while typing** — debounced reload updates selection and scrolls the first match into view without stealing focus from the search field.
- **Overlay empty states** — contextual messages for search, tag, content-kind, and format filters (including combined filters) with secondary hints and `role="status"`.
- **Overlay panel motion** — Raycast-style asymmetric open/close via motion tokens; slide with `translate3d` (no scale); native hide waits for frontend close animation (`window-hide-request` → `hide_main_window` after `transitionend` or fallback timeout).
- **Overlay reveal layout** — before the open animation, the panel reloads entries, syncs AI tagging from Settings, and sizes the native window to the correct height tier.
- **Overlay tag filter bar** — two-row HIG layout replaces the single-row top-8 tag list; progressive disclosure (content-kind segments only when AI tagging is on and history mixes text and images; tag bar stays visible on empty filter results); toolbar `.filter-chip` controls (renamed from `.tag-chip`); format vs semantic styling; unified accent active state; vertical divider between chip groups; content-kind-aware chip sets.
- **Overlay card preview typography** — SF Pro for plain text previews; SF Mono only for code-like entries (`textKind`).
- **Image history cards** — footer format tags removed; redundant «Image preview» label replaced with dimensions and file size; native `title` tooltip removed from cards.
- **HIG accessibility pass** — removed global `outline: none`; `:focus-visible` rings on clipboard cards and collection tabs; brighter `--color-text-subtle` / `--color-text-faint` plus `prefers-contrast: more`; `prefers-reduced-transparency` opaque surface fallbacks and disabled panel blur; text-control focus tokens (rest → hover → pointer focus → keyboard focus); Dynamic Type partial via rem typography tokens and `-apple-system-body` on `body`.
- **Tag filter chips vs card metadata** — toolbar `.filter-chip` (interactive pill with `aria-pressed`) visually separated from card `.entry-tag` (neutral micro-badge); removes false affordance when AI tags appear in both places.
- **Clipboard card selection** — distinct selected state (accent fill, `--shadow-card-selected`); roving `tabindex` (only the selected card is in Tab order); keyboard focus follows `selectedIndex` when not searching; mouse click and card action buttons (including Paste) sync selection.
- **Content kind segment** — `role="group"` with `aria-pressed` toggle buttons (Tab visits every segment; `←/→` stay reserved for card navigation).
- **Collection tabs** — custom collections use native `<button>` elements (Space and Enter); delete control is a sibling button with `aria-label`, not nested inside the tab.
- **Voice HUD baseline a11y** — `role="status"` + `aria-live="polite"` with sr-only «Recording voice»; decorative meter content stays `aria-hidden` (full SR lifecycle deferred to plan 04).
- **Settings custom Ollama model** — associated `<label>` when the Custom preset is selected.
- **HIG audit (overlay + shared)** — filter chip vs metadata badge (п. 20), selection vs hover (п. 13), title/`aria-label` dedup (п. 25), toggle styles in `form-controls.css` (п. 27), Dynamic Type partial (п. 34), test button `aria-describedby` (п. 40), add-collection focus ring (п. 41); plan renumbered and synced with `04-voice-hud-accessibility-full-cycle.md`.
- **Reduce Motion** — panel transitions, card hover lift, copied-feedback scale, voice HUD mic pulse and EQ wobble, status-dot pulse, button spinner, and Settings toggle slider respect `prefers-reduced-motion`.
- **Reduce Transparency** — overlay panel, voice HUD, and copied overlay disable `backdrop-filter` when `prefers-reduced-transparency` is on; opaque surface tokens in `tokens.css`.
- **Clipboard card actions** — Paste / Retag / Pin / Delete visible when the card is keyboard-selected or `:focus-within`, not only on hover; primary **Paste** button (`activateEntry`, accent styling, `aria-busy` while activating) replaces the redundant Copy control; `aria-label` on action buttons; single-click copy announces «Copied to clipboard» via a screen-reader live region only after a successful copy; card `role="button"` activates paste on Enter and Space.
- **README** — Smart Actions and usage table document the Paste button instead of the old Copy control.
- **Accessibility in Settings** — silent checks vs macOS trust dialog are separated; one prompt per Settings visit; live AX probe; **Recheck** confirms when access is still valid; guidance after rebuild or reinstall; `open_accessibility_settings` IPC from Settings.
- **Settings window** — native title bar (draggable again) with a custom header drag region.
- **Voice overlay** — pre-created NSPanel with non-activating behavior so showing the overlay no longer steals focus from the target app; audio level meter uses a logarithmic dB scale for quiet laptop mics.
- **Tray click** — opens the menu only; use **Open Copyosity** or `Cmd+Shift+V` to show the clipboard panel.
- **Makefile** — portable `APP_DIR` (`CURDIR`); `make fix` auto-fixes frontend and backend issues; `make lint` verifies lint/format rules without changes; `make check` runs svelte-check, Oxlint, Oxfmt, Clippy, rustfmt, and `cargo test`.
- **Settings unload feedback** — Ollama step 3 shows three distinct states: model ready (green), model on disk but unloaded (static yellow), model not installed (red); `model_loaded` comes from Ollama `/api/ps`.
- **Settings Ollama actions** — Unload, Test, Start, and Download run on a background thread (`spawn_blocking`) so the WebView stays responsive; busy buttons use `is-busy` (not `disabled`) so WebKit keeps spinner animations running.
- **Voice transcription Settings UI** — compact on/off toggle; Whisper fields sit in a disabled fieldset when off; active toggle uses muted `--color-success-control` (no glow).
- **Settings Save button** — muted blue aligned with other Settings accents; stable width during save (overlay spinner, reserved “Saved” slot); macOS-style press/focus feedback without hover lift.
- **Settings form controls** — compact macOS-style inputs (32px, 13px type), consistent section spacing, shared `form-controls.css` reused across form blocks (including toggle switches moved from `settings/+page.svelte`); clear-history action moved into Storage section, footer reserved for Save only.
- **Settings section headers** — all block and subsection titles use a shared icon + label layout (`form-section-title--with-icon`, `form-title-icon`) aligned with the existing This Mac chip icon style.
- **Button hover/press** — removed `translateY` lift on hover; press uses inset darken/brightness instead of scale; async Settings actions expose `aria-busy` while loading.
- **Settings AI section** — merged “Local AI Status” and “AI Model” into a single **AI Tagging** block with on/off toggle; Setup checklist uses `status-list` with symmetric step padding; Ollama model picker lives in the same section.
- **Settings Ollama model UI** — preset options show memory estimate plus **Fits** / **Too large** and **Installed** in the picker; **This Mac** subsection (inset memory panel with RAM and recommended Ollama budget) is separated from the model field; custom models show a hint that memory use cannot be estimated.
- **Settings Permissions** — accessibility status uses the shared `status-list` pattern.
- **Voice transcription fieldset** — reuses `toggle-section-body` instead of a duplicate disabled-state class.
- **Retag button** — shown on text cards only when AI tagging is ready; hidden when tagging is off or Ollama is not set up.
- **Enabling AI tagging** — triggers tag backfill for existing untagged entries (on Save or toggle); `ensure_runtime` runs only while tagging is enabled.
- **Settings a11y polish** — removed duplicate `title` attributes where `aria-label` already describes toggles and list actions; Test tagging button uses `aria-describedby` when the model is dirty; form hints, status hints, and `<code>` snippets are selectable for copy.
- **Collection add-name input** — `aria-label="Collection name"`; shared keyboard/pointer focus rings from `form-controls.css`.
- **UI typography and radii** — overlay, settings, and shared components use rem font-size tokens instead of scattered px literals; macOS HIG concentric corner radii (panel 20px, card 18px, inset 6px) replace ad-hoc values.
- **UI color system** — main window, Settings, clipboard cards, search bar, voice overlay, and shared form/button styles use CSS variables from `tokens.css` instead of scattered hex/rgba literals.
- **Success palette** — cool sage green scale (`--color-success` for dots and overlays, `--color-success-text` for hints and save feedback, `--color-success-control` for toggles) tuned to dark-theme accent/danger weight.
- **Settings toggles** — accent focus ring when off, `--ring-success-control` when on; no neon fill or outer glow.
- **Privacy excluded apps** — list-first Apple-style layout; feedback stays inline near actions (footer notice is Save-only); remembered app name used when Settings or the panel is frontmost; Remove uses accent link style, Add uses warning restrict style; inset grouped list with empty state, Choose Application as a full-width secondary button below the list, and list row styles centralized in `form-controls.css`; Add/Remove row actions meet 28px hit targets.
- **App exclusion identity** — excluded apps, clipboard skip checks, and picker/frontmost detection now use macOS bundle IDs (`com.apple.Safari`) with display names in the UI; legacy display-name rows migrate on startup; `lsappinfo` shell lookup removed.
- **Settings clear-history feedback** — confirmation sits in the Storage action row (button left, status right) instead of the Save footer; neutral `--color-text-tertiary` for destructive completion, success green reserved for Save; `form-note-success` / `form-note-neutral` variants; status copy stays on one line (`white-space: nowrap`).
- **Settings Storage clear history** — single full-width **Clear history** menu replaces the old unpinned-only danger button; confirm before unpinned or all; menu disabled when history is empty; counts refresh on `clipboard-changed`, `history-changed`, focus, and `settings-shown`; stale “cleared” notice clears when counts change; IPC failures show an inline notice.
- **Settings selection chrome** — `ui-no-select` / `ui-selectable-text` in `form-controls.css` (chrome non-selectable, copyable titles/labels/hints/meta); `.settings-page` uses `ui-no-select` (HIG audit п. 29).
- **Text selection** — global `::selection` uses brand accent tokens (replaces per-component overrides and macOS purple system selection in WebKit).
- **Form hints** — unified hint line-height and vertical padding tokens (`--line-height-hint`, `--padding-hint-y`); `form-action-stack` for menu + inline notice rows.
- **Dropdown surfaces** — opaque `--surface-menu` / hover tokens for `ActionMenu` and confirm dialog (no blur bleed-through).
- **Button keyboard focus ring** — `app-btn` shows accent ring on `:focus` when `data-input-modality="keyboard"` so Tab focus is visible in menus and confirm dialogs (WebKit `:focus-visible` quirk).
- **Accessibility enable hint** — pending “enable in the list” copy uses macOS `systemOrange` (`--color-warning-text`); verified state stays success green.
- **Settings model-dirty hint** — “Model changed — save settings first, then test.” uses warning orange instead of error red.
- **Voice overlay layout** — mic icon and level meter share a centered row with tighter side padding, 12pt gap between icon and meter, and larger 22/24px proportions in a compact 96×44 HUD window.
- **Voice overlay mic color** — live recording indicator uses `--color-recording` so it reads as active capture, not an error state.
- **Clipboard card layout and spacing** — fixed 168px preview slot with a uniform 8-line text clamp (preview height no longer shrinks when tags are present); footer shows AI metadata tags (`.entry-tag`) above the source-app and character-count row, with metadata pinned to the card bottom; card height 288px (+8px) with 8px breathing room between preview and footer.

### Fixed

- **Paste into Cursor and other Electron apps** — auto-paste runs on a background thread so the panel can hide and restore focus first; tries Accessibility paste, then synthetic Cmd+V via the session event tap when the target app is frontmost.
- **Paste into Messages** — text and images paste into the compose field; `com.apple.MobileSMS` / `com.apple.iChat` skip unreliable `AXPaste` and use keyboard simulation; AX tree walk no longer targets the conversation scroll area ahead of the compose field.
- **Double paste on activate** — synthetic Cmd+V posts to the session event tap only; posting to session and HID taps together inserted duplicate text/images in Cursor, Messages, and other targets.
- **Tray menu on first launch** — the hidden main panel no longer competes with the status-bar menu popup on the first click.
- **Image history backfill** — re-copying the same image updates legacy rows missing full-size `image_data`, `image_format`, or image dimensions/size; existing `jpeg` format labels and tags are normalized to `jpg`; startup `backfill_missing_image_meta` fills width/height/byte size for older rows.
- **Settings partial updates** — changing one field no longer wipes Whisper/voice/microphone settings.
- **Clipboard card action icons** — paste, retag, pin, and delete use uniform 16×16 SVG icons instead of mismatched Unicode glyphs; pinned star is filled and highlighted; Paste uses a distinct insert-into-target glyph with accent-primary styling.
- **Clipboard card text preview** — long text no longer bleeds into the inner border or bottom padding; preview uses a fixed-height slot with grid clipping and a CSS ellipsis (`line-clamp`: 8 lines).
- **Clipboard card footer** — tags sit on a separate row above the source app / character-count line so metadata no longer wraps into the count label; toolbar filter chips and card entry tags use distinct visual styles.
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
- **Opening Settings with overlay visible** — always-on-top panel is hidden via `finalize_panel_hide` so Settings receives hover, pointer cursor, and focus rings; deferred paste-after-hide still runs when activating an entry.
- **Overlay dismissal on Space switch** — changing macOS Spaces no longer looks like an outside click/focus loss and accidentally closes the clipboard panel.
- **Finder image capture** — PNG/JPG/JPEG/GIF copied in Finder are stored as image cards (not filename-only text); failed file encode no longer falls through to `get_text()`; probe vs stored-hash mismatch no longer blocks the first copy or requires copying another item first.
- **History clear / delete last card** — clearing history or deleting the last unpinned entry no longer loops re-inserting whatever remains on the system clipboard; re-copying the same file after clear/delete adds it back correctly.
- **Clipboard card timers** — click debounce and copied-feedback timeouts clear on unmount.
- **Invisible cards on panel open** — removed per-card stagger enter animation that could leave cards at `opacity: 0` in WebKit; panel slide now carries open/close motion.
- **False «Copied» feedback** — copy overlay and screen-reader announcement appear only after a successful `copy_entry`; IPC failures announce «Copy failed» instead of showing success.
- **Selection / focus desync** — clicking a card or its Paste / Pin / Delete / Retag actions updates `selectedIndex`; arrow-key navigation keeps focus in the search field while browsing results; search typing no longer jumps focus to the first card on each debounced reload.
- **Enter in search triggering paste** — Enter in the search field pastes the selected match when results exist; other text inputs still do not trigger paste.
- **Card grid horizontal scroll** — arrow-key navigation snaps the selected card fully into the padded viewport instead of clipping at the left or right edge; first and last cards align to grid start/end (`scroll-padding-inline`).
- **Add excluded app by name** — `app_not_found` warnings surface when Tauri nests the IPC error string; add failures name the entered app, active-app candidate, or selected picker app; `add_excluded_app` takes a plain camelCase string argument again (no wrapper struct).
- **Settings history counts** — Storage counts no longer lag behind overlay deletes, pins, or copies while Settings stays open.
- **Clear-history menu** — disabled when history is empty; unpinned-only item disabled when everything is pinned.
- **Clear-history notice** — “All/Unpinned history cleared” no longer persists after history counts change (for example new copies or overlay deletes).
- **Clear-history errors** — failed unpinned/all clear IPC calls show an inline notice instead of failing silently.
- **Confirm dialog keyboard leak** — ↑/↓ and other scroll keys no longer move the Settings page while a confirm dialog is open.

### Dependencies

- **Frontend** — `@sveltejs/kit` 2.63, `svelte` 5.56.2, `svelte-check` 4.6; dev dependency `@types/node` for `tsconfig` `node` types.
- **Tauri** — synced npm (`@tauri-apps/api` 2.11, `@tauri-apps/cli` 2.11) and Rust (`tauri` 2.11); `tauri-plugin-opener` 2.5.4, `global-shortcut` 2.3.2, `sql` 2.4.0.
- **macOS** — `objc2`, `objc2-app-kit` (`NSPasteboard`, `NSPasteboardItem`), `objc2-foundation` (`NSData`, `NSURL` for GIF pasteboard writes and file-URL reads).

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
