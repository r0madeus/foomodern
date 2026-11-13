<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { appSettings } from './appSettings.svelte'
  import Cover from './Cover.svelte'
  import Icon from './Icon.svelte'
  import TintedBackdrop from './TintedBackdrop.svelte'
  import { count, t } from './i18n.svelte'
  import { library, type AlbumRow, type TrackRow } from './library.svelte'
  import type { Rgb } from './nowCover.svelte'
  import {
    isAlbumInLibrary,
    isTrackInLibrary,
    online,
    type OnlineAlbum,
    type OnlineTrack,
  } from './online.svelte'
  import { formatTime, player } from './player.svelte'
  import { tintFrom } from './tint'

  let { album, onBack }: { album: OnlineAlbum; onBack: () => void } = $props()

  const activeAlbum = $derived(online.albumDetail?.album ?? album)
  const tracks = $derived(online.albumDetail?.tracks ?? [])
  const isDesktop = $derived(appSettings.platform !== 'android')
  const ytdlpAvailable = $derived(online.status?.ytdlpAvailable ?? false)
  const albumJob = $derived(online.jobFor('album', activeAlbum.title, activeAlbum.artist))

  let localAlbums = $state<AlbumRow[]>([])
  let localTracks = $state<TrackRow[]>([])
  $effect(() => {
    void library.stats.version
    const title = activeAlbum.title.trim()
    if (!title) {
      localAlbums = []
      localTracks = []
      return
    }
    let stale = false
    Promise.all([
      library.albumsPage(title)(0, 20),
      library.tracksPage('title', title)(0, 200),
    ])
      .then(([albs, trs]) => {
        if (stale) return
        localAlbums = albs.items
        localTracks = trs.items
      })
      .catch(() => {})
    return () => {
      stale = true
    }
  })

  const albumInLibrary = $derived(isAlbumInLibrary(activeAlbum, localAlbums))

  // Tint the Spotify-style album hero using the locally cached cover image.
  let color = $state<Rgb | null>(null)
  $effect(() => {
    const cover = activeAlbum.cover
    color = null
    if (!cover || cover.startsWith('http://') || cover.startsWith('https://')) return
    let stale = false
    invoke<Rgb | null>('library_cover_color', { cover })
      .then((c) => {
        if (!stale) color = c
      })
      .catch(() => {})
    return () => {
      stale = true
    }
  })
  const tint = $derived(tintFrom(color))

  function trackJob(track: OnlineTrack) {
    return online.jobFor('track', track.title, track.artist)
  }

  function isAlbumBusy(): boolean {
    return albumJob?.state === 'queued' || albumJob?.state === 'downloading'
  }
</script>

<div class="h-full overflow-y-auto px-5 pb-6">
  <button
    type="button"
    class="mb-4 rounded-lg px-2 py-1 text-sm text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800"
    onclick={onBack}
  >
    {t('← Search')}
  </button>

  <div
    class="relative isolate mb-6 flex flex-col gap-4 overflow-hidden rounded-2xl p-4 text-white sm:flex-row sm:items-end sm:gap-6 sm:p-6"
  >
    <TintedBackdrop background="linear-gradient(135deg, {tint.top}, {tint.surface})" />
    <Cover
      cover={activeAlbum.cover ?? activeAlbum.coverUrl}
      title={activeAlbum.title}
      class="size-32 shrink-0 self-center rounded-lg text-5xl shadow-2xl shadow-black/40 sm:size-48 sm:self-auto sm:text-6xl"
    />
    <div class="min-w-0">
      <p class="text-xs font-semibold tracking-widest text-white/70 uppercase">{t('Album')}</p>
      <h2 class="mt-1 truncate text-2xl font-bold tracking-tight sm:text-4xl">{activeAlbum.title}</h2>
      <p class="mt-2 truncate font-medium text-white/90">{activeAlbum.artist}</p>
      <p class="mt-1 text-sm text-white/60">
        {activeAlbum.year ? `${activeAlbum.year} · ` : ''}{count(
          tracks.length || activeAlbum.trackCount,
          '{n} track',
          '{n} tracks',
        )}
      </p>
      <div class="mt-4 flex flex-wrap items-center gap-2">
        {#if isDesktop}
          <button
            type="button"
            disabled={!ytdlpAvailable || tracks.length === 0 || online.playingId !== null}
            class="flex items-center gap-2 rounded-full bg-teal-500 py-2 pr-5 pl-4 text-sm font-semibold text-zinc-950 hover:scale-[1.03] hover:bg-teal-400 disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:scale-100"
            onclick={() => {
              if (tracks[0]) void online.playTrack(tracks[0])
            }}
          >
            <Icon name="play" class="size-5" />
            {online.playingId && tracks.some((tr) => tr.id === online.playingId)
              ? t('Loading…')
              : t('Play')}
          </button>

          <button
            type="button"
            disabled={!ytdlpAvailable || tracks.length === 0 || isAlbumBusy()}
            class="flex items-center gap-2 rounded-full border border-white/25 bg-white/10 px-4 py-2 text-sm font-medium whitespace-nowrap text-white hover:bg-white/20 disabled:cursor-not-allowed disabled:opacity-50"
            onclick={() => void online.downloadAlbum(activeAlbum, tracks)}
          >
            {#if albumJob?.state === 'done'}
              <Icon name="check" class="size-4 text-teal-300" />
              {t('Downloaded')}
            {:else if albumJob?.state === 'downloading'}
              <Icon name="download" class="size-4 animate-pulse" />
              {t('Downloading {done}/{total}', {
                done: albumJob.doneTracks,
                total: albumJob.totalTracks,
              })}
            {:else if albumJob?.state === 'queued'}
              <Icon name="download" class="size-4" />
              {t('Queued')}
            {:else}
              <Icon name="download" class="size-4" />
              {t('Download album')}
            {/if}
          </button>
        {/if}

        {#if albumInLibrary && albumJob?.state !== 'done'}
          <span class="rounded-full bg-white/15 px-3 py-1 text-xs font-medium text-white/90">
            {t('In library')}
          </span>
        {/if}
      </div>
    </div>
  </div>

  {#if online.loadingAlbum && tracks.length === 0}
    <p class="text-sm text-zinc-500">{t('Loading…')}</p>
  {:else if online.albumError}
    <div class="flex items-center gap-3 rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700 dark:border-red-900/60 dark:bg-red-950/40 dark:text-red-300">
      <span class="min-w-0 flex-1">{online.albumError}</span>
      <button
        type="button"
        class="rounded-lg bg-red-600 px-3 py-1 text-xs font-medium text-white hover:bg-red-500"
        onclick={() => void online.openAlbum(album)}
      >
        {t('Retry')}
      </button>
    </div>
  {:else if tracks.length === 0}
    <p class="text-sm text-zinc-500">{t('No tracks found for this album.')}</p>
  {:else}
    <ol class="divide-y divide-zinc-100 dark:divide-zinc-800/70">
      {#each tracks as track, i (track.id)}
        {@const job = trackJob(track)}
        {@const inLib = isTrackInLibrary(track, localTracks)}
        {@const isCurrent =
          online.playingId === track.id ||
          (player.snapshot.current?.title === track.title &&
            player.snapshot.current?.path.startsWith('http'))}
        {@const busy = job?.state === 'queued' || job?.state === 'downloading'}
        <li class="group flex items-center gap-2">
          <button
            type="button"
            disabled={!ytdlpAvailable || online.playingId !== null}
            class={[
              'flex h-11 min-w-0 flex-1 items-center gap-3 rounded-lg px-3 text-left text-sm disabled:cursor-not-allowed disabled:opacity-60',
              isCurrent
                ? 'bg-teal-50 text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
                : 'text-zinc-700 hover:bg-zinc-100/70 dark:text-zinc-300 dark:hover:bg-zinc-800/50',
            ]}
            onclick={() => void online.playTrack(track)}
          >
            <span class="w-7 text-right text-xs tabular-nums text-zinc-400">
              {track.trackNo ?? i + 1}
            </span>
            <div class="min-w-0 flex-1">
              <p class="truncate font-medium">{track.title}</p>
              {#if track.artist && track.artist !== activeAlbum.artist}
                <p class="truncate text-xs text-zinc-500">{track.artist}</p>
              {/if}
            </div>
            {#if online.playingId === track.id}
              <span class="text-xs text-teal-600 dark:text-teal-400">{t('Loading…')}</span>
            {:else if inLib && job?.state !== 'done'}
              <span class="rounded-full bg-zinc-200/80 px-2 py-0.5 text-[11px] font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300">
                {t('In library')}
              </span>
            {/if}
            <span class="text-xs tabular-nums text-zinc-500">
              {formatTime(track.durationSeconds)}
            </span>
          </button>

          {#if isDesktop}
            <button
              type="button"
              disabled={!ytdlpAvailable || busy}
              title={job?.state === 'failed' && job.error ? job.error : t('Download track')}
              aria-label={t('Download “{title}”', { title: track.title })}
              class={[
                'flex h-8 shrink-0 items-center gap-1.5 rounded-full px-3 text-xs font-medium transition disabled:cursor-not-allowed disabled:opacity-50',
                job?.state === 'done'
                  ? 'bg-teal-500/15 text-teal-700 dark:text-teal-300'
                  : job?.state === 'failed'
                    ? 'bg-red-500/15 text-red-600 hover:bg-red-500/25 dark:text-red-400'
                    : 'text-zinc-500 hover:bg-zinc-200/70 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-zinc-100',
              ]}
              onclick={() => void online.downloadTrack(track)}
            >
              {#if job?.state === 'done'}
                <Icon name="check" class="size-4" />
                <span>{t('Downloaded')}</span>
              {:else if job?.state === 'downloading'}
                <Icon name="download" class="size-4 animate-pulse" />
                <span>{t('Downloading…')}</span>
              {:else if job?.state === 'queued'}
                <Icon name="download" class="size-4" />
                <span>{t('Queued')}</span>
              {:else if job?.state === 'failed'}
                <Icon name="download" class="size-4" />
                <span>{t('Retry')}</span>
              {:else}
                <Icon name="download" class="size-4" />
                <span class="hidden sm:inline">{t('Download')}</span>
              {/if}
            </button>
          {/if}
        </li>
      {/each}
    </ol>
  {/if}
</div>
