# Features backlog

Living backlog for product features, fixes, and cross-cutting work. **Open items are listed first**; checked items below record what shipped and where it came from. Release details: [CHANGELOG.md](../../CHANGELOG.md).

Not a feature spec — items with a linked `feature-*.md` keep the full design there; open items without a spec stay detailed in this file.

**Related plans:** [feature-overlay-content-tag-filters.md](feature-overlay-content-tag-filters.md) · [feature-voice-hud-accessibility.md](feature-voice-hud-accessibility.md) · [feature-appearance-theme.md](feature-appearance-theme.md) · [audit-hig.md](audit-hig.md) · [feature-quick-look-preview.md](feature-quick-look-preview.md)

**Legend:** **fork** — original Copyosity fork work · **upstream** — merged from upstream **v0.5.1** into Copyosity **0.6.0**

---

## Open — features

- [ ] **Shortcut recorder** (voice, overlay, palette, snippets)
  - Replace text inputs with a shortcut recorder control (System Settings pattern)
  - Cover all global shortcuts:
    - **Voice transcription** — today: text field `voice_shortcut` in Settings
    - **Clipboard overlay** — today: hardcoded `⌘⇧V` / `Ctrl+Shift+V` in Rust
    - **Agent / command palette** — today: hardcoded `⌘⇧Space` / `Ctrl+Shift+Space` in Rust
    - **Snippets / quick menu** — today: text field `quick_menu_shortcut` in Settings → Quick Menu; default `⌘⇧C` / `Ctrl+Shift+C` in Rust
  - Show symbols in the UI; persist a canonical string for Rust `parse_shortcut`
  - States: idle / recording / invalid / conflict
  - `aria-label`: “Shortcut, click to record”; `aria-live="polite"` while recording
  - Keypress commits the shortcut without requiring Enter on Save
  - Conflict detection across voice / overlay / palette / snippets shortcuts
  - Tray menu labels and overlay header tooltips read from the same display helper

- [ ] **Voice HUD accessibility** — full screen-reader lifecycle for the recording capsule; spec: [feature-voice-hud-accessibility.md](feature-voice-hud-accessibility.md)
  - Recording → processing → terminal (success / empty / error / not configured)
  - HUD stays visible during transcription; delayed hide after terminal announcement
  - Global announcer + phase state machine; no live-region spam from audio level
  - Rust `voice-a11y` events, seq, and permissions in capabilities

- [ ] **Custom collections**
  - Backend supports `set_entry_collection`; UI to assign/remove entries from cards is not wired
  - Today: create tab, filter by `collection_id`, delete — new collections stay empty without manual assignment

- [ ] **URL / link recognition** — treat detected URLs as a first-class clipboard kind, with a dedicated **link** tag and filter chips (same product pattern as image format tags). No spec file yet; design mirrors [feature-overlay-content-tag-filters.md](feature-overlay-content-tag-filters.md) image pipeline.
  - **Detection** — on text capture, parse `http(s)://`, `www.`, and common TLD shapes; support single-URL clipboard rows and multi-line text where the primary payload is one URL; normalize (strip trailing punctuation, lowercase host for dedup, keep display URL for cards).
  - **Storage** — extend entry model beyond `text` / `image` (e.g. `content_kind` / `link_url` column or `detected_url` + `url_host`); persist canonical URL for paste-back and search; do not rely on AI semantic tags for “this is a link”.
  - **Tag model** — pin a format-style tag (e.g. `link` or `url`) separate from semantic tags; chip bar always shows **Link** counts when link entries exist (like `png` / `jpg` / `gif` for images); filter SQL counts link rows via column + tag UNION (same dedup rules as `image_format`).
  - **Overlay UI** — card header/badge **Link** (or short host, e.g. `github.com`); optional footer line with truncated URL; Row B format chip **link** when AI tagging is off; when AI is on, link chip stays in the format group (semantic chips unchanged). Content Kind row may gain **Links** segment (deferred until Row A is re-enabled).
  - **Search** — include normalized URL and host in overlay search / DB index (like `ocr_text` for images).
  - **Paste** — paste restores the stored URL string (not markdown wrapper unless source had it).
  - **Backfill** — one-time migration for legacy text rows that are URL-only (`UPDATE` kind + tag from `text_content` heuristic).
  - **Non-goals (v1):** fetching page titles, favicons, or Open Graph previews; breaking apart markdown `[label](url)` into two entries.

- [ ] **Appearance — Light / Dark / Automatic** — system theme switching; cool blue-gray light palette; persistence + live sync across all windows. Upstream emerald accent is separate. Spec: [feature-appearance-theme.md](feature-appearance-theme.md)

- [ ] **Command palette polish** (remainder) — baseline shipped in **0.6.0**; open:
  - **Accessibility:** `aria-live="polite"` for agent progress, errors, and terminal states; richer mic labels (recording vs idle)
  - **Hub-disabled UX:** visible empty/disabled state when `hub_enabled=false` (backend already gates palette IPC)
  - **Min-dot keyboard access:** restore via keyboard (today mouse-only drag/double-click)
  - **Error recovery:** retry action on agent/search failure (today error text only)

---

## Open — fixes

- [ ] **Production build transparency** — verify and fix on macOS 15+ (known Tauri issue [#13415](https://github.com/tauri-apps/tauri/issues/13415))

---

## Shipped — unreleased

- [x] **Quick Look preview** — launcher-style full entry preview for the selected card; in-panel dialog (no separate window), `Space` or `⌘Y` toggles it (`⌘Y` from search), `↓`/`→` exits search to the first visible card, trackpad scroll syncs selection on the horizontal board (vertical: use arrows after scroll). Type-chip eye (clip-path pill collapse) + secondary-click **Preview** on cards; full-resolution image fetch + GIF playback; Image / Recognised text segmented toggle when OCR exists. `←/→`/`↑/↓` keep browsing while preview is open. Spec: [feature-quick-look-preview.md](feature-quick-look-preview.md).

---

## Shipped — 0.6.0

Fork merge release: upstream **v0.5.1** integrated with fork overlay, paste pipeline, security hardening, and HIG polish. Release details: [CHANGELOG.md](../../CHANGELOG.md#060---unreleased).

### Fork — security hardening

- [x] **Explicit Tauri capabilities for the** `settings` **window** — first scoped `settings.json` instead of a shared broad default.

  ```
  Four-window ACL — `main.json`, `settings.json`, `voice_overlay.json`, and `palette.json` with explicit `commands.allow` via permission sets.
  ```

- [x] **Validate Ollama model names before** `ollama pull` — `ollama::validate_model_name` (trim, length ≤ 128, safe character set) before pull and settings persistence. `pull_ollama_model` stays on the settings capability set only.

- [x] `cargo audit` **in release workflow** — dependency audit step in GitHub Actions before release artifacts ship.

- [x] **Per-window IPC command scoping** — sensitive commands limited per window: `paste_entry` / `activate_entry` on `main` only; `clear_history`, `start_ollama_server`, exclusion editing on `settings` only; `voice_overlay` events-only; palette scoped to hub search/agent/voice IPC.

### Fork — developer toolchain

- [x] **Lefthook pre-commit** — parallel staged auto-fix (Oxfmt, Oxlint, Stylelint, `cargo fmt`, `cargo clippy --fix --lib`); full gate remains `make check` / CI.

- [x] **Oxlint / Oxfmt / Stylelint +** `make fix` / `make lint` / `make check` — validation contract in `AGENTS.md`.

- [x] **macOS Intel (x86_64) release matrix** — `make build-macos-intel`, arch-specific DMGs in `dist/macos/`. Plan: [build-macos-intel.md](build-macos-intel.md).

### Fork — features

Clipboard overlay and macOS integration:

- [x] **macOS paste pipeline** — `clipboard_macos/` on **objc2**: paste-target remember/restore, AX tree walk, synthetic Cmd+V, Accessibility trust; panel hides before user-initiated paste; Messages / Electron targets.

- [x] **Unified clipboard writes** — `clipboard_write.rs` **Copy** / **Paste** modes; monitor skips own writes and concealed pasteboard.

- [x] **Infinite scroll** — lazy loading on horizontal scroll (`get_entries` with `limit` + `offset`; prefetch, backfill, **Try again** on failed pages).

- [x] **Overlay content & tag filters** — server-side `search` / `tag` / `content_kind`; format chips always, semantic chips when AI tagging on; DB-wide chip counts; image card meta (dimensions, file size). Spec: [feature-overlay-content-tag-filters.md](feature-overlay-content-tag-filters.md).

- [x] **Overlay scroll-snap** — horizontal snap to whole cards; selection syncs to leading visible card; keyboard `←/→` re-anchor policy.

- [x] **Overlay keyboard hints** — footer strip; optional via **Settings → History → Keyboard shortcuts** (default on); static height **415 / 450 px**.

- [x] **Overlay search** — `⌘F` / `/`; two-step Esc; Unicode case-insensitive DB search; denser search field on vibrancy.

- [x] **Image capture & card meta** — PNG / JPG / GIF from pasteboard or Finder (~20 MB); format badges and filter chips; dimensions and file size on cards (`image_format`, `image_width`, `image_height`, `image_byte_size`). _Deferred follow-up:_ full image-pipeline reconciliation (legacy rows, retag format-tag drift) — track in a future backlog item if needed.

- [x] **On-device OCR** (macOS Vision) — `ocr_text` in DB, overlay search, live `entry-ocr` updates; recognized text under image thumbnails on cards (`imageOcrPreviewText` / `ClipboardCard`). Quick Look shows the same OCR text via an **Image / Recognised text** segmented toggle ([feature-quick-look-preview.md](feature-quick-look-preview.md)).

Settings and product policy:

- [x] **AI tagging toggle** — off by default; `is_tagging_ready` IPC; Ollama onboarding per [docs/product/ollama-onboarding.md](../product/ollama-onboarding.md); tag backfill when enabled.

- [x] **Voice transcription toggle** — off by default; Whisper fields disabled when off.

- [x] **Privacy — excluded apps** — native picker, add by name, overlay **Exclude [App]**; bundle IDs as stable keys.

- [x] **Clear history** — unpinned / all with confirm; live counts via `history-changed`.

- [x] **Design system & HIG pass (baseline)** — `tokens.css`, `form-controls.css`, reduced motion/transparency, overlay a11y baseline. Tracker: [audit-hig.md](audit-hig.md).

### From upstream (v0.5.1)

Not original fork work; merged into **0.6.0**:

- [x] **NeuralDeep Hub** — settings pane, hub tagging / transcription / search, `/v1/models` list.

- [x] **Command palette / Agent** — `Cmd+Shift+Space`, ReAct agent, session history, draggable palette window.

- [x] **Hub multimodal image tagging**.

- [x] **Voice overlay capsule** — pulse dot, scrolling waveform, duration timer (replaces fork mic + EQ HUD).

- [x] **Emerald accent theme** (`#10b981`) — upstream palette; separate from Light/Dark/Automatic backlog.

- [x] **Active-screen board positioning** + **vertical mini-clipboard** (`board_vertical`) — docked-right list, compact cards, `↑/↓` hints.

- [x] **Voice polishing** — context-aware LLM cleanup; selected-text command mode.

- [x] **Settings sidebar layout** — NeuralDeep · Voice · Local AI · History · Permissions panes; stroke SVG icons.

- [x] **Platform gating** — `get_platform` IPC; macOS-only sections hidden on Windows.

- [x] **Windows experimental CI** — hollow shell build (`build-windows` job). macOS remains the supported product.

### Command palette polish

Completed in **0.6.0** (open remainder tracked above):

- [x] Agent/Web mode switch with Tab; session history (localStorage, 50 sessions)
- [x] Streaming agent progress + error display; markdown answers
- [x] Draggable top bar, resize grip, minimize-to-dot
- [x] Keyboard hints footer; voice input; Insert / Copy / Close actions
- [x] Shared `overlay-icon-btn` + HIG hover on palette controls
- [x] Overlay header hub icon for palette launch (distinct from clipboard search)
