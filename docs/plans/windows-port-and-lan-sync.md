# Windows port (visual parity) + LAN sync — plan

Status: **in progress, experimental, not for release.** Branch `feat/windows-port-lan-sync`
(from `main` 1.0.4). Nothing from this plan ships through `scripts/release.sh`, the updater
or the landing page until the owner says so. macOS behaviour must not change.

## Goals

1. **Windows build that looks and behaves like the Mac app.** Same windows, same layout,
   same animations, same settings. The only intended differences are the ones Windows
   forces: key names (`Ctrl` instead of `⌘`, `Alt` instead of `⌥`) and system-drawn bits
   we cannot repaint (title-bar-less windows are fine, the tray lives in the notification
   area instead of the menu bar).
2. **LAN sync between Macs and the Windows PC** as an experimental settings section:
   devices find each other on the local network, pair once with a code, then clipboard
   history, pins and snippets flow both ways. LAN only, end-to-end encrypted, no cloud.
3. Test setup: the MacBook Air and the Windows PC both run this branch's build.
   The Air currently has the released 1.0.3; for the test it gets a dev build of this
   branch installed by hand (not via the updater).

## Part A. Windows port

### A1. Native layer: macOS module → Windows equivalent

| area                        | macOS today                                | Windows implementation                                                                                                                                                                                       |
| --------------------------- | ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| clipboard watch             | NSPasteboard `changeCount` poll            | `AddClipboardFormatListener` on a message-only window (`WM_CLIPBOARDUPDATE`), `GetClipboardSequenceNumber` as a cheap guard                                                                                  |
| clipboard read              | NSPasteboard types                         | `CF_UNICODETEXT`, `HTML Format`, `CF_DIBV5`/`PNG`, `CF_HDROP` (files)                                                                                                                                        |
| clipboard write             | NSPasteboard + concealed/transient markers | `SetClipboardData`; mark our own writes with `ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory=0`, `CanUploadToCloudClipboard=0` so Win+V history and cloud clipboard stay clean |
| paste into the previous app | AX / Cmd+V via CGEvent                     | remember the foreground `HWND` before the overlay opens, restore it (`AttachThreadInput` + `SetForegroundWindow`), then `SendInput` Ctrl+V. Start from `win_platform.rs` on `prerelease/windows-port`        |
| source app + icon           | NSWorkspace frontmost app, bundle id       | `GetForegroundWindow` → PID → `QueryFullProcessImageNameW`; icon via `SHGetFileInfoW` → PNG; exe name plays the role of bundle id                                                                            |
| app exclusions              | bundle ids                                 | executable names/paths, same table and UI                                                                                                                                                                    |
| overlay / palette panels    | NSPanel, non-activating, window levels     | Tauri window `always_on_top`, `skip_taskbar`, `WS_EX_TOOLWINDOW`, `WS_EX_NOACTIVATE` where the Mac panel does not take focus; hide on focus loss; open on the monitor with the cursor                        |
| background material         | NSVisualEffectView                         | Acrylic (Win10 1809+) / Mica (Win11) via `window-vibrancy`; tint tuned to match Mac screenshots; solid fallback                                                                                              |
| window corners and shadow   | macOS default                              | `DWMWA_WINDOW_CORNER_PREFERENCE=ROUND` on Win11; transparent window + CSS radius on Win10                                                                                                                    |
| tray                        | NSStatusItem (`tray_macos.rs`)             | Tauri tray icon, same menu items and order                                                                                                                                                                   |
| global hotkeys              | Cmd-based                                  | same shortcuts with Ctrl; settings show `Ctrl`/`Alt`/`Win`                                                                                                                                                   |
| autostart                   | login item                                 | `tauri-plugin-autostart` (registry Run key)                                                                                                                                                                  |
| OCR                         | Vision `VNRecognizeTextRequest`            | `Windows.Media.Ocr` (WinRT via `windows` crate), same command and result shape                                                                                                                               |
| permissions onboarding      | Accessibility, Screen Recording            | not needed on Windows; those onboarding steps are skipped, no other UI change                                                                                                                                |

The exact list of modules, `cfg(target_os = "macos")` blocks and current stubs is in
**Inventory** below (filled from the code audit).

### A2. Visual parity

- One Svelte UI for both systems; no separate Windows layout.
- **Font.** The UI uses the system font, which is SF Pro on the Mac. SF Pro may not be
  redistributed to Windows. Plan: bundle **Inter** (OFL) and use it on Windows only, with
  size/letter-spacing tuned so line breaks match the Mac screenshots. Open question for the
  owner: use Inter on both systems for a pixel-identical look?
- **Emoji.** Segoe UI Emoji differs from Apple Color Emoji. Acceptable, or bundle a colour
  emoji font (size cost ~10 MB). Owner decision, default: system emoji.
- **Scrollbars, focus rings, selection colour, form controls:** styled in CSS so the
  WebView2 defaults never show.
- **Key labels:** one helper maps the shortcut model to `⌘⌥⇧⌃` on macOS and
  `Ctrl Alt Shift Win` on Windows; every place that prints a shortcut goes through it.
- **Check:** a screenshot set of the same screens (overlay with history, palette, quick
  menu, settings sections, snippets editor, empty states) on the Mac and on Windows,
  compared side by side with a pixel diff report. Parity is done when the only diffs are
  text antialiasing, key labels and emoji.

### A3. Build and delivery

- CI already has `windows-build.yml` (installer artifact) and a `build-windows` check.
  Builds of this branch come from `workflow_dispatch` artifacts only: no `win-v*` tags,
  no GitHub release, no `latest.json`, so no auto-update reaches anyone.
- Unsigned build: SmartScreen will warn on first launch of the test PC. Signing is a
  release-time question, not in scope.

## Part B. LAN sync (experimental)

### B1. UX

- Settings → new section **«Синхронизация (эксперимент)»**, shown only when the build has
  the `lan-sync` feature and the experimental toggle is on.
- The section lists devices found on the network (name, OS, last seen), a **Pair** button,
  paired devices with status (online, synced N min ago, errors) and per-type switches:
  clipboard history, pins, snippets, images.
- Pairing: device A shows a 6-digit code, device B enters it. After that the pair is
  remembered; unpair from either side.

### B2. Protocol

- **Discovery:** mDNS service `_copyosity._tcp.local` (`mdns-sd` crate), TXT record with
  device id, name, OS and protocol version.
- **Pairing:** SPAKE2 over the 6-digit code (`spake2` crate) gives a shared secret without
  sending the code; each side stores the peer's device id and long-term key in the OS
  credential store (Keychain / Windows Credential Manager).
- **Transport:** TCP on a fixed port (47900, configurable), every frame encrypted with
  ChaCha20-Poly1305 under the paired key. Peers outside private address ranges
  (RFC 1918, link-local) are refused. No relay, no internet.
- **Data model:** reuse the cloud sync design from `docs/architecture/sync-api.md` §4–6:
  stable `entry_uuid`, `content_hash` for dedup, `kind`, last-writer-wins by `client_ts`,
  tombstones for deletes, a per-peer cursor. Local DB migration adds `entry_uuid`,
  `updated_at`, `deleted`, `origin_device` to entries and snippets.
- **Flow:** on connect, both sides exchange cursors and send what the other has not seen;
  afterwards new copies are pushed live, so a copy on one machine appears in the other's
  history within a second. Images over 20 MB stay local.
- **Loop safety:** entries that arrive from a peer are stored but not written to the system
  clipboard, and are not re-broadcast to the device they came from.

### B3. Scope guard

- Cargo feature `lan-sync`, off by default; `release.sh` never enables it.
- Windows Defender Firewall will prompt on the first listen; the section explains it.

## Stages

| #   | stage                                                         | done when                                    |
| --- | ------------------------------------------------------------- | -------------------------------------------- |
| 0   | branch, plan, inventory, CI Windows build of this branch      | installer artifact from CI                   |
| 1   | Windows clipboard watch + read/write + DB                     | copies on the PC appear in history           |
| 2   | overlay/palette windows, hotkeys, tray, paste-back, autostart | daily flow works on the PC                   |
| 3   | visual parity pass + screenshot diff                          | only allowed diffs remain                    |
| 4   | source app icons, exclusions, OCR, remaining features         | feature list matches the Mac                 |
| 5   | LAN sync: discovery + pairing                                 | Air and PC see and pair with each other      |
| 6   | LAN sync: text, pins, snippets, then images                   | copy on one, paste from history on the other |
| 7   | test on Air + PC for a few days, fix, decide on release       | owner decision                               |

## Progress

| date  | done                                                                                                                                                                                      |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 11.10 | plan + inventory; paste-back via SendInput (from `prerelease/windows-port`)                                                                                                               |
| 11.10 | stage 1: clipboard change gate (`GetClipboardSequenceNumber`), own-write marker, password-manager markers, Copy writes kept out of Win+V history                                          |
| 11.10 | stage 2 (part): overlay on top while open, closes on focus loss; `simulate_cmd_v` pastes on Windows                                                                                       |
| 11.10 | stage 4 (part): source app = exe name + FileDescription, app exclusions enabled on Windows                                                                                                |
| 11.10 | key labels: `⌘⇧Space` → `Ctrl+Shift+Space` on Windows via `keyLabel()`                                                                                                                    |
| 11.10 | stages 5–6 (code): `lan_sync` module (triggers + `sync_log`, SPAKE2 pairing, ChaCha20-Poly1305 channel, mDNS), Settings → Sync (beta); 7 unit tests; not yet run between two real devices |

## Open questions

1. Font: Inter on Windows only, or on both systems?
2. Emoji: system or bundled?
3. Windows PC access for testing: the home tunnels (`pc-home`, `pc-direct`) were down on
   11.10; builds come from CI until the PC is reachable.
4. Windows 10 or 11 on the PC (affects Mica vs Acrylic and corners).

## Inventory (code audit 11.10, anchors from `main` aad3c2b)

What is macOS-only today and what Windows currently gets:

| module / area                                  | macOS                                                                         | Windows today                                                                                 | work                                                                                                                                                                                                                            |
| ---------------------------------------------- | ----------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `clipboard_monitor.rs`                         | `changeCount` gate, skips own writes and concealed items                      | re-hashes the whole clipboard every 300/750 ms (:650); own writes not suppressed              | WM_CLIPBOARDUPDATE listener, own-write marker                                                                                                                                                                                   |
| `clipboard_write.rs`                           | arboard + exclude-from-history, own-write marker, GIF as file URL             | plain `set_text`/`set_image`; GIF write fails                                                 | history/cloud exclusion formats, marker, GIF via CF_HDROP or PNG                                                                                                                                                                |
| `clipboard_macos/{mod,accessibility,paste}.rs` | NSPasteboard, AX focus/paste, CGEvent Cmd+V, osascript fallback               | `accessibility_trusted` = true, paste module absent                                           | paste via `win_platform` (exists on `prerelease/windows-port`, now cherry-picked)                                                                                                                                               |
| `lib.rs` `simulate_cmd_v`                      | CGEvent Cmd+V                                                                 | empty stub (:2010)                                                                            | route to `win_platform::spawn_automated_paste` (palette insert, voice paste)                                                                                                                                                    |
| `lib.rs` selection capture                     | Cmd+C via CGEvent (:2018–2085)                                                | None                                                                                          | SendInput Ctrl+C + clipboard read                                                                                                                                                                                               |
| `macos_app.rs`                                 | frontmost app identity, names                                                 | `frontmost_app_identity` = None, so `source_app` is NULL and exclusions never match           | foreground HWND → exe path/name, icon                                                                                                                                                                                           |
| `app_exclusion.rs`                             | NSOpenPanel picker                                                            | `Ok(None)`                                                                                    | file dialog for `.exe`                                                                                                                                                                                                          |
| main overlay (`lib.rs:804`)                    | NSPanel, non-activating, level 24, docked bottom                              | plain `show()`+`set_focus()` (steals focus), not on top                                       | `WS_EX_NOACTIVATE`/`TOOLWINDOW`, topmost, keep target app focused                                                                                                                                                               |
| `overlay_dismiss.rs`                           | global mouse monitor, Cmd+↑, Space/app notifications                          | no-op stubs, so the overlay never closes on click-outside                                     | low-level mouse hook or focus-loss handling                                                                                                                                                                                     |
| command palette (`lib.rs:1605/1650`)           | NSPanel                                                                       | never opens                                                                                   | Tauri window with same flags                                                                                                                                                                                                    |
| voice overlay (`lib.rs:1421`)                  | NSPanel HUD                                                                   | never created                                                                                 | topmost no-activate window                                                                                                                                                                                                      |
| `quick_menu.rs`                                | NSMenu popup at cursor                                                        | no-op                                                                                         | `TrackPopupMenu` or a small webview menu                                                                                                                                                                                        |
| `tray_macos.rs`                                | menu-bar blink fix                                                            | stubs                                                                                         | Tauri tray default is fine on Windows                                                                                                                                                                                           |
| `ocr.rs`                                       | Vision                                                                        | None                                                                                          | `Windows.Media.Ocr`                                                                                                                                                                                                             |
| `screen.rs`                                    | screencapture, CGWindowList                                                   | callers gated                                                                                 | Graphics Capture / BitBlt for agent screenshots                                                                                                                                                                                 |
| `mactools.rs`                                  | Notes, Reminders, Calendar via osascript                                      | `Err("macOS only")`                                                                           | no Windows equivalent; hide those agent tools (functional, not visual difference)                                                                                                                                               |
| `ollama.rs`                                    | —                                                                             | Unix paths, 8 GB memory guess, console windows flash                                          | `GlobalMemoryStatusEx`, Windows paths, `CREATE_NO_WINDOW`                                                                                                                                                                       |
| whisper / transcription, hub, agent, tagging   | cross-platform                                                                | works                                                                                         | HUD + paste-back only                                                                                                                                                                                                           |
| blur                                           | CSS `backdrop-filter` over a transparent window (WKWebView blurs the desktop) | WebView2 cannot blur the desktop                                                              | **native Acrylic/Mica**, biggest visual risk                                                                                                                                                                                    |
| fonts                                          | `-apple-system`, SF Mono                                                      | Segoe UI, generic monospace                                                                   | bundle fonts (see A2)                                                                                                                                                                                                           |
| hotkeys                                        | Cmd-based                                                                     | Cmd→Win key; voice `option+space` clashes with Alt+Space; palette placeholder hardcoded `cmd` | Windows defaults: Ctrl+Shift+V, Ctrl+Shift+Space, Ctrl+Shift+C, voice off Alt+Space                                                                                                                                             |
| hardcoded `⌘` in UI                            | —                                                                             | shown on Windows                                                                              | `ClipboardCard.svelte:435`, `+page.svelte:85,95,1329,1396`, `palette/+page.svelte:300–302,868`, `settings/+page.svelte:1176,1834`, tray "Agent Search ⌘⇧Space" (`lib.rs:1065`); Cmd+↑ hide is metaKey-only (`+page.svelte:847`) |
| `platformIsMacOS()` gates                      | —                                                                             | hides palette and exclude buttons, Permissions pane, Excluded apps                            | un-gate as Windows backends land                                                                                                                                                                                                |
| app icons                                      | `source_app_icon` never filled on any platform                                | —                                                                                             | optional, same on both                                                                                                                                                                                                          |
| i18n                                           | "Mac" in launch-at-login hints (`i18n/dict.ts:191,246,355`)                   | —                                                                                             | neutral wording                                                                                                                                                                                                                 |

Sync hooks: insert `db.rs:900` (dedupe by hash; duplicate bumps `created_at`), delete
`:1221`, clear `:1419/:1425`, retention `:1814`, pin `:1232`, collection `:1241`, tags
`:1511`, OCR `:1664`, snippets `:1347–1413`. Entries and snippets have integer ids only,
no UUID/updated_at/tombstones: the migration in B2 is required.
