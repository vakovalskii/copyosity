<script lang="ts">
  import type { ContentKind } from "$lib/overlay-filters";
  import SegmentControl from "$lib/components/SegmentControl.svelte";

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

<SegmentControl
  ariaLabel="Content type"
  ariaKind="pressed"
  items={segments.map((segment) => ({ id: segment.id, label: segment.label }))}
  value={value}
  onSelect={(id) => select(id as ContentKind)}
/>
