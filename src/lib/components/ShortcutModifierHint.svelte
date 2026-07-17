<script lang="ts">
  const {
    examples,
    detail,
  }: {
    /** Shortcut strings shown on the examples line, e.g. `cmd+shift+c`. */
    examples: string[];
    /** Optional third line — behavior or scope (kept separate from examples per HIG). */
    detail?: string;
  } = $props();
</script>

<div class="form-hint form-hint--stacked">
  <span class="form-hint-line">
    Use: <code>cmd</code>, <code>option</code>, <code>ctrl</code>, <code>shift</code> + key.
  </span>
  <span class="form-hint-line">
    Examples:
    {#each examples as example, index (example)}
      {#if index > 0}, {/if}<code>{example}</code>
    {/each}
  </span>
  {#if detail}
    <span class="form-hint-line">{detail}</span>
  {/if}
</div>
