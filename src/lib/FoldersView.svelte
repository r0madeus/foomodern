<script lang="ts">
  import { count, t } from './i18n.svelte'
  import Icon from './Icon.svelte'
  import { library } from './library.svelte'

  let { embedded = false }: { embedded?: boolean } = $props()
</script>

<div class={embedded ? '' : 'mx-auto max-w-2xl px-5 pb-6'}>
  <div class="mb-3 flex items-center justify-between gap-3">
    {#if embedded}
      <p class="text-sm font-medium">{t("Music folders")}</p>
    {:else}
      <h2 class="text-lg font-semibold tracking-tight">{t("Music folders")}</h2>
    {/if}
    <div class="flex gap-2">
      <button
        class="rounded-full border border-zinc-200 px-3.5 py-1.5 text-xs font-medium text-zinc-600 transition hover:bg-zinc-100 disabled:opacity-40 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800"
        onclick={() => library.rescan()}
        disabled={library.folders.length === 0 || library.scanning}
      >
        {t("Rescan")}
      </button>
      <button
        class="flex items-center gap-1.5 rounded-full bg-teal-600 px-3.5 py-1.5 text-xs font-semibold text-white transition hover:bg-teal-500"
        onclick={() => library.addFolder()}
      >
        <Icon name="folder" class="size-3.5" /> {t("Add folder")}
      </button>
    </div>
  </div>

  {#if library.scan}
    <p class="mb-3 text-sm text-teal-700 dark:text-teal-400" role="status">
      Scanning {library.scan.folder} – {library.scan.scanned.toLocaleString()} files…
    </p>
  {:else if library.scanning}
    <p class="mb-3 text-sm text-teal-700 dark:text-teal-400" role="status">{t("Scanning…")}</p>
  {/if}

  {#if library.folders.length === 0}
    <p class="text-sm text-zinc-500">{t("No folders yet. Add a folder to build your library.")}</p>
  {:else}
    <ul class="divide-y divide-zinc-200/70 overflow-hidden rounded-xl border border-zinc-200/80 bg-zinc-50/70 dark:divide-zinc-800/80 dark:border-zinc-800 dark:bg-zinc-950/40">
      {#each library.folders as f (f.id)}
        <li class="group flex items-center gap-3 px-3.5 py-2.5 text-sm">
          <Icon name="folder" class="size-4 shrink-0 text-teal-600 dark:text-teal-400" />
          <span class="min-w-0 flex-1 truncate text-xs sm:text-sm" title={f.path}>{f.path}</span>
          <button
            class="rounded-full p-1 text-zinc-400 transition hover:bg-zinc-200 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
            onclick={() => library.removeFolder(f.id)}
            aria-label="Remove folder {f.path}"
          >
            <Icon name="close" class="size-4" />
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  <p class="mt-3 text-xs text-zinc-400">
    {count(library.stats.tracks, '{n} track', '{n} tracks')} · {count(library.stats.albums, '{n} album', '{n} albums')}
  </p>
</div>
