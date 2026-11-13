<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { onMount } from 'svelte'
  import Icon from '../Icon.svelte'
  import EmptyLibrary from '../EmptyLibrary.svelte'
  import FoldersView from '../FoldersView.svelte'
  import SettingsView from '../SettingsView.svelte'
  import LyricsScreen from './LyricsScreen.svelte'
  import PageHeader from './PageHeader.svelte'
  import AlbumGrid from './AlbumGrid.svelte'
  import AlbumPage from './AlbumPage.svelte'
  import SongsTab from './SongsTab.svelte'
  import SearchTab from './SearchTab.svelte'
  import LibraryTab from './LibraryTab.svelte'
  import PlaylistPage from './PlaylistPage.svelte'
  import MiniPlayer from './MiniPlayer.svelte'
  import NowPlaying from './NowPlaying.svelte'
  import QueueSheet from './QueueSheet.svelte'
  import UpdateBanner from '../UpdateBanner.svelte'
  import { updates } from '../updates.svelte'
  import { pushLayer, popLayer, clearLayers } from './layers'
  import { player } from '../player.svelte'
  import { library } from '../library.svelte'
  import { lyrics } from '../lyrics.svelte'
  import { playlists } from '../playlists.svelte'

  // The phone UI: four tabs, pages pushed on top of them, a mini player and
  // a full-screen player. Every pushed screen is a history entry so the
  // Android back button closes it.
  type Tab = 'home' | 'songs' | 'search' | 'library'
  const TABS: { id: Tab; label: string; icon: 'album' | 'music' | 'search' | 'library' }[] = [
    { id: 'home', label: 'Albums', icon: 'album' },
    { id: 'songs', label: 'Songs', icon: 'music' },
    { id: 'search', label: 'Search', icon: 'search' },
    { id: 'library', label: 'Library', icon: 'library' },
  ]

  let tab = $state<Tab>('home')
  let album = $state<number | null>(null)
  let playlist = $state<number | null>(null)
  let page = $state<'folders' | 'settings' | null>(null)
  let playerOpen = $state(false)
  let queueOpen = $state(false)
  let lyricsOpen = $state(false)

  function selectTab(next: Tab) {
    clearLayers()
    if (tab === 'search' && next !== 'search') library.setQuery('')
    tab = next
  }

  function open(set: () => void, unset: () => void) {
    set()
    pushLayer(unset)
  }
  const openAlbum = (id: number) => open(() => (album = id), () => (album = null))
  const openPlaylist = (id: number) => open(() => (playlist = id), () => (playlist = null))
  const openPage = (p: 'folders' | 'settings') => open(() => (page = p), () => (page = null))
  const openPlayer = () => open(() => (playerOpen = true), () => (playerOpen = false))
  const openQueue = () => open(() => (queueOpen = true), () => (queueOpen = false))
  const openLyrics = () => open(() => (lyricsOpen = true), () => (lyricsOpen = false))


  // A deleted playlist closes its page.
  $effect(() => {
    if (playlist !== null && playlists.loaded && !playlists.byId(playlist)) popLayer()
  })

  async function applyInsets() {
    try {
      const i = await invoke<{ top: number; bottom: number }>('app_window_insets')
      const root = document.documentElement.style
      root.setProperty('--inset-top', `${i.top}px`)
      root.setProperty('--inset-bottom', `${i.bottom}px`)
    } catch {
      // keep the env() fallback
    }
  }

  onMount(() => {
    lyrics.setOpen(false) // the desktop side panel state is not a phone screen
    void applyInsets()
    window.addEventListener('resize', applyInsets)
    return () => window.removeEventListener('resize', applyInsets)
  })
</script>

<div class="fixed inset-0 flex flex-col bg-zinc-950 text-zinc-50" style:padding-top="var(--inset-top)">
  {#if updates.available}
    <div class="mx-3 mt-2 shrink-0"><UpdateBanner /></div>
  {/if}
  {#if player.error}
    <button class="mx-3 mt-2 shrink-0 rounded-lg bg-red-950/80 px-3 py-2 text-left text-sm text-red-200" onclick={() => (player.error = null)}>
      {player.error}
    </button>
  {/if}

  <main class="relative min-h-0 flex-1">
    {#if tab === 'home'}
      <div class="flex h-full flex-col">
        <PageHeader title="Albums" />
        {#if library.stats.albums === 0}
          <EmptyLibrary />
        {:else}
          <AlbumGrid onOpen={openAlbum} />
        {/if}
      </div>
    {:else if tab === 'songs'}
      <SongsTab />
    {:else if tab === 'search'}
      <SearchTab onOpenAlbum={openAlbum} />
    {:else}
      <LibraryTab onOpenPlaylist={openPlaylist} onOpenPage={openPage} />
    {/if}

    {#if page !== null}
      <div class="absolute inset-0 flex flex-col bg-zinc-950">
        <PageHeader title={page === 'folders' ? 'Music folders' : 'Settings'} onBack={popLayer} />
        <div class="min-h-0 flex-1 overflow-y-auto">
          {#if page === 'folders'}<FoldersView />{:else}<SettingsView />{/if}
        </div>
      </div>
    {/if}
    {#if playlist !== null}
      <PlaylistPage id={playlist} onBack={popLayer} />
    {/if}
    {#if album !== null}
      <AlbumPage albumId={album} onBack={popLayer} />
    {/if}

    {#if playlists.notice}
      <p role="status" class="pointer-events-none absolute inset-x-4 bottom-3 z-30 rounded-lg bg-zinc-100 px-3 py-2.5 text-sm text-zinc-900 shadow-lg">
        {playlists.notice}
      </p>
    {/if}
  </main>

  <div class="shrink-0 bg-gradient-to-t from-black via-black/95 to-black/0 pt-1" style:padding-bottom="var(--inset-bottom)">
    <MiniPlayer onOpen={openPlayer} />
    <nav aria-label="Main" class="grid grid-cols-4">
      {#each TABS as t (t.id)}
        <button
          class={['flex flex-col items-center gap-1 pt-2 pb-2.5 text-[11px]', tab === t.id ? 'text-white' : 'text-zinc-400']}
          aria-current={tab === t.id ? 'page' : undefined}
          onclick={() => selectTab(t.id)}
        >
          <Icon name={t.icon} class="size-6" />
          {t.label}
        </button>
      {/each}
    </nav>
  </div>

  {#if playerOpen}
    <NowPlaying onClose={popLayer} onQueue={openQueue} onLyrics={openLyrics} />
  {/if}
  {#if queueOpen}
    <QueueSheet onClose={popLayer} />
  {/if}
  {#if lyricsOpen}
    <LyricsScreen onClose={popLayer} />
  {/if}
</div>
