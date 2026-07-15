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

## Releasing the app (macOS)

One command does the whole pipeline:

```bash
scripts/release.sh <version> "release notes"      # e.g. scripts/release.sh 0.9.9 "Fix X"
```

It runs, in order: bump version (tauri.conf.json + package.json + Cargo.toml) →
`make fix && make check` → commit + push → build both arches signed
(`RELEASE_CONFIG=1`, updater key) → `scripts/finalize.sh` (DMG + notarize +
staple + **clean** updater tarballs) → generate GitHub `latest.json` → tag →
`gh release create` → `scripts/publish-mirrors.sh` (vkovalskii.com mirror) →
verify both endpoints + assert 0 AppleDouble entries. Artifacts land in
`dist/updater/<version>/` (git-ignored).

**Distribution = macOS only** (Apple Silicon + Intel), notarized + stapled.
Windows/Linux are inert stubs — do not ship them. "Notarization" is Apple-only.

**Prereqs / environment (this machine):**
- Updater signing key: `.tauri/copyosity-updater.key` (git-ignored, empty
  password; also GitHub secret `TAURI_SIGNING_PRIVATE_KEY`). Losing it breaks all
  future auto-updates.
- Notary keychain profile: `copyosity` (not AC_PASSWORD).
- Developer ID: `Developer ID Application: Valeriy Kovalsky (A933C2TJXU)`.
- `hdiutil`/`notarytool`/`stapler` need the Bash tool with
  `dangerouslyDisableSandbox: true`.
- Tests are locale-fragile → the script sets `LC_ALL=en_US.UTF-8`.
- `gh`'s HTTPS API needs `HTTPS_PROXY` exported in the shell on this network;
  `git push`, notarytool, and the mirror do not.

**Updater delivery.** `tauri.conf.json → plugins.updater.endpoints` is tried in
order: (1) `https://vkovalskii.com/copyosity/latest.json` (mirror, static, no
redirects), (2) GitHub `releases/latest/download/latest.json` (fallback). The
mirror origin is ssh `srv-rnd-demos-mcp` (DNS → Yandex Cloud, **not** the Hetzner
box); reload its nginx with `docker kill -s HUP vkovalskii-nginx`. Full runbook +
the three updater gotchas we hit (AppleDouble unpack, EXDEV cross-volume rename,
menu-bar exit guard vs relaunch, webview CSP) live in the auto-update memory and
`docs/architecture/palette-agent-chat.md` is unrelated — see commit history.

**Hard-won updater invariants (do not regress):**
- Updater tarballs must have **0 AppleDouble (`._*`) entries** — always pack with
  `COPYFILE_DISABLE=1 tar --no-mac-metadata --no-xattrs` (finalize.sh does this).
  Verify with `python3 -c "import tarfile;..."` — `tar tzf` hides them.
- Install must run with `TMPDIR` on the **app's own volume** or `rename()` fails
  with EXDEV (os error 18) across volumes — the `install_update` Rust command
  handles this; the Settings button uses it.
- The menu-bar exit guard only prevents code-less `ExitRequested` so the updater
  relaunch (which carries `RESTART_EXIT_CODE`) can restart the app.
- The fix ships in the *installing* build, so each of these required one manual
  install before in-app updates worked.

## Product & architecture docs

Load by task — do not load everything up front:

| Topic                                             | File                                                                                   |
| ------------------------------------------------- | -------------------------------------------------------------------------------------- |
| **macOS menu-bar tray (blink fix — read first)**  | [docs/architecture/macos-tray-menu.md](docs/architecture/macos-tray-menu.md)           |
| Command palette agent chat (Vercel AI SDK)        | [docs/architecture/palette-agent-chat.md](docs/architecture/palette-agent-chat.md)     |
| macOS paste automation (AXPaste, Cmd+V, Messages) | [docs/architecture/macos-paste-pipeline.md](docs/architecture/macos-paste-pipeline.md) |
| UI icons (stroke SVG)                             | [docs/architecture/ui-icons.md](docs/architecture/ui-icons.md)                         |
| Local AI / Ollama onboarding                      | [docs/product/ollama-onboarding.md](docs/product/ollama-onboarding.md)                 |
