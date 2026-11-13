<script lang="ts">
  import Icon from './Icon.svelte'
  import { library } from './library.svelte'
</script>

<div class="mx-auto max-w-2xl px-5 pb-6">
  <div class="mb-4 flex items-center justify-between gap-3">
    <h2 class="text-lg font-semibold tracking-tight">Music folders</h2>
    <div class="flex gap-2">
      <button
        class="rounded-lg px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-200/70 disabled:opacity-40 dark:text-zinc-300 dark:hover:bg-zinc-800"
        onclick={() => library.rescan()}
        disabled={library.folders.length === 0 || library.scanning}
      >
        Rescan
      </button>
      <button
        class="flex items-center gap-2 rounded-lg bg-teal-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-teal-500"
        onclick={() => library.addFolder()}
      >
        <Icon name="folder" class="size-4" /> Add folder
      </button>
    </div>
  </div>

  {#if library.scan}
    <p class="mb-3 text-sm text-teal-700 dark:text-teal-400" role="status">
      Scanning {library.scan.folder} – {library.scan.scanned.toLocaleString()} files…
    </p>
  {:else if library.scanning}
    <p class="mb-3 text-sm text-teal-700 dark:text-teal-400" role="status">Scanning…</p>
  {/if}

  {#if library.folders.length === 0}
    <p class="text-sm text-zinc-500">No folders yet. Add a folder to build your library.</p>
  {:else}
    <ul class="divide-y divide-zinc-100 rounded-lg border border-zinc-200 dark:divide-zinc-800 dark:border-zinc-800">
      {#each library.folders as f (f.id)}
        <li class="group flex items-center gap-3 px-3 py-2 text-sm">
          <Icon name="folder" class="size-4 shrink-0 text-zinc-400" />
          <span class="min-w-0 flex-1 truncate" title={f.path}>{f.path}</span>
          <button
            class="rounded p-1 text-zinc-400 hover:bg-zinc-200 hover:text-zinc-700 dark:hover:bg-zinc-700 dark:hover:text-zinc-100"
            onclick={() => library.removeFolder(f.id)}
            aria-label="Remove folder {f.path}"
          >
            <Icon name="close" class="size-4" />
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  <p class="mt-4 text-xs text-zinc-400">
    {library.stats.tracks.toLocaleString()} tracks · {library.stats.albums.toLocaleString()} albums
  </p>
</div>
