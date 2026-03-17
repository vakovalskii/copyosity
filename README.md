# Copyosity

A fast, privacy-first clipboard manager for macOS. Lives in your menu bar, opens instantly with a hotkey, and never steals focus from your active app.

Built with Tauri 2, Svelte 5, Rust, and SQLite.

## Why Copyosity

- **No focus stealing** — uses macOS NSPanel, your cursor stays exactly where it was
- **Local AI tagging** — automatic smart tags via Ollama, everything runs on your machine
- **Instant access** — Cmd+Shift+V opens history in ~100ms, Escape hides it
- **Privacy by design** — no cloud, no telemetry, clipboard stays in local SQLite

## Features

### Clipboard History
- Automatic capture of text and images from all apps
- Horizontal card-based UI with source app labels
- Search across all clipboard text
- Configurable retention (1 day to 6 months)

### Smart Actions
- **Single click** — copy to clipboard
- **Double click** — paste directly into the active cursor position
- **Copy button** (⎘) on each card
- **"Copied" animation** — visual confirmation before the window collapses
- **Keyboard navigation** — arrow keys to browse, Enter to paste, Escape to dismiss

### AI Tagging
- Automatic tagging powered by local Ollama (Qwen3 models)
- Step-by-step setup in Settings: install check, server status, model download, tagging test
- Filter by tags — quickly find URLs, code snippets, meeting notes, etc.
- Heuristic detection for OTPs, tokens, and opaque codes (no AI needed)

### Organization
- **Starred items** — pin important clips to keep them forever
- **Collections** — group clips into custom tabs
- **Excluded apps** — block specific apps from being recorded (passwords, banking, etc.)

### System Integration
- Menu bar tray icon (pink + turquoise)
- Global shortcut: `Cmd + Shift + V`
- Runs as Accessory app (no Dock icon)
- macOS code-signed and notarized

## Install

1. Download `Copyosity_0.2.1_aarch64.dmg` from the [latest release](https://github.com/vakovalskii/copyosity/releases/latest).
2. Open the DMG and drag **Copyosity** to Applications.
3. Launch the app — it appears in the menu bar.

### Permissions

macOS will ask for:
- **Accessibility** — needed for paste automation (Cmd+V simulation) and global shortcut
- **Input Monitoring** — may be required for reliable hotkey detection

### Local AI (optional)

For automatic clipboard tagging:
1. Install [Ollama](https://ollama.com/download)
2. Open Copyosity Settings — follow the step-by-step status panel
3. The app will start the server and download the model for you

## Usage

| Action | What it does |
|--------|-------------|
| `Cmd + Shift + V` | Open / close clipboard history |
| Single click on card | Copy to clipboard |
| Double click on card | Paste into active cursor |
| `Escape` | Hide window |
| Arrow keys + Enter | Navigate and paste |
| Click ⎘ button | Copy without closing |
| Click ★ button | Star / unstar |
| Click gear icon | Open Settings |

## Privacy

- All data stored locally in `~/Library/Application Support/com.vkovalskii.copyosity/`
- AI tagging runs on `127.0.0.1` via Ollama — nothing leaves your machine
- Exclude sensitive apps in Settings → Privacy
- Clear history anytime from Settings

## Development

```bash
npm install
npm run tauri dev
```

### Checks

```bash
npm run check              # Svelte + TypeScript
cd src-tauri && cargo test # 39 unit tests
cd src-tauri && cargo check
```

### Release

```bash
make release-macos    # Build, sign, notarize
make notarize-info    # Check notarization status
```
