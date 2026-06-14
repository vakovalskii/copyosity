<script lang="ts">
  const {
    value = "",
    onchange,
  }: {
    value?: string;
    onchange?: (v: string) => void;
  } = $props();

  let inputEl: HTMLInputElement | undefined = $state();
  let focused = $state(false);

  function handleInput(e: Event) {
    const target = e.target as HTMLInputElement;
    onchange?.(target.value);
  }

  function handleClear() {
    onchange?.("");
    inputEl?.focus();
  }

  export function focus() {
    inputEl?.focus();
    inputEl?.select();
  }

  export function blur() {
    inputEl?.blur();
  }

  export function isFocused() {
    return focused;
  }
</script>

<div class="search-bar text-control-host" role="search">
  <svg
    class="search-icon"
    width="16"
    height="16"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    aria-hidden="true"
  >
    <circle cx="11" cy="11" r="8" />
    <line x1="21" y1="21" x2="16.65" y2="16.65" />
  </svg>
  <input
    bind:this={inputEl}
    type="search"
    placeholder="Search clipboard history..."
    aria-label="Search clipboard history"
    autocomplete="off"
    spellcheck="false"
    {value}
    oninput={handleInput}
    onfocus={() => {
      focused = true;
    }}
    onblur={() => {
      focused = false;
    }}
  />
  <button
    type="button"
    class="clear-btn"
    class:hidden={!value}
    aria-label="Clear search"
    aria-hidden={!value}
    tabindex={value ? 0 : -1}
    disabled={!value}
    onclick={handleClear}
  >
    <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
      <path
        d="M2.2 2.2 9.8 9.8M9.8 2.2 2.2 9.8"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
      />
    </svg>
  </button>
</div>

<style>
  .search-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px 6px 12px;
    border-radius: var(--radius-control);
    width: 280px;
    flex: 0 0 280px;
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
    font-size: var(--font-size-md);
    width: 100%;
    min-width: 0;
    font-family: inherit;
    user-select: text;
  }

  input::placeholder {
    color: var(--color-text-placeholder);
  }

  input::-webkit-search-cancel-button {
    display: none;
  }

  .clear-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: var(--surface-10);
    color: var(--color-text-tertiary);
    cursor: pointer;
    flex-shrink: 0;
    transition:
      background var(--duration-fast) var(--ease-interactive),
      color var(--duration-fast) var(--ease-interactive);
  }

  .clear-btn:hover {
    background: var(--surface-12);
    color: var(--color-text-secondary);
  }

  .clear-btn:focus-visible {
    outline: none;
    box-shadow: var(--ring-accent);
  }

  .clear-btn.hidden {
    visibility: hidden;
    pointer-events: none;
  }
</style>
