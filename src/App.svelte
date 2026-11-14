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
  const KEY = 'foomodern.view'

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

  const SIDEBAR_KEY = 'foomodern.sidebar.collapsed'
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

  onMount(() => {
    artists.registerNavigator(openArtist)
    library.registerAlbumNavigator(openAlbum)
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

    if (e.key === 'MediaPlayPause') {
      e.preventDefault()
      if (player.snapshot.current || player.snapshot.queueLength > 0) void player.togglePause()
      return
    }
    if (e.key === 'MediaTrackNext') {
      e.preventDefault()
      if (player.snapshot.current) void player.next()
      return
    }
    if (e.key === 'MediaTrackPrevious') {
      e.preventDefault()
      if (player.snapshot.current) void player.previous()
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
            <span class="flex size-7 shrink-0 items-center justify-center rounded-lg bg-gradient-to-br from-teal-400 to-emerald-600 text-white shadow-xs">
              <Icon name="music" class="size-4" />
            </span>
            <h1 class="truncate text-base font-bold tracking-tight">FooModern</h1>
          </div>
        {/if}
        <button
          type="button"
          class="rounded-lg p-1.5 text-zinc-500 transition hover:bg-zinc-200/80 hover:text-zinc-900 outline-none focus-visible:ring-2 focus-visible:ring-teal-500 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
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
                ? 'bg-teal-50 font-medium text-teal-900 dark:bg-teal-950/50 dark:text-teal-100'
                : 'text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800/70',
            ]}
            title={sidebarCollapsed ? t(LABELS[v]) : undefined}
            aria-label={sidebarCollapsed ? t(LABELS[v]) : undefined}
            aria-current={view === v ? 'page' : undefined}
            onclick={() => {
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

      {#if pinned.entries.length > 0 && !sidebarCollapsed}
        <nav aria-labelledby="pinned-heading" class="flex max-h-48 shrink-0 flex-col gap-1">
          <div class="flex items-center justify-between px-2">
            <h2
              id="pinned-heading"
              class="text-xs font-medium uppercase tracking-wide text-zinc-400"
            >
              {t('Pinned items')}
            </h2>
          </div>
          <div class="flex min-h-0 flex-col gap-0.5 overflow-y-auto">
            {#each pinned.entries as entry (`${entry.kind}:${entry.id}`)}
              {@const active =
                (entry.kind === 'album' && view === 'albums' && selectedAlbumId === entry.id) ||
                (entry.kind === 'playlist' && view === 'playlist' && playlistId === entry.id)}
              <div class="group flex items-center gap-0.5">
                <button
                  type="button"
                  class={[
                    'flex min-w-0 flex-1 items-center gap-2 rounded-lg px-2 py-1.5 text-left',
                    active
                      ? 'bg-teal-50 font-medium text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
                      : 'text-zinc-700 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800',
                  ]}
                  aria-current={active ? 'page' : undefined}
                  onclick={() =>
                    entry.kind === 'album' ? openAlbum(entry.id) : openPlaylist(entry.id)}
                >
                  {#if entry.kind === 'album'}
                    <Cover
                      cover={entry.cover}
                      title={entry.title}
                      class="size-8 shrink-0 rounded text-xs"
                    />
                  {:else}
                    <PlaylistCover
                      cover={entry.cover}
                      albumCovers={entry.albumCovers}
                      name={entry.title}
                      class="size-8 shrink-0 rounded"
                      iconClass="size-4"
                    />
                  {/if}
                  <span class="min-w-0 flex-1">
                    <span class="block truncate text-sm leading-tight">{entry.title}</span>
                    <span class="block truncate text-[11px] text-zinc-500">
                      {#if entry.kind === 'album'}
                        {t('Album')}{entry.subtitle ? ` · ${entry.subtitle}` : ''}
                      {:else}
                        {t('Playlist')} · {count(entry.trackCount, '{n} track', '{n} tracks')}
                      {/if}
                    </span>
                  </span>
                </button>
                <button
                  type="button"
                  title={t('Unpin')}
                  aria-label={t('Unpin “{title}”', { title: entry.title })}
                  class="shrink-0 rounded p-1 text-teal-600 opacity-75 hover:bg-zinc-200 hover:opacity-100 dark:text-teal-400 dark:hover:bg-zinc-800"
                  onclick={() => void pinned.toggle(entry.kind, entry.id)}
                >
                  <Icon name="pin" class="size-3.5" />
                </button>
              </div>
            {/each}
          </div>
        </nav>
      {/if}

      <nav aria-labelledby="playlists-heading" class="flex min-h-0 flex-1 flex-col gap-0.5">
        {#if !sidebarCollapsed}
          <div class="flex items-center justify-between px-2">
            <h2 id="playlists-heading" class="text-xs font-medium uppercase tracking-wide text-zinc-400">{t("Playlists")}</h2>
            <span class="flex items-center gap-0.5">
              <button
                class="rounded p-0.5 text-zinc-400 outline-none hover:bg-zinc-200 hover:text-zinc-700 focus-visible:ring-2 focus-visible:ring-teal-500 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
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
                class="rounded p-0.5 text-zinc-400 outline-none hover:bg-zinc-200 hover:text-zinc-700 focus-visible:ring-2 focus-visible:ring-teal-500 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
                aria-label={t("New playlist")}
                title={t("New playlist")}
                onclick={() => (creatingPlaylist = !creatingPlaylist)}
              >
                <Icon name="plus" class="size-4" />
              </button>
            </span>
          </div>
        {:else}
          <h2 id="playlists-heading" class="sr-only">{t("Playlists")}</h2>
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
              class="w-full rounded-xl border border-zinc-200 bg-white px-2.5 py-1 text-sm outline-none focus-visible:ring-2 focus-visible:ring-teal-500 dark:border-zinc-700 dark:bg-zinc-900"
              placeholder={t("Playlist name")}
              aria-label={t("New playlist name")}
              bind:value={newPlaylistName}
              onkeydown={(e) => e.key === 'Escape' && (creatingPlaylist = false)}
              {@attach (el) => el.focus()}
            />
          </form>
        {/if}
        <div class="flex min-h-0 flex-col gap-0.5 overflow-y-auto">
          <button
            type="button"
            class={[
              'flex items-center rounded-lg px-2 py-1.5 text-sm',
              sidebarCollapsed ? 'justify-center' : 'justify-between gap-2 text-left',
              view === 'favorites'
                ? 'bg-teal-50 font-medium text-teal-900 dark:bg-teal-950/50 dark:text-teal-100'
                : 'text-zinc-700 hover:bg-zinc-200/70 dark:text-zinc-200 dark:hover:bg-zinc-800/70',
            ]}
            title={sidebarCollapsed ? t('Favorites') : undefined}
            aria-label={sidebarCollapsed ? t('Favorites') : undefined}
            aria-current={view === 'favorites' ? 'page' : undefined}
            onclick={() => go('favorites')}
          >
            <span class="flex min-w-0 items-center gap-2">
              <span class="flex size-5 shrink-0 items-center justify-center rounded bg-gradient-to-br from-teal-400 to-indigo-600 text-white">
                <Icon name="star" class="size-3" />
              </span>
              {#if !sidebarCollapsed}
                <span class="truncate">{t('Favorites')}</span>
              {/if}
            </span>
            {#if !sidebarCollapsed && favorites.ids.size > 0}
              <span class="text-xs tabular-nums text-zinc-400">
                {favorites.ids.size.toLocaleString()}
              </span>
            {/if}
          </button>
          {#each playlists.list as p (p.id)}
            {@const active = view === 'playlist' && playlistId === p.id}
            {@const isPinned = pinned.isPinned('playlist', p.id)}
            <div class="group flex items-center gap-0.5">
              <button
                class={[
                  'flex min-w-0 flex-1 items-center rounded-lg px-2 py-1.5 text-sm',
                  sidebarCollapsed ? 'justify-center' : 'gap-2 text-left',
                  active
                    ? 'bg-teal-50 font-medium text-teal-900 dark:bg-teal-950/50 dark:text-teal-100'
                    : 'text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800/70',
                ]}
                title={sidebarCollapsed ? p.name : undefined}
                aria-label={sidebarCollapsed ? p.name : undefined}
                aria-current={active ? 'page' : undefined}
                onclick={() => openPlaylist(p.id)}
              >
                <PlaylistCover
                  cover={p.cover}
                  albumCovers={p.albumCovers}
                  name={p.name}
                  class="size-6 shrink-0 rounded"
                  iconClass="size-3.5"
                />
                {#if !sidebarCollapsed}
                  <span class="min-w-0 flex-1 truncate">{p.name}</span>
                  <span class="shrink-0 text-xs tabular-nums text-zinc-400">{p.trackCount.toLocaleString()}</span>
                {/if}
              </button>
              {#if !sidebarCollapsed}
                <button
                  type="button"
                  title={t(isPinned ? 'Unpin' : 'Pin')}
                  aria-label={t(isPinned ? 'Unpin “{title}”' : 'Pin “{title}”', { title: p.name })}
                  class={[
                    'shrink-0 rounded p-1 transition hover:bg-zinc-200 dark:hover:bg-zinc-800',
                    isPinned
                      ? 'text-teal-600 opacity-100 dark:text-teal-400'
                      : 'text-zinc-400 opacity-0 group-focus-within:opacity-100 group-hover:opacity-100',
                  ]}
                  onclick={() => void pinned.toggle('playlist', p.id)}
                >
                  <Icon name="pin" class="size-3.5" />
                </button>
              {/if}
            </div>
          {:else}
            {#if !creatingPlaylist && !sidebarCollapsed}
              <p class="px-2 text-xs text-zinc-400">{t("No playlists yet.")}</p>
            {/if}
          {/each}
        </div>
      </nav>

      <div class="mt-auto flex flex-col gap-2">
        {#if library.scanning}
          <p
            class={['flex items-center gap-2 px-2 text-xs text-teal-700 dark:text-teal-400', sidebarCollapsed && 'justify-center']}
            role="status"
            title={library.scan ? t('Scanning · {n}', { n: library.scan.scanned.toLocaleString(i18n.locale) }) : t('Scanning…')}
          >
            <span class="size-2 shrink-0 animate-pulse rounded-full bg-teal-500"></span>
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
              ? 'bg-teal-50 font-medium text-teal-900 dark:bg-teal-950/50 dark:text-teal-100'
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
