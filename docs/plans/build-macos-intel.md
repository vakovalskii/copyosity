# macOS Intel build and related improvements

**Archived plan — shipped in fork v0.4.0**, preserved through upstream **v0.5.1** merge and **v0.6.0**.
Not a living release gate; current backlog: [features-backlog.md](features-backlog.md). Release notes: [CHANGELOG.md](../../CHANGELOG.md).

Why the fork diverged on build tooling and macOS clipboard/paste before the upstream merge. Living paste architecture: [macos-paste-pipeline.md](../architecture/macos-paste-pipeline.md).

---

## Summary

| Phase                     | Scope                                                                          | Shipped   |
| ------------------------- | ------------------------------------------------------------------------------ | --------- |
| **1 — Build & clipboard** | Intel/ARM DMG matrix, dev toolchain, clipboard monitoring, paste target, objc2 | **0.4.0** |
| **2 — Pre-release gate**  | History dedup, unified `clipboard_write`, ACL per window, security/perf fixes  | **0.4.0** |

---

## Checklist — phase 1 (build & clipboard)

- [x] **`build-macos.sh`** — pipeline: frontend → Tauri bundle → DMG in `dist/macos/`
- [x] **`macos-target.sh`** — architecture via `MACOS_ARCH=auto | x86_64 | aarch64`
- [x] **Makefile** — `build-macos`, `build-macos-intel`, `build-macos-arm` and mirrored `release-macos-*`
- [x] **Named artifacts** — `Copyosity_0.4.0_x86_64.dmg` etc. in `dist/macos/`
- [x] **`tauri.unsigned.json`** — ad-hoc for local builds; release with Developer ID via `RELEASE_CONFIG=1`
- [x] **`release-macos.sh`** — same build pipeline as local build
- [x] **Makefile `APP_DIR`** — no hardcoded project path
- [x] **`env -u npm_config_devdir`** — stable `npm install` / Tauri build (incl. Cursor)
- [x] **`with-tauri.sh` / `env-rust.sh`** — `cargo` and `tauri` in PATH
- [x] **`.vscode/settings.json`** — same workaround for integrated terminal
- [x] **`.gitignore`** — `/dist` for build artifacts
- [x] **README** — Intel/ARM commands and `dist/macos/` path
- [x] **Frontend toolchain** — SvelteKit / Svelte / Vite update, `cookie` override
- [x] **Clipboard monitoring** — `changeCount`; order: files → raster → text; concealed; ignore Copyosity / excluded apps
- [x] **Image decoders** — jpeg, webp, gif, bmp, tiff via `image` crate for on-disk paths
- [x] **`CaptureContext` / `try_capture_from_clipboard`** — single pasteboard parsing entry point
- [x] **`clipboard_macos.rs`** — pasteboard API, synthetic Cmd+V, remember/restore paste target
- [x] **`clipboard_write.rs`** — `exclude_from_history`, mark as own entry
- [x] **copy vs activate** — split between buffer-only and paste into another app; Enter = `activateEntry`
- [x] **Accessibility** — `check_accessibility` + UI in Settings; Settings window to foreground (`objc2-app-kit`)
- [x] **objc → objc2 migration** — `cocoa` replaced where already in use
- [x] **Frontend** — Enter in feed = paste; Permissions in Settings; card copy/paste model unchanged
- [x] **Voice shortcut** — shared macOS pasteboard logic moved to `clipboard_macos`

## Checklist — phase 2 (pre-release gate)

- [x] **History dedup** — `content_hash` = base hash only; on macOS `last_content_hash` on `changeCount` change; hash reset when unpinned history is emptied
- [x] **DB unit tests** — fix `update_settings`; partial update tests (Whisper/voice/mic not overwritten)
- [x] **Accessibility UX** — no prompt in `finish_paste`; `check_accessibility(prompt)` — prompt only from Request button
- [x] **clipboard_write unified** — Copy/Paste modes; voice, copy, activate, paste via one module
- [x] **paste pipeline** — `paste_text_into_target`; `paste_entry` + text `activate_entry`
- [x] **Monitor 300 ms** — `changeCount` only as event, not part of hash
- [x] **objc2** — rewrite `clipboard_macos`; remove `objc` crate where possible (including AX in `commands.rs`)
- [x] **Capabilities split** — `main.json` / `settings.json` / `voice_overlay.json`; explicit `allow-*` (palette added in **0.5.1**)
- [x] **Security hardening** — Ollama model name validation before `ollama pull`; `cargo audit` in release CI
- [x] **Perf** — single `get_frontmost_app` in `file_list` loop; image file size limit (~20 MB)
- [x] **Verification** — `make check`, `cargo test`, `make build-macos-intel` green

---

## 1. Intel build (x86_64)

**Goal:** produce a reproducible `.app` and DMG for Intel Mac, in parallel with Apple Silicon, without tying to a single developer machine.

- `scripts/build-macos.sh` — unified pipeline: frontend → Tauri bundle → DMG in `dist/macos/`.
- `scripts/macos-target.sh` — architecture via `MACOS_ARCH=auto | x86_64 | aarch64`.
- `Makefile`: `build-macos`, `build-macos-intel`, `build-macos-arm` and mirrored `release-macos-*`.
- Named artifacts in `dist/macos/` (e.g. `Copyosity_0.4.0_x86_64.dmg`).
- `tauri.unsigned.json` — ad-hoc signing for local builds; release with Developer ID via `RELEASE_CONFIG=1` in `release-macos.sh`.

**How to build:** `make build-macos-intel` (Intel) or `make build-macos` / `make build-macos-arm` on the matching machine.

---

## 2. Build infrastructure and dev environment

**Goal:** Intel/ARM builds and `npm run tauri` work on any machine and in the IDE without manual path setup.

- `APP_DIR ?= $(CURDIR)` in Makefile — no hardcoded project path.
- `env -u npm_config_devdir` for npm — stable `npm install` / Tauri build (incl. Cursor).
- `scripts/with-tauri.sh`, `scripts/env-rust.sh` — `cargo` and `tauri` in PATH.
- `.vscode/settings.json` — same workaround for integrated terminal.
- `.gitignore`: `/dist` — build artifacts directory.
- `README.md` — Intel/ARM commands and `dist/macos/` path.
- SvelteKit / Svelte / Vite update, `cookie` override — current frontend toolchain on a clean clone.

---

## 3. macOS — clipboard and history

**Goal:** catch macOS copies more reliably, show images correctly, and avoid polluting history with the app's own actions.

### Monitoring

- `NSPasteboard.changeCount` — poll only when the clipboard actually changed (**300 ms** interval).
- Read order: **files → raster → text** — when copying an image file in Finder, history gets file pixels, not the utility icon from the pasteboard.
- `image` crate: jpeg, webp, gif, bmp, tiff decoders for on-disk paths; screenshots and Copy Image still via raster API.
- Ignore concealed pasteboard (passwords and hidden content).
- Ignore Copyosity as source and apps from excluded list.
- `CaptureContext`, `try_capture_from_clipboard` — single clipboard content parsing entry point.

### Write, copy, and paste

- `clipboard_macos.rs` — pasteboard API, `changeCount`, concealed, synthetic Cmd+V, remember and restore target app before paste.
- `clipboard_write.rs` — unified **Copy** / **Paste** modes; `exclude_from_history` and own-entry mark so card copy doesn't duplicate history.
- `remember_paste_target` / `restore_paste_target` — double-click / Enter paste into the app from which the panel was opened.
- `copy_entry` / `activate_entry` — split between buffer-only and paste into another app.
- `paste_text_into_target` — shared paste path for `activate_entry` and `paste_entry`.
- Enter in main window — `activateEntry` for text and images.
- `check_accessibility(prompt)` — no system prompt on every paste; prompt only from Settings Request button.
- Settings window — correct bring-to-front (`objc2-app-kit`).
- Dependency `cocoa` replaced with `objc2` / `objc2-app-kit`.

### History dedup (phase 2)

Previously `content_hash = "{base}:{changeCount}"` broke dedup in `insert_entry`. Target behavior:

1. `content_hash` = **base_hash** only (text / raster / file).
2. On new `changeCount` — probe content hash; if it matches `last_content_hash` → do not write to DB.
3. Keep `mark_own_clipboard_write`, `should_ignore_capture`, `is_concealed`.
4. Reset `last_content_hash` when unpinned history is emptied (`clear_history` or deleting the last unpinned entry).

---

## 4. Frontend

- Main feed: Enter = paste selected entry (`activateEntry`) and close panel.
- Settings: Permissions block (Accessibility), hint about re-adding the app in Privacy after a new build.
- Card: single click — copy, double click — paste (this model unchanged).

| Action        | Implementation                              |
| ------------- | ------------------------------------------- |
| Single click  | `copy_entry` / `Copy`                       |
| Double click  | `activate_entry`                            |
| Enter + text  | `activate_entry` → `paste_text_into_target` |
| Enter + image | `activate_entry`                            |

---

## 5. Voice shortcut

Transcription puts text in the clipboard and simulates Cmd+V; shared macOS pasteboard logic in `clipboard_macos`. Voice flow uses `restore_paste_target` before Cmd+V, same as `activate_entry`.

---

## 6. Security and capabilities (phase 2)

| Topic              | Decision                                                                                         |
| ------------------ | ------------------------------------------------------------------------------------------------ |
| History duplicates | Identical content does not spawn entries; copy/paste from the app do not enter history           |
| Monitor interval   | **300 ms**; `changeCount` only triggers polling, not part of hash                                |
| Clipboard write    | Unified `clipboard_write.rs` — **Copy** / **Paste** modes                                        |
| Tauri ACL          | Separate capabilities per window (`main` / `settings` / `voice_overlay`; `palette` in **0.5.1**) |
| objc               | Migrate `clipboard_macos.rs` to **objc2**                                                        |
| Ollama             | Model name validation (allowlist/regex) before `ollama pull`                                     |
| Release CI         | `cargo audit` before artifacts ship                                                              |
| Perf               | Single `get_frontmost_app` per `file_list` batch; ~20 MB limit for `encode_image_file`           |

Per-window command scoping (approx.):

| Capability file                   | Window          | Commands (approx.)                                                  |
| --------------------------------- | --------------- | ------------------------------------------------------------------- |
| `capabilities/main.json`          | `main`          | entries, copy, activate, hide, events                               |
| `capabilities/settings.json`      | `settings`      | settings, excluded apps, clear_history, ollama, check_accessibility |
| `capabilities/voice_overlay.json` | `voice_overlay` | minimum (events)                                                    |

---

## 7. Affected repository areas

| Area          | Files                                                                                                                                                          |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Build         | `Makefile`, `README.md`, `scripts/build-macos.sh`, `macos-target.sh`, `env-rust.sh`, `with-tauri.sh`, `with-npm.sh`, `release-macos.sh`, `tauri.unsigned.json` |
| Config / deps | `.gitignore`, `.vscode/settings.json`, `package.json`, `package-lock.json`                                                                                     |
| Rust backend  | `clipboard_monitor.rs`, `clipboard_macos/`, `clipboard_write.rs`, `commands.rs`, `lib.rs`, `Cargo.toml`                                                        |
| UI            | `+page.svelte`, `settings/+page.svelte`, `ClipboardCard.svelte`                                                                                                |

---

## 8. Verification (as shipped)

`make check`; `cd src-tauri && cargo test`; `make build-macos-intel`.

Manual smoke:

- Twice Cmd+C same text — one history entry.
- Finder image file — pixels in history, not utility icon.
- Single-click copy — no new history entry.
- Double-click / Enter — paste into target app.
- Voice transcription — paste without history entry.
- Settings window — does not invoke paste commands.
