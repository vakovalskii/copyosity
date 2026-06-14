<script lang="ts">
  import { onMount } from "svelte";

  import ChevronDown from "$lib/components/ChevronDown.svelte";

  export type ActionMenuItem = {
    id: string;
    label: string;
    disabled?: boolean;
    destructive?: boolean;
  };

  const TYPEAHEAD_RESET_MS = 700;

  const {
    label,
    items,
    block = false,
    disabled = false,
    onselect,
  }: {
    label: string;
    items: ActionMenuItem[];
    block?: boolean;
    disabled?: boolean;
    onselect: (id: string) => void;
  } = $props();

  const menuId = `action-menu-${crypto.randomUUID()}`;

  let open = $state(false);
  let root = $state<HTMLDivElement | null>(null);
  let trigger = $state<HTMLButtonElement | null>(null);
  let panel = $state<HTMLDivElement | null>(null);
  let activeIndex = $state(-1);
  let typeahead = $state("");
  let typeaheadTimer: ReturnType<typeof setTimeout> | undefined;

  const menuDisabled = $derived(disabled || items.every((item) => item.disabled));

  $effect(() => {
    if (menuDisabled) open = false;
  });

  function firstEnabledIndex(): number {
    return items.findIndex((item) => !item.disabled);
  }

  function lastEnabledIndex(): number {
    for (let index = items.length - 1; index >= 0; index -= 1) {
      if (!items[index]?.disabled) return index;
    }
    return -1;
  }

  function stepEnabledIndex(from: number, direction: 1 | -1): number {
    const enabledIndexes = items
      .map((item, index) => (!item.disabled ? index : -1))
      .filter((index) => index >= 0);

    if (enabledIndexes.length === 0) return -1;

    const currentPosition = enabledIndexes.indexOf(from);
    const startPosition =
      currentPosition >= 0 ? currentPosition : direction === 1 ? -1 : 0;
    const nextPosition =
      (startPosition + direction + enabledIndexes.length) % enabledIndexes.length;

    return enabledIndexes[nextPosition] ?? -1;
  }

  function focusItemAt(index: number) {
    if (!panel || index < 0) return;
    const buttons = panel.querySelectorAll<HTMLButtonElement>('[role="menuitem"]');
    buttons[index]?.focus({ preventScroll: true });
  }

  function enabledItemButtons(): HTMLButtonElement[] {
    if (!panel) return [];
    return Array.from(
      panel.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)'),
    );
  }

  function resetTypeahead() {
    typeahead = "";
    if (typeaheadTimer) {
      clearTimeout(typeaheadTimer);
      typeaheadTimer = undefined;
    }
  }

  function scheduleTypeaheadReset() {
    if (typeaheadTimer) clearTimeout(typeaheadTimer);
    typeaheadTimer = setTimeout(() => {
      typeahead = "";
      typeaheadTimer = undefined;
    }, TYPEAHEAD_RESET_MS);
  }

  function findTypeaheadIndex(prefix: string, startAfter: number): number {
    const lowerPrefix = prefix.toLowerCase();
    const enabledItems = items
      .map((item, index) => ({ item, index }))
      .filter(({ item }) => !item.disabled);
    const searchOrder = [
      ...enabledItems.filter(({ index }) => index > startAfter),
      ...enabledItems.filter(({ index }) => index <= startAfter),
    ];

    return (
      searchOrder.find(({ item }) => item.label.toLowerCase().startsWith(lowerPrefix))?.index ??
      -1
    );
  }

  function openMenu() {
    if (menuDisabled) return;
    open = true;
    activeIndex = -1;
    resetTypeahead();
  }

  function closeMenu(returnFocus = true) {
    open = false;
    activeIndex = -1;
    resetTypeahead();
    if (returnFocus) trigger?.focus({ preventScroll: true });
  }

  function toggleMenu() {
    if (menuDisabled) return;
    if (open) {
      closeMenu(true);
      return;
    }
    openMenu();
  }

  function setActiveIndex(index: number, focus = false) {
    if (index < 0 || items[index]?.disabled) return;
    activeIndex = index;
    if (focus) focusItemAt(index);
  }

  function moveActive(direction: 1 | -1, focus = false) {
    const nextIndex =
      activeIndex < 0
        ? direction === 1
          ? firstEnabledIndex()
          : lastEnabledIndex()
        : stepEnabledIndex(activeIndex, direction);
    setActiveIndex(nextIndex, focus);
  }

  function handleSelect(id: string, isDisabled?: boolean) {
    if (isDisabled) return;
    closeMenu(true);
    onselect(id);
  }

  function handleDocumentPointerDown(e: PointerEvent) {
    if (!open || !root) return;
    if (!root.contains(e.target as Node)) closeMenu(false);
  }

  function handleTypeaheadKey(key: string) {
    const nextTypeahead = typeahead + key.toLowerCase();
    const matchIndex = findTypeaheadIndex(nextTypeahead, activeIndex);
    typeahead = nextTypeahead;
    scheduleTypeaheadReset();

    if (matchIndex >= 0) {
      setActiveIndex(matchIndex, true);
      return;
    }

    const singleCharIndex = findTypeaheadIndex(key.toLowerCase(), activeIndex);
    if (singleCharIndex >= 0) {
      typeahead = key.toLowerCase();
      scheduleTypeaheadReset();
      setActiveIndex(singleCharIndex, true);
    }
  }

  function isPrintableKey(e: KeyboardEvent): boolean {
    return e.key.length === 1 && !e.metaKey && !e.ctrlKey && !e.altKey;
  }

  function handleMenuKeydown(e: KeyboardEvent) {
    if (!open) return;

    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        e.stopPropagation();
        moveActive(1, true);
        break;
      case "ArrowUp":
        e.preventDefault();
        e.stopPropagation();
        moveActive(-1, true);
        break;
      case "Home":
        e.preventDefault();
        e.stopPropagation();
        setActiveIndex(firstEnabledIndex(), true);
        break;
      case "End":
        e.preventDefault();
        e.stopPropagation();
        setActiveIndex(lastEnabledIndex(), true);
        break;
      case "Enter":
      case " ":
        e.preventDefault();
        e.stopPropagation();
        if (activeIndex >= 0) {
          const item = items[activeIndex];
          if (item) handleSelect(item.id, item.disabled);
        }
        break;
      case "Escape":
        e.preventDefault();
        e.stopPropagation();
        closeMenu(true);
        break;
      case "Tab": {
        const enabledButtons = enabledItemButtons();
        if (enabledButtons.length === 0) {
          e.preventDefault();
          e.stopPropagation();
          break;
        }

        const active = document.activeElement;
        const first = enabledButtons[0];
        const last = enabledButtons[enabledButtons.length - 1];

        if (e.shiftKey) {
          if (active === first || !enabledButtons.includes(active as HTMLButtonElement)) {
            e.preventDefault();
            e.stopPropagation();
            last.focus({ preventScroll: true });
          }
          break;
        }

        if (active === last || !enabledButtons.includes(active as HTMLButtonElement)) {
          e.preventDefault();
          e.stopPropagation();
          first.focus({ preventScroll: true });
        }
        break;
      }
      default:
        if (isPrintableKey(e)) {
          e.preventDefault();
          e.stopPropagation();
          handleTypeaheadKey(e.key);
        }
        break;
    }
  }

  function handleTriggerKeydown(e: KeyboardEvent) {
    if (menuDisabled) return;

    if (open) {
      handleMenuKeydown(e);
      return;
    }

    switch (e.key) {
      case "ArrowDown":
      case "Enter":
      case " ":
        e.preventDefault();
        openMenu();
        break;
      case "ArrowUp":
        e.preventDefault();
        openMenu();
        break;
      default:
        break;
    }
  }

  onMount(() => {
    document.addEventListener("pointerdown", handleDocumentPointerDown);
    return () => document.removeEventListener("pointerdown", handleDocumentPointerDown);
  });

  function handleFocusOut(e: FocusEvent) {
    if (!open || !root) return;
    const nextTarget = e.relatedTarget;
    if (nextTarget instanceof Node && root.contains(nextTarget)) return;

    requestAnimationFrame(() => {
      if (!open || !root) return;
      const active = document.activeElement;
      if (active instanceof Node && root.contains(active)) return;
      closeMenu(false);
    });
  }
</script>

<div
  class="action-menu"
  class:block
  bind:this={root}
  onkeydowncapture={handleMenuKeydown}
  onfocusout={handleFocusOut}
>
  <button
    bind:this={trigger}
    class="form-btn form-btn-danger app-btn action-menu-trigger"
    type="button"
    disabled={menuDisabled}
    aria-haspopup="menu"
    aria-expanded={open && !menuDisabled}
    aria-controls={menuId}
    onclick={toggleMenu}
    onkeydown={handleTriggerKeydown}
  >
    <span class="action-menu-label">{label}</span>
    <ChevronDown />
  </button>

  {#if open && !menuDisabled}
    <div
      bind:this={panel}
      id={menuId}
      class="action-menu-panel"
      role="menu"
      aria-label={label}
    >
      {#each items as item, index (item.id)}
        <button
          class="action-menu-item app-btn"
          class:destructive={item.destructive}
          type="button"
          role="menuitem"
          tabindex={item.disabled ? -1 : 0}
          disabled={item.disabled}
          onclick={() => handleSelect(item.id, item.disabled)}
          onmouseenter={() => setActiveIndex(index)}
          onfocus={() => setActiveIndex(index)}
        >
          {item.label}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .action-menu {
    position: relative;
    display: inline-flex;
  }

  .action-menu.block {
    display: flex;
    width: 100%;
  }

  .action-menu-trigger {
    gap: 0.5rem;
  }

  .action-menu.block .action-menu-trigger {
    width: 100%;
    justify-content: space-between;
  }

  .action-menu-label {
    min-width: 0;
    text-align: left;
  }

  .action-menu-panel {
    position: absolute;
    top: calc(100% + 0.375rem);
    left: 0;
    z-index: 50;
    display: flex;
    flex-direction: column;
    width: 100%;
    min-width: 100%;
    padding: 0.25rem;
    background: var(--surface-menu);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-control);
    box-shadow: var(--shadow-elevated);
  }

  .action-menu-item {
    justify-content: flex-start;
    width: 100%;
    padding: 0.4375rem 0.625rem;
    background: transparent;
    border: none;
    border-radius: calc(var(--radius-control) - 2px);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    text-align: left;
    white-space: nowrap;
  }

  .action-menu-item:hover:not(:disabled),
  .action-menu-item:focus-visible {
    background: var(--surface-menu-hover);
  }

  .action-menu-item.destructive {
    color: var(--color-danger-text);
  }

  .action-menu-item.destructive:hover:not(:disabled),
  .action-menu-item.destructive:focus-visible {
    background: var(--surface-menu-hover-destructive);
    color: var(--color-danger-text-hover);
  }

  .action-menu-item:disabled {
    opacity: var(--opacity-section-disabled);
    cursor: not-allowed;
  }
</style>
