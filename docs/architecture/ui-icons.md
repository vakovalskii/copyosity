# UI icons

Read when adding or changing settings, overlay, or palette icons.

## Policy

- Product UI uses inline stroke SVG (`SectionIcon.svelte`, `ChevronDown.svelte`, and component-local paths).
- Icon sizes come from `--icon-size-*` in `tokens.css`.
- Do not introduce SF Symbols stacks, icon fonts, or external icon libraries unless explicitly changing that policy.

## Related code

- `src/lib/components/SectionIcon.svelte`
- `src/lib/components/ChevronDown.svelte`
- `src/lib/styles/tokens.css` (`--icon-size-*`)
