<script lang="ts">
  import FieldStatusFooter from "./FieldStatusFooter.svelte";
  import ShortcutModifierHint from "./ShortcutModifierHint.svelte";

  let {
    value = $bindable(""),
    placeholder,
    ariaLabel,
    examples,
    detail,
    notice,
    noticeTone = "ok",
    onSave,
    saveLabel = "Save",
    saveDisabled = false,
  }: {
    value?: string;
    placeholder?: string;
    ariaLabel: string;
    examples: string[];
    detail?: string;
    notice?: string;
    noticeTone?: "ok" | "warn" | "neutral" | "fail";
    onSave?: () => void | Promise<void>;
    saveLabel?: string;
    saveDisabled?: boolean;
  } = $props();

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && onSave) {
      void onSave();
    }
  }
</script>

<div class="form-field form-field--with-status-footer">
  <div class="form-field-main">
    <div class="form-inline">
      <input
        class="form-input"
        type="text"
        {placeholder}
        aria-label={ariaLabel}
        bind:value={value}
        onkeydown={handleKeydown}
      />
      {#if onSave}
        <button
          class="form-btn form-btn-secondary app-btn"
          type="button"
          disabled={saveDisabled}
          onclick={() => void onSave()}
        >
          {saveLabel}
        </button>
      {/if}
    </div>
    <ShortcutModifierHint {examples} {detail} />
  </div>
  <FieldStatusFooter message={notice} tone={noticeTone} />
</div>
