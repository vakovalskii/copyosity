#!/usr/bin/env bash
# npm 11+ warns on npm_config_devdir (used by node-gyp; set by some IDE sandboxes).
set -euo pipefail
unset npm_config_devdir 2>/dev/null || true
exec npm "$@"
