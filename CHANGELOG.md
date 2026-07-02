# Changelog

All notable changes to Copyosity are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] - Unreleased

### Added

- **Overlay filters** — server-side search, tag, and format filters with DB-wide chip counts; tag bar with PNG / GIF / JPG chips and semantic AI tags when tagging is on; infinite scroll with **Try again** on failed pages.
- **Overlay search** — `⌘F` and `/`; Unicode case-insensitive search including OCR text on images; horizontal scroll-snap; optional keyboard-hints footer (**Settings → History**).
- **History / Starred tabs** — macOS segmented control for pinned vs unpinned; custom collection pills with compact remove control.
- **Image clipboard** — PNG / JPG / GIF from pasteboard and Finder (~20 MB); animated GIFs; format badges; dimensions and file size on cards; OCR preview under thumbnails.
- **Primary Paste on cards** — accent **Paste** button; Enter, Space, and double-click paste the selected entry.
- **macOS paste target** — overlay, command palette, and voice paste into the app that was frontmost when opened; improved paste into Messages and Electron apps.
- **Clipboard capture** — Finder image files store pixels (not the file icon); ignore concealed clipboard content; suppress duplicate entries and Copyosity's own copy/paste from history.
- **NeuralDeep Hub master switch** — one toggle for command palette, hub transcription/polish, and tray Agent Search.
- **Privacy** — native app picker for excluded apps; **Exclude [App]** from the overlay; clear unpinned or all history with confirmation.
- **Settings toggles** — AI tagging and voice transcription (off by default); overlay keyboard hints; Ollama onboarding states and model validation before download.
- **Intel Mac builds** — x86_64 DMG alongside Apple Silicon.
- **Tray menu** — **Open Clipboard `⌘⇧V`**; Agent Search disabled when hub is off.
- **Command palette** — HIG hover/focus on mode badge, history, actions, and toolbar.
- **Accessibility** — focus-visible rings on cards, tabs, and search; search in Tab order; `prefers-reduced-motion`, `prefers-reduced-transparency`, and `prefers-contrast: more` support.

### Changed

- **Overlay** — sparkles icon opens command palette; semantic tag chips when NeuralDeep Hub tagging is configured; abbreviated tag labels; synonym merge in filter counts (e.g. `javascript` / `js`); format chips sorted by count; fixed panel height **415 / 450 px**; vertical board shows tag chips in the header strip.
- **Tagging** — auto-tag and **Retag** work with NeuralDeep Hub or local Ollama; with AI tagging off, semantic tags are hidden in the UI.
- **Cards** — single-click copies; SF Pro for plain text previews, SF Mono for code-like content; improved search-field readability on vibrancy.
- **Command palette & Settings** — visible over fullscreen apps on all Spaces.
- **Settings** — inset-list HIG layout; boolean toggles apply immediately without losing unsaved edits; hub switch updates shortcuts and tray; unified form controls and multi-line field sizing.
- **Tray** — left-click opens menu only; open the overlay via **Open Clipboard** or `⌘⇧V`.

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
