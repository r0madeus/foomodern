<script lang="ts">
  import { onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import PlayerBar from './lib/PlayerBar.svelte'
  import SongsView from './lib/SongsView.svelte'
  import AlbumsView from './lib/AlbumsView.svelte'
  import QueueView from './lib/QueueView.svelte'
  import FoldersView from './lib/FoldersView.svelte'
  import { player } from './lib/player.svelte'
  import { library } from './lib/library.svelte'

  const AUDIO_EXTENSIONS = [
    'mp3', 'flac', 'ogg', 'oga', 'opus', 'm4a', 'm4b', 'aac', 'wav', 'wv', 'ape',
    'wma', 'aif', 'aiff', 'dsf', 'dff', 'mka', 'mpc', 'tta', 'alac',
  ]

  type View = 'songs' | 'albums' | 'queue' | 'folders'
  const VIEWS: View[] = ['songs', 'albums', 'queue', 'folders']
  const LABELS: Record<View, string> = { songs: 'Songs', albums: 'Albums', queue: 'Queue', folders: 'Folders' }
  const KEY = 'foomodern.view'

  function loadView(): View {
    try {
      const v = localStorage.getItem(KEY)
      if (VIEWS.includes(v as View)) return v as View
    } catch {
      // storage unavailable: fall through
    }
    return 'songs'
  }

  let view = $state<View>(loadView())

  function go(next: View) {
    view = next
    try {
      localStorage.setItem(KEY, next)
    } catch {
      // ignore
    }
  }

  const counts = $derived<Record<View, number | null>>({
    songs: library.stats.tracks,
    albums: library.stats.albums,
    queue: player.snapshot.queueLength,
    folders: library.folders.length,
  })

  onMount(() => {
    player.init().catch((e) => (player.error = String(e)))
    library.init().catch((e) => (player.error = String(e)))
  })

  async function pickFiles(append: boolean) {
    const picked = await open({
      multiple: true,
      directory: false,
      filters: [{ name: 'Audio', extensions: AUDIO_EXTENSIONS }],
    })
    if (!picked || picked.length === 0) return
    if (append) await player.appendFiles(picked)
    else await player.openFiles(picked)
  }
</script>

<div class="flex h-screen flex-col bg-zinc-50 text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100">
  <div class="flex min-h-0 flex-1">
    <aside class="flex w-52 shrink-0 flex-col gap-4 border-r border-zinc-200 px-3 py-4 dark:border-zinc-800">
      <h1 class="px-2 text-lg font-semibold tracking-tight">FooModern</h1>
      <nav aria-label="Library" class="flex flex-col gap-0.5">
        {#each VIEWS as v (v)}
          <button
            class={[
              'flex items-center justify-between rounded-lg px-2 py-1.5 text-left text-sm',
              view === v
                ? 'bg-teal-50 font-medium text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
                : 'text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800',
            ]}
            aria-current={view === v ? 'page' : undefined}
            onclick={() => go(v)}
          >
            {LABELS[v]}
            {#if counts[v]}
              <span class="text-xs tabular-nums text-zinc-400">{counts[v]?.toLocaleString()}</span>
            {/if}
          </button>
        {/each}
      </nav>
      {#if library.scanning}
        <p class="mt-auto flex items-center gap-2 px-2 text-xs text-teal-700 dark:text-teal-400" role="status">
          <span class="size-2 animate-pulse rounded-full bg-teal-500"></span>
          Scanning{library.scan ? ` · ${library.scan.scanned.toLocaleString()}` : '…'}
        </p>
      {/if}
    </aside>

    <div class="flex min-w-0 flex-1 flex-col pt-4">
      {#if player.error}
        <p class="mx-5 mb-2 rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700 dark:bg-red-950/50 dark:text-red-300">
          {player.error}
        </p>
      {/if}
      <main class="min-h-0 flex-1">
        {#if view === 'songs'}
          <SongsView />
        {:else if view === 'albums'}
          <AlbumsView />
        {:else if view === 'queue'}
          <QueueView onOpenFiles={pickFiles} />
        {:else}
          <div class="h-full overflow-y-auto"><FoldersView /></div>
        {/if}
      </main>
    </div>
  </div>

  <PlayerBar />
</div>
