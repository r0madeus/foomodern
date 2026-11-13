<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import Cover from './Cover.svelte'
  import { library, type AlbumRow, type TrackRow } from './library.svelte'
  import { player, formatTime } from './player.svelte'

  let { albumId, onBack }: { albumId: number; onBack: () => void } = $props()

  let data = $state<{ album: AlbumRow; tracks: TrackRow[] } | null>(null)

  $effect(() => {
    void library.stats.version
    let stale = false
    invoke<{ album: AlbumRow; tracks: TrackRow[] }>('library_album', { albumId })
      .then((d) => {
        if (!stale) data = d
      })
      .catch((e) => (player.error = String(e)))
    return () => (stale = true)
  })

  const currentPath = $derived(player.snapshot.current?.path ?? null)
</script>

<div class="h-full overflow-y-auto px-5 pb-6">
  <button class="mb-4 rounded-lg px-2 py-1 text-sm text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800" onclick={onBack}>
    ← Albums
  </button>
  {#if data}
    <div class="mb-6 flex items-end gap-6">
      <Cover cover={data.album.cover} title={data.album.title} class="size-48 shrink-0 rounded-xl text-6xl shadow" />
      <div class="min-w-0">
        <h2 class="truncate text-2xl font-semibold tracking-tight">{data.album.title}</h2>
        <p class="truncate text-zinc-500">{data.album.albumArtist}</p>
        <p class="mt-1 text-sm text-zinc-400">
          {data.album.year ? `${data.album.year} · ` : ''}{data.album.trackCount}
          {data.album.trackCount === 1 ? 'track' : 'tracks'}
        </p>
        <button
          class="mt-4 flex items-center gap-2 rounded-lg bg-teal-600 px-4 py-2 text-sm font-medium text-white hover:bg-teal-500"
          onclick={() => library.playAlbum(albumId, 0)}
        >
          Play
        </button>
      </div>
    </div>
    <ol class="divide-y divide-zinc-100 dark:divide-zinc-800/70">
      {#each data.tracks as t, i (t.id)}
        <li>
          <button
            class={[
              'flex h-10 w-full items-center gap-3 px-3 text-left text-sm',
              t.path === currentPath
                ? 'bg-teal-50 text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
                : 'text-zinc-700 hover:bg-zinc-100/70 dark:text-zinc-300 dark:hover:bg-zinc-800/50',
            ]}
            onclick={() => library.playAlbum(albumId, i)}
          >
            <span class="w-8 text-right text-xs tabular-nums text-zinc-400">{t.trackNo ?? i + 1}</span>
            <span class="min-w-0 flex-1 truncate font-medium">{t.title}</span>
            <span class="text-xs tabular-nums text-zinc-500">
              {formatTime(t.durationMs === null ? null : t.durationMs / 1000)}
            </span>
          </button>
        </li>
      {/each}
    </ol>
  {:else}
    <p class="text-zinc-500">Loading…</p>
  {/if}
</div>
