<script lang="ts">
  type SegmentItem = {
    id: string;
    label: string;
    title?: string;
  };

  const {
    items,
    value = null,
    onSelect,
    onButtonMount,
    ariaLabel = "Segmented control",
    ariaKind = "selected",
    compact = false,
    rootClass = "",
  }: {
    items: SegmentItem[];
    value?: string | null;
    onSelect?: (id: string) => void;
    onButtonMount?: (id: string, el: HTMLButtonElement) => void;
    ariaLabel?: string;
    ariaKind?: "selected" | "pressed";
    compact?: boolean;
    rootClass?: string;
  } = $props();

  const resolvedValue = $derived(value ?? null);

  function select(id: string) {
    if (resolvedValue === id) return;
    onSelect?.(id);
  }

  function registerButton(el: HTMLButtonElement, id: string) {
    onButtonMount?.(id, el);
  }
</script>

<div
  class={`segment-track segment-control segment-control--grid ${compact ? "segment-control--compact" : ""} ${rootClass}`}
  role={ariaKind === "selected" ? "tablist" : "group"}
  aria-label={ariaLabel}
  style={`--segment-count: ${items.length};`}
>
  {#each items as item (item.id)}
    <button
      type="button"
      class="segment-item app-btn"
      role={ariaKind === "selected" ? "tab" : undefined}
      aria-selected={ariaKind === "selected" ? resolvedValue === item.id : undefined}
      aria-pressed={ariaKind === "pressed" ? resolvedValue === item.id : undefined}
      title={item.title}
      use:registerButton={item.id}
      onclick={() => select(item.id)}
    >
      {item.label}
    </button>
  {/each}
</div>

<style>
  .segment-control--grid {
    display: grid;
    grid-template-columns: repeat(var(--segment-count), minmax(0, 1fr));
    align-items: stretch;
  }

  /* Normalize vertical alignment/states across all usages. */
  .segment-control .segment-item {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: auto;
    min-height: 0;
    min-width: 0;
    align-self: stretch;

    /* Match preview segments for consistent hitbox + label centering. */
    padding-inline: 10px;
  }

  .segment-control--compact .segment-item {
    padding-inline: 6px;
    font-size: var(--font-size-xs);
    letter-spacing: 0.02em;
  }
</style>

