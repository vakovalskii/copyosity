<script lang="ts">
  import { onMount } from "svelte";
  import { initInputModality } from "$lib/input-modality";
  import "$lib/styles/tokens.css";
  import "$lib/styles/button-interaction.css";
  import "$lib/styles/form-controls.css";

  const { children } = $props();

  onMount(initInputModality);
</script>

{@render children()}
