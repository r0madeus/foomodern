<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import PlayerBar from './lib/PlayerBar.svelte'
  import Icon from './lib/Icon.svelte'
  import SongsView from './lib/SongsView.svelte'
  import AlbumsView from './lib/AlbumsView.svelte'
  import QueueView from './lib/QueueView.svelte'
  import FoldersView from './lib/FoldersView.svelte'
  import SettingsView from './lib/SettingsView.svelte'
  import PlaylistView from './lib/PlaylistView.svelte'
  import LyricsPanel from './lib/LyricsPanel.svelte'
  import TagEditor from './lib/TagEditor.svelte'
  import { player } from './lib/player.svelte'
  import { library } from './lib/library.svelte'
  import { lyrics } from './lib/lyrics.svelte'
  import { playlists } from './lib/playlists.svelte'

  const AUDIO_EXTENSIONS = [
    'mp3', 'flac', 'ogg', 'oga', 'opus', 'm4a', 'm4b', 'aac', 'wav', 'wv', 'ape',
    'wma', 'aif', 'aiff', 'dsf', 'dff', 'mka', 'mpc', 'tta', 'alac',
  ]

  type View = 'songs' | 'albums' | 'queue' | 'folders' | 'settings' | 'playlist'
  // Views shown in the main navigation. Playlists live in their own sidebar
  // section and Settings sits at the bottom.
  type NavView = 'songs' | 'albums' | 'queue' | 'folders'
  const NAV: NavView[] = ['songs', 'albums', 'queue', 'folders']
  const VIEWS: View[] = [...NAV, 'settings']
  const LABELS: Record<NavView, string> = { songs: 'Songs', albums: 'Albums', queue: 'Queue', folders: 'Folders' }
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

  let playlistId = $state<number | null>(null)
  let creatingPlaylist = $state(false)
  let newPlaylistName = $state('')

  function go(next: View) {
    view = next
    try {
      // A playlist is not remembered across launches (ids can disappear).
      if (next !== 'playlist') localStorage.setItem(KEY, next)
    } catch {
      // ignore
    }
  }

  function openPlaylist(id: number) {
    playlistId = id
    go('playlist')
  }

  async function createPlaylist() {
    const name = newPlaylistName.trim()
    if (name === '') return
    const made = await playlists.create(name)
    if (!made) return
    creatingPlaylist = false
    newPlaylistName = ''
    openPlaylist(made.id)
  }

  // Leave a playlist that no longer exists (deleted, or after a reset).
  $effect(() => {
    if (view === 'playlist' && playlists.loaded && !playlists.byId(playlistId)) go('songs')
  })

  const counts = $derived<Record<NavView, number | null>>({
    songs: library.stats.tracks,
    albums: library.stats.albums,
    queue: player.snapshot.queueLength,
    folders: library.folders.length,
  })

  onMount(() => {
    player.init().catch((e) => (player.error = String(e)))
    library.init().catch((e) => (player.error = String(e)))
    playlists.init().catch((e) => (player.error = String(e)))
  })

  $effect(() => {
    const isOpen = lyrics.open
    const path = player.snapshot.current?.path ?? null
    const dur = untrack(() => player.snapshot.durationSeconds)
    untrack(() => {
      if (isOpen || path === null) void lyrics.load(path, dur)
    })
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
        {#each NAV as v (v)}
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

      <nav aria-labelledby="playlists-heading" class="flex min-h-0 flex-col gap-0.5">
        <div class="flex items-center justify-between px-2">
          <h2 id="playlists-heading" class="text-xs font-medium uppercase tracking-wide text-zinc-400">Playlists</h2>
          <button
            class="rounded p-0.5 text-zinc-400 outline-none hover:bg-zinc-200 hover:text-zinc-700 focus-visible:ring-2 focus-visible:ring-teal-500 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
            aria-label="New playlist"
            title="New playlist"
            onclick={() => (creatingPlaylist = !creatingPlaylist)}
          >
            <Icon name="plus" class="size-4" />
          </button>
        </div>
        {#if creatingPlaylist}
          <form
            class="px-1 pb-1"
            onsubmit={(e) => {
              e.preventDefault()
              void createPlaylist()
            }}
          >
            <input
              class="w-full rounded-md border border-zinc-200 bg-white px-2 py-1 text-sm outline-none focus-visible:ring-2 focus-visible:ring-teal-500 dark:border-zinc-700 dark:bg-zinc-900"
              placeholder="Playlist name"
              aria-label="New playlist name"
              bind:value={newPlaylistName}
              onkeydown={(e) => e.key === 'Escape' && (creatingPlaylist = false)}
              {@attach (el) => el.focus()}
            />
          </form>
        {/if}
        <div class="flex min-h-0 flex-col gap-0.5 overflow-y-auto">
          {#each playlists.list as p (p.id)}
            {@const active = view === 'playlist' && playlistId === p.id}
            <button
              class={[
                'flex items-center justify-between gap-2 rounded-lg px-2 py-1.5 text-left text-sm',
                active
                  ? 'bg-teal-50 font-medium text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
                  : 'text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800',
              ]}
              aria-current={active ? 'page' : undefined}
              onclick={() => openPlaylist(p.id)}
            >
              <span class="truncate">{p.name}</span>
              <span class="text-xs tabular-nums text-zinc-400">{p.trackCount.toLocaleString()}</span>
            </button>
          {:else}
            {#if !creatingPlaylist}
              <p class="px-2 text-xs text-zinc-400">No playlists yet.</p>
            {/if}
          {/each}
        </div>
      </nav>

      <div class="mt-auto flex flex-col gap-2">
        {#if library.scanning}
          <p class="flex items-center gap-2 px-2 text-xs text-teal-700 dark:text-teal-400" role="status">
            <span class="size-2 animate-pulse rounded-full bg-teal-500"></span>
            Scanning{library.scan ? ` · ${library.scan.scanned.toLocaleString()}` : '…'}
          </p>
        {/if}
        <button
          class={[
            'rounded-lg px-2 py-1.5 text-left text-sm',
            view === 'settings'
              ? 'bg-teal-50 font-medium text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
              : 'text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800',
          ]}
          aria-current={view === 'settings' ? 'page' : undefined}
          onclick={() => go('settings')}
        >
          Settings
        </button>
      </div>
    </aside>

    <div class="relative flex min-w-0 flex-1 flex-col pt-4">
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
          <QueueView onOpenFiles={pickFiles} onSaved={openPlaylist} />
        {:else if view === 'playlist' && playlistId !== null}
          <PlaylistView id={playlistId} onDeleted={() => go('songs')} />
        {:else if view === 'settings'}
          <SettingsView />
        {:else}
          <div class="h-full overflow-y-auto"><FoldersView /></div>
        {/if}
      </main>
      {#if playlists.notice}
        <p
          role="status"
          class="pointer-events-none absolute right-6 bottom-24 z-40 max-w-sm rounded-lg bg-zinc-900 px-3 py-2 text-sm text-white shadow-lg dark:bg-zinc-100 dark:text-zinc-900"
        >
          {playlists.notice}
        </p>
      {/if}
    </div>

    {#if lyrics.open}
      <LyricsPanel />
    {/if}
  </div>

  <PlayerBar />
</div>

<TagEditor />
