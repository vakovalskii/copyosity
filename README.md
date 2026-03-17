# Copyosity

Copyosity is a macOS clipboard history app built with SvelteKit, Tauri 2, Rust, and SQLite.

## Features

- Global shortcut to open clipboard history
- Fast horizontal card-based history UI
- Starred items tab
- Collection tabs
- Search across clipboard text
- Image clipboard previews
- Native macOS tray icon

## Install

1. Download `Copyosity_0.1.0_aarch64.dmg` from the latest release.
2. Drag `Copyosity.app` to `Applications`.
3. Launch the app once.

If macOS asks for permissions, allow:

- `Accessibility`
  Needed for paste automation and global shortcut behavior.
- `Input Monitoring`
  May be required by macOS for reliable shortcut/paste interaction.

## Usage

- Open Copyosity with `Cmd + Shift + V`
- Click any text card to copy-paste it back into the current app
- Use `Starred` to keep important items
- Use the gear button to configure model, privacy and history retention
- Click outside the window or press `Esc` to hide it

## Privacy

- Clipboard history is stored locally in SQLite
- Local tagging uses Ollama on `127.0.0.1`
- You can exclude apps from being stored or tagged in Settings

## Stack

- Svelte 5
- SvelteKit
- Vite
- Tauri 2
- Rust
- SQLite via `rusqlite`

## Development

```bash
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

For macOS release packaging and notarization helpers:

```bash
make release-macos
make notarize-info
make notarize-wait
```
