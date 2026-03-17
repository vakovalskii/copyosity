# Workflow Rules

These rules are mandatory for all future work in this repository.

1. After each release, create a new development branch before continuing feature work.
2. Push all new features and fixes to the active development branch, not directly to the release branch.
3. After every code generation or code change, verify that the application still compiles.
4. Record completed work in git with clear commits and keep project process notes updated in this file and in `CLAUDE.md`.

## Required Validation

After each meaningful code change, run:

```bash
npm run check
cd src-tauri && cargo check
```

Do not treat the task as complete until both checks pass, unless a blocker is explicitly documented.

## Branching

- Release work can land on a release branch or `main`.
- All follow-up development must continue in a new branch created after the release.
- Current development branch names should be descriptive, for example `prerelease/dev-fixes`.

## Commit Discipline

- Commit completed fixes and features with descriptive messages.
- If process or workflow changes, update both `AGENTS.md` and `CLAUDE.md`.

## TODO — Next Iteration

Security hardening (from audit 2026-03-17):

- [ ] Add explicit Tauri capabilities for the `settings` window
- [ ] Validate ollama model names against a whitelist/regex before passing to `ollama pull`
- [ ] Add `cargo audit` step to GitHub Actions release workflow
- [ ] Consider per-window IPC command scoping for sensitive commands (`paste_entry`, `clear_history`, `start_ollama_server`)

Features:

- [ ] Infinite scroll — lazy loading entries on horizontal scroll (backend already supports limit+offset)
- [ ] Production build transparency fix — test on macOS 15+ (known Tauri issue #13415)
