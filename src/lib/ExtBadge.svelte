<script lang="ts">
  import { fileExtension, prefs } from './prefs.svelte'

  let { path }: { path: string } = $props()
  const ext = $derived(fileExtension(path))
</script>

{#if prefs.showExtensions && ext}
  <span
    class="ml-2 shrink-0 rounded bg-zinc-200/80 px-1.5 py-px align-middle text-[10px] font-semibold tracking-wide text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300"
  >
    {ext}
  </span>
{/if}
