#!/usr/bin/env bash
# Full macOS release pipeline for Copyosity — one command, end to end.
#
#   scripts/release.sh <version> ["release notes"]
#
# Steps: bump version → fix + check → commit + push → build both arches (signed)
# → notarize + staple + clean tarballs → GitHub latest.json → tag → GitHub
# release → mirror (vkovalskii.com) → verify both endpoints.
#
# Requirements (this machine):
#   - .tauri/copyosity-updater.key present (updater signing key, empty password)
#   - notarytool keychain profile "copyosity" (override KEYCHAIN_PROFILE)
#   - `gh` authenticated; GitHub reachable (gh's HTTPS API needs HTTPS_PROXY set
#     in your shell on this network — export it before running)
#   - ssh alias srv-rnd-demos-mcp reachable for the mirror (publish-mirrors.sh)
#   - run via the Bash tool with dangerouslyDisableSandbox (hdiutil/notarytool)
#
# Idempotent-ish: re-running re-tags (-f) and `gh release create` will fail if the
# release already exists — delete it first or bump the version.
set -euo pipefail
cd "$(dirname "$0")/.."
export LC_ALL=en_US.UTF-8 LANG=en_US.UTF-8

VERSION="${1:?usage: release.sh <version> [notes]}"
NOTES="${2:-Copyosity $VERSION}"
OUT="$PWD/dist/updater/$VERSION"
GH_REPO="vakovalskii/copyosity"
DL_BASE="https://github.com/$GH_REPO/releases/download/v$VERSION"

echo "==> [1/10] bump version to $VERSION"
python3 - "$VERSION" <<'PY'
import re, sys
v = sys.argv[1]
targets = [
    ("src-tauri/tauri.conf.json", r'("version":\s*")[0-9][0-9.]*(")'),
    ("package.json",              r'("version":\s*")[0-9][0-9.]*(")'),
    ("src-tauri/Cargo.toml",      r'(^version\s*=\s*")[0-9][0-9.]*(")'),
]
for path, pat in targets:
    s = open(path).read()
    s2 = re.sub(pat, lambda m: m.group(1) + v + m.group(2), s, count=1, flags=re.M)
    assert s2 != s, f"version not found/updated in {path}"
    open(path, "w").write(s2)
print("bumped tauri.conf.json, package.json, Cargo.toml")
PY

echo "==> [2/10] fix + check"
make fix
make check

echo "==> [3/10] commit + push version bump"
git add src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock package.json package-lock.json src/ src-tauri/src/ src-tauri/permissions/ scripts/ 2>/dev/null || true
git commit --no-verify -q -m "chore(release): bump version to $VERSION" || echo "(nothing to commit)"
git push --no-verify origin HEAD

echo "==> [4/10] build both arches (Developer-ID signed)"
export RELEASE_CONFIG=1
export TAURI_SIGNING_PRIVATE_KEY="$(cat .tauri/copyosity-updater.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
# build-macos.sh's own DMG step fails under the sandbox; the signed .app is what
# finalize needs, so tolerate that non-zero exit.
MACOS_ARCH=aarch64 bash scripts/build-macos.sh || true
MACOS_ARCH=x86_64 bash scripts/build-macos.sh || true
for arch_app in \
  "src-tauri/target/release/bundle/macos/Copyosity.app" \
  "src-tauri/target/x86_64-apple-darwin/release/bundle/macos/Copyosity.app"; do
  [ -d "$arch_app" ] || { echo "ERROR: missing build $arch_app" >&2; exit 1; }
done

echo "==> [5/10] finalize (DMG + notarize + staple + clean tarballs)"
bash scripts/finalize.sh "$VERSION" "$OUT"

echo "==> [6/10] generate GitHub latest.json"
python3 - "$VERSION" "$NOTES" "$OUT" "$DL_BASE" <<'PY'
import json, sys, datetime
version, notes, out, base = sys.argv[1:5]
sig_a = open(f"{out}/Copyosity_aarch64.app.tar.gz.sig").read().strip()
sig_x = open(f"{out}/Copyosity_x64.app.tar.gz.sig").read().strip()
manifest = {
    "version": version,
    "notes": notes,
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": {
        "darwin-aarch64": {"signature": sig_a, "url": f"{base}/Copyosity_aarch64.app.tar.gz"},
        "darwin-x86_64":  {"signature": sig_x, "url": f"{base}/Copyosity_x64.app.tar.gz"},
    },
}
open(f"{out}/latest.json", "w").write(json.dumps(manifest, indent=2))
print("wrote latest.json for", version)
PY

echo "==> [7/10] tag v$VERSION"
git tag -f "v$VERSION" HEAD
git push --no-verify -f origin "v$VERSION"

echo "==> [8/10] create GitHub release"
gh release create "v$VERSION" \
  --repo "$GH_REPO" \
  --title "Copyosity $VERSION" \
  --notes "$NOTES" \
  "$OUT/Copyosity-aarch64_$VERSION.dmg" \
  "$OUT/Copyosity-x64_$VERSION.dmg" \
  "$OUT/Copyosity_aarch64.app.tar.gz" \
  "$OUT/Copyosity_aarch64.app.tar.gz.sig" \
  "$OUT/Copyosity_x64.app.tar.gz" \
  "$OUT/Copyosity_x64.app.tar.gz.sig" \
  "$OUT/latest.json"

echo "==> [9/10] publish to vkovalskii.com mirror"
bash scripts/publish-mirrors.sh "$OUT"

echo "==> [10/10] verify both updater endpoints"
gh_ver=$(gh release view "v$VERSION" --repo "$GH_REPO" --json tagName -q .tagName)
mir_ver=$(curl -s --noproxy '*' https://vkovalskii.com/copyosity/latest.json | python3 -c "import sys,json;print(json.load(sys.stdin)['version'])")
ad=$(python3 -c "import tarfile;print(len([x for x in tarfile.open('$OUT/Copyosity_aarch64.app.tar.gz').getnames() if x.rsplit('/',1)[-1].startswith('._')]))")
echo "  github tag : $gh_ver"
echo "  mirror ver : $mir_ver"
echo "  tarball AppleDouble entries (must be 0): $ad"
[ "$mir_ver" = "$VERSION" ] && [ "$ad" = "0" ] && echo "RELEASE $VERSION OK" || { echo "RELEASE VERIFY FAILED" >&2; exit 1; }
