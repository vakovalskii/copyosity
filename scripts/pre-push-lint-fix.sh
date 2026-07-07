#!/usr/bin/env bash
# Final auto-fix safety net before push: any file type (code or docs) gets
# Oxfmt/Oxlint/Stylelint fix applied. If fixing changes the tree, the push is
# blocked so the fix gets committed first — nothing unformatted leaves the repo.
set -euo pipefail

if [ "$#" -eq 0 ]; then
	exit 0
fi

files=("$@")
existing=()
for f in "${files[@]}"; do
	[ -f "$f" ] && existing+=("$f")
done

if [ "${#existing[@]}" -eq 0 ]; then
	exit 0
fi

js_ts_svelte=()
css_svelte=()
for f in "${existing[@]}"; do
	case "$f" in
	*.js | *.ts | *.svelte) js_ts_svelte+=("$f") ;;
	esac
	case "$f" in
	*.css | *.svelte) css_svelte+=("$f") ;;
	esac
done

if [ "${#js_ts_svelte[@]}" -gt 0 ]; then
	npx --no-install oxlint --fix "${js_ts_svelte[@]}"
fi

if [ "${#css_svelte[@]}" -gt 0 ]; then
	npx --no-install stylelint --fix "${css_svelte[@]}"
fi

npx --no-install oxfmt --write --no-error-on-unmatched-pattern "${existing[@]}"

if ! git diff --quiet -- "${existing[@]}"; then
	printf '%s\n' \
		"Auto-fix (Oxfmt/Oxlint/Stylelint) changed files in this push." \
		"Stage the changes, commit, and push again." >&2
	exit 1
fi
