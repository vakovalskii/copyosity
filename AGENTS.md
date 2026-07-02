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
- Pre-commit: Lefthook (`lefthook.yml`) auto-fixes staged files in parallel with Oxfmt, Oxlint, Stylelint, `cargo fmt` (staged paths only), and `cargo clippy --fix --lib` (faster than `--all-targets`; full Clippy gate is `make check` / CI). Install hooks via `npm install` / `lefthook install`; local overrides in gitignored `lefthook-local.yml`.

## Branching

- Release work can land on a release branch or `main`.
- All follow-up development must continue in a new branch created after the release.
- Current development branch names should be descriptive, for example `prerelease/dev-fixes`.

## Commit Discipline

- Commit completed fixes and features with descriptive messages.
- If process or workflow changes, update both `AGENTS.md` and `CLAUDE.md`.

## Load by task

| Topic                                             | File                                                                                                   |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| macOS paste automation (AXPaste, Cmd+V, Messages) | [docs/architecture/macos-paste-pipeline.md](docs/architecture/macos-paste-pipeline.md)                 |
| Features backlog                                  | [docs/plans/features-backlog.md](docs/plans/features-backlog.md)                                       |
| Overlay content & tag filters                     | [docs/plans/feature-overlay-content-tag-filters.md](docs/plans/feature-overlay-content-tag-filters.md) |
| Appearance / light mode (theme switching)         | [docs/plans/feature-appearance-theme.md](docs/plans/feature-appearance-theme.md)                       |
