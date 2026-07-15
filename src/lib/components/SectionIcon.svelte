<script lang="ts">
  export type SectionIconName =
    | "permissions"
    | "ai-tagging"
    | "text-polish"
    | "ollama-model"
    | "this-mac"
    | "storage"
    | "privacy"
    | "voice"
    | "recording"
    | "setup"
    | "clipboard-panel"
    | "hub"
    | "hotkey"
    | "snippets"
    | "general"
    | "globe"
    | "updates"
    | "version"
    | "update-log"
    | "connection"
    | "gauge"
    | "login";

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
  {:else if name === "text-polish"}
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h7" />
    <path d="M14 2v6h6" />
    <path d="M8 13h5M8 17h4" />
    <path d="m18 14 1 2 2 1-2 1-1 2-1-2-2-1 2-1z" />
    <path d="M15 20h2" />
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
  {:else if name === "hotkey"}
    <rect x="2" y="6" width="20" height="12" rx="2" />
    <path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8" />
  {:else if name === "snippets"}
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
    <path d="M14 2v6h6" />
    <path d="M8 13h8M8 17h5M8 9h2" />
  {:else if name === "general"}
    <path d="M4 21v-7" />
    <path d="M4 10V3" />
    <circle cx="4" cy="14" r="2" />
    <path d="M12 21v-9" />
    <path d="M12 8V3" />
    <circle cx="12" cy="12" r="2" />
    <path d="M20 21v-5" />
    <path d="M20 12V3" />
    <circle cx="20" cy="17" r="2" />
  {:else if name === "globe"}
    <circle cx="12" cy="12" r="10" />
    <path d="M2 12h20" />
    <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
  {:else if name === "updates"}
    <path d="M12 3v10" />
    <path d="m7 10 5 5 5-5" />
    <path d="M5 21h14" />
  {:else if name === "version"}
    <circle cx="12" cy="12" r="10" />
    <path d="m9 12 2 2 4-4" />
  {:else if name === "update-log"}
    <polyline points="4 17 10 11 4 5" />
    <line x1="12" x2="20" y1="19" y2="19" />
    <line x1="12" x2="20" y1="15" y2="15" />
    <line x1="12" x2="20" y1="11" y2="11" />
  {:else if name === "connection"}
    <path d="M9 17H7A5 5 0 0 1 7 7h2" />
    <path d="M15 7h2a5 5 0 1 1 0 10h-2" />
    <line x1="8" x2="16" y1="12" y2="12" />
  {:else if name === "gauge"}
    <path d="m12 14 4-4" />
    <path d="M3.34 19a10 10 0 1 1 17.32 0" />
  {:else if name === "login"}
    <path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4" />
    <polyline points="10 17 15 12 10 7" />
    <line x1="15" x2="3" y1="12" y2="12" />
  {:else}
    {@const exhaustive: never = name}
  {/if}
</svg>
