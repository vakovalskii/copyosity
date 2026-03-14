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
  />
</div>

<style>
  .search-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    min-width: 200px;
  }

  .search-icon {
    color: #666;
    flex-shrink: 0;
  }

  input {
    background: none;
    border: none;
    outline: none;
    color: #e0e0e0;
    font-size: 13px;
    width: 100%;
    font-family: inherit;
  }

  input::placeholder {
    color: #555;
  }
</style>
