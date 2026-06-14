<script lang="ts">
  import { onMount, tick } from "svelte";
  import {
    getConfirmRequest,
    resolveConfirm,
    subscribeConfirm,
  } from "$lib/confirm";

  let request = $state(getConfirmRequest());
  let cancelButton = $state<HTMLButtonElement | null>(null);
  let confirmButton = $state<HTMLButtonElement | null>(null);
  let dialog = $state<HTMLDivElement | null>(null);

  onMount(() => {
    return subscribeConfirm(() => {
      request = getConfirmRequest();
    });
  });

  const SCROLL_KEYS = new Set([
    "ArrowDown",
    "ArrowUp",
    "ArrowLeft",
    "ArrowRight",
    "PageDown",
    "PageUp",
    "Home",
    "End",
    " ",
  ]);

  $effect(() => {
    if (!request || !dialog) return;
    void tick().then(() => {
      requestAnimationFrame(() => {
        dialog?.focus({ preventScroll: true });
      });
      return undefined;
    });
  });

  $effect(() => {
    if (!request) return;

    const root = document.documentElement;
    const body = document.body;
    const prevRootOverflow = root.style.overflow;
    const prevBodyOverflow = body.style.overflow;
    root.style.overflow = "hidden";
    body.style.overflow = "hidden";

    return () => {
      root.style.overflow = prevRootOverflow;
      body.style.overflow = prevBodyOverflow;
    };
  });

  function focusableButtons(): HTMLButtonElement[] {
    return [cancelButton, confirmButton].filter(
      (button): button is HTMLButtonElement => button !== null,
    );
  }

  function trapDialogKey(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!request) return;

    if (e.key === "Escape") {
      trapDialogKey(e);
      resolveConfirm(false);
      return;
    }

    if (SCROLL_KEYS.has(e.key)) {
      const active = document.activeElement;
      if (e.key === " " && active instanceof HTMLButtonElement && dialog?.contains(active)) {
        return;
      }
      trapDialogKey(e);
      return;
    }

    if (e.key !== "Tab") return;

    const buttons = focusableButtons();
    if (buttons.length === 0) {
      trapDialogKey(e);
      return;
    }

    const first = buttons[0];
    const last = buttons[buttons.length - 1];
    const active = document.activeElement;
    const activeIsButton = buttons.includes(active as HTMLButtonElement);
    const activeInsideDialog =
      active instanceof Node && dialog ? dialog.contains(active) : false;

    if (activeInsideDialog && !activeIsButton) {
      return;
    }

    if (!activeInsideDialog || !activeIsButton) {
      trapDialogKey(e);
      if (e.shiftKey) {
        last.focus({ preventScroll: true });
      } else {
        first.focus({ preventScroll: true });
      }
      return;
    }

    if (e.shiftKey && active === first) {
      trapDialogKey(e);
      last.focus({ preventScroll: true });
      return;
    }

    if (!e.shiftKey && active === last) {
      trapDialogKey(e);
      first.focus({ preventScroll: true });
    }
  }

  function handleBackdropClick() {
    resolveConfirm(false);
  }

  function handleDialogClick(e: MouseEvent) {
    e.stopPropagation();
  }
</script>

<svelte:window onkeydowncapture={handleKeydown} />

{#if request}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="confirm-backdrop" onclick={handleBackdropClick}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      bind:this={dialog}
      class="confirm-dialog"
      role="alertdialog"
      aria-modal="true"
      tabindex="-1"
      aria-labelledby="confirm-dialog-title"
      aria-describedby="confirm-dialog-message"
      onclick={handleDialogClick}
    >
      <h2 id="confirm-dialog-title" class="confirm-title">{request.title}</h2>
      <div id="confirm-dialog-message" class="confirm-message">
        {#if request.messageBody}
          <p class="confirm-message-primary">
            {#each request.messageBody.primary as part, index (index)}
              {#if part.emph}<strong>{part.text}</strong>{:else}{part.text}{/if}
            {/each}
          </p>
          {#if request.messageBody.secondary?.length}
            <p class="confirm-message-secondary">
              {#each request.messageBody.secondary as part, index (index)}
                {#if part.emph}<strong>{part.text}</strong>{:else}{part.text}{/if}
              {/each}
            </p>
          {/if}
        {:else}
          <p class="confirm-message-primary">{request.message}</p>
        {/if}
      </div>
      <div class="confirm-actions">
        <button
          bind:this={cancelButton}
          class="form-btn form-btn-ghost app-btn"
          type="button"
          onclick={() => resolveConfirm(false)}
        >
          {request.cancelLabel}
        </button>
        <button
          bind:this={confirmButton}
          class="form-btn app-btn"
          class:form-btn-danger={request.destructiveConfirm}
          class:form-btn-secondary={!request.destructiveConfirm}
          type="button"
          onclick={() => resolveConfirm(true)}
        >
          {request.confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .confirm-backdrop {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1.25rem;
    background: rgb(0 0 0 / 45%);
  }

  .confirm-dialog {
    width: min(100%, 22rem);
    padding: 1rem 1.125rem;
    background: var(--surface-menu);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-surface);
    box-shadow: var(--shadow-elevated);
  }

  .confirm-title {
    margin: 0 0 0.5rem;
    font-size: var(--font-size-md);
    font-weight: 600;
    color: var(--color-text-primary);
  }

  .confirm-message {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
  }

  .confirm-message-primary {
    margin: 0;
    font-size: var(--font-size-sm);
    line-height: 1.45;
    color: var(--color-text-secondary);
  }

  .confirm-message-primary strong {
    font-weight: 600;
    color: var(--color-text-primary);
    white-space: nowrap;
  }

  .confirm-message-secondary {
    margin: 0;
    font-size: var(--font-size-xs);
    line-height: 1.4;
    color: var(--color-text-tertiary);
  }

  .confirm-message-secondary strong {
    font-weight: 600;
    color: var(--color-text-secondary);
    white-space: nowrap;
  }

  .confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1rem;
  }
</style>
