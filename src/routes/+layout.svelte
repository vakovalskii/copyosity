<script lang="ts">
  import { onMount } from "svelte";
  import { initInputModality } from "$lib/input-modality";
  import "$lib/styles/tokens.css";
  import "$lib/styles/window-chrome.css";
  import "$lib/styles/focus-rings.css";
  import "$lib/styles/button-interaction.css";
  import "$lib/styles/form-controls.css";
  import "$lib/styles/segment-control.css";
  import "$lib/styles/overlay-chrome.css";
  import "$lib/styles/overlay-filters.css";

  const { children } = $props();

  onMount(initInputModality);
</script>

{@render children()}
