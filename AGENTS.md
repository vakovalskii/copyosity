# Workflow Rules

These rules are mandatory for all future work in this repository.

1. After each release, create a new development branch before continuing feature work.
2. Push all new features and fixes to the active development branch, not directly to the release branch.
3. After code changes, run the narrowest fix/check cycle below before claiming done.
4. Record completed work in git with clear commits and keep project process notes updated in this file and in `CLAUDE.md` when the workflow index changes.

## Required Validation

Command contract: `make fix` auto-fixes, `make lint` checks lint/format only, and `make check` is the final gate (types, compile, lint, format, tests).

| Changed                      | Run                                        |
| ---------------------------- | ------------------------------------------ |
| Frontend only                | `make fix-frontend && make check-frontend` |
| Rust/backend only            | `make fix-backend && make check-backend`   |
| Full stack / shared workflow | `make fix && make check`                   |

After macOS tray, overlay, or activation-policy changes, also run `make verify-tray` on a GUI Mac with Accessibility granted for System Events. Exit code `2` means automation was unavailable (inconclusive), not a pass.

**Before editing tray startup, `tray_macos.rs`, window levels, or activation policy:** read [docs/architecture/macos-tray-menu.md](docs/architecture/macos-tray-menu.md) end-to-end. Partial “simplifications” break either the 1st or 2nd tray click — only the combined scheme in that doc is verified.

## Linting stack

- Frontend: `svelte-check`, Oxlint, Oxfmt, Stylelint.
- Backend: `cargo check`, Clippy (`-D warnings --all-targets`), rustfmt, `cargo test`.
- Git hooks: `lefthook.yml` (see comments there for glob/install caveats). Pre-commit auto-fixes staged files; pre-push re-fixes the full push, then runs typecheck + tests (~9s) as a safety net before CI. Install via `npm install` / `lefthook install`; local overrides in gitignored `lefthook-local.yml`.

## Branching

- Release work can land on a release branch or `main`.
- All follow-up development must continue in a new branch created after the release.
- Current development branch names should be descriptive, for example `prerelease/dev-fixes`.
- Do not continue feature development directly on the last release branch.
- Do not skip compilation checks after generating code.
- Do not leave workflow changes undocumented.

## Commit Discipline

- Commit completed fixes and features with descriptive messages.
- If process or workflow changes, update `AGENTS.md` and keep `CLAUDE.md` in sync when the docs index changes.

## Releasing (macOS only)

- One command: `scripts/release.sh <version> "notes"` — bump → fix+check →
  commit+push → build both arches (signed) → notarize+staple+clean tarballs →
  GitHub `latest.json` → tag → GitHub release → vkovalskii.com mirror → verify.
- Run it via the Bash tool with `dangerouslyDisableSandbox` (hdiutil/notarytool);
  export `HTTPS_PROXY` first (gh's API needs it on this network).
- Ship macOS aarch64 + x86_64 only, notarized. No Windows/Linux (inert stubs).
- Updater invariants: tarballs must have **0 AppleDouble (`._*`)** entries;
  install runs with `TMPDIR` on the app's own volume (EXDEV). Full details +
  prereqs in **CLAUDE.md → "Releasing the app"**.
- The landing page (`main:/docs/index.html`, served at vakovalskii.github.io/copyosity)
  is version- and download-link-synced by `release.sh`. If you edit it by hand,
  run `make fix-frontend` before pushing — `make check` runs `oxfmt --check` on it
  and CI goes red otherwise, even with `git push --no-verify`.

## Load by task

Read the linked doc when the task touches that area — do not load everything up front.

| Topic                                             | File                                                                                   |
| ------------------------------------------------- | -------------------------------------------------------------------------------------- |
| **macOS menu-bar tray (blink fix — read first)**  | [docs/architecture/macos-tray-menu.md](docs/architecture/macos-tray-menu.md)           |
| Command palette agent chat (Vercel AI SDK)        | [docs/architecture/palette-agent-chat.md](docs/architecture/palette-agent-chat.md)     |
| macOS paste automation (AXPaste, Cmd+V, Messages) | [docs/architecture/macos-paste-pipeline.md](docs/architecture/macos-paste-pipeline.md) |
| UI icons (stroke SVG)                             | [docs/architecture/ui-icons.md](docs/architecture/ui-icons.md)                         |
| Local AI / Ollama onboarding                      | [docs/product/ollama-onboarding.md](docs/product/ollama-onboarding.md)                 |
