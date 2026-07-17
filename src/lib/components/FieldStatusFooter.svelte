<script lang="ts">
  let {
    message = "",
    tone = "ok",
  }: {
    message?: string;
    tone?: "ok" | "warn" | "neutral" | "fail";
  } = $props();

  const visible = $derived(!!message);
</script>

<div class="field-status-reveal" class:visible aria-hidden={!visible}>
  <div class="field-status-reveal-inner">
    {#if message}
      <div
        class="status-hint field-status-footer"
        class:ok={tone === "ok"}
        class:neutral={tone === "neutral"}
        class:warn={tone === "warn"}
        class:fail={tone === "fail"}
        role={tone === "fail" ? "alert" : "status"}
        aria-live="polite"
      >
        {message}
      </div>
    {/if}
  </div>
</div>
