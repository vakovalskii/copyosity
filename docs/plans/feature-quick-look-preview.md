# Quick Look preview on Space

**Status: backlog.** Finder-style full entry preview for the clipboard overlay.

**Related:** [audit-hig.md](audit-hig.md) item 14 (native `title` removed; Quick Look is the replacement) · backlog [features-backlog.md](features-backlog.md) · [feature-overlay-content-tag-filters.md](feature-overlay-content-tag-filters.md)

---

## Why

Card previews are intentionally truncated; users still need to read full text or inspect an image before pasting. A native `title` tooltip was removed because it is slow, non-native, and conflicts with keyboard actions. macOS users already know **Space → preview** from Finder Quick Look — we should match that mental model instead of inventing a hover affordance.

## Goal

When a card is selected in the clipboard overlay, **Space** opens a dedicated **Quick Look** view: larger, readable, non-destructive inspection of the entry. **Paste remains a separate action** (Return, double-click, Paste toolbar button). Preview must not copy or paste unless the user explicitly chooses paste from within Quick Look (optional; default is inspect-only).

## Keyboard model (breaking change from today)

| Key            | Today                            | After Quick Look                                                         |
| -------------- | -------------------------------- | ------------------------------------------------------------------------ |
| `Space`        | Paste (via card `role="button"`) | Open / close Quick Look for selected entry                               |
| `↵` / `Enter`  | Paste                            | Paste (unchanged)                                                        |
| `Double-click` | Paste                            | Paste (unchanged)                                                        |
| `Esc`          | Clear search / dismiss overlay   | If Quick Look open → close preview first; else existing overlay behavior |

### Implementation notes

- Remove `Space` from `ClipboardCard` `handleCardKeydown`; handle preview at overlay level (`+page.svelte`) when focus is on the card list, not in search or tag filters.
- Update footer hints (`KeyboardHints` in `+page.svelte`) and audit item 19 copy: add `Space` preview; keep `↵ paste` / `Double-click paste`.
- Segment controls and filter chips stay **Tab + Enter/Space only** (no arrow capture) per [feature-overlay-content-tag-filters.md](feature-overlay-content-tag-filters.md).

## UX (Finder-aligned)

- **Open:** `Space` on selected card → preview appears centered above the desktop (or as a sibling panel), visually distinct from the narrow overlay strip.
- **Close:** `Space` again, `Esc`, or click outside — same dismiss priority as overlay (`Esc` closes innermost surface first).
- **Navigate:** while preview is open, `←` / `→` (horizontal board) or `↑` / `↓` (vertical board) change the selected entry and refresh preview content (Finder behavior).
- **Window:** prefer a **separate Tauri `WebviewWindow`** (`quicklook`, frameless, vibrancy/material, not in dock) over an in-panel modal — keeps overlay width fixed and matches system Quick Look scale. Fallback: full-screen dimmed `role="dialog"` inside main webview if a second window is blocked by capabilities.
- **Content by type:**
  - _Text_ — full `text_content`, scrollable, `textKind`-aware font (SF Mono for code kinds per audit item 11); show metadata row (source app, char count, tags).
  - _Image_ — largest available asset (thumbnail or full blob if stored); dimensions and file size in header; show `ocr_text` when present.
  - _File_ — filename, size, type; no fake preview for opaque binaries.
- **Non-goals:** editing, re-tagging, or delete from Quick Look v1; toolbar actions stay on the card.

## Technical approach

1. **State:** `quickLookEntryId: number | null` in overlay page store; open/close helpers; sync with `selectedIndex`.
2. **UI:** `QuickLookPanel.svelte` (or dedicated route `quicklook/+page.svelte` for the extra window) reusing `formatImageFooterLabel`, `detectTextKind`, and tag display helpers from `ClipboardCard`.
3. **Rust (if separate window):** `open_quick_look_window` / `close_quick_look_window` commands; size to content with max bounds (~70% of screen); `always_on_top` during preview; scoped capability for the `quicklook` label only.
4. **Lifecycle:** closing overlay must close Quick Look; opening Quick Look must not hide the overlay until user dismisses both.
5. **Performance:** lazy-fetch large text/image payload if pagination only holds summaries today; show lightweight spinner inside preview shell.

## Accessibility

- Preview surface: `role="dialog"`, `aria-modal="true"`, `aria-label` derived from entry type (“Clipboard entry preview”).
- Move focus into preview on open; restore focus to selected card on close.
- `aria-live="polite"` announcement when preview opens or entry changes via arrow keys.
- Respect `prefers-reduced-motion` and `prefers-reduced-transparency` (reuse tokens from audit items 21–22).

## Acceptance criteria

- [ ] `Space` on selected card opens full preview; second `Space` or `Esc` closes it.
- [ ] `Enter` pastes without opening preview; double-click and Paste button unchanged.
- [ ] `←` / `→` or `↑` / `↓` with preview open updates content in place (layout-dependent).
- [ ] No native `title` tooltip on cards (already done).
- [ ] Footer hints and [audit-hig.md](audit-hig.md) item 14 / 19 updated when shipped; entry in [CHANGELOG.md](../../CHANGELOG.md).
