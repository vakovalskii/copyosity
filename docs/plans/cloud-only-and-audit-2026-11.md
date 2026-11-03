# Cloud-only Copyosity + app audit (2026-11)

Status: **plan** — branch `feat/cloud-only`, draft PR. Audit snapshot taken on 2026-11-03 against `main` @ `aad3c2b` (1.0.4 bumped, not notarized; 1.0.3 is what users run).

**Decision:** drop all local-model functionality (Ollama tagging, custom/local Whisper server) and keep only cloud (NeuralDeep hub) AI features. On-device Apple Vision OCR stays — it is OS OCR, not an LLM.

Related: [features-backlog.md](features-backlog.md) · [audit-hig.md](audit-hig.md) · [feature-appearance-theme.md](feature-appearance-theme.md) · [../AUTOUPDATE.md](../AUTOUPDATE.md)

---

## 1. Audit findings

### P0 — fix before the next release

1. **Launch auto-update bypasses `install_update` safeguards.** `src/lib/updater.ts:47` (`autoUpdateOnLaunch`) calls `update.downloadAndInstall()` from JS, not the Rust `install_update` command (`src-tauri/src/lib.rs:1794`). So the default update path has no same-volume `TMPDIR` (EXDEV, os error 18), no App Translocation check, no read-only volume probe. Failures only land in the localStorage log.
2. **No first-run onboarding.** App starts as Accessory (`lib.rs:528`): no Dock icon, hidden window, tray only. Nothing tells a new user about ⌘⇧V, permissions or the hub key.
3. **Accessibility is never requested on startup.** Only the tray paste passes `prompt=true` (`lib.rs:960`); overlay, quick menu, voice pass `false` (`commands.rs:734,748`, `lib.rs:1393,2001`). New user's paste-back silently does nothing (`clipboard_macos/paste.rs:24-31` just logs).
4. **Retention cleanup only runs at launch.** `cleanup_old_entries` is called from setup (`lib.rs:632`) and on settings save (`commands.rs:418`). A tray app running for weeks never prunes. Observed live: 1.4 GB DB, 905 full-res images = 1.3 GB, 340 unpinned rows older than the 30-day retention. No `VACUUM`/`incremental_vacuum`, so the file never shrinks.

### P1

- **Updates:** checked once per launch, no periodic check; "restart to apply" notification has no action; Settings then re-offers and re-downloads the same version (running version is still old); launch install and Settings install can race (`settings/+page.svelte:421` auto-checks on open). Launch errors are silent.
- **Silent failures (eprintln only):** paste without Accessibility, mic start failure (`lib.rs:1299`), Screen Recording missing for voice screenshot (`lib.rs:1250`), hotkey registration failures (`lib.rs:604-616`). No in-app toast system; only 3 OS notifications exist (update downloading/installed, voice failed).
- **Permissions pane** covers Accessibility only — no Microphone / Screen Recording status.
- **`scripts/release.sh`:** pushes the version bump + `docs/index.html` _before_ notarization (this is how 1.0.4 broke `main`, see `aad3c2b`); `git add src/ src-tauri/src/ scripts/` sweeps in unrelated changes; `build-macos.sh || true` + "dir exists" check can ship a stale `.app` under the new version; AppleDouble check covers aarch64 tarball only.
- **i18n:** `dict.ts` has 52 keys; most UI is hardcoded English (`ClipboardCard`, `QuickLookPanel`, `ActionMenu`, `ConfirmDialog`, `SnippetsEditor` have zero `$t(`). Visible in RU: "New line", "just now", "Exclude Preview from history".
- **Settings save model is mixed:** hotkeys save immediately, everything else waits for a floating Save bar.
- **Design:** dark-only, no native vibrancy, 11–12px type, uppercase web-style section titles, `--color-text-faint` ≈ 4:1 on small text (fails AA), two competing accents (emerald / purple).

### P2

- Mirror tarballs are unversioned (no rollback; non-atomic copy window). Mirror is endpoint #1, so a stale-but-valid `latest.json` there masks GitHub.
- `docs/AUTOUPDATE.md` drifted (no mirror, wrong relaunch description, no release.sh/AppleDouble/EXDEV notes).
- Tray label hard-codes "⌘⇧Space" though the palette shortcut is configurable (`lib.rs:1065`).
- Giant components: `settings/+page.svelte` 2367, `+page.svelte` 1941, `palette/+page.svelte` 1359, `ClipboardCard.svelte` 1353 lines.
- Stale branches: `fix/voice-paste-and-mic-entitlements`, `pr-4` (merged); `feature/local-streaming-asr` (local ASR — obsolete after this decision); `prerelease/windows-port` (superseded by PR #7).

---

## 2. Cloud-only removal plan

### What goes

- `src-tauri/src/ollama.rs` (whole file, ~1280 lines incl. 32 tests).
- IPC: `check_ollama_status`, `start_ollama_server`, `pull_ollama_model`, `unload_ollama_model`, `test_ollama_tagging`, `get_model_catalog`; event `ollama-pull-done`.
- Startup: `ollama::set_active_model` (`lib.rs:631`), `ensure_runtime` + `backfill_existing_tags` (`lib.rs:648-651`) — this currently auto-starts `ollama serve` and silently pulls a multi-GB model.
- `db.rs`: `AppSettings.ollama_model`, `ModelOption`/`ModelCatalog`, getter/setter, tests.
- `commands.rs`: `ollama_model` arg of `update_app_settings`, post-save Ollama block, the 5 commands.
- `tagging.rs`: Ollama fallbacks; OCR-text fallback re-pointed at hub.
- Voice: custom server branch in `transcription.rs` + settings `whisper_server_url` / `whisper_server_token` and their UI. **Keep** recording (`whisper.rs`, cpal/hound), `transcribe_audio`, and `whisper_server_model` (hub passes the model id; optionally rename to `transcribe_model`).
- Frontend: `api.ts` Ollama wrappers + `OllamaStatus`, `types.ts` `ModelCatalog`, settings "Local AI" pane (`settings/+page.svelte:1580-1811`) + nav + ~85 script references, "falls back to Ollama" label (`:1160`), `nav.ai` i18n keys in 7 locales.
- Docs: `docs/product/ollama-onboarding.md`, `docs/screenshots/settings-local-ai.png`, README "Local AI (Ollama)" section, `docs/index.html` mentions (run `make fix-frontend` after), row in `CLAUDE.md` / `AGENTS.md` table, `docs/TESTING.md`.

### What must move first (compile / behaviour dependencies)

1. `ollama::normalize_tags` → `tagging.rs` (used by `hub.rs:194,377`).
2. `heuristic_tags` / `looks_like_opaque_code` → `tagging.rs`, and **run it before the hub call** so OTPs / tokens / IDs are tagged locally and never sent to the hub (privacy guard; today only the Ollama path has it).
3. `backfill_existing_tags` → `tagging.rs`, gated on hub tagging readiness instead of the Ollama CLI, otherwise hub users never get old entries tagged.
4. `SectionIcon` `ollama-model` icon is reused by Hub and Voice panes — rename, don't delete.
5. `AppSettings` field list must change in lockstep across `db.rs`, `commands.rs::update_app_settings`, `transcription.rs` test fixture and `api.ts` camelCase mapping.
6. `overlay-filters.ts:10-21` `isSemanticTagUiEnabled` ORs `ai_tagging_enabled` — switch to hub-only; update tests.

### Settings migration (one-time, flag key, modelled on `migrate_hub_enabled_if_needed`, `db.rs:558`)

- `ai_tagging_enabled=true` **and** `hub_token` set → `hub_enabled=true`, `hub_tagging_enabled=true`.
- `voice_transcription_enabled=true` with custom `whisper_server_url` **and** hub token → `hub_transcribe_enabled=true`; without a token, voice stops — show a one-time notice.
- Delete `ollama_model`, `whisper_server_url`, `whisper_server_token`, `ai_tagging_enabled`.

### Settings IA after removal (8 → 6 panes, General first)

1. **General** — launch at login, language, palette + Quick Menu shortcuts, About / Updates group.
2. **Cloud AI** (was NeuralDeep) — connection, model, Auto-tag clipboard, use for voice.
3. **Voice** — recording, transcription model, polishing; dictionary / custom prompt under "Advanced".
4. **Quick Menu** — snippets.
5. **History** — layout, privacy / excluded apps, storage + retention.
6. **Permissions** — Accessibility, Microphone, Screen Recording.

Expected size: −~1,280 Rust lines, −~400 Svelte/TS lines, −5 IPC commands, −2 startup background jobs. No Cargo/npm deps become removable (`ureq`, `serde_json`, `cpal`, `hound`, `objc2-vision`, `ai`/`@ai-sdk/*` are all still used by cloud paths).

---

## 3. Execution order (this PR, then follow-ups)

| #   | Scope                                                                                                                   | Where                |
| --- | ----------------------------------------------------------------------------------------------------------------------- | -------------------- |
| 1   | Moves (normalize_tags, opaque-code heuristic, backfill) → delete Ollama + custom Whisper → migration → settings 6 panes | this PR              |
| 2   | P0: launch update via `install_update`; periodic retention cleanup + incremental vacuum; Accessibility check at startup | this PR or follow-up |
| 3   | First-run window (Accessibility → hotkey → hub key) + OS notifications for paste / mic / hotkey failures                | follow-up            |
| 4   | `release.sh`: push after notarization, verify built version, explicit `git add`, check both tarballs                    | follow-up            |
| 5   | i18n sweep, settings auto-apply, component splits, visual polish list from the audit                                    | follow-up            |

## 4. Verification

- `make fix && make check` (cross-cutting change).
- Manual on a clean user: fresh DB → Settings shows 6 panes, no Ollama strings; hub tagging tags new text and images; OTP-like strings get local tags and no hub request; voice transcribes via hub.
- Migration: seed DB with `ai_tagging_enabled=true` + hub token → hub tagging on after launch; seed custom whisper URL without token → notice shown.
- `make verify-tray` is not required unless tray/activation code is touched (P0 #3 touches startup — run it then).
