# Copyosity — Pre-release Test Plan

Full functional coverage for every shipping feature **except the Local AI (Ollama)
onboarding**, which is out of scope for this pass.

Each area lists:

- **Auto** — automated tests that must be green (`make check`).
- **Manual** — concrete steps on the dev stand (`npm run tauri dev`) with the
  expected result. Watch the terminal: the Rust side logs `[voice] …`,
  `copyosity: global shortcut registered = …`, etc.

Legend: ✅ automated · 👁 manual only (native / OS-integration).

---

## 0. Dev stand & build gate

- **Auto:** `make check` → backend **164** tests + frontend **198** tests, plus
  `cargo clippy -D warnings`, `svelte-check`, `oxlint`, `stylelint`, `oxfmt`.
  Frontend tests must run under `LC_ALL=en_US.UTF-8` (some assert `toLocaleString`).
- **Manual boot:** `npm run tauri dev`. Expect in the log:
  - `copyosity: global shortcut registered = true`
  - `[voice] registering shortcut: "option+space"`
  - no Rust panic, no `RUST_BACKTRACE`.

---

## 1. Clipboard history capture

- ✅ `overlay-entries-logic`, `overlay-entries-display`, `entry-tagged-payload`,
  `entry-ocr-payload`, backend `clipboard_monitor`, `db`.
- 👁 Copy text in any app → open overlay (`⌘⇧V`) → newest entry is on top.
- 👁 Copy an image / screenshot → appears as an image card with dimensions + size.
- 👁 Copyosity's own copy/paste does **not** create a new history entry.
- 👁 Copy from a "concealed"/transient pasteboard (e.g. a password manager) → ignored.

## 2. Overlay search & filters

- ✅ `overlay-search`, `overlay-filters`, `overlay-display-query`,
  `overlay-pagination`, `collection-tabs`, `card-tag-label`.
- 👁 `⌘F` or `/` focuses search; typing filters live.
- 👁 Format chips (PNG/GIF/JPG) and tag chips narrow results; counts update.
- 👁 Fast scroll to the bottom loads more without a visible glitch (pagination
  prefetch — see `overlay-pagination`).
- 👁 `Esc` clears the query first, then a second `Esc` hides the overlay
  (`overlayEscapeAction`).

## 3. History / Starred tabs & collections

- ✅ `collection-tabs`, `overlay-browse-sync`.
- 👁 Segmented control switches History ↔ Starred.
- 👁 Star (★) an entry → it appears under Starred; unstar removes it.
- 👁 Collection pills with color dots filter to that collection.

## 4. Image clipboard & OCR

- ✅ `image-meta`, `image_format` (backend), `ocr` (backend).
- 👁 Copy a screenshot with text → OCR preview text shows under the thumbnail.
- 👁 Search for a word that only appears **inside** the image → the image card matches.
- 👁 Animated GIF shows a GIF badge and animates.

## 5. App exclusions

- ✅ `exclusion-label`, backend `app_exclusion`.
- 👁 Settings → add an app to the exclusion list → copies from it are not captured.
- 👁 With a sensitive app frontmost, the overlay offers **Exclude [App]**.

## 6. Voice to text ⚠️ regression-fixed area

- ✅ backend `transcription`, `whisper` (429 → tariff hint), hub `no_think`/`format_hub_error`.
- 👁 **Crash fix:** hold `⌥Space`, speak, release. Transcript is pasted into the
  frontmost app **and** the app **stays running** (previously it crashed right
  after transcription because `NSPanel.hide()` ran off the main thread — now hopped
  to the main thread). Verify Copyosity is still alive in the log / menu bar.
- 👁 Capsule shows a spinner + "Transcribing…" while the transcript is produced
  (`voice-transcribing`).
- 👁 The transcript also lands in clipboard history (source "Voice"), even if the
  paste into the target fails (3 clipboard-write retries).
- 👁 Toggle **Transcribe with NeuralDeep Hub** in Voice settings; a hub 429 shows
  the "raise your tariff" message rather than a raw error.
- 👁 Context-aware polishing cleans filler/punctuation for the target app.

## 7. Automatic tagging (hub)

- ✅ backend `agent`, hub error/no-think helpers.
- 👁 With the hub enabled, a new entry gets short tags shortly after capture.
- 👁 **Retag** re-runs tagging on an entry.

## 8. Command / agent palette ⚠️ new features

- ✅ `palette-window` (restore size/position), backend `palette_window`, `agent`,
  `macos_window::snapped_position` (edge-snap geometry).
- 👁 **Configurable shortcut:** default `⌘⇧Space`; change it in Settings →
  NeuralDeep → palette hotkey and confirm the new combo opens the palette (and the
  old one no longer does). Hub must be enabled.
- 👁 Web mode returns search results; Agent mode streams progress then a markdown answer.
- 👁 **Model dropdown** in the palette top bar (Agent mode) — pick a model; it
  persists and the agent uses it (defaults to the hub chat model).
- 👁 **Screenshot button** (camera, Agent mode) — toggle it on, ask a question →
  the agent receives a screenshot of the app that was frontmost when the palette
  opened (needs Screen Recording permission). The flag resets after each run.
- 👁 **Sticky-to-edges:** drag the palette near a screen edge → it snaps to the edge.
- 👁 **Transparency (glass) mode:** toggle in the top bar → the panel becomes more
  translucent; the choice persists.
- 👁 **History:** the clock button lists recent sessions; clicking one restores it;
  **Clear** empties the list.
- 👁 Insert / Copy / Close work; window is draggable, resizable, minimises to a dot.

## 9. Quick menu (Clipy-style)

- ✅ backend `quick_menu` build logic, `snippet-folders-ui`.
- 👁 `⌘⇧C` pops a native menu at the cursor.
- 👁 Recent history items 1–9 have number accelerators; overflow in submenus (≤100).
- 👁 Snippet folders appear as submenus; picking an item pastes into the app that
  was frontmost when the menu opened (two clicks, no overlay).
- 👁 **Edit Snippets…** opens Settings → Quick Menu.

## 10. Snippets

- ✅ `snippet-folders-ui`, backend `db` snippet CRUD (migration v4).
- 👁 Settings → Quick Menu: create/rename/delete folders and snippets; inline rename.
- 👁 Destructive delete uses the styled confirm dialog (not the native `confirm()`).

## 11. Native macOS actions

- 👁 Ask the agent to create a Note → it appears in Notes.
- 👁 Create / list Reminders; read upcoming Calendar events (Apple Events prompt
  appears on first use — needs the `apple-events` entitlement).

## 12. Smart paste

- ✅ backend `paste`, `clipboard_write`.
- 👁 Single click copies; Enter / Space / double-click / Paste button pastes into
  the app that was frontmost when the overlay opened.
- 👁 Paste works into Messages, Electron apps, and native targets.

## 13. Privacy — clear history

- ✅ `destructive-actions` (confirm copy for unpinned/all, singular/plural, pinned note).
- 👁 Settings → History → **Clear unpinned** keeps pinned items; **Clear all**
  warns it includes N pinned items; both require confirmation.

## 14. Overlay dismiss behaviour ⚠️ new behaviour

- ✅ `overlay_dismiss` pure helpers (`point_in_screen_rect`, `within_show_grace`,
  `dismiss_suppressed`).
- 👁 Click outside the panel → dismisses.
- 👁 Switch Space (gesture) → panel **stays** (space-change grace).
- 👁 `⌘↑` while the overlay is visible → hides it.
- 👁 **New:** `⌘Tab` / click another window / the Dock → overlay **hides**
  (`install_app_switch_dismiss`; Copyosity's own activations are ignored).

## 15. Settings window ⚠️ regression-fixed area

- 👁 **Version fix:** Settings → Updates shows `Current: <version>` (not `Current: …`).
  Root cause was the missing `core:app` capability on the **settings** window.
- 👁 **Icon + persistence fix:** opening Settings switches the app to `Regular`
  activation → a Dock icon appears and the window no longer vanishes when focus
  changes; closing Settings reverts to `Accessory` (menu-bar-only, no Dock icon).
- 👁 Every pane loads: NeuralDeep, Voice, Quick Menu, Local AI, History, Permissions, Updates.

## 16. Updates / auto-update ⚠️ regression-fixed area

- ✅ `updater.ts` exports (`autoUpdateOnLaunch`, `notify`, `checkForUpdate`).
- 👁 Settings → Updates → **Check now** reports the current/next version.
- 👁 **Download & install** downloads the signed update **and restarts** into it
  (needs the `process` capability that `relaunch()` requires — the earlier
  "restart does nothing / Install failed" bug).
- 👁 On launch with a newer release published, a native notification appears.
- 👁 Endpoint check: `curl -sL <releases/latest>/latest.json` returns the newest
  version for `darwin-aarch64` and `darwin-x86_64`.

## 17. Accessibility

- 👁 Keyboard-only: Tab / arrows show focus rings (`input-modality`); mouse clicks do not.
- 👁 Reduced-motion, reduced-transparency, and increased-contrast system settings
  are respected.
- 👁 Voice HUD announces via `aria-live`.

---

## Sign-off checklist (before a release)

- [ ] `make check` green (backend 160 + frontend 198, clippy/lint/format clean).
- [ ] Dev stand boots with no panic; global shortcut registered.
- [ ] §6 voice: transcript delivered **and app survives** (crash regression).
- [ ] §15 settings: version shows, Dock icon appears, window persists on focus change.
- [ ] §16 updates: install **restarts** into the new version.
- [ ] §14 overlay hides on app switch.
- [ ] DMGs signed + notarized + stapled (`spctl -a -t open` → _Notarized Developer ID_).
- [ ] `latest.json` signature = raw `.sig`; endpoint serves the new version.
