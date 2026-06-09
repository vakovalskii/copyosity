<script lang="ts">
  let { value = "", onchange }: { value?: string; onchange?: (v: string) => void } = $props();

  let inputEl: HTMLInputElement | undefined = $state();

  function handleInput(e: Event) {
    const target = e.target as HTMLInputElement;
    onchange?.(target.value);
  }

  export function focus() {
    inputEl?.focus();
  }
</script>

<div class="search-bar">
  <svg class="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
    <circle cx="11" cy="11" r="8" />
    <line x1="21" y1="21" x2="16.65" y2="16.65" />
  </svg>
  <input
    bind:this={inputEl}
    type="text"
    placeholder="Search clipboard history..."
    {value}
    oninput={handleInput}
    tabindex="-1"
  />
</div>

<style>
  .search-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    background: var(--surface-6);
    border: 1px solid var(--border-soft);
    border-radius: 8px;
    min-width: 200px;
  }

  .search-icon {
    color: var(--color-text-subtle);
    flex-shrink: 0;
  }

  input {
    background: none;
    border: none;
    outline: none;
    color: var(--color-text-body);
    font-size: 13px;
    width: 100%;
    font-family: inherit;
  }

  input::placeholder {
    color: var(--color-text-faint);
  }
</style>
