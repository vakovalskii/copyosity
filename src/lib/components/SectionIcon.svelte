<script lang="ts">
  export type SectionIconName =
    | "permissions"
    | "ai-tagging"
    | "ollama-model"
    | "this-mac"
    | "storage"
    | "privacy"
    | "voice"
    | "recording"
    | "setup"
    | "clipboard-panel"
    | "hub";

  const {
    name,
    class: className = "form-title-icon",
  }: {
    name: SectionIconName;
    class?: string;
  } = $props();
</script>

<svg
  class={className}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === "permissions"}
    <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10" />
    <path d="m9 12 2 2 4-4" />
  {:else if name === "ai-tagging"}
    <path d="M12.586 2.586A2 2 0 0 0 11.172 2H4a2 2 0 0 0-2 2v7.172a2 2 0 0 0 .586 1.414l8.704 8.704a2.426 2.426 0 0 0 3.42 0l6.58-6.58a2.426 2.426 0 0 0 0-3.42z" />
    <circle cx="7.5" cy="7.5" r=".5" fill="currentColor" stroke="none" />
  {:else if name === "ollama-model"}
    <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
    <path d="M3.29 7 12 12l8.71-5" />
    <path d="M12 22V12" />
  {:else if name === "this-mac"}
    <rect x="6" y="6" width="12" height="12" rx="1.25" />
    <path d="M9 6V4M12 6V4M15 6V4M9 18v2M12 18v2M15 18v2M6 9H4M6 12H4M6 15H4M18 9h2M18 12h2M18 15h2" />
  {:else if name === "storage"}
    <ellipse cx="12" cy="5" rx="9" ry="3" />
    <path d="M3 5v6c0 1.66 4.03 3 9 3s9-1.34 9-3V5" />
    <path d="M3 11v6c0 1.66 4.03 3 9 3s9-1.34 9-3v-6" />
  {:else if name === "privacy"}
    <rect x="3" y="11" width="18" height="11" rx="2" />
    <path d="M7 11V7a5 5 0 0 1 10 0v4" />
  {:else if name === "voice"}
    <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
    <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
    <path d="M12 19v3" />
  {:else if name === "recording"}
    <path d="M2 10v4" />
    <path d="M6 6v12" />
    <path d="M10 8v8" />
    <path d="M14 4v16" />
    <path d="M18 8v8" />
    <path d="M22 10v4" />
  {:else if name === "setup"}
    <path d="M11 18H3" />
    <path d="M11 12H3" />
    <path d="M11 6H3" />
    <path d="m15 18 2 2 4-4" />
    <path d="m15 6 2 2 4-4" />
  {:else if name === "clipboard-panel"}
    <rect x="4" y="5" width="16" height="14" rx="2" />
    <path d="M4 9h16" />
    <path d="M8 13h8" />
    <path d="M8 16h5" />
  {:else if name === "hub"}
    <path d="M12 3 4.5 8.5v7L12 21l7.5-5.5v-7L12 3z" />
    <path d="M12 8v8" />
    <path d="M8.5 10.5 12 8l3.5 2.5" />
  {/if}
</svg>
