# New features and improvements

> **Release target: 0.4.0** — everything in this plan is scoped to ship with the 0.4.0 release. Shipped work is also recorded in [CHANGELOG.md](../../CHANGELOG.md).

Backlog and completed items formerly tracked in `AGENTS.md`.

**Related plans:** [05-overlay-content-and-tag-filters.md](05-overlay-content-and-tag-filters.md) (overlay filters, WIP) · [04-voice-hud-accessibility-full-cycle.md](04-voice-hud-accessibility-full-cycle.md) (voice HUD a11y) · [02-hig-audit.md](02-hig-audit.md) (UI audit)

---

## Security hardening

**Status: complete (0.3.0–0.4.0).** Security audit 2026-03-17; all items below are done before 0.4.0 ships.

- [x] Explicit Tauri capabilities for the `settings` window (`src-tauri/capabilities/settings.json`; 0.3.0)
- [x] Validate Ollama model names before `ollama pull` (`ollama::validate_model_name`; 0.3.0)
- [x] `cargo audit` step in GitHub Actions release workflow (0.4.0)
- [x] Per-window IPC command scoping for sensitive commands (`paste_entry`, `clear_history`, `start_ollama_server`; 0.4.0)

---

## Features

**Status: in progress — due in 0.4.0.**

- [ ] **Overlay content & tag filters** — **WIP**; baseline landed, more ideas still to implement; details in [05-overlay-content-and-tag-filters.md](05-overlay-content-and-tag-filters.md)
  - Two-level filtering: content kind (All / Text / Images) + tag chips (image format + AI tags)
  - AI-off mode: format chips only; panel height tiers (420 / 440 / 480 px)
  - Image card meta (dimensions, file size); filter vs metadata badge separation
  - Follow-up UX and filter behavior — open; extend the linked plan as ideas land

- [ ] **Infinite scroll** — lazy loading entries on horizontal scroll (backend already supports `limit` + `offset`)

- [ ] **Shortcut recorder** (voice + future overlay shortcut)
  - Replace text inputs with a shortcut recorder control
  - Show symbols in the UI; persist a canonical string for Rust
  - States: idle / recording / invalid / conflict
  - `aria-label`: “Shortcut, click to record”; `aria-live="polite"` while recording
  - Keypress commits the shortcut without requiring Enter on Save (System Settings pattern)

- [ ] **Voice transcription improvements** — large HUD accessibility and transcription lifecycle pass; see [04-voice-hud-accessibility-full-cycle.md](04-voice-hud-accessibility-full-cycle.md)
  - Full screen-reader lifecycle: recording → processing → terminal (success / empty / error / not configured)
  - HUD stays visible during transcription; delayed hide after terminal announcement
  - Global announcer + phase state machine; no live-region spam from audio level
  - Rust `voice-a11y` events, seq, and permissions in capabilities

- [ ] **Custom collections**
  - “Name…” field appears when the user clicks **+** to the right of **Clipboard History** / **Starred** tabs — creates a new user-defined collection tab for grouping clipboard entries
  - Backend already supports assigning entries (`set_entry_collection`); finish the UI so cards can add/move entries (`setEntryCollection` is not wired today)
  - Today: create a collection, switch to it (filters by `collection_id`), delete it — but new collections stay empty until entries are assigned another way (e.g. DB directly)
  - Ship end-to-end grouping: assign/remove entries from cards (or equivalent UX) so collections are usable without manual data fixes

---

## Fixes

**Status: in progress — due in 0.4.0.**

- [ ] **Production build transparency** — verify and fix on macOS 15+ (known Tauri issue [#13415](https://github.com/tauri-apps/tauri/issues/13415))
