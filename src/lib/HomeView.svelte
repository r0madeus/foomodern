<script lang="ts">
  import AddToPlaylist from './AddToPlaylist.svelte'
  import ArtistLinks from './ArtistLinks.svelte'
  import Cover from './Cover.svelte'
  import EmptyLibrary from './EmptyLibrary.svelte'
  import Equalizer from './Equalizer.svelte'
  import FavoriteButton from './FavoriteButton.svelte'
  import Icon from './Icon.svelte'
  import MixView from './MixView.svelte'
  import PlaylistCover from './PlaylistCover.svelte'
  import { count, t } from './i18n.svelte'
  import { favorites } from './favorites.svelte'
  import { greetingKey, home, mixSubtitle, mixTitle } from './home.svelte'
  import { library } from './library.svelte'
  import { pinned } from './pinned.svelte'
  import { formatTime, player } from './player.svelte'
  import { playlists } from './playlists.svelte'

  let {
    onOpenAlbum,
    onOpenPlaylist,
    onOpenFavorites,
    onOpenStats,
  }: {
    onOpenAlbum: (albumId: number) => void
    onOpenPlaylist: (playlistId: number) => void
    onOpenFavorites: () => void
    onOpenStats: () => void
  } = $props()

  let selectedMixId = $state<string | null>(null)

  $effect(() => {
    void player.snapshot.current?.path
    void home.refresh()
  })

  interface QuickTile {
    key: string
    kind: 'album' | 'playlist'
    id: number
    title: string
    subtitle: string
    cover: string | null
    albumCovers: string[]
  }

  const quickTiles = $derived.by<QuickTile[]>(() => {
    const feed = home.feed
    const tiles: QuickTile[] = []
    const seen = new Set<string>()
    for (const p of pinned.entries) {
      const key = `${p.kind}:${p.id}`
      seen.add(key)
      tiles.push({
        key,
        kind: p.kind,
        id: p.id,
        title: p.title,
        subtitle:
          p.kind === 'playlist'
            ? count(p.trackCount, '{n} track', '{n} tracks')
            : p.subtitle || t('Album'),
        cover: p.cover,
        albumCovers: p.albumCovers,
      })
      if (tiles.length >= 7) return tiles
    }
    for (const pl of playlists.list) {
      const key = `playlist:${pl.id}`
      if (seen.has(key)) continue
      seen.add(key)
      tiles.push({
        key,
        kind: 'playlist',
        id: pl.id,
        title: pl.name,
        subtitle: count(pl.trackCount, '{n} track', '{n} tracks'),
        cover: pl.cover,
        albumCovers: pl.albumCovers,
      })
      if (tiles.length >= 7) return tiles
    }
    for (const a of feed?.recentlyPlayedAlbums ?? feed?.recentlyAddedAlbums ?? []) {
      const key = `album:${a.id}`
      if (seen.has(key)) continue
      seen.add(key)
      tiles.push({
        key,
        kind: 'album',
        id: a.id,
        title: a.title,
        subtitle: a.albumArtist,
        cover: a.cover,
        albumCovers: [],
      })
      if (tiles.length >= 7) break
    }
    return tiles
  })

  const currentPath = $derived(player.snapshot.current?.path ?? null)

  function openQuickTile(tile: QuickTile) {
    if (tile.kind === 'album') onOpenAlbum(tile.id)
    else onOpenPlaylist(tile.id)
  }

  function playQuickTile(tile: QuickTile) {
    if (tile.kind === 'album') void library.playAlbum(tile.id, 0)
    else void playlists.play(tile.id, 0)
  }

  async function shuffleLibrary() {
    if (library.stats.tracks === 0) return
    await player.setShuffle(true)
    await library.playAll(Math.floor(Math.random() * library.stats.tracks))
  }
</script>

{#if selectedMixId !== null}
  <MixView
    id={selectedMixId}
    onBack={() => (selectedMixId = null)}
    {onOpenAlbum}
    onSavedPlaylist={onOpenPlaylist}
  />
{:else if library.stats.tracks === 0 && !home.feed?.hasHistory && (home.feed?.favoriteCount ?? 0) === 0}
  <EmptyLibrary />
{:else}
  {@const feed = home.feed}
  <div class="h-full overflow-y-auto px-5 pb-8">
    <div class="mx-auto flex max-w-7xl flex-col gap-8">
      <!-- Greeting & quick toolbar -->
      <header class="flex flex-wrap items-end justify-between gap-4 pt-1">
        <div>
          <h1 class="text-3xl font-bold tracking-tight text-zinc-900 dark:text-zinc-50">
            {t(greetingKey())}
          </h1>
          <p class="mt-1 text-sm text-zinc-500 dark:text-zinc-400">
            {count(library.stats.tracks, '{n} song', '{n} songs')} · {count(
              library.stats.albums,
              '{n} album',
              '{n} albums',
            )}
          </p>
        </div>
        <div class="flex items-center gap-2">
          <button
            type="button"
            class="flex items-center gap-1.5 rounded-full bg-teal-600 px-4 py-2 text-sm font-semibold text-white shadow-sm transition hover:bg-teal-500 disabled:opacity-40"
            disabled={library.stats.tracks === 0}
            onclick={() => void shuffleLibrary()}
          >
            <Icon name="shuffle" class="size-4" />
            {t('Shuffle')}
          </button>
          <button
            type="button"
            class="flex items-center gap-1.5 rounded-full border border-zinc-200 bg-white/80 px-3.5 py-2 text-sm font-medium text-zinc-700 transition hover:bg-zinc-100 dark:border-zinc-800 dark:bg-zinc-900/70 dark:text-zinc-200 dark:hover:bg-zinc-800"
            onclick={onOpenStats}
          >
            <Icon name="chart" class="size-4 text-teal-600 dark:text-teal-400" />
            {t('Listening stats')}
          </button>
        </div>
      </header>

      <!-- Quick Access Grid -->
      <section aria-labelledby="home-quick-heading">
        <h2 id="home-quick-heading" class="sr-only">{t('Quick access')}</h2>
        <div class="grid grid-cols-1 gap-2.5 sm:grid-cols-2 lg:grid-cols-4">
          <div class="group relative flex items-center overflow-hidden rounded-xl border border-zinc-200/80 bg-white/80 transition hover:bg-zinc-100 dark:border-zinc-800/80 dark:bg-zinc-900/70 dark:hover:bg-zinc-800/80">
            <button
              type="button"
              class="flex min-w-0 flex-1 items-center gap-3 pr-12 text-left"
              onclick={onOpenFavorites}
            >
              <span class="flex size-14 shrink-0 items-center justify-center bg-gradient-to-br from-teal-400 to-indigo-600 text-white">
                <Icon name="star" class="size-6" />
              </span>
              <span class="min-w-0 flex-1 py-2">
                <span class="block truncate text-sm font-semibold text-zinc-900 dark:text-zinc-50">
                  {t('Favorites')}
                </span>
                <span class="block truncate text-xs text-zinc-500 dark:text-zinc-400">
                  {count(favorites.ids.size, '{n} track', '{n} tracks')}
                </span>
              </span>
            </button>
            {#if favorites.ids.size > 0}
              <button
                type="button"
                aria-label={t('Play')}
                class="absolute right-2.5 flex size-9 items-center justify-center rounded-full bg-teal-500 text-zinc-950 opacity-0 shadow-md transition group-focus-within:opacity-100 group-hover:opacity-100 hover:scale-105 hover:bg-teal-400"
                onclick={() => void favorites.play(0)}
              >
                <Icon name="play" class="size-4" />
              </button>
            {/if}
          </div>

          {#each quickTiles as tile (tile.key)}
            <div class="group relative flex items-center overflow-hidden rounded-xl border border-zinc-200/80 bg-white/80 transition hover:bg-zinc-100 dark:border-zinc-800/80 dark:bg-zinc-900/70 dark:hover:bg-zinc-800/80">
              <button
                type="button"
                class="flex min-w-0 flex-1 items-center gap-3 pr-12 text-left"
                onclick={() => openQuickTile(tile)}
              >
                {#if tile.kind === 'album'}
                  <Cover cover={tile.cover} title={tile.title} class="size-14 shrink-0 rounded-none text-base" />
                {:else}
                  <PlaylistCover
                    cover={tile.cover}
                    albumCovers={tile.albumCovers}
                    name={tile.title}
                    class="size-14 shrink-0 rounded-none"
                    iconClass="size-6"
                  />
                {/if}
                <span class="min-w-0 flex-1 py-2">
                  <span class="block truncate text-sm font-semibold text-zinc-900 dark:text-zinc-50">
                    {tile.title}
                  </span>
                  <span class="block truncate text-xs text-zinc-500 dark:text-zinc-400">
                    {tile.subtitle}
                  </span>
                </span>
              </button>
              <button
                type="button"
                aria-label={t('Play')}
                class="absolute right-2.5 flex size-9 items-center justify-center rounded-full bg-teal-500 text-zinc-950 opacity-0 shadow-md transition group-focus-within:opacity-100 group-hover:opacity-100 hover:scale-105 hover:bg-teal-400"
                onclick={() => playQuickTile(tile)}
              >
                <Icon name="play" class="size-4" />
              </button>
            </div>
          {/each}
        </div>
      </section>

      <!-- Dynamic Mixes -->
      {#if feed && feed.mixes.length > 0}
        <section aria-labelledby="home-mixes-heading">
          <div class="mb-3 flex items-baseline justify-between gap-2">
            <h2 id="home-mixes-heading" class="text-lg font-bold text-zinc-900 dark:text-zinc-50">
              {t('Daily Mixes')}
            </h2>
          </div>
          <div class="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-4">
            {#each feed.mixes as mix (mix.id)}
              {@const title = mixTitle(mix)}
              <div class="group relative rounded-2xl border border-zinc-200/70 bg-white/60 p-3 transition hover:bg-zinc-100 dark:border-zinc-800/70 dark:bg-zinc-900/50 dark:hover:bg-zinc-800/70">
                <button
                  type="button"
                  class="block w-full text-left"
                  onclick={() => (selectedMixId = mix.id)}
                >
                  <PlaylistCover
                    cover={null}
                    albumCovers={mix.albumCovers}
                    name={title}
                    class="aspect-square w-full rounded-xl shadow-md"
                    iconClass="size-12"
                  />
                  <span class="mt-3 block truncate text-sm font-semibold text-zinc-900 dark:text-zinc-50">
                    {title}
                  </span>
                  <span class="mt-0.5 line-clamp-2 text-xs text-zinc-500 dark:text-zinc-400">
                    {mixSubtitle(mix)}
                  </span>
                </button>
                <button
                  type="button"
                  aria-label={t('Play')}
                  class="absolute right-5 bottom-[4.75rem] flex size-10 items-center justify-center rounded-full bg-teal-500 text-zinc-950 opacity-0 shadow-lg transition group-focus-within:opacity-100 group-hover:translate-y-0 group-hover:opacity-100 translate-y-1 hover:scale-105 hover:bg-teal-400"
                  onclick={() => void home.playMix(mix.id, 0)}
                >
                  <Icon name="play" class="size-5" />
                </button>
              </div>
            {/each}
          </div>
        </section>
      {:else if feed && !feed.hasHistory}
        <section class="rounded-2xl border border-dashed border-zinc-300 bg-zinc-50/70 px-5 py-4 text-sm text-zinc-600 dark:border-zinc-800 dark:bg-zinc-900/40 dark:text-zinc-400">
          {t('Start playing some songs to unlock personalized mixes and recent history.')}
        </section>
      {/if}

      <!-- Recently Played Albums -->
      {#if feed && feed.recentlyPlayedAlbums.length > 0}
        <section aria-labelledby="home-recent-albums-heading">
          <h2 id="home-recent-albums-heading" class="mb-3 text-lg font-bold text-zinc-900 dark:text-zinc-50">
            {t('Recently played')}
          </h2>
          <div class="flex gap-4 overflow-x-auto pb-2">
            {#each feed.recentlyPlayedAlbums as album (album.id)}
              <div class="group relative w-40 shrink-0 rounded-xl p-2 transition hover:bg-zinc-100 dark:hover:bg-zinc-800/60">
                <button
                  type="button"
                  class="block w-full text-left"
                  onclick={() => onOpenAlbum(album.id)}
                >
                  <Cover
                    cover={album.cover}
                    title={album.title}
                    class="aspect-square w-full rounded-lg text-2xl shadow-md"
                  />
                  <span class="mt-2.5 block truncate text-sm font-semibold text-zinc-900 dark:text-zinc-50">
                    {album.title}
                  </span>
                  <span class="block truncate text-xs text-zinc-500 dark:text-zinc-400">
                    {album.albumArtist}
                  </span>
                </button>
                <button
                  type="button"
                  aria-label={t('Play')}
                  class="absolute right-4 bottom-16 flex size-9 items-center justify-center rounded-full bg-teal-500 text-zinc-950 opacity-0 shadow-lg transition group-focus-within:opacity-100 group-hover:opacity-100 hover:scale-105 hover:bg-teal-400"
                  onclick={() => void library.playAlbum(album.id, 0)}
                >
                  <Icon name="play" class="size-4" />
                </button>
              </div>
            {/each}
          </div>
        </section>
      {/if}

      <!-- Recently Played Tracks & Favorites Preview -->
      {#if feed && (feed.recentlyPlayedTracks.length > 0 || feed.favoriteTracks.length > 0)}
        <div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
          {#if feed.recentlyPlayedTracks.length > 0}
            <section aria-labelledby="home-recent-tracks-heading" class="rounded-2xl border border-zinc-200/80 bg-white/60 p-4 dark:border-zinc-800/80 dark:bg-zinc-900/50">
              <h2 id="home-recent-tracks-heading" class="mb-3 text-base font-bold text-zinc-900 dark:text-zinc-50">
                {t('Recently played tracks')}
              </h2>
              <div class="flex flex-col divide-y divide-zinc-200/60 dark:divide-zinc-800/60">
                {#each feed.recentlyPlayedTracks as item (item.track.id)}
                  {@const track = item.track}
                  <div class="group flex items-center gap-3 py-2">
                    <button
                      type="button"
                      class="flex min-w-0 flex-1 items-center gap-3 text-left"
                      onclick={() => void player.openFiles([track.path])}
                    >
                      <Cover cover={track.cover} title={track.title} class="size-10 shrink-0 rounded text-xs" />
                      <span class="min-w-0 flex-1">
                        <span class={['flex items-center gap-1.5 truncate text-sm font-medium', track.path === currentPath ? 'text-teal-600 dark:text-teal-400' : 'text-zinc-900 dark:text-zinc-100']}>
                          {#if track.path === currentPath && player.snapshot.status === 'playing'}
                            <Equalizer class="shrink-0 text-teal-600 dark:text-teal-400" />
                          {/if}
                          <span class="truncate">{track.title}</span>
                        </span>
                        <span class="block truncate text-xs text-zinc-500">
                          <ArtistLinks artist={track.artist} />
                        </span>
                      </span>
                    </button>
                    <span class="hidden text-xs tabular-nums text-zinc-400 sm:inline">
                      {formatTime(track.durationMs === null ? null : track.durationMs / 1000)}
                    </span>
                    <FavoriteButton trackId={track.id} title={track.title} />
                    <AddToPlaylist trackIds={[track.id]} editTags label={t('More actions for “{title}”', { title: track.title })} />
                  </div>
                {/each}
              </div>
            </section>
          {/if}

          {#if feed.favoriteTracks.length > 0}
            <section aria-labelledby="home-favorites-heading" class="rounded-2xl border border-zinc-200/80 bg-white/60 p-4 dark:border-zinc-800/80 dark:bg-zinc-900/50">
              <div class="mb-3 flex items-center justify-between gap-2">
                <h2 id="home-favorites-heading" class="text-base font-bold text-zinc-900 dark:text-zinc-50">
                  {t('Favorites')}
                </h2>
                <button
                  type="button"
                  class="text-xs font-medium text-teal-600 hover:underline dark:text-teal-400"
                  onclick={onOpenFavorites}
                >
                  {t('See all')}
                </button>
              </div>
              <div class="flex flex-col divide-y divide-zinc-200/60 dark:divide-zinc-800/60">
                {#each feed.favoriteTracks as track, i (track.id)}
                  <div class="group flex items-center gap-3 py-2">
                    <button
                      type="button"
                      class="flex min-w-0 flex-1 items-center gap-3 text-left"
                      onclick={() => void favorites.play(i)}
                    >
                      <Cover cover={track.cover} title={track.title} class="size-10 shrink-0 rounded text-xs" />
                      <span class="min-w-0 flex-1">
                        <span class={['flex items-center gap-1.5 truncate text-sm font-medium', track.path === currentPath ? 'text-teal-600 dark:text-teal-400' : 'text-zinc-900 dark:text-zinc-100']}>
                          {#if track.path === currentPath && player.snapshot.status === 'playing'}
                            <Equalizer class="shrink-0 text-teal-600 dark:text-teal-400" />
                          {/if}
                          <span class="truncate">{track.title}</span>
                        </span>
                        <span class="block truncate text-xs text-zinc-500">
                          <ArtistLinks artist={track.artist} />
                        </span>
                      </span>
                    </button>
                    <span class="hidden text-xs tabular-nums text-zinc-400 sm:inline">
                      {formatTime(track.durationMs === null ? null : track.durationMs / 1000)}
                    </span>
                    <FavoriteButton trackId={track.id} title={track.title} />
                    <AddToPlaylist trackIds={[track.id]} editTags label={t('More actions for “{title}”', { title: track.title })} />
                  </div>
                {/each}
              </div>
            </section>
          {/if}
        </div>
      {/if}

      <!-- Recently Added Albums -->
      {#if feed && feed.recentlyAddedAlbums.length > 0}
        <section aria-labelledby="home-added-albums-heading">
          <h2 id="home-added-albums-heading" class="mb-3 text-lg font-bold text-zinc-900 dark:text-zinc-50">
            {t('Recently added')}
          </h2>
          <div class="flex gap-4 overflow-x-auto pb-2">
            {#each feed.recentlyAddedAlbums as album (album.id)}
              <div class="group relative w-40 shrink-0 rounded-xl p-2 transition hover:bg-zinc-100 dark:hover:bg-zinc-800/60">
                <button
                  type="button"
                  class="block w-full text-left"
                  onclick={() => onOpenAlbum(album.id)}
                >
                  <Cover
                    cover={album.cover}
                    title={album.title}
                    class="aspect-square w-full rounded-lg text-2xl shadow-md"
                  />
                  <span class="mt-2.5 block truncate text-sm font-semibold text-zinc-900 dark:text-zinc-50">
                    {album.title}
                  </span>
                  <span class="block truncate text-xs text-zinc-500 dark:text-zinc-400">
                    {album.albumArtist}{album.year ? ` · ${album.year}` : ''}
                  </span>
                </button>
                <button
                  type="button"
                  aria-label={t('Play')}
                  class="absolute right-4 bottom-16 flex size-9 items-center justify-center rounded-full bg-teal-500 text-zinc-950 opacity-0 shadow-lg transition group-focus-within:opacity-100 group-hover:opacity-100 hover:scale-105 hover:bg-teal-400"
                  onclick={() => void library.playAlbum(album.id, 0)}
                >
                  <Icon name="play" class="size-4" />
                </button>
              </div>
            {/each}
          </div>
        </section>
      {/if}
    </div>
  </div>
{/if}
