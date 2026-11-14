<script lang="ts">
  import { count, i18n, t } from './lib/i18n.svelte'
  import { onMount, tick, untrack } from 'svelte'
  import { fly } from 'svelte/transition'
  import { open } from '@tauri-apps/plugin-dialog'
  import { motionDuration } from './lib/motion'
  import Cover from './lib/Cover.svelte'
  import PlaylistCover from './lib/PlaylistCover.svelte'
  import PlayerBar from './lib/PlayerBar.svelte'
  import Icon from './lib/Icon.svelte'
  import HomeView from './lib/HomeView.svelte'
  import SongsView from './lib/SongsView.svelte'
  import AlbumsView from './lib/AlbumsView.svelte'
  import ArtistsView from './lib/ArtistsView.svelte'
  import FavoritesView from './lib/FavoritesView.svelte'
  import StatsView from './lib/StatsView.svelte'
  import QueueView from './lib/QueueView.svelte'
  import SearchView from './lib/SearchView.svelte'
  import SettingsView from './lib/SettingsView.svelte'
  import PlaylistView from './lib/PlaylistView.svelte'
  import LyricsPanel from './lib/LyricsPanel.svelte'
  import TagEditor from './lib/TagEditor.svelte'
  import MetadataMatchModal from './lib/MetadataMatchModal.svelte'
  import { player } from './lib/player.svelte'
  import { library } from './lib/library.svelte'
  import { lyrics } from './lib/lyrics.svelte'
  import { playlists } from './lib/playlists.svelte'
  import { favorites } from './lib/favorites.svelte'
  import { history } from './lib/history.svelte'
  import { artists } from './lib/artists.svelte'
  import { pinned } from './lib/pinned.svelte'
  import { home } from './lib/home.svelte'
  import { appSettings } from './lib/appSettings.svelte'
  import { theme } from './lib/theme.svelte'
  import { online } from './lib/online.svelte'
  import MobileApp from './lib/mobile/MobileApp.svelte'
  import NowPlayingView from './lib/NowPlayingView.svelte'
  import UpdateBanner from './lib/UpdateBanner.svelte'
  import { updates } from './lib/updates.svelte'
  import { nowCover } from './lib/nowCover.svelte'
  import SidebarDownloads from './lib/SidebarDownloads.svelte'
  import TrackMenu from './lib/TrackMenu.svelte'
  import { trackMenu } from './lib/contextMenu.svelte'
  import { tagEditor } from './lib/tags.svelte'

  const AUDIO_EXTENSIONS = [
    'mp3', 'flac', 'ogg', 'oga', 'opus', 'm4a', 'm4b', 'aac', 'wav', 'wv', 'ape',
    'wma', 'aif', 'aiff', 'dsf', 'dff', 'mka', 'mpc', 'tta', 'alac',
  ]

  type View =
    | 'home'
    | 'songs'
    | 'albums'
    | 'artists'
    | 'search'
    | 'stats'
    | 'favorites'
    | 'queue'
    | 'settings'
    | 'playlist'
  // Views shown in the main navigation. Playlists live in their own sidebar
  // section, Queue is toggled from the bottom bar, and Settings sits at the bottom.
  type NavView = 'home' | 'songs' | 'albums' | 'artists' | 'search' | 'stats'
  const NAV: NavView[] = ['home', 'songs', 'albums', 'artists', 'search', 'stats']
  const VIEWS: View[] = [...NAV, 'favorites', 'queue', 'settings']
  const LABELS: Record<NavView, string> = {
    home: 'Home',
    songs: 'Songs',
    albums: 'Albums',
    artists: 'Artists',
    search: 'Search',
    stats: 'Listening stats',
  } // English keys, shown through t()
  const ICONS: Record<NavView, 'home' | 'music' | 'album' | 'artist' | 'search' | 'chart'> = {
    home: 'home',
    songs: 'music',
    albums: 'album',
    artists: 'artist',
    search: 'search',
    stats: 'chart',
  }
  const KEY = 'neomodern.view'

  function loadView(): View {
    try {
      const v = localStorage.getItem(KEY)
      if (VIEWS.includes(v as View)) return v as View
    } catch {
      // storage unavailable: fall through
    }
    return 'home'
  }

  let view = $state<View>(loadView())

  // Phones (and very narrow windows) get the touch UI, always dark.
  const narrow = window.matchMedia('(max-width: 639px)')
  let mobile = $state(narrow.matches)
  $effect(() => {
    const onChange = () => (mobile = narrow.matches)
    narrow.addEventListener('change', onChange)
    return () => narrow.removeEventListener('change', onChange)
  })
  $effect(() => {
    const dark = mobile || theme.isDark
    document.documentElement.classList.toggle('dark', dark)
    document.documentElement.style.colorScheme = dark ? 'dark' : 'light'
  })
  $effect(() => {
    document.documentElement.lang = i18n.lang
  })

  const SIDEBAR_KEY = 'neomodern.sidebar.collapsed'
  let sidebarCollapsed = $state(false)
  try {
    sidebarCollapsed = localStorage.getItem(SIDEBAR_KEY) === '1'
  } catch {
    // storage unavailable
  }

  function toggleSidebar() {
    sidebarCollapsed = !sidebarCollapsed
    try {
      localStorage.setItem(SIDEBAR_KEY, sidebarCollapsed ? '1' : '0')
    } catch {
      // ignore
    }
  }

  let playlistId = $state<number | null>(null)
  let selectedAlbumId = $state<number | null>(null)
  let selectedArtist = $state<string | null>(null)
  let selectedMixId = $state<string | null>(null)
  let creatingPlaylist = $state(false)
  let newPlaylistName = $state('')
  let beforeQueue = $state<{ view: View; playlistId: number | null }>({ view: 'songs', playlistId: null })

  function go(next: View) {
    view = next
    nowPlaying = false
    try {
      // A playlist is not remembered across launches (ids can disappear).
      if (next !== 'playlist') localStorage.setItem(KEY, next)
    } catch {
      // ignore
    }
  }

  function toggleQueue() {
    if (view === 'queue' && !nowPlaying) {
      if (
        beforeQueue.view === 'playlist' &&
        beforeQueue.playlistId !== null &&
        playlists.byId(beforeQueue.playlistId)
      ) {
        openPlaylist(beforeQueue.playlistId)
      } else {
        go(beforeQueue.view === 'queue' ? 'songs' : beforeQueue.view)
      }
      return
    }
    if (view !== 'queue') {
      beforeQueue = { view, playlistId }
    }
    go('queue')
  }

  function openPlaylist(id: number) {
    playlistId = id
    go('playlist')
  }

  function openAlbum(id: number) {
    selectedAlbumId = id
    go('albums')
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
    home: null,
    songs: library.stats.tracks,
    albums: library.stats.albums,
    artists: null,
    search: null,
    stats: null,
  })

  function openArtist(name: string) {
    selectedArtist = name
    go('artists')
  }

  // Desktop navigators; the phone UI (MobileApp) registers its own while it is
  // shown, so the desktop ones must not overwrite them.
  $effect(() => {
    if (mobile) return
    artists.registerNavigator(openArtist)
    library.registerAlbumNavigator(openAlbum)
  })

  onMount(() => {
    void artists.initScan()
    player.init().catch((e) => (player.error = String(e)))
    library.init().catch((e) => (player.error = String(e)))
    playlists.init().catch((e) => (player.error = String(e)))
    favorites.init().catch((e) => (player.error = String(e)))
    history.init().catch((e) => (player.error = String(e)))
    pinned.init().catch((e) => (player.error = String(e)))
    home.init().catch((e) => (player.error = String(e)))
    void appSettings.load().then(() => {
      if (appSettings.platform !== 'android') void online.init()
      updates.autoCheck()
    })
  })

  // The full-screen player (desktop "now playing", or the phone UI) shows
  // lyrics inline, so they are loaded there even with the panel closed.
  let nowPlaying = $state(false)
  $effect(() => {
    const inline = mobile || nowPlaying
    lyrics.always = inline
    const wanted = lyrics.open || inline
    const path = player.snapshot.current?.path ?? null
    const dur = untrack(() => player.snapshot.durationSeconds)
    untrack(() => {
      if (wanted || path === null) void lyrics.load(path, dur)
    })
  })

  $effect(() => {
    void nowCover.update(player.snapshot.current?.path ?? null)
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

  function isTypingTarget(el: EventTarget | null): boolean {
    if (!(el instanceof HTMLElement)) return false
    if (el.isContentEditable) return true
    return el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT'
  }

  async function onGlobalKey(e: KeyboardEvent) {
    if (mobile) return
    if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === 'k') {
      e.preventDefault()
      online.closeAlbum()
      go('search')
      await tick()
      const input = document.getElementById('global-search-input') as HTMLInputElement | null
      input?.focus()
      input?.select()
      return
    }

    if (e.altKey) return
    if (
      trackMenu.current !== null ||
      tagEditor.request !== null ||
      document.querySelector('[aria-modal="true"], [role="dialog"], [role="menu"]')
    ) {
      return
    }
    const active = document.activeElement
    if (isTypingTarget(e.target) || isTypingTarget(active)) return

    if ((e.ctrlKey || e.metaKey) && !e.shiftKey) {
      switch (e.key) {
        case 'ArrowLeft':
          e.preventDefault()
          if (player.snapshot.current) void player.previous()
          return
        case 'ArrowRight':
          e.preventDefault()
          if (player.snapshot.current) void player.next()
          return
        case 'ArrowUp':
          e.preventDefault()
          void player.adjustVolume(5)
          return
        case 'ArrowDown':
          e.preventDefault()
          void player.adjustVolume(-5)
          return
      }
      return
    }

    if (e.ctrlKey || e.metaKey || e.shiftKey) return

    const interactive =
      active instanceof HTMLElement &&
      active.closest(
        'button, a, summary, input, textarea, select, [role="slider"], [role="menuitem"], [role="switch"], [role="checkbox"], [role="radio"]',
      ) !== null

    if (e.code === 'Space' || e.key === ' ') {
      if (interactive) return
      if (!player.snapshot.current && player.snapshot.queueLength === 0) return
      e.preventDefault()
      void player.togglePause()
      return
    }

    if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
      if (interactive || !player.snapshot.current) return
      e.preventDefault()
      void player.seekRelative(e.key === 'ArrowLeft' ? -5 : 5)
      return
    }

    switch (e.key.toLowerCase()) {
      case 'm':
        e.preventDefault()
        void player.toggleMute()
        break
      case 's':
        e.preventDefault()
        void player.toggleShuffle()
        break
      case 'r':
        e.preventDefault()
        void player.cycleRepeat()
        break
    }
  }
</script>

<svelte:window onkeydown={onGlobalKey} />

{#if mobile}
  <MobileApp />
{:else}
<div class="flex h-screen flex-col bg-zinc-50 text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100">
  <div class="flex min-h-0 flex-1">
    <aside
      class={[
        'flex shrink-0 flex-col gap-4 border-r border-zinc-200/80 bg-zinc-100/70 py-4 transition-[width] duration-200 dark:border-zinc-800/80 dark:bg-zinc-900/50',
        sidebarCollapsed ? 'w-[4.25rem] px-2' : 'w-56 px-3',
      ]}
    >
      <div class={['flex items-center', sidebarCollapsed ? 'justify-center' : 'justify-between px-1.5']}>
        {#if !sidebarCollapsed}
          <div class="flex min-w-0 items-center gap-2">
            <span class="flex size-7 shrink-0 items-center justify-center rounded-lg bg-gradient-to-br from-accent-400 to-accent-700 text-white shadow-xs">
              <Icon name="music" class="size-4" />
            </span>
            <h1 class="truncate text-base font-bold tracking-tight">NeoModern</h1>
          </div>
        {/if}
        <button
          type="button"
          class="rounded-lg p-1.5 text-zinc-500 transition hover:bg-zinc-200/80 hover:text-zinc-900 outline-none focus-visible:ring-2 focus-visible:ring-accent-500 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
          aria-label={t(sidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar')}
          title={t(sidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar')}
          onclick={toggleSidebar}
        >
          <Icon name="sidebar" class="size-4" />
        </button>
      </div>

      <nav aria-label={t("Library")} class="flex flex-col gap-0.5">
        {#each NAV as v (v)}
          <button
            class={[
              'flex items-center rounded-lg py-1.5 text-sm transition-colors',
              sidebarCollapsed ? 'justify-center px-2' : 'justify-between px-2',
              view === v
                ? 'bg-accent-50 font-medium text-accent-900 dark:bg-accent-950/50 dark:text-accent-100'
                : 'text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800/70',
            ]}
            title={sidebarCollapsed ? t(LABELS[v]) : undefined}
            aria-label={sidebarCollapsed ? t(LABELS[v]) : undefined}
            aria-current={view === v ? 'page' : undefined}
            onclick={() => {
              if (v === 'home') selectedMixId = null
              if (v === 'albums') selectedAlbumId = null
              if (v === 'artists') selectedArtist = null
              go(v)
            }}
          >
            <span class="flex items-center gap-2.5">
              <Icon name={ICONS[v]} class="size-[18px] opacity-85" />
              {#if !sidebarCollapsed}
                <span>{t(LABELS[v])}</span>
              {/if}
            </span>
            {#if !sidebarCollapsed}
              {#if counts[v]}
                <span class="text-xs tabular-nums text-zinc-400">{counts[v]?.toLocaleString()}</span>
              {:else if v === 'search'}
                <kbd class="rounded border border-zinc-300/80 px-1 py-0.5 font-sans text-[10px] leading-none text-zinc-400 dark:border-zinc-700">
                  Ctrl K
                </kbd>
              {/if}
            {/if}
          </button>
        {/each}
      </nav>

      <!-- One library list, Spotify style: Favorites, then pinned albums and
           playlists, then the other playlists. Every row has the same size. -->
      <nav aria-labelledby="library-heading" class="flex min-h-0 flex-1 flex-col gap-1">
        {#if !sidebarCollapsed}
          <div class="flex items-center justify-between px-2">
            <h2 id="library-heading" class="text-xs font-medium uppercase tracking-wide text-zinc-400">{t('Your library')}</h2>
            <span class="flex items-center gap-0.5">
              <button
                class="rounded p-0.5 text-zinc-400 outline-none hover:bg-zinc-200 hover:text-zinc-700 focus-visible:ring-2 focus-visible:ring-accent-500 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
                aria-label={t("Import playlist (M3U)")}
                title={t("Import playlist (M3U)")}
                onclick={async () => {
                  const made = await playlists.importM3u()
                  if (made) openPlaylist(made.id)
                }}
              >
                <Icon name="upload" class="size-4" />
              </button>
              <button
                class="rounded p-0.5 text-zinc-400 outline-none hover:bg-zinc-200 hover:text-zinc-700 focus-visible:ring-2 focus-visible:ring-accent-500 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
                aria-label={t("New playlist")}
                title={t("New playlist")}
                onclick={() => (creatingPlaylist = !creatingPlaylist)}
              >
                <Icon name="plus" class="size-4" />
              </button>
            </span>
          </div>
        {:else}
          <h2 id="library-heading" class="sr-only">{t('Your library')}</h2>
        {/if}
        {#if creatingPlaylist && !sidebarCollapsed}
          <form
            class="px-1 pb-1"
            onsubmit={(e) => {
              e.preventDefault()
              void createPlaylist()
            }}
          >
            <input
              class="w-full rounded-xl border border-zinc-200 bg-white px-2.5 py-1 text-sm outline-none focus-visible:ring-2 focus-visible:ring-accent-500 dark:border-zinc-700 dark:bg-zinc-900"
              placeholder={t("Playlist name")}
              aria-label={t("New playlist name")}
              bind:value={newPlaylistName}
              onkeydown={(e) => e.key === 'Escape' && (creatingPlaylist = false)}
              {@attach (el) => el.focus()}
            />
          </form>
        {/if}

        {#snippet libraryRow(
          key: string,
          title: string,
          subtitle: string,
          active: boolean,
          open: () => void,
          art: import('svelte').Snippet,
          pin: { pinned: boolean; toggle: () => void } | null,
        )}
          <div class="group flex shrink-0 items-center gap-0.5" data-key={key}>
            <button
              type="button"
              class={[
                'flex min-w-0 flex-1 items-center rounded-lg p-1.5 text-left',
                sidebarCollapsed ? 'justify-center' : 'gap-2.5',
                active
                  ? 'bg-accent-50 text-accent-900 dark:bg-accent-950/50 dark:text-accent-100'
                  : 'text-zinc-700 hover:bg-zinc-200/70 dark:text-zinc-200 dark:hover:bg-zinc-800/70',
              ]}
              title={sidebarCollapsed ? title : undefined}
              aria-label={sidebarCollapsed ? title : undefined}
              aria-current={active ? 'page' : undefined}
              onclick={open}
            >
              {@render art()}
              {#if !sidebarCollapsed}
                <span class="min-w-0 flex-1">
                  <span class={['block truncate text-sm leading-snug', active && 'font-medium']}>{title}</span>
                  <span class="flex min-w-0 items-center gap-1 text-[11px] leading-snug text-zinc-500">
                    {#if pin?.pinned}
                      <Icon name="pin" class="size-3 shrink-0 text-accent-600 dark:text-accent-400" />
                    {/if}
                    <span class="truncate">{subtitle}</span>
                  </span>
                </span>
              {/if}
            </button>
            {#if pin && !sidebarCollapsed}
              <button
                type="button"
                title={t(pin.pinned ? 'Unpin' : 'Pin')}
                aria-label={t(pin.pinned ? 'Unpin “{title}”' : 'Pin “{title}”', { title })}
                aria-pressed={pin.pinned}
                class="shrink-0 rounded p-1 text-zinc-400 opacity-0 transition hover:bg-zinc-200 hover:text-zinc-700 focus-visible:opacity-100 group-hover:opacity-100 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
                onclick={pin.toggle}
              >
                <Icon name="pin" class="size-3.5" />
              </button>
            {/if}
          </div>
        {/snippet}

        {#snippet favoritesArt()}
          <span class="flex size-9 shrink-0 items-center justify-center rounded bg-gradient-to-br from-accent-400 to-indigo-600 text-white">
            <Icon name="star" class="size-4" />
          </span>
        {/snippet}

        <div class="flex min-h-0 flex-col gap-0.5 overflow-y-auto">
          {@render libraryRow(
            'favorites',
            t('Favorites'),
            `${t('Playlist')} · ${count(favorites.ids.size, '{n} track', '{n} tracks')}`,
            view === 'favorites',
            () => go('favorites'),
            favoritesArt,
            null,
          )}
          {#each pinned.entries as entry (`pin:${entry.kind}:${entry.id}`)}
            {#snippet pinnedArt()}
              {#if entry.kind === 'album'}
                <Cover cover={entry.cover} title={entry.title} class="size-9 shrink-0 rounded text-xs" />
              {:else}
                <PlaylistCover
                  cover={entry.cover}
                  albumCovers={entry.albumCovers}
                  name={entry.title}
                  class="size-9 shrink-0 rounded"
                  iconClass="size-4"
                />
              {/if}
            {/snippet}
            {@render libraryRow(
              `pin:${entry.kind}:${entry.id}`,
              entry.title,
              entry.kind === 'album'
                ? `${t('Album')}${entry.subtitle ? ` · ${entry.subtitle}` : ''}`
                : `${t('Playlist')} · ${count(entry.trackCount, '{n} track', '{n} tracks')}`,
              (entry.kind === 'album' && view === 'albums' && selectedAlbumId === entry.id) ||
                (entry.kind === 'playlist' && view === 'playlist' && playlistId === entry.id),
              () => (entry.kind === 'album' ? openAlbum(entry.id) : openPlaylist(entry.id)),
              pinnedArt,
              { pinned: true, toggle: () => void pinned.toggle(entry.kind, entry.id) },
            )}
          {/each}
          {#each playlists.list.filter((p) => !pinned.isPinned('playlist', p.id)) as p (p.id)}
            {#snippet playlistArt()}
              <PlaylistCover
                cover={p.cover}
                albumCovers={p.albumCovers}
                name={p.name}
                class="size-9 shrink-0 rounded"
                iconClass="size-4"
              />
            {/snippet}
            {@render libraryRow(
              `pl:${p.id}`,
              p.name,
              `${t('Playlist')} · ${count(p.trackCount, '{n} track', '{n} tracks')}`,
              view === 'playlist' && playlistId === p.id,
              () => openPlaylist(p.id),
              playlistArt,
              { pinned: false, toggle: () => void pinned.toggle('playlist', p.id) },
            )}
          {/each}
          {#if playlists.list.length === 0 && pinned.entries.length === 0 && !creatingPlaylist && !sidebarCollapsed}
            <p class="px-2 pt-1 text-xs text-zinc-400">{t('Create a playlist with +, or pin an album from its page or a song’s menu.')}</p>
          {/if}
        </div>
      </nav>

      <div class="mt-auto flex flex-col gap-2">
        {#if library.scanning}
          <p
            class={['flex items-center gap-2 px-2 text-xs text-accent-700 dark:text-accent-400', sidebarCollapsed && 'justify-center']}
            role="status"
            title={library.scan ? t('Scanning · {n}', { n: library.scan.scanned.toLocaleString(i18n.locale) }) : t('Scanning…')}
          >
            <span class="size-2 shrink-0 animate-pulse rounded-full bg-accent-500"></span>
            {#if !sidebarCollapsed}
              <span class="truncate">
                {library.scan ? t('Scanning · {n}', { n: library.scan.scanned.toLocaleString(i18n.locale) }) : t('Scanning…')}
              </span>
            {/if}
          </p>
        {/if}
        <SidebarDownloads
          collapsed={sidebarCollapsed}
          onExpand={() => {
            if (sidebarCollapsed) toggleSidebar()
          }}
        />
        <button
          class={[
            'flex items-center rounded-lg px-2 py-1.5 text-sm transition-colors',
            sidebarCollapsed ? 'justify-center' : 'gap-2.5 text-left',
            view === 'settings'
              ? 'bg-accent-50 font-medium text-accent-900 dark:bg-accent-950/50 dark:text-accent-100'
              : 'text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800/70',
          ]}
          title={sidebarCollapsed ? t('Settings') : undefined}
          aria-label={sidebarCollapsed ? t('Settings') : undefined}
          aria-current={view === 'settings' ? 'page' : undefined}
          onclick={() => go('settings')}
        >
          <Icon name="sliders" class="size-[18px] opacity-85" />
          {#if !sidebarCollapsed}
            <span>{t("Settings")}</span>
          {/if}
        </button>
      </div>
    </aside>

    <div class="relative flex min-w-0 flex-1 flex-col pt-4">
      {#if updates.available}
        <div class="mx-5 mb-2"><UpdateBanner /></div>
      {/if}
      {#if player.error}
        <p class="mx-5 mb-2 rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700 dark:bg-red-950/50 dark:text-red-300">
          {player.error}
        </p>
      {/if}
      <main class="min-h-0 flex-1">
        {#key view === 'playlist' ? `${view}:${playlistId}` : view}
          <div class="animate-view-enter h-full">
            {#if view === 'home'}
              <HomeView
                {selectedMixId}
                onSelectMix={(id) => (selectedMixId = id)}
                onOpenAlbum={(id) => openAlbum(id)}
                onOpenPlaylist={(id) => openPlaylist(id)}
                onOpenFavorites={() => go('favorites')}
                onOpenStats={() => go('stats')}
              />
            {:else if view === 'songs'}
              <SongsView />
            {:else if view === 'albums'}
              <AlbumsView {selectedAlbumId} onSelectAlbum={(id) => (selectedAlbumId = id)} />
            {:else if view === 'artists'}
              <ArtistsView
                {selectedArtist}
                onSelectArtist={(name) => (selectedArtist = name)}
                onOpenAlbum={(id) => openAlbum(id)}
              />
            {:else if view === 'search'}
              <SearchView
                onOpenSettings={() => go('settings')}
                onOpenLocalAlbum={(id) => openAlbum(id)}
              />
            {:else if view === 'stats'}
              <StatsView onOpenAlbum={(id) => openAlbum(id)} />
            {:else if view === 'favorites'}
              <FavoritesView />
            {:else if view === 'queue'}
              <QueueView onOpenFiles={pickFiles} onSaved={openPlaylist} />
            {:else if view === 'playlist' && playlistId !== null}
              <PlaylistView id={playlistId} onDeleted={() => go('songs')} />
            {:else}
              <SettingsView />
            {/if}
          </div>
        {/key}
      </main>
      {#if playlists.notice}
        <p
          role="status"
          transition:fly={{ y: 10, duration: motionDuration(180) }}
          class="pointer-events-none absolute right-6 bottom-24 z-40 max-w-sm rounded-lg bg-zinc-900 px-3 py-2 text-sm text-white shadow-lg dark:bg-zinc-100 dark:text-zinc-900"
        >
          {playlists.notice}
        </p>
      {/if}
      {#if nowPlaying}
        <NowPlayingView onClose={() => (nowPlaying = false)} />
      {/if}
    </div>

    {#if lyrics.open && !nowPlaying}
      <LyricsPanel />
    {/if}
  </div>

  <PlayerBar
    {nowPlaying}
    onToggleNowPlaying={() => (nowPlaying = !nowPlaying)}
    queueOpen={view === 'queue' && !nowPlaying}
    onToggleQueue={toggleQueue}
  />
</div>
{/if}

<TagEditor />
<MetadataMatchModal />
<TrackMenu />
