#!/usr/bin/env bash
# npm/npx 11+ warns on npm_config_devdir (node-gyp; set by some IDE sandboxes).
set -euo pipefail
unset npm_config_devdir 2>/dev/null || true
if [ $# -eq 0 ]; then
  exec npm
fi
exec "$@"
