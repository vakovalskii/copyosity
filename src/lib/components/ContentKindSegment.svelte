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

<div class="segment-track" role="group" aria-label="Content type">
  {#each segments as segment (segment.id)}
    <button
      type="button"
      class="segment-item app-btn"
      aria-pressed={value === segment.id}
      onclick={() => select(segment.id)}
    >
      {segment.label}
    </button>
  {/each}
</div>
