# Copyosity

Copyosity is a macOS clipboard history app built with SvelteKit, Tauri 2, Rust, and SQLite.

<video src="https://github.com/vakovalskii/copyosity/releases/download/v0.1.0/Screen.Recording.2026-03-
  15.at.01.03.35.compressed.mp4" controls width="900"></video>

## Features

- Global shortcut to open clipboard history
- Fast horizontal card-based history UI
- Starred items tab
- Collection tabs
- Search across clipboard text
- Image clipboard previews
- Native macOS tray icon

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

On macOS, the app can be signed with a `Developer ID Application` identity through `src-tauri/tauri.conf.json`.
