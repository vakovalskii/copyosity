# Changelog

All notable changes to Copyosity are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.3] - 2026-07-09

### Fixed

- **App version now shows in Settings → Updates** — added the `core:app` capability so `getVersion()` resolves; the pane no longer shows `Current: …`.

### Changed

- **Switching apps hides the overlay** — Cmd+Tab, clicking another window, or the Dock now dismisses the clipboard overlay (it's a transient picker). The panel still opens over the frontmost app without stealing focus, and Cmd+↑ still hides it.

## [0.6.2] - 2026-07-09

### Added

- **Voice transcripts land in history + clipboard** — every VoiceMod transcription is recorded in clipboard history and written to the pasteboard with retries, so it's never lost even if the paste into the target app fails.
- **Transcribing indicator** — the voice capsule now shows a spinner + "Transcribing…" while the transcript is being produced.
- **Auto-update on launch** — Copyosity checks for updates at startup, installs them in the background, and posts a native notification (applied on next launch).
- **Cmd+↑ hides the overlay** — press Cmd+↑ to dismiss the clipboard board (works whether or not the board has focus).
- **Hub transcription toggle in Voice settings** — "Transcribe with NeuralDeep Hub" now sits next to the voice activation toggle.

### Changed

- Default hub model is now **qwen3.6-35b-a3b** with reasoning disabled (`/no_think`) for snappy voice polishing and tagging.
- Faster board scrolling — pages are prefetched ~2 viewports ahead so fast scrolling no longer stutters at the edge.

### Fixed

- **Microphone entitlement** — release builds are signed with the microphone (and Apple Events) entitlements so voice recording works under the hardened runtime.
- **Hub rate limits (429)** — a clear "raise your tariff" message (honoring `Retry-After`) instead of a bare HTTP code; voice transcription failures now surface as a native notification.

## [0.6.1] - 2026-07-05

### Added

- **Quick menu** — native Clipy-style pop-up at the cursor on a global hotkey (default `⌘⇧C`): recent clipboard history with number keys 1–9, overflow in submenus (up to 100 entries), and snippets grouped by folder; pick an item to paste into the app that was frontmost when the menu opened — two clicks, no overlay.
- **Snippets** — reusable text templates in folders; **Settings → Quick Menu** editor; snippets appear in the quick menu; **Edit Snippets…** from the menu opens that settings pane.
- **Image cards** — **Copy recognised text** action copies Vision OCR text to the clipboard when available.

### Changed

- **Menu bar app** — no Dock icon and no Cmd+Tab entry; Copyosity runs from the menu bar only.
- **Command palette** — Spotlight-style query field with toolbar and search grouped in a header strip; updated session-history, new-session, compact-to-dot, and close icons; status hints and errors use the same secondary-line style as the overlay and settings (larger status text, no dividers under the query, consistent spacing); voice and loading indicators sit in the query row; hints clear when you type and do not persist after close or reopen; keyboard-hints footer stays visible whenever the palette is open without an answer (including during history, loading, and agent progress).
- **Command palette min-dot** — drag anywhere on the dot to move it; double-click or Enter expands; palette restores centered on the screen where you left the dot; expanded size is remembered across minimize and app restarts (user-resized dimensions, not the default).
- **Image cards** — slightly tighter vertical padding on OCR text previews under thumbnails; format badge (PNG / JPG / GIF) in the header with a reworked action row so icons stay aligned when OCR copy is shown.
- **Settings → Quick Menu** — quick-menu hotkey and snippets editor share one pane with inset-list layout; modifier-key hints and Save on Enter for the shortcut field.
- **Settings → Voice** — dedicated hotkey subsection with the same shortcut field pattern; **Recording** / **Microphone** labels; polish model and custom-instructions fields stay grouped when polish is enabled.
- **Settings** — deep-link to a pane via `?pane=` (e.g. from the quick menu); sidebar title **Hub** (was NeuralDeep Hub).
- **Snippets editor** — collapsible folders (state remembered) and inline folder rename.
- **Overlay & collections** — tooltips on Settings, Close, Pin, Add collection, and Remove; close icon sized to match other header controls.
- **Keyboard hints** — improved wrapping and spacing in overlay and palette footers.

### Fixed

- **Settings → Accessibility** — trust check no longer falsely reports "not granted" when Copyosity is already enabled in System Settings (uses `AXIsProcessTrusted` instead of a live AX probe that could return `kAXErrorCannotComplete`).
- Command palette min-dot — rectangular window shadow no longer lingers after minimizing to the dot.
- Command palette min-dot — expanding after dragging the dot restores the palette on the same display (not the primary screen).

## [0.6.0] - 2026-07-02

Fork merge release: upstream **v0.5.1** plus macOS clipboard/paste pipeline, overlay filters, security hardening, and HIG polish from the fork.

### Added

- **Overlay filters** — server-side search, tag, and format filters with DB-wide chip counts; tag bar with PNG / GIF / JPG chips and semantic AI tags when tagging is on; infinite scroll with **Try again** on failed pages; stale tag filters auto-clear when the grid is empty but history still has entries.
- **Overlay search** — `⌘F` and `/`; two-step Escape (clear query, then dismiss); Unicode case-insensitive search including OCR text on images; horizontal scroll-snap; optional keyboard-hints footer (**Settings → History**); 28×28 pt clear-button hit target.
- **History / Starred tabs** — macOS segmented control for pinned vs unpinned; custom collection pills with 7 px color dot and compact remove control.
- **Image clipboard** — PNG / JPG / GIF from pasteboard and Finder (~20 MB); animated GIFs; format badges; dimensions and file size on cards; OCR preview under thumbnails; format tags always visible in the filter bar.
- **Primary Paste on cards** — accent **Paste** button; Enter, Space, and double-click paste the selected entry; card actions on hover or keyboard focus (not bare selection).
- **macOS paste pipeline** (`clipboard_macos/` on **objc2**) — paste-target remember/restore, AX tree walk, synthetic Cmd+V, concealed-pasteboard detection; overlay, command palette, and voice paste into the app that was frontmost when opened; improved paste into Messages and Electron apps.
- **Unified clipboard writes** (`clipboard_write.rs`) — **Copy** / **Paste** modes shared by copy, activate, paste, and voice flows.
- **Clipboard capture** — Finder image files store pixels (not the file icon); ignore concealed clipboard content; suppress duplicate entries and Copyosity's own copy/paste from history; hash reset on history clear.
- **NeuralDeep Hub master switch** — one toggle for command palette, hub transcription/polish, and tray Agent Search.
- **Privacy** — native app picker for excluded apps; bundle IDs as stable keys; **Exclude [App]** from the overlay; clear unpinned or all history with confirmation and live count sync.
- **Settings toggles** — AI tagging and voice transcription (off by default); overlay keyboard hints; Ollama onboarding states and model validation before download; `is_tagging_ready` IPC for gated **Retag**.
- **Intel Mac builds** — x86_64 DMG alongside Apple Silicon (`make build-macos-intel`, arch-specific artifacts in `dist/macos/`).
- **Per-window Tauri capabilities** — scoped IPC for `main`, `settings`, `voice_overlay`, and `palette` (`paste_entry` / `activate_entry` on main only; `clear_history` and Ollama on settings only).
- **Design system** — `tokens.css`, `form-controls.css`, `button-interaction.css`, `.inset-list` grouped rows; input-modality tracking for pointer vs keyboard focus rings; rem-based typography scale.
- **Developer toolchain** — Oxlint, Oxfmt, Stylelint, Lefthook pre-commit, `make fix` / `make lint` / `make check`; Vite 8; optional `sccache` for Rust dev builds.
- **Tray menu** — **Open Clipboard `⌘⇧V`**; Agent Search disabled when hub is off.
- **Command palette** — Agent/Web mode switch; session history; streaming agent progress and markdown answers; draggable top bar, resize grip, minimize-to-dot; voice input; Insert / Copy / Close actions; HIG hover/focus on mode badge, history, actions, and toolbar.
- **Accessibility** — focus-visible rings on cards, tabs, and search; search in Tab order; roving `tabindex` and selection chrome separated from keyboard focus; voice HUD `aria-live` baseline; `prefers-reduced-motion`, `prefers-reduced-transparency`, and `prefers-contrast: more` support.

### Changed

- **Merge** — integrated upstream **v0.5.1** with fork overlay, ACL, and platform gating; image-pipeline reconciliation deferred to a follow-up backlog item.
- **Overlay** — Raycast-style open/close motion; sparkles icon opens command palette; outside-click dismiss; semantic tag chips when NeuralDeep Hub tagging is configured; abbreviated tag labels; synonym merge in filter counts (e.g. `javascript` / `js`); format chips sorted by count; fixed panel height **415 / 450 px**; **All / Text / Images** content-kind segment temporarily hidden; vertical board shows tag chips in the header strip; contextual empty states per filter.
- **Tagging** — auto-tag and **Retag** work with NeuralDeep Hub or local Ollama; with AI tagging off, semantic tags are hidden in the UI; tag backfill when tagging is enabled.
- **Cards** — fixed-height preview and footer layout; single-click copies; SF Pro for plain text previews, SF Mono for code-like content; filter chips visually distinct from card metadata badges; improved search-field readability on vibrancy.
- **Iconography** — macOS SF Symbols in overlay actions, settings section icons, and voice HUD mic (upstream stroke SVG layout preserved elsewhere).
- **Command palette & Settings** — visible over fullscreen apps on all Spaces.
- **Settings** — inset-list HIG layout; native title bar; boolean toggles apply immediately without losing unsaved edits; hub switch updates shortcuts and tray; unified form controls and multi-line field sizing; Ollama actions on background threads with visible busy states; app display names from installed bundle metadata.
- **Clipboard monitor** — captures only on real pasteboard changes; ignores Copyosity's own writes and when Copyosity is frontmost.
- **Tray** — left-click opens menu only; open the overlay via **Open Clipboard** or `⌘⇧V`.

### Fixed

- Paste into Cursor, Messages, and other Electron/native targets (no double paste, compose-field focus, background-thread timing).
- Voice and command palette paste into the stale overlay target instead of the app active at hotkey/open time.
- Finder image capture via `public.file-url` pasteboard type (no filename-only text cards); re-copy after clear/delete.
- History clear and last-card delete no longer re-insert stale clipboard content.
- Image backfill for legacy rows (full-size data, format, dimensions, `jpg` normalization).
- Settings partial updates no longer wipe Whisper or voice settings; accessibility enable hint persists until granted; Ollama unload and tagging test reliability; Save button layout jump.
- Overlay dismissal on Space switch; invisible cards on panel open; selection/focus desync during search; card scroll clipping at grid edges; mouse pin no longer leaves the action toolbar stuck open; pinned star hover distinct from unpinned.
- Excluded-app add-by-name errors and bundle-ID resolution; clear-history menu counts, notices, and confirm-dialog keyboard leak.
- Settings Storage/Privacy spacing and clear-notice layout.

### Security

- Per-window IPC command scoping via Tauri capabilities (`main` / `settings` / `voice_overlay` / `palette`).
- Ollama model name validation before `ollama pull` and settings persistence.
- `cargo audit` in the release workflow before artifacts ship.
- Tauri 2.11 upstream IPC ACL hardening.

## [0.5.1] - 2026-06-30

### Added

- **Voice overlay capsule** — pulse dot, live scrolling waveform, elapsed timer (replaces mic + EQ HUD).
- **Active-screen board** + **vertical mini-clipboard** (`board_vertical`) — docked-right list, compact cards, `↑/↓` hints.

### Changed

- **Emerald accent theme** (`#10b981` / `#34d399`) — NeuralDeep-style palette refresh.

## [0.5.0] - 2026-06-28

### Added

- **Voice polishing** — context-aware LLM cleanup of transcriptions; **selected-text command mode**.
- **Settings sidebar** — NeuralDeep · Voice · Local AI · History · Permissions panes; live model dropdowns from hub `/v1/models`.
- **Agent tools** — macOS Notes, Reminders, Calendar via `osascript`.
- **Command palette** — minimize-to-dot; draggable minimized dot (double-click to restore); more agent steps with guaranteed final answer.

### Changed

- Settings layout refactor; voice and hub sections reorganized.

## [0.4.4] - 2026-06-26

### Added

- **Command palette session history** — local persistence, resize grip, remembered window position.

### Changed

- Palette draggable with real window move; hide keeps agent state; markdown answers; clearer hub auth flow.

## [0.4.3] - 2026-06-25

### Added

- **Hub multimodal image tagging** — vision model tags image clipboard entries when hub tagging is enabled.

## [0.4.2] - 2026-06-24

### Added

- **Command palette / ReAct agent** — `⌘⇧Space` research agent loop in a draggable, resizable palette window.

## [0.4.1] - 2026-06-23

### Fixed

- NeuralDeep Hub default URL and settings aligned with API docs.

## [0.4.0] - 2026-06-22

### Added

- **NeuralDeep Hub** — settings pane, hub tagging / transcription / search, `/v1/models` list.
- **Command palette web search** — real hub search results in the palette.
- **On-device OCR** (macOS Vision) — overlay search includes recognized image text; live updates as OCR completes.

### Changed

- Settings promote NeuralDeep Hub to the top; local AI settings grouped below.

## [0.3.0] - 2026-04-10

### Added

- **Voice transcription** — dictate from the voice overlay into the active app.
- **Model pull progress** — non-blocking Ollama download with live progress in Settings.
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
