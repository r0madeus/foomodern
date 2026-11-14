<script lang="ts">
  import { t } from '../i18n.svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { onMount } from 'svelte'
  import { fly } from 'svelte/transition'
  import Icon from '../Icon.svelte'
  import { motionDuration } from '../motion'
  import MobileSheet from './MobileSheet.svelte'
  import FoldersView from '../FoldersView.svelte'
  import SettingsView from '../SettingsView.svelte'
  import LyricsScreen from './LyricsScreen.svelte'
  import PageHeader from './PageHeader.svelte'
  import HomeTab from './HomeTab.svelte'
  import AlbumsPage from './AlbumsPage.svelte'
  import MixPage from './MixPage.svelte'
  import AlbumPage from './AlbumPage.svelte'
  import ArtistPage from './ArtistPage.svelte'
  import ArtistsPage from './ArtistsPage.svelte'
  import FavoritesPage from './FavoritesPage.svelte'
  import SongsTab from './SongsTab.svelte'
  import SearchTab from './SearchTab.svelte'
  import LibraryTab from './LibraryTab.svelte'
  import PlaylistPage from './PlaylistPage.svelte'
  import StatsPage from './StatsPage.svelte'
  import MiniPlayer from './MiniPlayer.svelte'
  import NowPlaying from './NowPlaying.svelte'
  import QueueSheet from './QueueSheet.svelte'
  import UpdateBanner from '../UpdateBanner.svelte'
  import { updates } from '../updates.svelte'
  import { pushLayer, popLayer, clearLayers, replaceTopLayers } from './layers'
  import { artists } from '../artists.svelte'
  import { home } from '../home.svelte'
  import { player } from '../player.svelte'
  import { library } from '../library.svelte'
  import { lyrics } from '../lyrics.svelte'
  import { playlists } from '../playlists.svelte'

  // The phone UI: three primary tabs, pages pushed on top of them, a
  // floating translucent bottom bar with mini player, and full-screen player.
  // Every pushed screen is a history entry so the Android back button closes it.
  type Tab = 'home' | 'search' | 'library'
  type Sheet =
    | { type: 'albums' }
    | { type: 'songs' }
    | { type: 'mix'; id: string }
    | { type: 'album'; id: number }
    | { type: 'artist'; name: string }
    | { type: 'playlist'; id: number }
    | { type: 'favorites' }
    | { type: 'artists' }
    | { type: 'stats' }
    | { type: 'page'; page: 'folders' | 'settings' }

  const TABS: { id: Tab; label: string; icon: 'home' | 'search' | 'library' }[] = [
    { id: 'home', label: 'Home', icon: 'home' },
    { id: 'search', label: 'Search', icon: 'search' },
    { id: 'library', label: 'Library', icon: 'library' },
  ]

  let tab = $state<Tab>('home')
  let sheets = $state<Sheet[]>([])
  let playerOpen = $state(false)
  let queueOpen = $state(false)
  let lyricsOpen = $state(false)

  function selectTab(next: Tab) {
    clearLayers()
    sheets = []
    if (tab === 'search' && next !== 'search') library.setQuery('')
    tab = next
  }

  function open(set: () => void, unset: () => void) {
    set()
    pushLayer(unset)
  }
  function pushSheet(sheet: Sheet) {
    sheets = [...sheets, sheet]
    pushLayer(() => {
      sheets = sheets.slice(0, -1)
    })
  }
  const openAlbums = () => pushSheet({ type: 'albums' })
  const openSongs = () => pushSheet({ type: 'songs' })
  const openMix = (id: string) => pushSheet({ type: 'mix', id })
  const openAlbum = (id: number) => pushSheet({ type: 'album', id })
  const openArtist = (name: string) => pushSheet({ type: 'artist', name })
  const openPlaylist = (id: number) => pushSheet({ type: 'playlist', id })
  const openFavorites = () => pushSheet({ type: 'favorites' })
  const openArtists = () => pushSheet({ type: 'artists' })
  const openStats = () => pushSheet({ type: 'stats' })
  const openPage = (p: 'folders' | 'settings') => pushSheet({ type: 'page', page: p })
  const openPlayer = () => open(() => (playerOpen = true), () => (playerOpen = false))
  const openQueue = () => open(() => (queueOpen = true), () => (queueOpen = false))
  const openLyrics = () => open(() => (lyricsOpen = true), () => (lyricsOpen = false))

  function navigateFromOverlays(sheet: Sheet) {
    const overlayCount = Number(playerOpen) + Number(queueOpen) + Number(lyricsOpen)
    if (overlayCount === 0) {
      pushSheet(sheet)
      return
    }
    lyricsOpen = false
    queueOpen = false
    playerOpen = false
    sheets = [...sheets, sheet]
    replaceTopLayers(overlayCount, () => {
      sheets = sheets.slice(0, -1)
    })
  }

  $effect(() => {
    artists.registerNavigator((name) => navigateFromOverlays({ type: 'artist', name }))
    library.registerAlbumNavigator((id) => navigateFromOverlays({ type: 'album', id }))
  })

  // A deleted playlist closes its page when it is on top.
  $effect(() => {
    const top = sheets.at(-1)
    if (top?.type === 'playlist' && playlists.loaded && !playlists.byId(top.id)) popLayer()
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
    home.init().catch((e) => (player.error = String(e)))
    void artists.initScan()
    void applyInsets()
    window.addEventListener('resize', applyInsets)
    return () => window.removeEventListener('resize', applyInsets)
  })
</script>

<div
  class="fixed inset-0 flex flex-col bg-zinc-950 text-zinc-50"
  style:padding-top="var(--inset-top)"
  style:--mobile-bar-clear={player.snapshot.current
    ? 'calc(8.75rem + var(--inset-bottom, 0px))'
    : 'calc(4.75rem + var(--inset-bottom, 0px))'}
>
  {#if updates.available}
    <div class="mx-3 mt-2 shrink-0"><UpdateBanner /></div>
  {/if}
  {#if player.error}
    <button class="mx-3 mt-2 shrink-0 rounded-lg bg-red-950/80 px-3 py-2 text-left text-sm text-red-200" onclick={() => (player.error = null)}>
      {player.error}
    </button>
  {/if}

  <main class="relative min-h-0 flex-1 overflow-hidden">
    {#key tab}
      <div class="animate-view-enter h-full">
        {#if tab === 'home'}
          <HomeTab
            onOpenAlbum={openAlbum}
            onOpenPlaylist={openPlaylist}
            onOpenMix={openMix}
            onOpenFavorites={openFavorites}
            onOpenAlbums={openAlbums}
            onOpenSongs={openSongs}
            onOpenStats={openStats}
          />
        {:else if tab === 'search'}
          <SearchTab onOpenAlbum={openAlbum} />
        {:else}
          <LibraryTab
            onOpenPlaylist={openPlaylist}
            onOpenFavorites={openFavorites}
            onOpenSongs={openSongs}
            onOpenAlbums={openAlbums}
            onOpenArtists={openArtists}
            onOpenStats={openStats}
            onOpenPage={openPage}
          />
        {/if}
      </div>
    {/key}

    {#each sheets as sheet, i (`${i}:${sheet.type}`)}
      <MobileSheet onBack={popLayer}>
        {#if sheet.type === 'page'}
          <div class="absolute inset-0 flex flex-col bg-zinc-950">
            <PageHeader title={t(sheet.page === 'folders' ? 'Music folders' : 'Settings')} onBack={popLayer} />
            <div class="min-h-0 flex-1 overflow-y-auto pb-[var(--mobile-bar-clear,5rem)]">
              {#if sheet.page === 'folders'}<FoldersView />{:else}<SettingsView />{/if}
            </div>
          </div>
        {:else if sheet.type === 'albums'}
          <AlbumsPage onBack={popLayer} onOpenAlbum={openAlbum} />
        {:else if sheet.type === 'songs'}
          <div class="absolute inset-0 flex flex-col bg-zinc-950">
            <SongsTab onBack={popLayer} />
          </div>
        {:else if sheet.type === 'mix'}
          <MixPage id={sheet.id} onBack={popLayer} />
        {:else if sheet.type === 'favorites'}
          <FavoritesPage onBack={popLayer} />
        {:else if sheet.type === 'artists'}
          <ArtistsPage onBack={popLayer} onOpenArtist={openArtist} />
        {:else if sheet.type === 'stats'}
          <StatsPage onBack={popLayer} onOpenAlbum={openAlbum} />
        {:else if sheet.type === 'playlist'}
          <PlaylistPage id={sheet.id} onBack={popLayer} />
        {:else if sheet.type === 'artist'}
          <ArtistPage name={sheet.name} onBack={popLayer} onOpenAlbum={openAlbum} />
        {:else if sheet.type === 'album'}
          <AlbumPage albumId={sheet.id} onBack={popLayer} />
        {/if}
      </MobileSheet>
    {/each}

    {#if playlists.notice}
      <p
        role="status"
        transition:fly={{ y: 12, duration: motionDuration(180) }}
        style:bottom="calc(var(--mobile-bar-clear, 4.75rem) + 0.5rem)"
        class="pointer-events-none absolute inset-x-4 z-30 rounded-lg bg-zinc-100 px-3 py-2.5 text-sm text-zinc-900 shadow-lg"
      >
        {playlists.notice}
      </p>
    {/if}

    <div
      class="pointer-events-none absolute inset-x-0 bottom-0 z-20 bg-gradient-to-t from-zinc-950 via-zinc-950/90 via-60% to-transparent pt-8"
      style:padding-bottom="var(--inset-bottom)"
    >
      <div class="pointer-events-auto">
        <MiniPlayer onOpen={openPlayer} />
        <nav
          aria-label="Main"
          class="grid grid-cols-3 bg-gradient-to-t from-black/85 via-black/55 to-transparent backdrop-blur-md"
        >
          {#each TABS as entry (entry.id)}
            <button
              class={['flex flex-col items-center gap-1 pt-2 pb-2.5 text-[11px] transition-colors', tab === entry.id ? 'text-white' : 'text-zinc-400']}
              aria-current={tab === entry.id ? 'page' : undefined}
              onclick={() => selectTab(entry.id)}
            >
              <Icon name={entry.icon} class="size-6" />
              {t(entry.label)}
            </button>
          {/each}
        </nav>
      </div>
    </div>
  </main>

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
