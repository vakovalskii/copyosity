# Quick Look preview

**Status: shipped.** Launcher-style full entry preview for the clipboard overlay.

**Related:** [audit-hig.md](audit-hig.md) item 14 (native `title` removed; Quick Look is the replacement) · backlog [features-backlog.md](features-backlog.md) · [feature-overlay-content-tag-filters.md](feature-overlay-content-tag-filters.md)

---

## Why

Card previews are intentionally truncated; users still need to read full text or inspect an image before pasting. The HTML `title` tooltip was removed because it is slow, ill-suited to reading full clipboard entries, and conflicts with keyboard actions. macOS users already know **Space → preview** from Finder Quick Look — we borrow that keyboard mnemonic (plus **⌘Y**, as in Raycast/LaunchBar) while matching the interaction model launcher-style tools use for detail/preview panes: an in-window dialog, not a second OS-level window.

## Goal

When a card is selected in the clipboard overlay, **Space** or **⌘Y** opens a dedicated **Quick Look** view: larger, readable, non-destructive inspection of the entry. **Paste remains a separate action** (Return, double-click, Paste toolbar button). Preview never copies or pastes — it is inspect-only in v1 (see [Non-goals](#non-goals)).

## Keyboard model (breaking change from before)

| Key            | Before                           | After Quick Look                                                                                                                   |
| -------------- | -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `Space`        | Paste (via card `role="button"`) | Open / close Quick Look for the selected entry (not while search is focused)                                                       |
| `⌘Y`           | —                                | Open / close Quick Look — **works from search** and elsewhere on the overlay                                                       |
| `↓` / `→`      | Browse cards                     | From **search focus**: blur search and select the first visible card (`↓` always; `→` on horizontal board too), then normal browse |
| `↵` / `Enter`  | Paste                            | Paste (unchanged)                                                                                                                  |
| `Double-click` | Paste                            | Paste (unchanged)                                                                                                                  |
| `Esc`          | Clear search / dismiss overlay   | If Quick Look open → close preview first; else existing overlay behavior                                                           |

### Implementation notes

- `Space` removed from `ClipboardCard` `handleCardKeydown` (Enter-only now); `Space` and `⌘Y` handled centrally in `+page.svelte`'s window-level `handleKeydown`. `Space` is guarded so it never fires while typing in a field/search or when focus is on a `<button>` (filter chips, segment controls, header icons keep native Space-to-click — see [feature-overlay-content-tag-filters.md](feature-overlay-content-tag-filters.md)). `⌘Y` **is not blocked in search** — preview without leaving the query.
- **Search → grid:** `↓` (vertical board) or `↓` / `→` (horizontal board) while the search field is focused blurs search, sets `selectedIndex` to the leading visible card (or `0`), and focuses that card.
- **Scroll sync:** after trackpad scroll idle / `scrollend`, selection syncs to the leading visible card on the **horizontal** board (`Space` previews what you scrolled to). **Vertical board:** trackpad sync is disabled — `scroll-snap-y` + selection updates caused scroll jank; use `↑`/`↓` or tap a card to change selection before `Space`.
- Footer hints (`KeyboardHints` in `+page.svelte`) and [audit-hig.md](audit-hig.md) item 19 list `Space preview` and `⌘Y preview` ahead of `↵ paste`.
- **Mouse:** `ClipboardCard` exposes `onpreview`; type-chip eye button and card context menu call `handleCardPreview` in `+page.svelte` (select + open Quick Look). Card `onclick` copy is unchanged. Context menu renders **outside** `.card` (sibling under `display: contents` host) so `overflow: hidden` on the card does not clip it; secondary click also listens on `pointerdown` button 2 for WKWebView. `Esc` dismisses an open context menu before Quick Look, search clear, or overlay hide (`overlay-card-context-menu.ts` + `resolveOverlayEscapeAction` in `quick-look-keyboard.ts`).
- **Type-chip pill:** format/type label lives in `CardTypeBadge.svelte`; on card hover / keyboard selection the pill **collapses** via `clip-path` (240ms, compositor-friendly `calc()` — no per-frame layout reads) to reveal a 40px eye button underneath. Only the type **icon** crossfades with the eye; the text label stays at full opacity and is cropped away by `clip-path` so no “ghost text” sliver during collapse. Badge has `pointer-events: none` so the eye button keeps hover/active HIG styling (`--surface-10` hover, inset press shadow).

## UX (launcher-aligned, not a second window)

- **Open:** `Space` or `⌘Y` on a selected card → `QuickLookPanel` covers the overlay window with a dimmed backdrop and a centered dialog. `⌘Y` works while the search field is focused.
- **Close:** `Space` or `⌘Y` again, `Esc`, or click on the backdrop/close button — `Esc` closes Quick Look before it falls through to search-clear / overlay-dismiss.
- **Navigate:** while preview is open, `←` / `→` (horizontal board) or `↑` / `↓` (vertical board) keep changing `selectedIndex` as usual; the preview content is derived from the selection and refreshes automatically (Finder-like browsing without extra wiring).
- **Mouse (no extra header icon):** card click still **copies** — preview uses two alternate paths so the crowded action row stays unchanged:
  - **Type chip hover** — hover the format/type pill (`PNG`, `Text`, …) in the card header; an eye overlay appears on the chip; click it to open Quick Look (`stopPropagation` so copy does not run).
  - **Secondary click (context menu)** — right-click a text/image card → **Preview** (pattern borrowed from [Alfred](https://www.alfredapp.com/help/features/universal-actions/) Clipboard History / Universal Actions: contextual actions on a list item without dedicating toolbar space).
- **Window:** rendered **inside the existing** `main` **overlay webview** as a `position: fixed` dialog (same technique as `ConfirmDialog`), not a separate Tauri `WebviewWindow`. Rejected the separate-window approach from the original spec — the main overlay is a fixed-size, bottom/right-docked panel; growing or repositioning it just to host Quick Look (or adding a second frameless window with its own capability file, NSPanel level, and lifecycle sync) is meaningfully more surface area for a preview that only needs to show what a card already renders, at a larger size. The in-panel dialog gets vibrancy/blur, correct z-order, and focus handling for free from the host window.
- **Content by type:**
  - _Text_ — full `text_content`, scrollable, `textKind`-aware font (SF Mono for code kinds, shared with `ClipboardCard` via `src/lib/text-kind.ts`); metadata row shows source app and char count.
  - _Image_ — full-resolution image (see [Image sizing and GIF playback](#image-sizing-and-gif-playback) below), sized to fill the dialog body (not capped at a small `vh` fraction like the first cut); format badge, dimensions, and file size from the same `image-meta.ts` helpers as the card. When `ocr_text` exists, an **Image / Recognised text** segmented control (reusing the app's `.segment-track`/`.segment-item` classes from `segment-control.css`, same pattern as `CollectionTabs`) switches the body between the image and the full OCR text — the two no longer compete for space in one small box.
- **Non-goals (v1):** editing, re-tagging, delete, or paste from Quick Look; a dedicated `File` content kind (the app currently only ever produces `text` or `image` entries — no `File` branch exists in the data model, so the original spec's mention of it was aspirational, not current); a native macOS `QLPreviewPanel` window (system Quick Look) — the in-panel dialog now fills its available space, so a second OS-level preview is deferred until real-world use shows it's still needed.

## Image sizing and GIF playback

The first cut showed `image_thumb` (a 240×160 thumbnail generated at capture time) capped at `60vh` — barely bigger than the card. Two fixes:

1. **Lazy full-resolution fetch.** `get_entries` (the overlay list) intentionally omits `image_data` for list-fetch cost — only `image_thumb` ships with every page. Quick Look now calls a new `get_entry(id)` command on open (and again whenever arrow-key browsing changes the selected entry) to fetch the single full `ClipboardEntry` including `image_data`, and swaps it in once it resolves; the thumb is shown immediately so there's no blank flash while the full image loads. A request-sequence guard drops stale responses if the user arrows past an entry before its fetch finishes.
2. **The image preview area now fills the dialog body** (`flex: 1 1 auto`, `object-fit: contain`) instead of a fixed `max-height: 60vh`, and the dialog itself is wider for images (`max-width: 820px` vs `640px` for text) — both scale with the in-panel approach, no native window resize involved.

**GIF animation as a side effect, not a separate fix:** `image_thumb` for an animated GIF is a **static first-frame PNG** (`encode_png_thumb` in `clipboard_monitor.rs` decodes via the `image` crate, which only reads the first frame, then re-encodes as PNG) — this is why GIFs never played anywhere they showed the thumb. `image_data` for a GIF, however, is the **original raw animated bytes** (`encode_stored_gif` stores `full_b64` straight from the source, never re-encoding it) — so once Quick Look swaps to the full image, a GIF entry's `<img>` tag naturally plays the animation; no Rust encoding change was needed, only wiring the fetch through. Card thumbnails in the list intentionally stay static (matches Finder's list-view icon behavior and avoids sending full animated-GIF bytes on every page load) — only the Quick Look full view animates.

## Technical approach (as implemented)

1. **State:** `quickLookOpen: boolean` in `+page.svelte`; `quickLookEntry` is `$derived` from `quickLookOpen && filteredEntries[selectedIndex]` — no separate id tracking needed, it always mirrors the current selection.
2. **UI:** `QuickLookPanel.svelte` — reuses `detectTextKind`/`usesMonoPreview` (`text-kind.ts`), `cardDisplayTags`, `cardTagDisplayLabel`, and `image-meta.ts` (`imageDataUrl`). Local state: `activeTab` (`"image" | "text"`, reset on entry change), `fullImageB64` (lazy-fetched full image), `fullImageRequestSeq` (race guard). `CardTypeBadge.svelte` — shared type/format pill for card header and Quick Look metadata.
3. **Keyboard policy:** `quick-look-keyboard.ts` — pure helpers for `Space`/`⌘Y` guards, search→grid (`↓`/`→`), Quick Look open while list pending/failed, overlay action block while preview open, and `Esc` priority (context menu → Quick Look → clear search → dismiss). Unit-tested in `quick-look-keyboard.test.ts`.
4. **Rust:** one new read-only command, `get_entry(id) -> Option<ClipboardEntry>` (`commands.rs`), backed by `db.get_entry_by_id`; in `main-commands.toml` `main-window-commands`. No new window, no schema change.
5. **Lifecycle:** `closeQuickLook`/`quickLookOpen = false` runs from `startVisualHide()` and `resetOverlayMotionState()`, so hiding or resetting the overlay always closes Quick Look. Focus returns to the previously-focused element on close, or falls back to `scrollToSelected()`.
6. **Performance:** text entries and the initial image paint cost zero extra IPC; only opening Quick Look on an _image_ entry triggers `get_entry`, capped by `MAX_IMAGE_FILE_BYTES` (~20 MB).
7. **Dev harness:** `src/routes/dev/vertical-overlay/` and `vertical-grid/` — browser-only regression pages with mock entries (no Tauri IPC) for vertical-board scroll/Quick Look/pill animation; not shipped in production builds.

## Accessibility

- Preview surface: `role="dialog"`, `aria-modal="true"`, `aria-label` derived from entry type (“Text entry preview” / “Image entry preview”).
- Focus moves into the dialog on open and whenever the previewed entry changes (arrow-key browsing); focus restores to the previously-focused element on close.
- `aria-live="polite"` status text inside the dialog announces the type and source app on open/entry change.
- Backdrop uses `--surface-scrim` and honors `prefers-reduced-transparency` (no blur when reduced); no motion/transition to disable since the panel snaps in — no separate `prefers-reduced-motion` handling needed here.
  - Image/OCR segmented control: `role="tablist"` / `role="tab"` + `aria-selected`, click or `Tab` to cycle (handled on the dialog; `←`/`→` stay reserved for browsing entries). Footer shows a centered `Tab` **· switch view** hint when OCR tabs are present. Segment track uses a fixed two-column grid (`quicklook-mode-segment`) so switching does not resize the control; shared `segment-control.css` keeps a transparent 1px border on every segment so the selected state only changes `border-color`, not layout.

## Acceptance criteria

- [x] `Space` or `⌘Y` on selected card opens full preview; same shortcut or `Esc` closes it (`⌘Y` from search included).
- [x] `↓` / `→` from search blurs the field and selects the first visible card.
- [x] Trackpad scroll syncs selection to the leading visible card on the horizontal board (vertical: disabled — conflicts with `scroll-snap-y`; use arrows).
- [x] Type-chip hover eye and secondary-click **Preview** on cards (mouse paths; card click still copies).
- [x] `Enter` pastes without opening preview; double-click and Paste button unchanged.
- [x] `←` / `→` or `↑` / `↓` with preview open updates content in place (derived from `selectedIndex`).
- [x] No native `title` tooltip on cards (already done pre-existing).
- [x] Footer hints and [audit-hig.md](audit-hig.md) item 14 / 19 updated; entry in [CHANGELOG.md](../../CHANGELOG.md).
- [x] Image entries show full-resolution content (lazy `get_entry` fetch), not the 240×160 list thumbnail.
- [x] Animated GIF entries play in Quick Look (fixed as a side effect of the full-resolution fetch — the thumb is a static first frame, `image_data` keeps the original animated bytes).
- [x] Image entries with OCR text get an **Image / Recognised text** segmented toggle instead of a cramped simultaneous image+OCR stack.

## Follow-ups (not in this pass)

- [ ] Native macOS Quick Look (`QLPreviewPanel`, `⌘Y`-style) for a true full-screen/zoomable image view — deferred; the in-panel dialog now fills its available space, revisit only if that proves insufficient.
