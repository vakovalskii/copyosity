<script lang="ts">
  import type { ContentKind } from "$lib/overlay-filters";

  const {
    value = "all",
    onchange,
  }: {
    value?: ContentKind;
    onchange?: (kind: ContentKind) => void;
  } = $props();

  const segments: { id: ContentKind; label: string }[] = [
    { id: "all", label: "All" },
    { id: "text", label: "Text" },
    { id: "image", label: "Images" },
  ];

  function select(kind: ContentKind) {
    if (kind === value) return;
    onchange?.(kind);
  }
</script>

<div class="content-kind-segment" role="group" aria-label="Content type">
  {#each segments as segment (segment.id)}
    <button
      type="button"
      class="segment-btn app-btn"
      aria-pressed={value === segment.id}
      onclick={() => select(segment.id)}
    >
      {segment.label}
    </button>
  {/each}
</div>

<style>
  .content-kind-segment {
    display: inline-flex;
    align-items: stretch;
    gap: 2px;
    padding: 2px;
    border-radius: var(--radius-control);
    background: var(--surface-3);
    border: 1px solid var(--border-soft);
  }

  .segment-btn {
    min-width: 72px;
    height: 28px;
    padding: 0 12px;
    border: none;
    border-radius: var(--radius-control-sm);
    background: transparent;
    color: var(--color-text-secondary);
    font: inherit;
    font-size: var(--font-size-md);
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition:
      background var(--duration-fast) var(--ease-interactive),
      color var(--duration-fast) var(--ease-interactive),
      box-shadow var(--duration-fast) var(--ease-interactive);
  }

  .segment-btn:hover:not(:disabled, [aria-busy="true"]) {
    color: var(--color-text-body);
    background: var(--surface-5);
  }

  .segment-btn[aria-pressed="true"] {
    background: var(--surface-7);
    color: var(--color-text-primary);
    box-shadow: var(--shadow-inset-highlight);
    border: 1px solid var(--border-default);
  }

  .segment-btn[aria-pressed="true"]:hover:not(:disabled, [aria-busy="true"]) {
    background: var(--surface-8);
    color: var(--color-text-primary);
    border-color: var(--border-medium);
  }

  .segment-btn:focus-visible {
    outline: none;
    box-shadow: var(--ring-accent-input);
  }
</style>
