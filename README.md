# Copyosity

A fast, native macOS clipboard manager with on-device intelligence.

Copyosity keeps a searchable history of everything you copy, reads text out of
copied images on-device, turns your voice into clean ready-to-paste text, and
exposes a small command palette for web search and quick actions — all from a
floating panel you summon with a hotkey. It runs as a menu-bar app and stays out
of your way until you need it.

Built with Tauri 2, Svelte 5, Rust, and SQLite.

## Screenshot

<!-- Placeholder: docs/screenshot.png is not committed yet. Add a screenshot of
     the floating panel / command palette here. CI or a teammate will drop the
     image in at docs/screenshot.png. -->

![Copyosity](docs/screenshot.png)

## Features

- **Clipboard history** — every copy is captured and stored in a local SQLite
  database, with pinning, collections, and full‑text search.
- **App exclusions** — exclude specific apps (e.g. password managers) so their
  clipboard contents are never recorded.
- **On-device image OCR** — copied images are run through Apple's Vision
  framework (`VNRecognizeTextRequest`) so the text inside screenshots and photos
  becomes searchable. No image ever leaves your Mac for OCR.
- **Voice to text** — hold a global hotkey to record from any microphone; the
  audio is transcribed and the result is pasted into whatever app is frontmost.
- **Context-aware polishing** — raw transcription is cleaned into natural,
  typed‑style text, taking the target app into account so the output fits where
  it lands.
- **Automatic tagging** — clipboard entries (and images) are tagged with short,
  practical labels to make history easier to scan and filter.
- **Command / agent palette** — a separate palette for web search and a small
  personal assistant that can act on your Mac.
- **Native macOS actions** — the assistant can create Notes, create and list
  Reminders, and read upcoming Calendar events via AppleScript / Apple Events.
- **Local AI option** — optional Ollama integration for fully local tagging,
  with in‑app onboarding (install / start server / download‑model states).
- **Menu-bar native UI** — a transparent, non‑activating floating panel
  (`NSPanel`) that appears over your current app without stealing focus, plus a
  tray icon and global shortcuts.

## Install

1. Download the latest signed and notarized `.dmg` from the
   [GitHub Releases](../../releases) page.
2. Open the DMG and drag **Copyosity** into **Applications**.
3. Launch it. On first run macOS will ask for **Accessibility** permission
   (needed to paste into other apps) and, depending on the features you use,
   **Microphone**, **Automation** (Notes/Reminders/Calendar), and screen access.

The build is signed with a Developer ID certificate and notarized + stapled by
Apple, so Gatekeeper opens it without warnings (and offline).

## Platform support

Copyosity is **macOS only** (Apple Silicon recommended). It is a macOS‑native
app built on `NSPanel`, `CGEvent`, the Vision framework, and Apple Events.
**Windows and Linux are not currently supported.**

## Build from source

Prerequisites:

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
- [Node.js](https://nodejs.org/) 20+
- macOS with Xcode command line tools

```bash
git clone <this-repo-url>
cd copyosity
npm install

# run in development (hot reload)
npm run tauri dev

# produce a release bundle (.app + .dmg)
npm run tauri build
```

The Tauri config (`src-tauri/tauri.conf.json`) drives the bundle. The frontend
is SvelteKit (static adapter); the backend is Rust via Tauri 2.

## NeuralDeep hub (optional)

Several cloud‑assisted features — model‑based tagging, web search, the assistant
agent, and transcription / polishing — can be powered by a **NeuralDeep hub**
endpoint. This is **entirely optional** and disabled until you configure it.

To enable it, open **Settings** in the app and provide:

- your **own** hub **base URL**, and
- your **own** API token (an `sk-...` style key).

These credentials are yours: you supply them, and they are stored locally in the
app's settings. **No tokens, keys, or endpoints are bundled with Copyosity.** If
you don't configure a hub, the app falls back to local behavior (e.g. local
tagging via Ollama and on‑device OCR) where available.

> Never commit your hub URL or token to a repository or share it publicly.

## Development workflow

After any code change, run the project checks before committing:

```bash
npm run check                 # SvelteKit sync + svelte-check (frontend/TS)
cd src-tauri && cargo check   # Rust backend
```

See `CLAUDE.md` and `AGENTS.md` for the full contributor workflow (branching,
commit discipline, and local‑AI onboarding rules).
