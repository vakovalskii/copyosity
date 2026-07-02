# Auto-update — how it works & how to release

Copyosity uses `tauri-plugin-updater`. On launch (and via **Settings → Updates**)
the app fetches a manifest from GitHub Releases, and if a newer, **signed**
version exists it downloads and installs it, then relaunches.

- **Update source:** `https://github.com/vakovalskii/copyosity/releases/latest/download/latest.json`
  (configured in `src-tauri/tauri.conf.json → plugins.updater.endpoints`).
- **Signature:** every update artifact is signed with the updater private key;
  the app verifies it against the public key in `tauri.conf.json → plugins.updater.pubkey`.
  This is **separate** from Apple Developer ID / notarization.

## Keys (one-time, already done)

- Private key: `.tauri/copyosity-updater.key` — **git-ignored, back it up somewhere safe.**
  If lost, you can never ship another auto-update (users must reinstall manually).
- Public key: `.tauri/copyosity-updater.key.pub` — embedded in `tauri.conf.json`.
- For CI, add the private key contents as the repo secret **`TAURI_SIGNING_PRIVATE_KEY`**
  (the key has an empty password). Both `release.yml` and `windows-build.yml` already
  reference it.

> Auto-update only works **from** a version that already ships the updater plugin
> (0.5.2+). Older installs (0.5.1 and earlier) must be updated manually once.

## Releasing an update (macOS, local)

```bash
# 1. bump version in package.json + src-tauri/tauri.conf.json, commit
# 2. build WITH the signing key so .app.tar.gz(.sig) are produced
export TAURI_SIGNING_PRIVATE_KEY="$(cat .tauri/copyosity-updater.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npm run tauri build            # DMG bundling may fail (known) — the .app + .app.tar.gz are still made

# 3. notarize the .app + build/notarize the DMG (see docs/NOTARIZATION.md)
# 4. generate the manifest
./scripts/make-latest-json.sh "Release notes here"

# 5. publish: attach the DMG, the *.app.tar.gz, AND latest.json to the release
gh release create vX.Y.Z \
  src-tauri/target/release/bundle/dmg/*.dmg \
  src-tauri/target/release/bundle/macos/*.app.tar.gz \
  latest.json \
  --title "Copyosity X.Y.Z" --notes "..."
```

`latest.json` must be a release asset so the `latest/download/latest.json` URL
resolves. Adding a `windows-x86_64` entry (from the CI `.sig`) is optional.
