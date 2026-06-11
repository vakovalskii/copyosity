# Copyosity UI — аудит по Apple HIG

Глобальный аудит UI Copyosity по [Apple Human Interface Guidelines](https://developer.apple.com/design/human-interface-guidelines/): clipboard overlay, voice HUD, settings и shared design system. Один файл — общие пункты (motion, tokens, transparency) правятся и отмечаются один раз.

**Прогресс:** только чекбоксы в чеклисте и `✅` в детальных разделах. Без статусных подписей («отложено», «отдельный PR», даты review и т.п.) — всё в списке будет сделано, пункт за пунктом.

**Метки scope:** `[Overlay]` clipboard panel · `[Settings]` settings window · `[Voice]` voice HUD · `[Shared]` tokens / form-controls / button-interaction / motion helper

| Поверхность | Файлы |
| ----------- | ----- |
| Clipboard overlay | [`+page.svelte`](../../src/routes/+page.svelte), [`ClipboardCard.svelte`](../../src/lib/components/ClipboardCard.svelte), [`SearchBar.svelte`](../../src/lib/components/SearchBar.svelte), [`CollectionTabs.svelte`](../../src/lib/components/CollectionTabs.svelte) |
| Voice HUD | [`overlay/+page.svelte`](../../src/routes/overlay/+page.svelte) |
| Settings | [`settings/+page.svelte`](../../src/routes/settings/+page.svelte), [`SectionIcon.svelte`](../../src/lib/components/SectionIcon.svelte) |
| Shared | [`tokens.css`](../../src/lib/styles/tokens.css), [`form-controls.css`](../../src/lib/styles/form-controls.css), [`button-interaction.css`](../../src/lib/styles/button-interaction.css), [`motion.ts`](../../src/lib/motion.ts) |

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

## Чеклист (roadmap)

### P1 — Accessibility

- [x] `[Overlay]` Search input в Tab order; focus ring через `:focus-within` на `.search-bar` (п. 4)
- [ ] `[Overlay]` убрать global `outline: none`; `focus-visible` на карточки и non-button табы (п. 1)
- [x] `[Overlay]` `card-actions` при `.selected` / `:focus-within`; `aria-label` на action buttons (п. 2)
- [ ] `[Overlay]` hit targets 28px+ — search clear button (20px) и card action buttons (24px) (п. 3)
- [ ] `[Shared]` Контраст `--color-text-subtle` / `--color-text-faint`; `prefers-contrast: more` (п. 5)
- [ ] `[Shared]` `form-input` / `form-select`: `:focus-visible` вместо `:focus` (п. 23)
- [ ] `[Settings]` Custom model input без связанного `<label>` при preset `__custom__` (п. 25)
- [ ] `[Voice]` HUD полностью `aria-hidden` — нет live region для состояния записи (п. 31)

### P2 — Native feel

- [x] `[Overlay]` `⌘F` / `/` → search; `←/→` зарезервированы под карточки (п. 4)
- [ ] `[Overlay]` Keyboard hints — контекстный hint в `SearchBar` + footer strip в `+page.svelte` (п. 19)
- [ ] `[Overlay]` Segmented control для History / Starred; tablist ARIA; упростить header (п. 8–9)
- [x] `[Overlay]` SF Pro для plain text, SF Mono только для code-like preview (п. 11)
- [ ] `[Settings]` Toggle / section patterns вынести в `form-controls.css` (п. 26)

### P3 — Polish

- [x] `[Overlay]` Empty state fix (фильтр по тегу / search) (п. 18)
- [x] `[Overlay]` Убрать `title` tooltip с карточки (п. 14)
- [ ] `[Overlay]` Delete undo / confirm (п. 12)
- [ ] `[Settings]` Clear history без confirm / undo (п. 22)
- [x] `[Shared]` `prefers-reduced-motion` — полное покрытие (п. 20)
- [ ] `[Shared]` `prefers-reduced-transparency` — blur fallback (п. 6, 21)
- [x] `[Overlay]` Image meta labels (dimensions вместо «Image preview») (п. 17)
- [ ] `[Shared]` Убрать дублирование `title` + `aria-label` на toggles и list actions (п. 24)

### P4 — Native depth

- [ ] `[Shared]` SF Symbols вместо custom stroke SVG (п. 15)
- [ ] `[Shared]` Native vibrancy / light mode (`prefers-color-scheme: light`) (п. 7)
- [ ] `[Overlay]` VoiceOver listbox (п. 34)
- [x] `[Overlay]` Scroll affordances на tag bar (п. 10)

---

## Что уже хорошо

| Область | Scope | Реализация |
| ------- | ----- | ---------- |
| Panel / HUD | Overlay, Voice | Прозрачные NSPanel-окна, `alwaysOnTop`, без кражи фокуса |
| Settings layout | Settings | Секции `form-section`, status steps, Ollama onboarding states по product policy |
| Системный шрифт | Overlay, Settings | `-apple-system, BlinkMacSystemFont` |
| Семантические цвета | Shared | danger / warning / success / accent tokens |
| Focus ring на кнопках | Shared | `button.app-btn:focus-visible` |
| Form focus | Shared | `form-input:focus` ring (`--ring-accent-input`) |
| Motion | Shared | Reduce Motion: panel, scroll, pulse, spinner, hover, copied, EQ bars, micro-transitions via tokens |
| Search field | Overlay | `role="search"`, clear button, `:focus-within` ring |
| Empty state | Overlay | Контекстные сообщения, `role="status"` |
| Toggles a11y | Settings | `role="switch"`, `aria-label`, `focus-visible` ring на slider |

---

## Clipboard overlay (п. 1–19)

### 1. Глобальное отключение outline `[Overlay]`

В `+page.svelte` на всех элементах `outline: none`. Карточки и div-табы коллекций не получают `:focus-visible`.

**Рекомендация:** убрать глобальный reset; `focus-visible` ring на карточках, табах, search.

### ✅ 2. Действия карточки при keyboard selection `[Overlay]`

`.card-actions` показываются при `.selected`, `:focus-within` и hover; action buttons используют `aria-label`.

### 3. Hit targets ниже minimum `[Overlay]`

Search clear 20×20 px; card action buttons 24×24 px. HIG: 28×28 pt minimum.

### ✅ 4. Поиск с клавиатуры `[Overlay]`

`⌘F`, `/`, `←/→`, `Escape`, Unicode search в БД.

**Follow-up:** стрелки в search не двигают курсор — нужны keyboard hints (п. 19).

### 5. Контраст вторичного текста `[Shared]` `[Overlay]`

`--color-text-subtle` / `--color-text-faint` на полупрозрачном фоне.

### 6. Material / Vibrancy `[Overlay]` `[Voice]` `[Shared]`

| Слой | Файл | Blur |
| ---- | ---- | ---- |
| Overlay panel | `+page.svelte` | `--panel-blur-visible` (34px) |
| Voice HUD | `overlay/+page.svelte` | 12px |
| Copied overlay | `ClipboardCard.svelte` | 6px |

Settings (`--surface-page` 96% opaque) менее критичен. См. п. 21, 32.

### 7. Только Dark `[Shared]`

Нет light-токенов и `prefers-color-scheme: light`.

### 8. Tabs — не segmented control `[Overlay]`

`CollectionTabs.svelte`: нет `aria-selected` / `role="tablist"`; collection tabs — `<div role="button">`; delete `×` только on hover.

### 9. Перегруженный header `[Overlay]`

Search + tabs + collections + Exclude + gear в одной строке. Exclude → overflow; search flex-grow.

### ✅ 10. Tag filter bar `[Overlay]`

Скрытый scrollbar; шрифт 11px. Рекомендация: 12–13px; scroll fade.

### ✅ 11. Моноширинный шрифт для всего preview `[Overlay]`

SF Mono на всём тексте карточки. HIG: SF Pro для body, Mono только для code.

### 12. Delete без подтверждения `[Overlay]`

Одно нажатие X удаляет запись. См. единый паттерн п. 29.

### 13. Selection vs Hover states `[Overlay]`

Selected card должен быть самым контрастным состоянием.

### ✅ 14. Native tooltip на карточке `[Overlay]`

`title={entry.text_content}` — убрать; Quick Look по `Space` (future).

### 15. Иконография — не SF Symbols `[Shared]`

Custom stroke SVG в overlay и settings.

### ✅ 16. Search field styling `[Overlay]`

Clear button, `:focus-within` ring, `role="search"`, `aria-label`.

### ✅ 17. Image cards — redundant label `[Overlay]`

«Image preview» → dimensions / file size.

### ✅ 18. Empty state copy `[Overlay]`

Контекстные сообщения при search / tag filter; `role="status"`.

### 19. Discoverability paste model и keyboard shortcuts `[Overlay]`

Footer shortcut strip + контекстный hint в search при focus (`← → browse results`).

---

## Shared / Motion & Materials (п. 20–21)

### ✅ 20. `prefers-reduced-motion` `[Shared]`

| Область | Файл | Reduce Motion |
| ------- | ---- | ------------- |
| Micro-transitions | `tokens.css` | `--duration-fast/standard/micro/hud/stagger` → `0.01ms` / `0ms` |
| Panel open/close | `+page.svelte` | `transition-duration: 0.01ms` (+ tokens) |
| Scroll к карточке | `motion.ts` | `behavior: "auto"` |
| Status dot checking | `form-controls.css` | статичный цвет |
| Tagging test spinner dot | `form-controls.css` | то же (`.checking`) |
| Voice mic pulse | `overlay/+page.svelte` | без animation |
| Voice EQ bars | `overlay/+page.svelte` | без stagger/wobble/height transition |
| Button spinner | `button-interaction.css` | замедлен (`--duration-spinner-reduced`) |
| Settings toggles | `settings/+page.svelte` | `transition: none` на slider |
| Card hover | `ClipboardCard.svelte` | без `translateY` |
| Copied feedback | `ClipboardCard.svelte` | fade вместо scale |

### 21. `prefers-reduced-transparency` `[Shared]`

```css
@media (prefers-reduced-transparency: reduce) {
  .app {
    backdrop-filter: none;
    background: var(--surface-app-opaque);
  }
}
```

**Файлы:** `+page.svelte`, `overlay/+page.svelte`, `ClipboardCard.svelte`, `tokens.css`.

---

## Settings (п. 22–29)

### 22. Clear history без подтверждения `[Settings]`

`form-btn-danger` «Clear unpinned history» — одно нажатие, только toast-notice после. Аналог п. 12.

**Рекомендация:** confirm dialog или «Cleared — Undo».

### 23. Form controls: `:focus` vs `:focus-visible` `[Shared]`

`form-controls.css`: ring на `:focus`, не `:focus-visible` — ring появляется и при mouse click.

**Рекомендация:** `:focus-visible` для inputs/selects; убрать outline на `:focus:not(:focus-visible)`.

### 24. Дублирование `title` и `aria-label` `[Settings]` `[Overlay]`

Toggles, exclude list actions, overlay exclude button — `title` дублирует `aria-label`.

**Рекомендация:** оставить `aria-label`; убрать `title`.

### 25. Custom model input `[Settings]`

При `selectedModelPreset === "__custom__"` input без `<label>` / `aria-label` — только placeholder.

### 26. Toggle styles локальны `[Settings]` `[Shared]`

`.toggle` / `.toggle-slider` только в `settings/+page.svelte`. Вынести в `form-controls.css`.

### ✅ 27. Ollama onboarding `[Settings]`

Status steps соответствуют product policy в `CLAUDE.md`. Spinner / checking dots покрыты Reduce Motion.

### 28. Settings `user-select: none` на body `[Settings]`

Глобально на `body`; inputs переопределяют на `text`. Hint `<code>` не selectable — minor.

### 29. Danger / destructive actions pattern `[Settings]` `[Overlay]`

Единый паттерн: overlay delete (п. 12) и settings clear history (п. 22). Исправлять одним решением (toast + undo).

---

## Voice HUD (п. 30–32)

### ✅ 30. EQ bars и mic — live feedback `[Voice]`

Reduce Motion: mic без pulse; bars — uniform height по level, без wobble/stagger/height transition (`motion.ts` + CSS).

### 31. Accessibility при записи `[Voice]`

Весь HUD `aria-hidden="true"`. Screen reader не получает сигнал о записи.

**Рекомендация:** `role="status"` + `aria-live="polite"` на overlay root.

### 32. Blur без transparency fallback `[Voice]`

`backdrop-filter: blur(12px)` — см. п. 6, 21.

---

## Низкий приоритет (п. 33–40)

| # | Scope | Тема |
| - | ----- | ---- |
| 33 | Shared | Dynamic Type — фиксированные px |
| 34 | Overlay | VoiceOver listbox / `aria-label` на карточках |
| 35 | Overlay | Pin indicator — только border-color |
| 36 | Overlay | Horizontal scroll-snap |
| 37 | Overlay | Card width 220px fixed |
| 38 | Overlay | Collections color dot 8px |
| 39 | Settings | Test button `disabled` без `aria-describedby` при `modelDirty` |
| 40 | Overlay | Add-collection inline input — нет `focus-visible` ring |

---

## Roadmap

```mermaid
flowchart LR
  P1[P1 Accessibility] --> P2[P2 Native feel]
  P2 --> P3[P3 Polish]
  P3 --- C2[Reduce motion done]
  P3 --- C2b[Reduce transparency]
```

| Приоритет | Задачи | Файлы |
| --------- | ------ | ----- |
| **P1** | Focus visible, card actions, contrast, form focus-visible, voice a11y | overlay components, `form-controls.css`, `overlay/+page.svelte` |
| **P2** | Keyboard hints, segmented tabs, font by type, toggle in form-controls | overlay components, `settings/+page.svelte` |
| **P3** | Delete/clear undo, tooltips, image meta, transparency, light mode | multiple |
| **P4** | SF Symbols, VoiceOver, scroll affordances | multiple |

---

## Референсы HIG

- [Materials](https://developer.apple.com/design/human-interface-guidelines/materials)
- [Accessibility](https://developer.apple.com/design/human-interface-guidelines/accessibility)
- [Buttons](https://developer.apple.com/design/human-interface-guidelines/buttons)
- [Search fields](https://developer.apple.com/design/human-interface-guidelines/search-fields)
- [Segmented controls](https://developer.apple.com/design/human-interface-guidelines/segmented-controls)
- [Typography](https://developer.apple.com/design/human-interface-guidelines/typography)

---

## Ограничение продукта

README: «never steals focus» — trade-off с HIG launcher pattern. Решение: type-to-search без auto-focus или shortcut-only focus (`⌘F` / `/`).
