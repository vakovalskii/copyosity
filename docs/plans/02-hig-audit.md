# Copyosity UI — Apple HIG Audit

Global UI audit of Copyosity against [Apple Human Interface Guidelines](https://developer.apple.com/design/human-interface-guidelines/): clipboard overlay, voice HUD, settings, and shared design system. One file — shared items (motion, tokens, transparency) are fixed and checked off once.

**Related plans:** overlay filters (items 10, 11, 14, 17, 20) — [05-overlay-content-and-tag-filters.md](05-overlay-content-and-tag-filters.md) · voice HUD a11y — [04-voice-hud-accessibility-full-cycle.md](04-voice-hud-accessibility-full-cycle.md) · backlog 0.4.0 — [03-new-features-and-improvements.md](03-new-features-and-improvements.md)

**Progress:** checkboxes in the checklist and `✅` in detailed sections only. No status labels (“deferred”, “separate PR”, review dates, etc.) — everything on the list will be done, item by item.

**Scope labels:** `[Overlay]` clipboard panel · `[Settings]` settings window · `[Voice]` voice HUD · `[Shared]` tokens / form-controls / button-interaction / motion helper

| Surface           | Files                                                                                                                                                                                                                                                                                                                                       |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Clipboard overlay | `[+page.svelte](../../src/routes/+page.svelte)`, `[ClipboardCard.svelte](../../src/lib/components/ClipboardCard.svelte)`, `[TagFilterBar.svelte](../../src/lib/components/TagFilterBar.svelte)`, `[SearchBar.svelte](../../src/lib/components/SearchBar.svelte)`, `[CollectionTabs.svelte](../../src/lib/components/CollectionTabs.svelte)` |
| Voice HUD         | `[overlay/+page.svelte](../../src/routes/overlay/+page.svelte)`                                                                                                                                                                                                                                                                             |
| Settings          | `[settings/+page.svelte](../../src/routes/settings/+page.svelte)`, `[SectionIcon.svelte](../../src/lib/components/SectionIcon.svelte)`                                                                                                                                                                                                      |
| Shared            | `[tokens.css](../../src/lib/styles/tokens.css)`, `[form-controls.css](../../src/lib/styles/form-controls.css)`, `[button-interaction.css](../../src/lib/styles/button-interaction.css)`, `[motion.ts](../../src/lib/motion.ts)`                                                                                                             |

```mermaid
flowchart TB
  subgraph overlay [Overlay]
    Search[SearchBar]
    Tabs[CollectionTabs]
    Cards[ClipboardCard]
  end
  subgraph voice [Voice HUD]
    Mic[Mic icon + EQ bars]
  end
  subgraph settings [Settings]
    Forms[form-sections]
    Toggles[AI / Voice switches]
  end
  subgraph shared [Shared]
    Tokens[tokens.css]
    Motion[motion.ts]
  end
  shared --> overlay
  shared --> voice
  shared --> settings
```

---

## Checklist (roadmap)

### P1 — Accessibility

- [x] `[Overlay]` Search input in Tab order; focus ring via `:focus-within` on `.search-bar` (item 4)
- [x] `[Overlay]` remove global `outline: none`; `focus-visible` on cards and non-button tabs (item 1)
- [x] `[Overlay]` Paste button on card — primary action instead of duplicate Copy; `Space` on card `role="button"`; `aria-busy` on activate (items 2, 19 partial)
- [x] `[Overlay]` hit target 28px+ — search clear only (28px); card actions 24px — intentional trade-off (item 3)
- [x] `[Overlay]` Search field — slightly less transparent background and placeholder for readability on vibrancy (item 3)
- [x] `[Shared]` Contrast `--color-text-subtle` / `--color-text-faint`; `prefers-contrast: more` (item 5)
- [x] `[Shared]` `form-input` / `form-select`: pointer vs keyboard focus rings via `input-modality` (item 24)
- [x] `[Settings]` Custom model input without associated `<label>` when preset is `__custom__` (item 26)
- [x] `[Voice]` Baseline live region on HUD while recording (item 32 — partial)
- [ ] `[Voice]` Full SR lifecycle (recording → processing → result) → [04-voice-hud-accessibility-full-cycle.md](04-voice-hud-accessibility-full-cycle.md)

### P2 — Native feel

- [x] `[Overlay]` `⌘F` / `/` → search; `←/→` reserved for cards (item 4)
- [x] `[Overlay]` Keyboard hints — footer strip in `+page.svelte` (item 19)
- [ ] `[Overlay]` Segmented control for History / Starred; tablist ARIA; simplify header (items 8–9)
- [x] `[Overlay]` SF Pro for plain text, SF Mono only for code-like preview (item 11)
- [x] `[Overlay]` Visually separate filter chip (toolbar) and metadata badge (card footer) (item 20)
- [x] `[Settings]` Toggle / section patterns moved to `form-controls.css` (item 27)

### P3 — Polish

- [x] `[Overlay]` Empty state fix (tag filter / search) (item 18)
- [x] `[Overlay]` Remove `title` tooltip from card (item 14)
- [x] `[Overlay]` Delete without confirm — product decision (item 12)
- [x] `[Overlay]` Fixed card size in layout — product decision (item 38)
- [x] `[Settings]` Clear history menu + confirm (item 23)
- [x] `[Shared]` `prefers-reduced-motion` — full coverage (item 21)
- [x] `[Shared]` `prefers-reduced-transparency` — blur fallback (items 6, 22)
- [x] `[Overlay]` Image meta labels (dimensions instead of “Image preview”) (item 17)
- [x] `[Shared]` Remove duplicate `title` + `aria-label` on toggles and list actions (item 25)

### P4 — Native depth

- [ ] `[Shared]` SF Symbols instead of custom stroke SVG (item 15)
- [ ] `[Shared]` Native vibrancy / light mode (`prefers-color-scheme: light`) (item 7)
- [x] `[Overlay]` VoiceOver listbox — product decision, not doing (item 35)
- [x] `[Overlay]` Scroll affordances on tag bar (item 10)

---

## What’s already good

| Area                  | Scope             | Implementation                                                                                     |
| --------------------- | ----------------- | -------------------------------------------------------------------------------------------------- |
| Panel / HUD           | Overlay, Voice    | Transparent NSPanel windows, `alwaysOnTop`, no focus stealing                                      |
| Settings layout       | Settings          | `form-section` sections, status steps, Ollama onboarding states per product policy                 |
| System font           | Overlay, Settings | `-apple-system, BlinkMacSystemFont`                                                                |
| Semantic colors       | Shared            | danger / warning / success / accent tokens                                                         |
| Focus ring on buttons | Shared            | `button.app-btn:focus-visible`                                                                     |
| Form focus            | Shared            | `form-input:focus` ring (`--ring-accent-input`)                                                    |
| Clear history         | Settings          | `ActionMenu` + `ConfirmDialog`; live counts via `history-changed` / `clipboard-changed`            |
| Motion                | Shared            | Reduce Motion: panel, scroll, pulse, spinner, hover, copied, EQ bars, micro-transitions via tokens |
| Search field          | Overlay           | `role="search"`, clear button, `:focus-within` ring                                                |
| Empty state           | Overlay           | Contextual messages, `role="status"`                                                               |
| Toggles a11y          | Settings          | `role="switch"`, `aria-label`, `focus-visible` ring on slider                                      |

---

## Clipboard overlay (items 1–20)

### ✅ 1. Global outline disabled `[Overlay]`

Removed global `outline: none` in `+page.svelte`; `focus-visible` ring on cards (`ClipboardCard`) and collection div-tabs (`CollectionTabs`).

### ✅ 2. Card actions on keyboard selection `[Overlay]`

`.card-actions` shown on hover and on keyboard focus (`:focus-within` + `data-input-modality="keyboard"`); on pinned cards — star always visible. `selected` alone does not reveal toolbar (mouse pin does not stick).

**Done in 0.4.0:** redundant Copy replaced with primary **Paste** (`activateEntry`, accent styling, `aria-busy` on activate); card click still copies; paste also via double-click, Enter, Space on card `role="button"`, and Paste toolbar button.

### ✅ 3. Hit targets and search readability `[Overlay]`

**HIG:** 28×28 pt minimum for interactive controls; input fields must remain readable on vibrancy backgrounds.

**Product decision (hit targets):** bring hit target to 28px **only** for clear in `SearchBar`. Card action buttons (24×24) and other dense toolbar controls are **not** enlarged — narrow card (~220px) and dense header do not allow it without layout loss; alternative paths (keyboard, click/double-click on card) already exist.

| Element                                  | Before   | Resolution                      |
| ---------------------------------------- | -------- | ------------------------------- |
| Search clear                             | 20×20 px | → 28×28 px (only place per HIG) |
| Card actions (paste, retag, pin, delete) | 24×24 px | Keep; exception documented      |

**Search readability:** `--surface-control` (6% white) on transparent panel caused “text on text” — placeholder and input were hard to read. Overlay search uses slightly denser `--surface-search` and stronger placeholder; still looks glassy, but contrast is sufficient.

### ✅ 4. Keyboard search `[Overlay]`

`⌘F`, `/`, `←/→`, `Escape`, Unicode search in DB.

**Follow-up:** arrows in search do not move cursor — browse/paste shortcuts documented in footer strip (item 19).

### ✅ 5. Secondary text contrast `[Shared]` `[Overlay]`

`--color-text-subtle` / `--color-text-faint` lightened; `@media (prefers-contrast: more)` in `tokens.css`.

### ✅ 6. Material / Vibrancy `[Overlay]` `[Voice]` `[Shared]`

| Layer          | File                   | Blur                          |
| -------------- | ---------------------- | ----------------------------- |
| Overlay panel  | `+page.svelte`         | `--panel-blur-visible` (34px) |
| Voice HUD      | `overlay/+page.svelte` | 12px                          |
| Copied overlay | `ClipboardCard.svelte` | 6px                           |

`prefers-reduced-transparency`: opaque token fallback, blur off. Settings (`--surface-page` 96% opaque) less critical.

### 7. Dark only `[Shared]`

No light tokens and no `prefers-color-scheme: light`.

### 8. Tabs — not segmented control `[Overlay]`

`CollectionTabs.svelte`: no `aria-selected` / `role="tablist"`; collection tabs are `<div role="button">`; delete `×` only on hover.

### 9. Overloaded header `[Overlay]`

Search + tabs + collections + Exclude + gear in one row. Exclude → overflow; search flex-grow.

### ✅ 10. Tag filter bar `[Overlay]`

Hidden scrollbar; 12px font; scroll fade. Filter chips — `.filter-chip` in `TagFilterBar`; separated from card metadata (item 20).

### ✅ 11. Monospace font for entire preview `[Overlay]`

SF Mono on all card text. HIG: SF Pro for body, Mono only for code.

### ✅ 12. Delete without confirmation `[Overlay]` — product decision

Single X press deletes entry without dialog. Launcher panel: targeted action on explicit delete button; extra confirm hurts speed. Bulk clear — Settings only with confirm (item 23).

### ✅ 13. Selection vs Hover states `[Overlay]`

Selected — light accent fill (`--surface-card-selected`, ~5–7% opacity), ring + `--shadow-card-selected`. Roving `tabindex`: focus follows `selectedIndex` (arrows / click); copied — overlay only, no second ring.

### ✅ 14. Native tooltip on card `[Overlay]`

`title={entry.text_content}` — remove; Quick Look via `Space` (future).

### 15. Iconography — not SF Symbols `[Shared]`

Custom stroke SVG in overlay and settings.

### ✅ 16. Search field styling `[Overlay]`

Clear button, `:focus-within` ring, `role="search"`, `aria-label`.

### ✅ 17. Image cards — redundant label `[Overlay]`

“Image preview” → dimensions / file size.

### ✅ 18. Empty state copy `[Overlay]`

Contextual messages for search / tag filter; `role="status"`.

### ✅ 19. Paste model discoverability and keyboard shortcuts `[Overlay]`

**Done:** Paste button on card — explicit mouse affordance for paste without double-click. Footer shortcut strip in `+page.svelte` (`KeyboardHints.svelte`); optional via **Settings → Clipboard Panel → Keyboard shortcuts** (default on). Overlay height +28 px when hints are on (`OVERLAY_HINTS_EXTRA_HEIGHT`).

| Zone         | Hint                                                                                          |
| ------------ | --------------------------------------------------------------------------------------------- |
| Footer strip | `Click copy` · `↵ paste` · `Double-click paste` · `← → browse` · `Esc clear search / dismiss` |

Do not duplicate Paste toolbar button verbatim in footer — “↵ paste” / “Double-click paste” is enough, since the button is visible on hover/selection.

### ✅ 20. Filter chip vs metadata badge — role conflict `[Overlay]` `[Shared]`

**Done:** two visual layers separated — toolbar filter vs card metadata.

| Layer       | Component       | Class                         | Behavior                                                                                                                                                               |
| ----------- | --------------- | ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Toolbar     | `TagFilterBar`  | `.filter-chip`                | `<button>`, pill + border, hover, `aria-pressed`, `.tag-count`                                                                                                         |
| Card footer | `ClipboardCard` | `.entry-tag` in `.entry-tags` | `<span>`, neutral micro-badge (rounded rect, `--surface-entry-tag`, no hover/accent); distinct from plain meta (`source-app`, `char-count`) and toolbar `.filter-chip` |

Token `--color-entry-tag` in `tokens.css`. Shared `.tag-chip` between components removed. Tag click on card not added — filtering only from toolbar.

**Before:** identical pill-chips (`api 2` top vs `api` bottom) — false affordance per HIG.

---

## Shared / Motion & Materials (items 21–22)

### ✅ 21. `prefers-reduced-motion` `[Shared]`

| Area                     | File                     | Reduce Motion                                                   |
| ------------------------ | ------------------------ | --------------------------------------------------------------- |
| Micro-transitions        | `tokens.css`             | `--duration-fast/standard/micro/hud/stagger` → `0.01ms` / `0ms` |
| Panel open/close         | `+page.svelte`           | `transition-duration: 0.01ms` (+ tokens)                        |
| Scroll to card           | `motion.ts`              | `behavior: "auto"`                                              |
| Status dot checking      | `form-controls.css`      | static color                                                    |
| Tagging test spinner dot | `form-controls.css`      | same (`.checking`)                                              |
| Voice mic pulse          | `overlay/+page.svelte`   | no animation                                                    |
| Voice EQ bars            | `overlay/+page.svelte`   | no stagger/wobble/height transition                             |
| Button spinner           | `button-interaction.css` | slowed (`--duration-spinner-reduced`)                           |
| Settings toggles         | `settings/+page.svelte`  | `transition: none` on slider                                    |
| Card hover               | `ClipboardCard.svelte`   | no `translateY`                                                 |
| Copied feedback          | `ClipboardCard.svelte`   | fade instead of scale                                           |

### ✅ 22. `prefers-reduced-transparency` `[Shared]`

Opaque surface tokens in `tokens.css`; `backdrop-filter: none` in `+page.svelte`, `overlay/+page.svelte`, `ClipboardCard.svelte`.

---

## Settings (items 23–30)

### ✅ 23. Clear history — menu and confirm `[Settings]`

**Before:** single button without confirmation. **Done:** `Clear history` with menu (unpinned / all…); `ConfirmDialog` with counts; neutral confirm (user already chose action in menu); success notice in action row; `clear_all_history` for pinned; menu disabled when history empty.

### ✅ 24. Form controls: pointer vs keyboard focus `[Shared]`

WebKit in Tauri often shows `:focus-visible` on mouse click. Solution: `input-modality.ts` sets `data-input-modality` on `<html>`; `form-controls.css` gives tight ring on `:focus`, and 3px keyboard halo only with `[data-input-modality="keyboard"]`.

### ✅ 25. Duplicate `title` and `aria-label` `[Settings]` `[Overlay]`

Removed `title` where it duplicated `aria-label`: overlay exclude button (`+page.svelte`), AI/Voice toggles, exclude list actions (`settings/+page.svelte`). Test button: `aria-describedby` when `modelDirty` (item 40), `title` removed.

### ✅ 26. Custom model input `[Settings]`

When `__custom__` — `<label for="custom-ollama-model">` + associated input.

### ✅ 27. Toggle styles local `[Settings]` `[Shared]`

`.toggle` / `.toggle-slider` moved to `form-controls.css` (next to `.toggle-section-body`); removed from `settings/+page.svelte`. Slider `border-radius` — `var(--radius-pill)`.

### ✅ 28. Ollama onboarding `[Settings]`

Status steps match product policy in `CLAUDE.md`. Spinner / checking dots covered by Reduce Motion.

### ✅ 29. Settings selection chrome `[Settings]`

`ui-no-select` / `ui-selectable-text` in `form-controls.css`: chrome (sections, rows, buttons) not selectable; text — headings, labels, status lines, hints, meta, inputs — `fit-content`, no padding fill. `.settings-page` carries `ui-no-select`.

### ✅ 30. Danger / destructive actions pattern `[Settings]`

**Done:** `ConfirmDialog` only in Settings for bulk clear (item 23); title — single `?`, message — declarative consequences with bold counts and `\u00A0`; `ActionMenu` (opaque dropdown, full-width in Storage). Overlay single delete — no confirm (item 12). Unified `.inset-list` pattern (dividers only between rows inside group); subsections — `form-subsection` + `form-subsection-rule` with symmetric `--space-subsection`; Storage — `form-field-group` + inline notice without extra divider.

---

## Voice HUD (items 31–33)

### ✅ 31. EQ bars and mic — live feedback `[Voice]`

Reduce Motion: mic without pulse; bars — uniform height by level, no wobble/stagger/height transition (`motion.ts` + CSS).

### 32. Accessibility while recording `[Voice]` (baseline)

**Done (baseline):** `role="status"` + `aria-live="polite"` on overlay root; decorative content in `aria-hidden` wrapper; sr-only “Recording voice”.

**Remaining:** full screen-reader lifecycle (repeat sessions, processing, terminal states) — [04-voice-hud-accessibility-full-cycle.md](04-voice-hud-accessibility-full-cycle.md) (source of truth for voice a11y).

### ✅ 33. Blur without transparency fallback `[Voice]`

`prefers-reduced-transparency` — see items 6, 22.

---

## Low priority (items 34–41)

### ✅ 34. Dynamic Type — fixed px `[Shared]`

Scale `--font-size-*` in `rem` (from `html { font-size: 100% }`), spacing `--space-*` in `rem`, `@supports (font: -apple-system-body)` on `body` for system font. Typography in overlay, settings, and `form-controls.css` moved to tokens; card size — `--card-width` / `--card-height` in rem. Radii and small icon-hit chrome remain in px.

**Limitation:** rem tokens do not follow macOS Dynamic Type Text size slider — full compliance needs `em` from `body` or environment-based scale. Fixed `--card-width` / `--card-height` in layout — product decision (item 38).

### ❌ 35. VoiceOver listbox / `aria-label` on cards `[Overlay]` — product decision

Cards in horizontal list without listbox semantics; no `aria-label` at entry level for SR. Affects **VoiceOver / screen reader only** — no impact on visual UI, mouse, or current keyboard navigation (`←/→`, Enter, Space, Paste). At this stage we do not plan such changes: overlay already covers P1 a11y (focus, actions, search); listbox is SR depth without behavior change for other users.

**Deferred scope (for clarity, what we’re declining):**

| Area                   | What was envisioned                                                                                                  |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `+page.svelte`         | `.grid-container` → `role="listbox"`, `aria-label`, `aria-orientation="horizontal"`, `aria-multiselectable="false"`  |
| `ClipboardCard.svelte` | `role="option"` instead of `role="button"`; `aria-selected`; `aria-posinset` / `aria-setsize`; stable `id` on option |
| New helper             | `buildEntryAriaLabel(entry)` — type, shortened preview, time, source app, pinned, tags                               |
| Details                | `alt=""` on thumb inside card with `aria-label`; `.focus()` on selected card on `←/→` for VO follow                  |
| Constraint             | Nested action buttons (Paste / Pin / Delete) conflict with strict listbox — would need separate compromise           |

### ✅ 36. Pin indicator — border-color only `[Overlay]`

**Before:** pinned state only via semi-transparent border. **Done:** warning border 50%; star always on Pin button; selection (fill) separated from keyboard focus ring (`data-input-modality`); after pointer action — blur from card.

### 37. Horizontal scroll-snap `[Overlay]`

Card scroll container without `scroll-snap` for keyboard / trackpad navigation.

### ❌ 38. Card width fixed in layout `[Overlay]` — product decision

`--card-width` / `--card-height` in rem (≈220×288 at 16px root) — intentionally fixed card size, not a layout bug. Entire overlay (preview, typography, actions, scroll, keyboard navigation) tuned to this width and height; horizontal scroll is expected UX with many entries. Adapting cards to panel width or items per screen not planned. Do not confuse with item 34 (typography and rem scale from root).

### 39. Collections color dot 8px `[Overlay]`

Collection color dot 8×8 px — below comfortable minimum for distinguishability.

### ✅ 40. Test button `disabled` without `aria-describedby` `[Settings]`

`aria-describedby="tagging-test-save-hint"` on Test when `modelDirty`; hint with `id`; `aria-label="Test tagging"`; `title` replaced with describedby.

### ✅ 41. Add-collection inline input — focus ring `[Overlay]`

`.add-form .form-input` in `CollectionTabs.svelte`: focus ring via `--ring-control-focus` / `--ring-accent-input` (keyboard modality); `aria-label="Collection name"`.

---

## Roadmap

```mermaid
flowchart LR
  P1[P1 Accessibility] --> P2[P2 Native feel]
  P2 --> P3[P3 Polish]
  P3 --- C2[Reduce motion done]
  P3 --- C2b[Reduce transparency]
```

| Priority | Tasks                                                                                                                                 | Files                                                                                                                         |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| **P1**   | focus visible, card actions, contrast, form focus-visible, voice a11y baseline; hit targets; voice SR full cycle                      | overlay components, `form-controls.css`, `overlay/+page.svelte`                                                               |
| **P2**   | keyboard hints, segmented tabs; font by type (item 11); filter vs metadata badges (item 20); toggle in form-controls (item 27)        | `TagFilterBar.svelte`, `ClipboardCard.svelte`, `tokens.css`, overlay components, `settings/+page.svelte`, `form-controls.css` |
| **P3**   | settings clear confirm; empty state, card tooltip, image meta; reduce motion, reduce transparency; title + aria-label dedup (item 25) | multiple                                                                                                                      |
| **P4**   | SF Symbols, light mode; scroll affordances on tag bar (item 10) — done; VoiceOver listbox (item 35) — not doing                       | multiple                                                                                                                      |

---

## HIG references

- [Materials](https://developer.apple.com/design/human-interface-guidelines/materials)
- [Accessibility](https://developer.apple.com/design/human-interface-guidelines/accessibility)
- [Buttons](https://developer.apple.com/design/human-interface-guidelines/buttons)
- [Labels](https://developer.apple.com/design/human-interface-guidelines/labels)
- [Search fields](https://developer.apple.com/design/human-interface-guidelines/search-fields)
- [Segmented controls](https://developer.apple.com/design/human-interface-guidelines/segmented-controls)
- [Typography](https://developer.apple.com/design/human-interface-guidelines/typography)

---

## Product constraint

README: “never steals focus” — trade-off with HIG launcher pattern. Resolution: type-to-search without auto-focus or shortcut-only focus (`⌘F` / `/`).
