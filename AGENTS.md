# Workflow Rules

These rules are mandatory for all future work in this repository.

1. After each release, create a new development branch before continuing feature work.
2. Push all new features and fixes to the active development branch, not directly to the release branch.
3. After code changes, run the narrowest fix/check cycle below before claiming done.
4. Record completed work in git with clear commits and keep project process notes updated in this file and in `CLAUDE.md`.

## Required Validation

Command contract: `make fix` auto-fixes, `make lint` checks lint/format only, and `make check` is the final gate (types, compile, lint, format, tests).

| Changed                      | Run                                        |
| ---------------------------- | ------------------------------------------ |
| Frontend only                | `make fix-frontend && make check-frontend` |
| Rust/backend only            | `make fix-backend && make check-backend`   |
| Full stack / shared workflow | `make fix && make check`                   |

## Linting stack

- Frontend: `svelte-check`, Oxlint, Oxfmt, Stylelint.
- Backend: `cargo check`, Clippy (`-D warnings --all-targets`), rustfmt, `cargo test`.
- Pre-commit: Husky + lint-staged auto-fixes staged files with Oxfmt, Oxlint, Stylelint, `cargo fmt` (staged paths only), and `cargo clippy --fix --lib` (faster than `--all-targets`; full Clippy gate is `make check` / CI).

## Branching

- Release work can land on a release branch or `main`.
- All follow-up development must continue in a new branch created after the release.
- Current development branch names should be descriptive, for example `prerelease/dev-fixes`.

## Commit Discipline

- Commit completed fixes and features with descriptive messages.
- If process or workflow changes, update both `AGENTS.md` and `CLAUDE.md`.

## Load by task

| Topic                                             | File                                                                                   |
| ------------------------------------------------- | -------------------------------------------------------------------------------------- |
| macOS paste automation (AXPaste, Cmd+V, Messages) | [docs/architecture/macos-paste-pipeline.md](docs/architecture/macos-paste-pipeline.md) |

Security hardening (audit 2026-03-17; completed through 0.4.0):

- [x] Explicit Tauri capabilities for the `settings` window (`src-tauri/capabilities/settings.json`; 0.3.0)
- [x] Validate Ollama model names before `ollama pull` (`ollama::validate_model_name`; 0.3.0)
- [x] `cargo audit` step in GitHub Actions release workflow (0.4.0)
- [x] Per-window IPC command scoping for sensitive commands (`paste_entry`, `clear_history`, `start_ollama_server`; 0.4.0)

## TODO — Next Iteration

Features:

- [ ] Infinite scroll — lazy loading entries on horizontal scroll (backend already supports limit+offset)
- [ ] Production build transparency fix — test on macOS 15+ (known Tauri issue #13415)
