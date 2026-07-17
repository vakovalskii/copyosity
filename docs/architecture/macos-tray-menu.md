# macOS menu-bar tray

Copyosity is a menu-bar (`Accessory`) app. The tray menu **blinks** (opens and instantly
dismisses) when AppKit activation, hidden window z-order, and `NSStatusItem` menu popup race
each other. This document is the **single source of truth** for the fix that works — read it
before changing tray startup, `tray_macos.rs`, window levels, or activation policy.

**Smoke gate:** `make verify-tray` on a real Mac (Accessibility for System Events). Exit code
`2` = automation unavailable (inconclusive), not a pass.

**Startup log (must match):** `[tray] startup: hidden main + deferred tray popup ready`

---

## Why this is hard

Three separate macOS/Tauri issues stack on top of each other:

1. **Tauri default menubar** — `.enable_macos_default_menu(true)` (the default) attaches a full
   Apple/File/Edit menubar to the hidden webview. That menubar fights `NSStatusItem` popups and
   causes unpredictable blink (sometimes 1st click, sometimes 2nd).

2. **`tray-icon` `performClick` bug** — In crates.io `tray-icon` 0.24.1, left-click uses
   `button.performClick(None)` instead of `popUpStatusItemMenu`. After the first open, repeat
   clicks can blink. We patch to [tray-icon#318](https://github.com/tauri-apps/tray-icon/pull/318)
   (`8cd6dce`). **Patch alone is not enough** for Accessory apps — see (3).

3. **Accessory activation race** — Menu-bar-only apps use `ActivationPolicy::Accessory`. When the
   user clicks the status item, AppKit activates the app and tries to show the menu in the **same
   event cycle**. The menu opens and is dismissed immediately. The fix is a **deferred popup** on
   the next run-loop turn (classic LSUIElement workaround), plus one-time warmup activation.

4. **Hidden window z-order** — The clipboard overlay `main` webview exists at startup (Handy
   pattern; `tauri.conf.json` has `"windows": []`). It must stay at **`HIDDEN_AUXILIARY_LEVEL`
   (3)** — below status-bar menu popups. Converting to NSPanel or raising to level 24 **at
   startup** regresses the first tray click. NSPanel conversion is **lazy** (first overlay show).

**Critical insight from debugging:** partial fixes alternate which click breaks:

| Approach tried                                | 1st click                   | 2nd+ click                  |
| --------------------------------------------- | --------------------------- | --------------------------- |
| Plain `show_menu_on_left_click(true)` + patch | Often OK                    | **Blinks** (`performClick`) |
| Hidden main, no deferred popup                | **Blinks**                  | Often OK                    |
| Tray before main, defer main to async         | App won't start / no window | —                           |
| **Full scheme below**                         | OK                          | OK                          |

Only the **combined** scheme below passed 5 consecutive clicks in `tauri dev` (user-confirmed).

---

## Working scheme (do not regress)

Apply **all** of these together. Removing or “simplifying” any one item tends to break a
different click.

### 1. Disable Tauri default menubar

```rust
builder.enable_macos_default_menu(false)  // lib.rs, before .setup()
```

**If removed:** Apple/File/Edit appear; tray menu blinks unpredictably.

### 2. `Accessory` before tray

```rust
app.set_activation_policy(ActivationPolicy::Accessory);  // first in TRAY STARTUP
```

**If moved after tray or removed:** Dock flash or activation races at startup.

### 3. Hidden `main` before tray (sync, main thread)

```rust
ensure_main_overlay_window(app.handle())?;  // before TrayIconBuilder
```

- Created in `setup` on the **main thread** (not async defer — async breaks `make dev`).
- `visible(false)`, level **3** via `macos_window::apply_hidden_auxiliary_webview`.
- `tauri.conf.json`: `"windows": []` — window is programmatic only.

**If tray is created before main:** devUrl/WebKit startup can still race; order above is
verified.

**If main creation is deferred to async:** window creation fails or app exits; `make dev` breaks.

### 4. Deferred tray popup (macOS only)

```rust
.show_menu_on_left_click(false)
.on_tray_icon_event(|tray, event| { /* Left + Down */ })
```

On left mouse **Down**:

1. `tray_macos::set_tray_highlight(tray, true)` — immediate pressed look.
2. `tray_macos::schedule_tray_menu_popup(tray)` — 1 ms tokio sleep, then `run_on_main_thread`:
   `highlight(true)` → `inner.show_menu()` → `highlight(false)`.

**Paste target on tray Down (lightweight only):** use
`clipboard_macos::note_tray_menu_paste_target()` — stores the frontmost non-self PID in
`LAST_NON_SELF_FRONTMOST_PID` for quick-menu fallback. Do **not** call `remember_paste_target()`
or `remember_paste_target_for_pid()` here: those walk Accessibility (AX focus, mouse position)
and run in the **same event cycle** as the deferred popup, which regresses the **1st click**
(`ccc9504` / post-merge App Store builds).

Full paste capture (`remember_paste_target_for_pid`) stays in `quick_menu::show` when the user
picks **Snippets** from the tray menu — that path runs after the menu is already open.

Implementation: [`tray_macos.rs`](../../src-tauri/src/tray_macos.rs),
[`clipboard_macos/mod.rs`](../../src-tauri/src/clipboard_macos/mod.rs) (`note_tray_menu_paste_target`).

**If switched to `show_menu_on_left_click(true)`:** 2nd/3rd click blink returns.

**If defer removed but `show_menu_on_left_click(false)` kept:** 1st click blinks.

**If highlight re-assert before `show_menu()` removed:** menu works but icon looks unpressed.

**If any AX / panel / activation work added to the Down handler:** 1st click blinks (keep Down
handler to highlight + schedule popup + PID note only).

### 5. Warmup activation

```rust
tray_macos::warmup_app_for_status_item_menu();  // after tray built
```

Calls `NSApplication::activateIgnoringOtherApps(true)` once at startup.

**If changed to `activate()` or `activateIgnoringOtherApps(false)`:** 1st click blinks (review
regression observed in practice).

### 6. `tray-icon` git patch

[`Cargo.toml`](../../src-tauri/Cargo.toml):

```toml
[patch.crates-io]
tray-icon = { git = "...", rev = "8cd6dce9bae905069416e524a077a3a9d7a6b451" }
```

**If removed:** `performClick` path returns; repeat clicks blink even with deferred popup.

### 7. Lazy NSPanel

`ensure_main_overlay_panel` runs on **first overlay show**, not in `setup`.

**If main → NSPanel at startup:** hidden panel competes with status item; 1st click blinks.

### 8. Do not pre-create auxiliary panels in `setup`

`voice_overlay` / `command_palette` are lazy (first hotkey). Pre-creating NSPanels after tray
disturbs z-order and regressed tray in review.

### 9. Activation policy restore

[`activation_macos.rs`](../../src-tauri/src/activation_macos.rs): `Regular` only for Settings;
`maybe_restore_accessory` when overlay hides **if** settings window still exists (`is_some()`,
not `is_visible()`).

**If restore uses `is_visible()`:** closing settings can leave app stuck in `Regular`.

### 10. Non-macOS

Plain `.show_menu_on_left_click(true)` — no `tray_macos` handler.

### 11. Startup webview stays minimal (no extra AppKit on hidden `main`)

`macos_window::apply_hidden_auxiliary_webview` (called from `ensure_main_overlay_window` at startup)
may only set collection behavior, level **3**, and `hidesOnDeactivate`. Do **not** call
`apply_transparent_webview` (WKWebView tree walk) or `to_panel` there — release/DMG transparency
fixes belong in **lazy** panel paths (`ensure_main_overlay_panel`, voice/palette first create).

**If `apply_transparent_webview` runs at startup on hidden main:** 1st click blink (touches
NSWindow during the tray click window).

### 12. `tauri.conf.json` must not declare `main`

Keep `"windows": []`. A static `main` entry (especially with `"alwaysOnTop": true`) creates a
second startup path that fights the programmatic hidden window and regressed tray after upstream
merges.

**If `main` reappears in `tauri.conf.json`:** unpredictable 1st/2nd click; remove it and rely on
`ensure_main_overlay_window` only.

---

## Post-merge / upstream drift (repeated regressions)

When merging from upstream or App Store release branches, diff **`TRAY STARTUP`** in
[`lib.rs`](../../src-tauri/src/lib.rs) first. These upstream patterns **fully break** the fix and
have landed multiple times (`cb8563c`, `069689b` era):

| Upstream “simplification”                                       | Symptom          |
| --------------------------------------------------------------- | ---------------- |
| `app.get_webview_window("main").unwrap().to_panel()` in `setup` | 1st click blink  |
| `panel.set_level(24)` (or `PANEL_LEVEL_ACTIVE`) at startup      | 1st click blink  |
| Plain `TrayIconBuilder::…build()` without `tray_macos` defer    | 2nd+ click blink |
| No `enable_macos_default_menu(false)`                           | Menubar / blink  |
| `ensure_voice_overlay` / `ensure_command_palette` in `setup`    | 1st click blink  |
| `remember_paste_target()` on tray Down                          | 1st click blink  |

After every merge that touches `lib.rs` setup or `macos_window.rs`, run **`make verify-tray`** or
manual 5 clicks in `make dev` before shipping.

---

## Key files (agent map)

| File                                                                   | Role                                                                  |
| ---------------------------------------------------------------------- | --------------------------------------------------------------------- |
| [`lib.rs`](../../src-tauri/src/lib.rs)                                 | `TRAY STARTUP` block, `ensure_main_overlay_window`, lazy panel        |
| [`tray_macos.rs`](../../src-tauri/src/tray_macos.rs)                   | Deferred popup, highlight, warmup                                     |
| [`activation_macos.rs`](../../src-tauri/src/activation_macos.rs)       | Accessory ↔ Regular                                                   |
| [`macos_window.rs`](../../src-tauri/src/macos_window.rs)               | `HIDDEN_AUXILIARY_LEVEL` (3) vs 24                                    |
| [`clipboard_macos/mod.rs`](../../src-tauri/src/clipboard_macos/mod.rs) | `note_tray_menu_paste_target` (tray Down); full capture in quick menu |
| [`Cargo.toml`](../../src-tauri/Cargo.toml)                             | `tray-icon` patch, `NSStatusItem` features                            |
| [`tauri.conf.json`](../../src-tauri/tauri.conf.json)                   | `"windows": []`                                                       |
| [`verify-tray-startup.sh`](../../scripts/verify-tray-startup.sh)       | Debug binary, 3 clicks                                                |
| [`verify-tray-dev.sh`](../../scripts/verify-tray-dev.sh)               | `tauri dev` + Vite, 3 clicks                                          |

---

## Do not reintroduce (review regressions)

| Change                                                                    | What breaks                                          |
| ------------------------------------------------------------------------- | ---------------------------------------------------- |
| Plain `show_menu_on_left_click(true)` on macOS                            | 2nd/3rd click blink                                  |
| Remove deferred popup (`tray_macos`)                                      | 1st click blink                                      |
| `warmup`: `activate()` / `activateIgnoringOtherApps(false)`               | 1st click blink                                      |
| Remove highlight re-assert in deferred popup                              | Icon not highlighted while menu open                 |
| Defer `ensure_main_overlay_window` to async                               | `make dev` / window creation                         |
| Re-pin window level in a startup loop                                     | 1st click blink (touches window during click window) |
| Tray-only startup (no hidden main)                                        | 2nd click blink                                      |
| Remove `enable_macos_default_menu(false)`                                 | Menubar pollution / blink                            |
| Pre-create voice/palette NSPanels in `setup`                              | Tray z-order / 1st click                             |
| Main → NSPanel at startup                                                 | 1st click blink                                      |
| Remove `tray-icon` patch                                                  | Repeat-click `performClick` bug                      |
| `remember_paste_target()` on tray Down (AX in click handler)              | 1st click blink                                      |
| `remember_paste_target_for_pid()` on tray Down                            | 1st click blink (same AX race)                       |
| `apply_transparent_webview` in `apply_hidden_auxiliary_webview` (startup) | 1st click blink                                      |
| Static `main` window in `tauri.conf.json` (`alwaysOnTop`, etc.)           | Z-order / activation races                           |
| Revert `TRAY STARTUP` to pre-`0bfce42` (to_panel in setup)                | 1st click blink                                      |
| `set_level(24)` on hidden main or hidden panel after hide                 | 1st click blink                                      |
| `dismiss_main_panel_if_visible` / `hide_command_palette` on tray Down     | Menu dismiss race / blink                            |
| `activate()` / `activateIgnoringOtherApps` in tray Down handler           | 1st click blink                                      |
| Merge upstream without re-reading this doc                                | Often reintroduces rows above                        |

---

## Agent checklist

Before merging tray/overlay/activation changes:

1. Read this doc end-to-end.
2. `make fix-backend && make check-backend`
3. `make verify-tray` on a GUI Mac (or at minimum `verify-tray-startup` + manual 5 clicks in
   `make dev`). The smoke script checks Accessory activation-policy at startup and runs 5 clicks
   (3 tight at 0.1 s, 2 slower at 0.3 s) to catch both load-induced and repeat-click regressions.
4. Confirm log: `hidden main + deferred tray popup ready`.
5. Do **not** “simplify” by dropping `tray_macos` or switching to plain tray — that was tried
   multiple times and always regressed one click or the other.
6. Tray Down handler: **highlight + deferred popup + `note_tray_menu_paste_target` only** — no AX,
   no panel hide/show, no activation calls.
7. After upstream merge: grep `lib.rs` for `to_panel` in `setup`, `remember_paste_target` in tray
   handler, and `"windows": [` in `tauri.conf.json`.
8. `ensure_main_overlay_panel` idempotency is guaranteed by the `AtomicBool` early-return and
   enforced at runtime via `debug_assert!(MainThreadMarker::new().is_some())`. A unit test is
   impractical without a full Tauri runtime — the code comment is the authoritative doc.
