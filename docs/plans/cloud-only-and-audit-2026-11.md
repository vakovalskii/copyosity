# Cloud-only Copyosity + app audit (2026-11)

Status: **§2 implemented** on `feat/cloud-only` (Ollama + custom Whisper removed, `tagging.rs` upload guard, fixed hub backfill, one-time settings migration with Settings notices, Local AI pane removed, General pane first). The 6-pane IA reshuffle and §3 rows 2–5 remain open.

Original status: **plan** — branch `feat/cloud-only`, draft PR. Audit snapshot taken on 2026-11-03 against `main` @ `aad3c2b` (1.0.4 bumped, not notarized; 1.0.3 is what users run).

**Decision:** drop all local-model functionality (Ollama tagging, custom/local Whisper server) and keep only cloud (NeuralDeep hub) AI features. On-device Apple Vision OCR stays — it is OS OCR, not an LLM.

Related: [features-backlog.md](features-backlog.md) · [audit-hig.md](audit-hig.md) · [feature-appearance-theme.md](feature-appearance-theme.md) · [../AUTOUPDATE.md](../AUTOUPDATE.md)

---

## 1. Audit findings

### P0 — fix before the next release

1. **Launch auto-update bypasses `install_update` safeguards.** `src/lib/updater.ts:47` (`autoUpdateOnLaunch`) calls `update.downloadAndInstall()` from JS, not the Rust `install_update` command (`src-tauri/src/lib.rs:1794`). So the default update path has no same-volume `TMPDIR` (EXDEV, os error 18), no App Translocation check, no read-only volume probe. Failures only land in the localStorage log.
2. **No first-run onboarding.** App starts as Accessory (`lib.rs:528`): no Dock icon, hidden window, tray only. Nothing tells a new user about ⌘⇧V, permissions or the hub key.
3. **Accessibility is never requested on startup.** The overlay paste does prompt (`finish_paste` → `finalize_panel_hide` → `spawn_automated_paste(true)`, `lib.rs:960`) and Settings prompts on open, but quick menu (`commands.rs:734,748`), voice (`lib.rs:1393`) and palette insert (`lib.rs:2001`) pass `false` — those paste-backs silently do nothing on a fresh Mac (`clipboard_macos/paste.rs:24-31` just logs). _(corrected after Codex review)_
4. **Retention cleanup only runs at launch.** `cleanup_old_entries` is called from setup (`lib.rs:632`) and on settings save (`commands.rs:418`). A tray app running for weeks never prunes. Observed live: 1.4 GB DB, 905 full-res images = 1.3 GB, 340 unpinned rows older than the 30-day retention. No `VACUUM`/`incremental_vacuum`, so the file never shrinks.

### P1

- **Updates:** checked once per launch, no periodic check; "restart to apply" notification has no action; Settings then re-offers and re-downloads the same version (running version is still old); launch install and Settings install can race (`settings/+page.svelte:421` auto-checks on open). Launch errors are silent.
- **Silent failures (eprintln only):** paste without Accessibility, mic start failure (`lib.rs:1299`), Screen Recording missing for voice screenshot (`lib.rs:1250`), hotkey registration failures (`lib.rs:604-616`). No in-app toast system; only 3 OS notifications exist (update downloading/installed, voice failed).
- **Permissions pane** covers Accessibility only — no Microphone / Screen Recording status.
- **`scripts/release.sh`:** pushes the version bump + `docs/index.html` _before_ notarization (this is how 1.0.4 broke `main`, see `aad3c2b`); `git add src/ src-tauri/src/ scripts/` sweeps in unrelated changes; `build-macos.sh || true` + "dir exists" check can ship a stale `.app` under the new version; AppleDouble check covers aarch64 tarball only.
- **i18n:** `dict.ts` has 52 keys; most UI is hardcoded English (`ClipboardCard`, `QuickLookPanel`, `ActionMenu`, `ConfirmDialog`, `SnippetsEditor` have zero `$t(`). Visible in RU: "New line", "just now", "Exclude Preview from history".
- **Settings save model is mixed:** hotkeys, booleans (`settings/+page.svelte:670`), locale and autostart save immediately; text fields and selects wait for a floating Save bar. _(corrected after Codex review)_
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

Revised after Codex review — **never silently turn on cloud uploads**:

- `ai_tagging_enabled=true` (local tagging) → do **not** enable hub tagging automatically, even with a saved token (the token may have been meant for voice/agent only). Set a `setup_notice=local_tagging_removed` flag; Settings / first-run shows "Local tagging was removed — enable cloud tagging?" with an explicit toggle.
- Existing `hub_enabled` / `hub_tagging_enabled` values are preserved as-is (explicit opt-outs stay off).
- Custom `whisper_server_url` users → `setup_notice=custom_whisper_removed`. If they opt into hub transcription, validate master switch + URL + token + model together. Until voice is usable, **gate recording** (don't start the mic) instead of failing after recording.
- Migration is atomic: read legacy values → write notice flags + deletions + completion marker in one transaction → refresh `settings_cache`. Runs before settings consumers and workers. Idempotent on re-run.
- Then delete `ollama_model`, `whisper_server_url`, `whisper_server_token`, `ai_tagging_enabled`.

### Settings IA after removal (8 → 6 panes, General first)

1. **General** — launch at login, language, palette + Quick Menu shortcuts, About / Updates group.
2. **Cloud AI** (was NeuralDeep) — connection, model, Auto-tag clipboard, use for voice.
3. **Voice** — recording, transcription model, polishing; dictionary / custom prompt under "Advanced".
4. **Quick Menu** — snippets.
5. **History** — layout, privacy / excluded apps, storage + retention.
6. **Permissions** — Accessibility, Microphone, Screen Recording.

### Corrections from the Codex review (2026-11-03, `gpt-6-astra`)

All verified by hand against the code.

1. **Privacy guard is not a guarantee.** `looks_like_opaque_code` rejects strings > 32 chars (`ollama.rs:904`), so `sk-…` keys, JWTs and credentials inside text still reach the hub; images go to `hub::tag_image` directly (`clipboard_monitor.rs:373`). Add a credential-pattern check (API keys, JWT, `password=`/`token=`, PEM, long high-entropy strings) and apply it before **every** automatic text and image upload. Keep "blocked, no tags" distinct from "eligible for cloud tagging". Don't advertise it as "tokens never leave the device".
2. **Backfill is broken, not just mis-gated.** The first pass marks untagged entries `done` because tags are empty (`ollama.rs:769`); the second pass skips any row with a tag-state (`db.rs:1560`), so old plain text is never sent to the hub. Fix eligibility (and re-open rows wrongly marked `done`/`skipped` without touching user tags) before moving it; distinguish transient failures; trigger backfill when hub becomes ready after startup, not only at launch.
3. **Launch update can't just call `install_update`.** The command is allowed only for the settings window (`permissions/settings-commands.toml:46`) and it always restarts (`lib.rs:1864`). Need: a shared guarded Rust install path, a main-window permission, a "background install, no restart" mode, and one install lock shared by launch and Settings.
4. **`incremental_vacuum` does nothing on existing DBs** — they are `auto_vacuum=NONE` (`db.rs:388`). Either run a controlled full `VACUUM` (idle time, free-disk check) or switch to `auto_vacuum=INCREMENTAL` + one-time `VACUUM` to apply it.
5. **Voice error path leaks UI state.** If the endpoint can't be resolved, `lib.rs:1334` returns after recording without emitting an error or hiding the capsule — it stays "transcribing". Same for palette voice (`lib.rs:1894,1915`). Every exit must report and hide.
6. **Lockstep list was incomplete.** Also: `src/lib/types.ts:61` `AppSettings`, the settings initializer (`settings/+page.svelte:89`), deferred/immediate key unions, the 6th command `get_model_catalog`, event `ollama-pull-progress`, Settings ACL entries (`permissions/settings-commands.toml`), `settings-pane.ts` + tests, pane validation in `commands.rs:222`, Makefile env knobs. Keep `is_tagging_ready` (the overlay still calls it — re-point at hub) and `EntryTaggedPayload`.

Expected size: −~1,280 Rust lines, −~400 Svelte/TS lines, −6 IPC commands, −1 startup job (Ollama auto-start/pull; backfill stays but moves). No Cargo/npm deps become removable (`ureq`, `serde_json`, `cpal`, `hound`, `objc2-vision`, `ai`/`@ai-sdk/*` are all still used by cloud paths).

---

## 3. Execution order (this PR, then follow-ups)

| #   | Scope                                                                                                                                                             | Where                |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------- |
| 0   | Migration design + tests first (outcomes per user state, atomic, idempotent)                                                                                      | this PR              |
| 1   | Moves (normalize_tags, heuristics + new credential guard on every upload, fixed backfill) → delete Ollama + custom Whisper → migration → settings 6 panes         | this PR              |
| 2   | P0: shared guarded update install (main-window ACL, no-restart mode, lock); periodic retention + real VACUUM; voice error cleanup; Accessibility check at startup | this PR or follow-up |
| 3   | First-run window (Accessibility → hotkey → hub key) + OS notifications for paste / mic / hotkey failures                                                          | follow-up            |
| 4   | `release.sh`: push after notarization, verify built version, explicit `git add`, check both tarballs                                                              | follow-up            |
| 5   | i18n sweep, settings auto-apply, component splits, visual polish list from the audit                                                                              | follow-up            |

## 4. Verification

- `make fix && make check` (cross-cutting change).
- Manual on a clean user: fresh DB → Settings shows 6 panes, no Ollama strings; hub tagging tags new text and images; OTP-like strings get local tags and no hub request; voice transcribes via hub.
- Migration tests: `ai_tagging_enabled=true` + hub token + `hub_tagging_enabled=false` → stays off, notice set; fresh user → nothing on; hub-only tagging → unchanged; custom whisper without token → notice, recording gated; re-run → no-op; interrupted run → consistent.
- Unit tests: credential guard (sk-keys, JWT, PEM, `password=`), backfill eligibility, endpoint failure hides the voice capsule, concurrent launch/Settings install.
- Launch updater exercised from the real main-window capability.
- `make verify-tray` is not required unless tray/activation code is touched (P0 #3 touches startup — run it then).

---

## 5. Codex full-app audit (2026-11-03, `gpt-6-astra`, read-only)

Report: `~/.codex-reviews/copyosity-full-1103-2230.md`. ✅ = re-verified by hand in code; others are traced statically by Codex and still need a check before fixing.

### Security / privacy / crash — fix first (own PR, before cloud-only)

| #   | Finding                                                                                                                                                                                                  | Where                                      | Check |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------ | ----- |
| S1  | **`make clean-all` deletes the updater signing key** (`rm -rf $(APP_DIR)/.tauri`). Losing it breaks auto-update for every installed copy.                                                                | `Makefile:129`                             | ✅    |
| S2  | **Excluded-app content gets captured after an app switch**: `should_skip_source` → `None` leaves `capture_pending=true`; the retry attributes it to the new frontmost app and saves (and cloud-tags) it. | `clipboard_monitor.rs:411,588`             | ✅    |
| S3  | **Double `CFRelease` in AX window traversal** — windows released in-loop are released again in the success cleanup → crash in paste targeting.                                                           | `clipboard_macos/accessibility.rs:137-150` | ✅    |
| S4  | AX focused element used after the mutex is dropped → use-after-free under concurrent target capture.                                                                                                     | `accessibility.rs:52`                      |       |
| S5  | **Palette Esc/hide keeps the mic recording** (`palette-hide` only resets focus; session in `palette_recording_mutex` untouched, unbounded buffer, uploaded on next stop).                                | `lib.rs:995`, `palette/+page.svelte:581`   | ✅    |
| S6  | Selected-text voice mode restores a concealed password with plain `set_text` → loses the concealment marker, monitor may store / cloud-tag it; non-text formats lost.                                    | `lib.rs:2064`                              |       |
| S7  | "Screenshot of the target window" captures the whole display; palette falls back to full screen silently.                                                                                                | `lib.rs:1239`, `screen.rs:56`              |       |
| S8  | Overlapping dictations share global target/context → A's transcript can paste into app B.                                                                                                                | `lib.rs:1389`                              |       |
| S9  | **Content hash covers only the first 4 KB + length** → different texts with equal prefix/length dedupe to the old entry (pastes stale content).                                                          | `clipboard_monitor.rs:156`                 | ✅    |
| S10 | Clipboard write failure still triggers paste → target gets the previous clipboard (possibly a password).                                                                                                 | `lib.rs:1381,1988`                         |       |
| S11 | Failed selected-text rewrite pastes the raw instruction over the user's selection.                                                                                                                       | `lib.rs:1188`                              |       |

### Reliability / release

- Release: `release.sh:80` stale-bundle publish (also in §1); `.github/workflows/release.yml:217` tag workflow publishes DMGs without `latest.json`/tarballs — pick one pipeline; mirror race (`publish-mirrors.sh:48,68`).
- Transcription has no HTTP timeout (`whisper.rs:206`) → worker blocks forever.
- Voice config failure leaves the capsule on "Transcribing…" (`lib.rs:1331`, also in §2 corrections).
- CSP allows only NeuralDeep domains, so a custom hub URL passes the connection test but agent chat fails (`tauri.conf.json:15`) — restrict hub URL in Settings or proxy agent requests through Rust.
- Retention / vacuum / first-run paste / launch updater — same as §1 P0.

### Revised PR order

1. **PR A — security & crash fixes** S1–S11 (+ tests: exclusion retry, full-payload hash, palette hide cancels recording, AX ownership).
2. **PR B (this one) — cloud-only** per §2 with Codex corrections.
3. **PR C — P0 runtime**: updater shared install path, periodic retention + VACUUM, transcription timeouts, voice error cleanup.
4. **PR D — onboarding + notifications**, **PR E — release pipeline**, **PR F — i18n/design**.
