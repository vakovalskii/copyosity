#!/usr/bin/env bash
# Publish a Copyosity release to the self-hosted update mirror.
#
# The Tauri updater tries endpoints in order (see src-tauri/tauri.conf.json):
#   1. https://vkovalskii.com/copyosity/latest.json   <- this mirror
#   2. https://github.com/.../releases/latest/download/latest.json  (fallback)
#
# The mirror is served by the `vkovalskii-nginx` container on the live origin
# (DNS vkovalskii.com -> Yandex Cloud box, ssh alias `srv-rnd-demos-mcp`), from
# a `location /copyosity/` alias into the certbot-webroot docker volume.
#
# This script takes a release directory that already contains the dual-arch
# updater tarballs + minisign sigs and the GitHub latest.json, rewrites the
# manifest URLs to the mirror, uploads everything, and reloads nginx.
#
# Usage: scripts/publish-mirrors.sh <release-dir>
#   <release-dir> must contain:
#     Copyosity_aarch64.app.tar.gz(.sig)
#     Copyosity_x64.app.tar.gz(.sig)
#     latest.json                     (the GitHub manifest; used for version/notes/sigs)
set -euo pipefail

REL="${1:?usage: publish-mirrors.sh <release-dir>}"
[ -d "$REL" ] || { echo "ERROR: release dir not found: $REL" >&2; exit 1; }

# --- mirror target -----------------------------------------------------------
MIRROR_SSH="${MIRROR_SSH:-srv-rnd-demos-mcp}"          # live origin (ssh alias)
MIRROR_BASE_URL="${MIRROR_BASE_URL:-https://vkovalskii.com/copyosity}"
NGINX_CONTAINER="${NGINX_CONTAINER:-vkovalskii-nginx}"
VOL="${VOL:-/var/lib/docker/volumes/vkovalskii-site_certbot-webroot/_data/copyosity}"
STAGING="${STAGING:-copyosity-mirror}"                 # ~/<STAGING> on the origin

AARCH_TGZ="Copyosity_aarch64.app.tar.gz"
X64_TGZ="Copyosity_x64.app.tar.gz"

for f in "$AARCH_TGZ" "$AARCH_TGZ.sig" "$X64_TGZ" "$X64_TGZ.sig" latest.json; do
  [ -f "$REL/$f" ] || { echo "ERROR: missing $REL/$f" >&2; exit 1; }
done

# --- build the mirror manifest (URLs -> mirror) ------------------------------
echo ">> generating mirror latest.json"
python3 - "$REL" "$MIRROR_BASE_URL" <<'PY'
import json, sys, os
rel, base = sys.argv[1], sys.argv[2].rstrip("/")
d = json.load(open(os.path.join(rel, "latest.json")))
for key, plat in d.get("platforms", {}).items():
    fname = plat["url"].rsplit("/", 1)[-1]        # keep the tarball filename
    plat["url"] = f"{base}/{fname}"
json.dump(d, open(os.path.join(rel, "latest.mirror.json"), "w"), indent=2)
print("mirror manifest v%s: %s" % (d["version"], ", ".join(d["platforms"])))
PY

# --- upload to staging, then move into the docker volume ---------------------
echo ">> uploading assets to $MIRROR_SSH:~/$STAGING"
ssh -o ConnectTimeout=25 "$MIRROR_SSH" "mkdir -p ~/$STAGING"
scp -o ConnectTimeout=25 \
  "$REL/$AARCH_TGZ" "$REL/$AARCH_TGZ.sig" \
  "$REL/$X64_TGZ" "$REL/$X64_TGZ.sig" \
  "$MIRROR_SSH:~/$STAGING/"
scp -o ConnectTimeout=25 "$REL/latest.mirror.json" "$MIRROR_SSH:~/$STAGING/latest.json"

echo ">> installing into volume + reloading nginx"
# SIGHUP reload avoids `docker exec` (the origin's /run tmpfs can be full).
ssh -o ConnectTimeout=25 "$MIRROR_SSH" bash -s -- "$VOL" "$STAGING" "$NGINX_CONTAINER" <<'REMOTE'
set -euo pipefail
VOL="$1"; STAGING="$2"; NGINX="$3"
sudo -n mkdir -p "$VOL"
for f in "$HOME/$STAGING"/*; do sudo -n cp "$f" "$VOL/"; done
sudo -n sh -c "chmod 755 '$VOL' && chmod 644 '$VOL'/*"
docker kill -s HUP "$NGINX" >/dev/null && echo "nginx reloaded (SIGHUP)"
REMOTE

# --- verify ------------------------------------------------------------------
echo ">> verifying mirror"
VER=$(python3 -c "import json;print(json.load(open('$REL/latest.json'))['version'])")
code=$(curl -s --noproxy '*' -o /dev/null -w '%{http_code}' "$MIRROR_BASE_URL/latest.json")
got=$(curl -s --noproxy '*' "$MIRROR_BASE_URL/latest.json" | python3 -c "import sys,json;print(json.load(sys.stdin)['version'])" 2>/dev/null || echo '?')
tcode=$(curl -s --noproxy '*' -o /dev/null -w '%{http_code}' "$MIRROR_BASE_URL/$AARCH_TGZ")
echo "  latest.json: HTTP $code (version $got, expected $VER)"
echo "  $AARCH_TGZ: HTTP $tcode"
if [ "$code" = "200" ] && [ "$got" = "$VER" ] && [ "$tcode" = "200" ]; then
  echo "OK: mirror serving v$VER at $MIRROR_BASE_URL"
else
  echo "WARN: mirror verification did not fully pass" >&2
  exit 1
fi
