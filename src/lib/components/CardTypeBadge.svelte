<script lang="ts">
  const {
    contentType,
    formatLabel = null,
    class: className = "",
  }: {
    contentType: string;
    /** Image format chip (PNG, GIF, …) when known; otherwise generic type label. */
    formatLabel?: string | null;
    class?: string;
  } = $props();

  const iconKind = $derived(
    contentType === "image" ? "image" : contentType === "text" ? "text" : "file",
  );

  const label = $derived(
    contentType === "image" && formatLabel
      ? formatLabel
      : contentType === "text"
        ? "Text"
        : contentType === "image"
          ? "Image"
          : "File",
  );

  const isFormatLabel = $derived(contentType === "image" && !!formatLabel);
</script>

<span class="card-type-badge {className}">
  <svg
    class="card-type-badge-icon"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width={iconKind === "image" ? "2" : "1.75"}
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    {#if iconKind === "text"}
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <path d="M14 2v6h6" />
      <path d="M8 13h8" />
      <path d="M8 17h6" />
    {:else if iconKind === "image"}
      <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
      <circle cx="8.5" cy="8.5" r="1.5" />
      <polyline points="21 15 16 10 5 21" />
    {:else}
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <path d="M14 2v6h6" />
    {/if}
  </svg>
  <span class="card-type-badge-label" class:card-type-badge-label--format={isFormatLabel}>
    {label}
  </span>
</span>

<style>
  .card-type-badge {
    display: inline-flex;
    align-items: center;
    gap: var(--card-type-gap);
    box-sizing: border-box;
    min-height: var(--size-card-action-hit);
    width: fit-content;
    padding: var(--card-type-pad);
    border-radius: var(--radius-pill);
    background: var(--surface-7);
    font-weight: 600;
    font-size: var(--font-size-sm);
    line-height: 1;
    letter-spacing: 0.02em;
    color: var(--color-text-secondary);
  }

  .card-type-badge-icon {
    width: var(--icon-size-card-type);
    height: var(--icon-size-card-type);
    flex-shrink: 0;
    opacity: 0.92;
  }

  .card-type-badge-label {
    flex-shrink: 0;
    white-space: nowrap;
  }

  .card-type-badge-label--format {
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
</style>
