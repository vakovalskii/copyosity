# Repository Workflow

Project workflow rules:

1. After a release is finished, start the next iteration in a new branch.
2. New features, fixes, and experiments must be pushed to that branch.
3. After every code generation pass or manual code edit, auto-fix the affected area first, then test compilation of the app.
4. Keep git history and workflow notes up to date by committing changes clearly and updating this file together with `AGENTS.md`.

## Mandatory Checks

Use the same validation contract as `AGENTS.md`: auto-fix first, then run the narrowest check that covers the edited area.

```bash
make fix-frontend && make check-frontend # frontend-only changes
make fix-backend && make check-backend   # Rust/backend-only changes
make fix && make check                   # full-stack or cross-cutting changes
```

After macOS tray, overlay, or activation-policy changes, also run `make verify-tray` on a GUI Mac with Accessibility granted for System Events. Exit code `2` means automation was unavailable (inconclusive), not a pass.

**Before editing tray startup, `tray_macos.rs`, window levels, or activation policy:** read [docs/architecture/macos-tray-menu.md](docs/architecture/macos-tray-menu.md) end-to-end. Partial “simplifications” break either the 1st or 2nd tray click — only the combined scheme in that doc is verified.

## Expected Practice

- Do not continue feature development directly on the last release branch.
- Do not skip compilation checks after generating code.
- Do not leave workflow changes undocumented.

## Product & architecture docs

Load by task — do not load everything up front:

| Topic                                             | File                                                                                   |
| ------------------------------------------------- | -------------------------------------------------------------------------------------- |
| **macOS menu-bar tray (blink fix — read first)**  | [docs/architecture/macos-tray-menu.md](docs/architecture/macos-tray-menu.md)           |
| Command palette agent chat (Vercel AI SDK)        | [docs/architecture/palette-agent-chat.md](docs/architecture/palette-agent-chat.md)     |
| macOS paste automation (AXPaste, Cmd+V, Messages) | [docs/architecture/macos-paste-pipeline.md](docs/architecture/macos-paste-pipeline.md) |
| UI icons (stroke SVG)                             | [docs/architecture/ui-icons.md](docs/architecture/ui-icons.md)                         |
| Local AI / Ollama onboarding                      | [docs/product/ollama-onboarding.md](docs/product/ollama-onboarding.md)                 |
