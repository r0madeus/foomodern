<script lang="ts">
  import { appSettings } from '../appSettings.svelte'
  import { count, t } from '../i18n.svelte'
  import { invoke } from '@tauri-apps/api/core'
  import ArtistLinks from '../ArtistLinks.svelte'
  import Cover from '../Cover.svelte'
  import Icon from '../Icon.svelte'
  import AddToPlaylist from '../AddToPlaylist.svelte'
  import TintedBackdrop from '../TintedBackdrop.svelte'
  import TrackItem from './TrackItem.svelte'
  import { metadataMatch } from '../metadata.svelte'
  import { library, type AlbumRow, type TrackRow } from '../library.svelte'
  import type { Rgb } from '../nowCover.svelte'
  import { player, formatTime } from '../player.svelte'
  import { tintFrom } from '../tint'

  let { albumId, onBack }: { albumId: number; onBack: () => void } = $props()

  let data = $state<{ album: AlbumRow; tracks: TrackRow[] } | null>(null)
  let color = $state<Rgb | null>(null)
  let scrolled = $state(0)

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

  $effect(() => {
    const cover = data?.album.cover ?? null
    color = null
    if (cover === null) return
    let stale = false
    invoke<Rgb | null>('library_cover_color', { cover })
      .then((c) => {
        if (!stale) color = c
      })
      .catch(() => {})
    return () => (stale = true)
  })
  const tint = $derived(tintFrom(color))

  const totalSeconds = $derived(
    data ? data.tracks.reduce((sum, t) => sum + (t.durationMs ?? 0), 0) / 1000 : 0,
  )
  const playingThis = $derived(
    data !== null &&
      player.snapshot.status === 'playing' &&
      data.tracks.some((t) => t.path === player.snapshot.current?.path),
  )

  async function shufflePlay() {
    if (!data || data.tracks.length === 0) return
    await player.setShuffle(true)
    await library.playAlbum(albumId, Math.floor(Math.random() * data.tracks.length))
  }
</script>

<div class="absolute inset-0 flex flex-col bg-zinc-950">
  <!-- Back bar: the title fades in once the big header scrolled away. -->
  <header class="absolute inset-x-0 top-0 z-10 flex h-14 items-center gap-1 px-1 transition-colors" class:bg-zinc-900={scrolled > 260}>
    <button class="rounded-full bg-black/30 p-2.5 text-white active:bg-black/50" aria-label={t("Back")} onclick={onBack}>
      <Icon name="back" class="size-6" />
    </button>
    {#if scrolled > 260 && data}
      <h1 class="min-w-0 flex-1 truncate text-base font-semibold">{data.album.title}</h1>
    {/if}
  </header>

  <div class="min-h-0 flex-1 overflow-y-auto pb-[var(--mobile-bar-clear,5rem)]" onscroll={(e) => (scrolled = e.currentTarget.scrollTop)}>
    {#if data}
      <div class="relative isolate overflow-hidden px-4 pt-16 pb-4">
        <TintedBackdrop background="linear-gradient(180deg, {tint.top} 0%, {tint.surface} 65%, #09090b 100%)" />
        <div class="relative">
          <Cover cover={data.album.cover} title={data.album.title} class="mx-auto aspect-square w-3/5 max-w-64 rounded-md text-5xl shadow-2xl shadow-black/60" />
          <h2 class="mt-5 text-2xl leading-tight font-bold">{data.album.title}</h2>
          <p class="mt-1 text-sm font-medium text-zinc-200">
            <ArtistLinks artist={data.album.albumArtist} />
          </p>
          <p class="mt-1 text-xs text-zinc-400">
            {t('Album · {details}', {
              details: [data.album.year, count(data.album.trackCount, '{n} song', '{n} songs'), formatTime(totalSeconds)]
                .filter(Boolean)
                .join(' · '),
            })}
          </p>
        </div>
      </div>
      <div class="flex items-center gap-2 px-4 pb-2">
        <AddToPlaylist {albumId} label="Add album to playlist" class="p-2.5 text-zinc-300" />
        {#if appSettings.allowMetadata}
          <button
            class="rounded-full px-3 py-2 text-sm font-medium text-zinc-300 active:bg-white/10"
            onclick={() => metadataMatch.openAlbum(albumId)}
          >
            {t("Find info")}
          </button>
        {/if}
        <span class="flex-1"></span>
        <button class="rounded-full p-3 text-zinc-300 active:bg-white/10" aria-label={t("Shuffle play")} onclick={shufflePlay}>
          <Icon name="shuffle" class="size-6" />
        </button>
        <button
          class="flex size-14 items-center justify-center rounded-full bg-teal-500 text-zinc-950 shadow-lg active:scale-95"
          aria-label={t(playingThis ? 'Pause' : 'Play')}
          onclick={() => (playingThis ? player.togglePause() : library.playAlbum(albumId, 0))}
        >
          <Icon name={playingThis ? 'pause' : 'play'} class="size-7" />
        </button>
      </div>
      {#each data.tracks as track, i (track.id)}
        <TrackItem
          title={track.title}
          subtitle={track.artist}
          path={track.path}
          trackId={track.id}
          {track}
          album=""
          number={track.trackNo ?? i + 1}
          onPlay={() => library.playAlbum(albumId, i)}
        />
      {/each}
    {:else}
      <div class="mx-auto mt-24 aspect-square w-3/5 animate-pulse rounded-md bg-zinc-800"></div>
    {/if}
  </div>
</div>
