<script lang="ts">
  export type KeyboardHint =
    | string
    | {
        /** Mouse or gesture label shown in trigger chrome (e.g. Click, Double-click). */
        prefix?: string;
        keys?: string | string[];
        action?: string;
      };

  const {
    hints,
    align = "center",
  }: {
    hints: KeyboardHint[];
    align?: "start" | "center";
  } = $props();
</script>

<div class="keyboard-hints" class:align-start={align === "start"} aria-hidden="true">
  {#each hints as hint, i}
    {#if i > 0}
      <span class="hint-sep" aria-hidden="true">·</span>
    {/if}
    <span class="hint-item">
      {#if typeof hint === "string"}
        <span class="hint-text">{hint}</span>
      {:else}
        <span class="hint-triggers">
          {#if hint.prefix}
            <span class="hint-trigger">{hint.prefix}</span>
          {/if}
          {#if hint.keys}
            {#if Array.isArray(hint.keys)}
              {#each hint.keys as key}
                <kbd class="hint-trigger">{key}</kbd>
              {/each}
            {:else}
              <kbd class="hint-trigger">{hint.keys}</kbd>
            {/if}
          {/if}
        </span>
        {#if hint.action}
          <span class="hint-action">{hint.action}</span>
        {/if}
      {/if}
    </span>
  {/each}
</div>

<style>
  .keyboard-hints {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: 0;
    font-size: var(--font-size-2xs);
    line-height: var(--line-height-tight);
    color: var(--color-text-faint);
    font-weight: 400;
    letter-spacing: 0.01em;
    user-select: none;
  }

  .keyboard-hints.align-start {
    justify-content: flex-start;
  }

  .hint-sep {
    display: inline-flex;
    align-items: center;
    align-self: center;
    margin: 0 0.375rem;
    font-size: 1.25em;
    line-height: 1;
    font-weight: 500;
    opacity: 0.5;
  }

  .hint-item {
    display: inline-flex;
    align-items: center;
    gap: 0.3125rem;
    white-space: nowrap;
  }

  .hint-triggers {
    display: inline-flex;
    align-items: center;
    gap: 0.125rem;
    flex-shrink: 0;
  }

  .hint-trigger,
  kbd.hint-trigger {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    box-sizing: border-box;
    height: 1.125rem;
    padding: 0 6px;
    font-family: inherit;
    font-size: 1em;
    font-weight: 500;
    line-height: 1;
    border-radius: var(--radius-code);
    background: var(--surface-kbd);
    border: 1px solid var(--border-kbd);
    color: var(--color-text-subtle);
    box-shadow: var(--shadow-kbd);
  }

  .hint-action {
    display: inline-flex;
    align-items: center;
    line-height: 1;
    color: var(--color-text-faint);
    font-weight: 400;
  }

  .hint-text {
    color: var(--color-text-faint);
  }
</style>
