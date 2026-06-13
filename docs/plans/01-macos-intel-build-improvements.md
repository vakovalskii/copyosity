# macOS Intel build and related improvements

Кратко: зачем меняли код и инфраструктуру в этом наборе правок.  
Follow-up и доработки перед релизом — в [05-macos-intel-pre-release.md](05-macos-intel-pre-release.md).

## Чеклист

- [x] **`build-macos.sh`** — pipeline: frontend → Tauri bundle → DMG в `dist/macos/`
- [x] **`macos-target.sh`** — архитектура через `MACOS_ARCH=auto | x86_64 | aarch64`
- [x] **Makefile** — `build-macos`, `build-macos-intel`, `build-macos-arm` и зеркальные `release-macos-*`
- [x] **Именованные артефакты** — `Copyosity_0.3.0_x86_64.dmg` и т.п. в `dist/macos/`
- [x] **`tauri.unsigned.json`** — ad-hoc для локальных сборок; release с Developer ID через `RELEASE_CONFIG=1`
- [x] **`release-macos.sh`** — тот же build pipeline, что и локальная сборка
- [x] **Makefile `APP_DIR`** — без hardcoded пути к проекту
- [x] **`env -u npm_config_devdir`** — стабильный `npm install` / Tauri build (в т.ч. Cursor)
- [x] **`with-tauri.sh` / `env-rust.sh`** — `cargo` и `tauri` в PATH
- [x] **`.vscode/settings.json`** — тот же workaround для integrated terminal
- [x] **`.gitignore`** — `/dist` для артефактов сборки
- [x] **README** — команды Intel/ARM и путь `dist/macos/`
- [x] **Frontend toolchain** — обновление SvelteKit / Svelte / Vite, override `cookie`
- [x] **Мониторинг буфера** — `changeCount`; порядок файлы → raster → текст; concealed; ignore Copyosity / excluded apps
- [x] **Декодеры изображений** — jpeg, webp, gif, bmp, tiff через `image` crate для путей с диска
- [x] **`CaptureContext` / `try_capture_from_clipboard`** — единая точка разбора pasteboard
- [x] **`clipboard_macos.rs`** — pasteboard API, synthetic Cmd+V, remember/restore paste target
- [x] **`clipboard_write.rs`** — `exclude_from_history`, пометка «своя» запись
- [x] **copy vs activate** — разделение «только в буфер» и «вставить в другое приложение»; Enter = `activateEntry`
- [x] **Accessibility** — `check_accessibility` + UI в Settings; Settings window на передний план (`objc2-app-kit`)
- [x] **Миграция objc → objc2** — `cocoa` заменён там, где уже используется
- [x] **Frontend** — Enter в ленте = вставка; Permissions в Settings; copy/paste модель карточки без изменений
- [x] **Voice shortcut** — общая macOS pasteboard-логика вынесена в `clipboard_macos`

---

## 1. Сборка под Intel (x86_64)

**Цель:** получить воспроизводимую `.app` и DMG для Intel Mac, параллельно с Apple Silicon, без привязки к одной машине разработчика.

**Что сделано:**

- `scripts/build-macos.sh` — единый pipeline: frontend → Tauri bundle → DMG в `dist/macos/`.
- `scripts/macos-target.sh` — архитектура через `MACOS_ARCH=auto | x86_64 | aarch64`.
- `Makefile`: `build-macos`, `build-macos-intel`, `build-macos-arm` и зеркальные `release-macos-*`.
- Именованные артефакты в `dist/macos/` (например `Copyosity_0.3.0_x86_64.dmg`).
- `tauri.unsigned.json` — ad-hoc подпись для локальных сборок; release с Developer ID через `RELEASE_CONFIG=1` в `release-macos.sh`.
- `release-macos.sh` использует тот же build pipeline, что и локальная сборка.

**Как собрать:** `make build-macos-intel` (Intel) или `make build-macos` / `make build-macos-arm` на соответствующей машине.

---

## 2. Инфраструктура сборки и dev-окружения

**Цель:** чтобы Intel/ARM сборки и `npm run tauri` работали на любой машине и в IDE без ручной настройки путей.

- `APP_DIR ?= $(CURDIR)` в Makefile — не hardcoded путь к проекту.
- `env -u npm_config_devdir` для npm — стабильный `npm install` / Tauri build (в т.ч. Cursor).
- `scripts/with-tauri.sh`, `scripts/env-rust.sh` — `cargo` и `tauri` в PATH.
- `.vscode/settings.json` — тот же workaround для integrated terminal.
- `.gitignore`: `/dist` — каталог артефактов сборки.
- `README.md` — команды Intel/ARM и путь `dist/macos/`.
- Обновление SvelteKit / Svelte / Vite, override `cookie` — актуальный frontend toolchain на чистом clone.

---

## 3. macOS — буфер обмена и история

**Цель:** надёжнее ловить копирование на macOS, корректно показывать картинки, не засорять историю действиями самого приложения.

### Мониторинг

- `NSPasteboard.changeCount` — опрос только когда буфер реально менялся.
- Порядок чтения: **файлы → raster → текст** — при копировании image-файла в Finder в историю попадают пиксели файла, а не служебная иконка с pasteboard.
- `image` crate: декодеры jpeg, webp, gif, bmp, tiff для путей с диска; скриншоты и «Copy Image» по-прежнему через raster API.
- Игнор concealed pasteboard (пароли и скрытый контент).
- Игнор источника Copyosity и приложений из excluded list.
- Модули `CaptureContext`, `try_capture_from_clipboard` — единая точка разбора содержимого буфера.

### Запись, копирование и вставка

- `clipboard_macos.rs` — pasteboard API, `changeCount`, concealed, синтетический Cmd+V, запоминание и восстановление целевого приложения перед вставкой.
- `clipboard_write.rs` — запись в буфер с `exclude_from_history` и пометкой «своя» запись, чтобы copy из карточки не дублировал историю.
- `remember_paste_target` / `restore_paste_target` — double-click / Enter вставляют в приложение, из которого открыли панель.
- `copy_entry` / `activate_entry` — разделение «только в буфер» и «вставить в другое приложение».
- Enter в главном окне — `activateEntry` для текста и изображений.
- `check_accessibility` + UI в Settings — права для автоматической вставки и горячих клавиш.
- Окно Settings — корректный вывод на передний план (`objc2-app-kit`).
- Зависимость `cocoa` заменена на `objc2` / `objc2-app-kit` там, где это уже используется.

---

## 4. Frontend

- Главная лента: Enter = вставка выбранной записи (`activateEntry`) и закрытие панели.
- Settings: блок Permissions (Accessibility), подсказка про повторное добавление приложения в Privacy после новой сборки.
- Карточка: один клик — copy, двойной — paste (без смены этой модели).

---

## 5. Voice shortcut

- Транскрипция по-прежнему кладёт текст в буфер и имитирует Cmd+V; общая macOS-логика pasteboard вынесена в `clipboard_macos`.

---

## 6. Затронутые области репозитория

| Область       | Файлы                                                                                                                                                          |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Сборка        | `Makefile`, `README.md`, `scripts/build-macos.sh`, `macos-target.sh`, `env-rust.sh`, `with-tauri.sh`, `with-npm.sh`, `release-macos.sh`, `tauri.unsigned.json` |
| Конфиг / deps | `.gitignore`, `.vscode/settings.json`, `package.json`, `package-lock.json`                                                                                     |
| Rust backend  | `clipboard_monitor.rs`, `clipboard_macos.rs`, `clipboard_write.rs`, `commands.rs`, `lib.rs`, `Cargo.toml`                                                      |
| UI            | `+page.svelte`, `settings/+page.svelte`, `ClipboardCard.svelte`                                                                                                |
