<script lang="ts">
  import SectionIcon from "./SectionIcon.svelte";
  import ShortcutField from "./ShortcutField.svelte";

  let {
    value = $bindable(""),
    placeholder,
    examples,
    detail,
    notice,
    noticeTone = "ok",
    onSave,
  }: {
    value?: string;
    placeholder?: string;
    examples: string[];
    detail?: string;
    notice?: string;
    noticeTone?: "ok" | "warn" | "neutral" | "fail";
    onSave: () => void | Promise<void>;
  } = $props();
</script>

<div class="form-subsection">
  <div class="form-subsection-title form-subsection-title--with-icon">
    <SectionIcon name="hotkey" class="form-subsection-icon" />Hotkey
  </div>
  <div class="inset-list">
    <ShortcutField
      bind:value={value}
      {placeholder}
      ariaLabel="Hotkey"
      {examples}
      {detail}
      {notice}
      {noticeTone}
      {onSave}
    />
  </div>
</div>
