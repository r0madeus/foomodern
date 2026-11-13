<script lang="ts">
  import { t } from './i18n.svelte'
  import { library } from './library.svelte'
</script>

<div class="flex h-full flex-col items-center justify-center gap-3 text-center text-zinc-500">
  {#if library.scanning}
    <p>{t("Scanning your music folders…")}</p>
  {:else}
    <p>{t("Your library is empty.")}</p>
    <button
      class="rounded-lg bg-teal-600 px-4 py-2 text-sm font-medium text-white hover:bg-teal-500"
      onclick={() => library.addFolder()}
    >
      {t("Add a music folder")}
    </button>
  {/if}
</div>
