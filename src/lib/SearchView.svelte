<script lang="ts">
  import AddToPlaylist from './AddToPlaylist.svelte'
  import Cover from './Cover.svelte'
  import ExtBadge from './ExtBadge.svelte'
  import FavoriteButton from './FavoriteButton.svelte'
  import Icon from './Icon.svelte'
  import OnlineAlbumDetail from './OnlineAlbumDetail.svelte'
  import { count, t } from './i18n.svelte'
  import { library, type AlbumRow, type TrackRow } from './library.svelte'
  import {
    isAlbumInLibrary,
    isTrackInLibrary,
    online,
    type DownloadJob,
  } from './online.svelte'
  import { formatTime, player } from './player.svelte'

  let {
    onOpenSettings,
    onOpenLocalAlbum,
  }: { onOpenSettings: () => void; onOpenLocalAlbum: (albumId: number) => void } = $props()

  const ytdlpAvailable = $derived(online.status?.ytdlpAvailable ?? false)
  const trimmedQuery = $derived(online.query.trim())
  const hasCurrentOnline = $derived(
    trimmedQuery !== '' && online.searchedQuery === trimmedQuery && online.results !== null,
  )
  const hasFinishedDownloads = $derived(
    online.downloads.some((j) => j.state === 'done' || j.state === 'failed'),
  )
  const activeCount = $derived(
    online.downloads.filter((j) => j.state === 'queued' || j.state === 'downloading').length,
  )
  const currentPath = $derived(player.snapshot.current?.path ?? null)

  let debouncedLocalQuery = $state('')
  $effect(() => {
    const q = trimmedQuery
    if (q === '') {
      debouncedLocalQuery = ''
      return
    }
    const timer = setTimeout(() => (debouncedLocalQuery = q), 150)
    return () => clearTimeout(timer)
  })

  let localAlbums = $state<AlbumRow[]>([])
  let localTracks = $state<TrackRow[]>([])
  let localLoadedFor = $state('')
  $effect(() => {
    void library.stats.version
    const q = debouncedLocalQuery
    if (q === '') {
      localAlbums = []
      localTracks = []
      localLoadedFor = ''
      return
    }
    let stale = false
    Promise.all([
      library.albumsPage(q)(0, 12),
      library.tracksPage(library.sort, q)(0, 20),
    ])
      .then(([albs, trs]) => {
        if (stale) return
        localAlbums = albs.items
        localTracks = trs.items
        localLoadedFor = q
      })
      .catch(() => {})
    return () => {
      stale = true
    }
  })

  function submitSearch(e: SubmitEvent) {
    e.preventDefault()
    void online.search()
  }

  function jobPercent(job: DownloadJob): number {
    if (job.totalTracks <= 0) return 0
    return Math.min(100, Math.round((job.doneTracks / job.totalTracks) * 100))
  }
</script>

<div class="flex h-full flex-col">
  {#if online.selectedAlbum !== null}
    <div class="min-h-0 flex-1">
      <OnlineAlbumDetail album={online.selectedAlbum} onBack={() => online.closeAlbum()} />
    </div>
  {:else}
    <div class="flex shrink-0 flex-col gap-3 px-5 pt-4 pb-3">
      <form class="flex items-center gap-2" onsubmit={submitSearch}>
        <div class="relative flex min-w-0 flex-1 items-center">
          <Icon
            name="search"
            class="pointer-events-none absolute left-3 size-4 text-zinc-400"
          />
          <input
            id="global-search-input"
            type="search"
            bind:value={online.query}
            placeholder={t('Search your library & online (Enter for online)…')}
            aria-label={t('Search library and online')}
            class="h-10 w-full rounded-full border border-zinc-200 bg-white pr-16 pl-9 text-sm outline-none placeholder:text-zinc-400 focus:border-teal-500 dark:border-zinc-800 dark:bg-zinc-900 dark:placeholder:text-zinc-500"
          />
          {#if online.query}
            <button
              type="button"
              aria-label={t('Clear search')}
              class="absolute right-2 rounded-full p-1 text-zinc-400 hover:bg-zinc-100 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-200"
              onclick={() => online.clearSearch()}
            >
              <Icon name="close" class="size-4" />
            </button>
          {:else}
            <kbd class="pointer-events-none absolute right-3 hidden rounded border border-zinc-200 px-1.5 py-0.5 text-[10px] font-medium text-zinc-400 sm:inline-flex dark:border-zinc-700">
              Ctrl+K
            </kbd>
          {/if}
        </div>
        <button
          type="submit"
          disabled={online.searching || trimmedQuery === ''}
          class="h-10 shrink-0 rounded-full bg-teal-500 px-5 text-sm font-semibold text-zinc-950 hover:bg-teal-400 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {online.searching ? t('Searching…') : t('Online search')}
        </button>
      </form>

      {#if online.status && !online.status.ytdlpAvailable}
        <div class="flex flex-wrap items-center gap-3 rounded-xl border border-amber-200 bg-amber-50 px-3.5 py-2.5 text-xs text-amber-900 dark:border-amber-900/60 dark:bg-amber-950/40 dark:text-amber-200">
          <span class="min-w-0 flex-1">
            {t(
              'yt-dlp was not found. You can still browse albums; install yt-dlp or set its path in Settings to stream and download.',
            )}
          </span>
          <div class="flex shrink-0 items-center gap-2">
            <button
              type="button"
              class="rounded-lg border border-amber-300 px-2.5 py-1 font-medium hover:bg-amber-100 dark:border-amber-800 dark:hover:bg-amber-900/50"
              onclick={() => void online.refreshStatus()}
            >
              {t('Refresh')}
            </button>
            <button
              type="button"
              class="rounded-lg bg-amber-900 px-2.5 py-1 font-medium text-white hover:bg-amber-800 dark:bg-amber-700 dark:hover:bg-amber-600"
              onclick={onOpenSettings}
            >
              {t('Settings')}
            </button>
          </div>
        </div>
      {/if}

      {#if online.error}
        <p class="rounded-xl bg-red-50 px-3.5 py-2 text-xs text-red-700 dark:bg-red-950/40 dark:text-red-300">
          {online.error}
        </p>
      {/if}
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-5 pb-6">
      {#if trimmedQuery === ''}
        <div class="flex h-full min-h-56 flex-col items-center justify-center gap-2 text-center">
          <div class="rounded-full bg-zinc-200/70 p-4 text-zinc-500 dark:bg-zinc-800/70 dark:text-zinc-400">
            <Icon name="search" class="size-7" />
          </div>
          <p class="mt-1 font-medium">{t('Search library and online')}</p>
          <p class="max-w-md text-sm text-zinc-500">
            {t(
              'Type to search your local library immediately, or press Enter to search, stream, and download online.',
            )}
          </p>
          {#if online.status}
            <p class="mt-2 max-w-lg truncate text-xs text-zinc-400">
              {t('Download folder')}: {online.status.downloadDir}
            </p>
          {/if}
        </div>
      {:else}
        <div class="flex flex-col gap-8">
          <section aria-labelledby="local-results-heading" class="flex flex-col gap-4">
            <h3
              id="local-results-heading"
              class="text-xs font-semibold tracking-wider text-zinc-500 uppercase"
            >
              {t('In your library')}
            </h3>

            {#if localAlbums.length === 0 && localTracks.length === 0}
              {#if localLoadedFor === debouncedLocalQuery && debouncedLocalQuery === trimmedQuery}
                <p class="text-sm text-zinc-500">
                  {t('No matches in your library.')}
                </p>
              {/if}
            {:else}
              {#if localAlbums.length > 0}
                <div class="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-4">
                  {#each localAlbums as album (album.id)}
                    <button
                      type="button"
                      class="group flex flex-col rounded-xl p-2 text-left transition hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60"
                      onclick={() => onOpenLocalAlbum(album.id)}
                    >
                      <Cover
                        cover={album.cover}
                        title={album.title}
                        class="aspect-square w-full rounded-lg text-3xl shadow-sm transition group-hover:brightness-95"
                      />
                      <span class="mt-2 truncate text-sm font-semibold">{album.title}</span>
                      <span class="truncate text-xs text-zinc-500">
                        {album.albumArtist}{album.year ? ` · ${album.year}` : ''}
                      </span>
                    </button>
                  {/each}
                </div>
              {/if}

              {#if localTracks.length > 0}
                <ol class="divide-y divide-zinc-100 dark:divide-zinc-800/70">
                  {#each localTracks as track, i (track.id)}
                    <li class="group flex items-center gap-1 py-0.5">
                      <button
                        type="button"
                        class={[
                          'flex h-11 min-w-0 flex-1 items-center gap-3 rounded-lg px-2 text-left text-sm',
                          track.path === currentPath
                            ? 'bg-teal-50 text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
                            : 'text-zinc-800 hover:bg-zinc-100/80 dark:text-zinc-200 dark:hover:bg-zinc-800/50',
                        ]}
                        onclick={() => void library.playQuery(debouncedLocalQuery, i)}
                      >
                        <Cover
                          cover={track.cover}
                          title={track.title}
                          class="size-9 shrink-0 rounded text-sm"
                        />
                        <div class="min-w-0 flex-1">
                          <p class="flex items-center truncate font-medium">
                            <span class="truncate">{track.title}</span>
                            <ExtBadge path={track.path} />
                          </p>
                          <p class="truncate text-xs text-zinc-500">
                            {track.artist}{track.album ? ` · ${track.album}` : ''}
                          </p>
                        </div>
                        <span class="text-xs tabular-nums text-zinc-500">
                          {formatTime(track.durationMs === null ? null : track.durationMs / 1000)}
                        </span>
                      </button>
                      <FavoriteButton trackId={track.id} title={track.title} revealOnHover />
                      <span class="opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 pointer-coarse:opacity-100">
                        <AddToPlaylist
                          trackIds={[track.id]}
                          editTags
                          label={t('More actions for “{title}”', { title: track.title })}
                        />
                      </span>
                    </li>
                  {/each}
                </ol>
              {/if}
            {/if}
          </section>

          <section aria-labelledby="online-results-heading" class="flex flex-col gap-5">
            <h3
              id="online-results-heading"
              class="text-xs font-semibold tracking-wider text-zinc-500 uppercase"
            >
              {t('Online')}
            </h3>

            {#if online.searching}
              <p class="text-sm text-zinc-500">{t('Searching online…')}</p>
            {:else if online.searchError && online.searchedQuery === trimmedQuery}
              <div class="flex items-center gap-3 rounded-xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700 dark:border-red-900/60 dark:bg-red-950/40 dark:text-red-300">
                <span class="min-w-0 flex-1">{online.searchError}</span>
                <button
                  type="button"
                  class="rounded-lg bg-red-600 px-3 py-1 text-xs font-medium text-white hover:bg-red-500"
                  onclick={() => void online.search()}
                >
                  {t('Retry')}
                </button>
              </div>
            {:else if hasCurrentOnline && online.results}
              {#if online.results.albums.length === 0 && online.results.tracks.length === 0}
                <div class="py-4 text-sm text-zinc-500">
                  {t('No online results for “{q}”.', { q: trimmedQuery })}
                </div>
              {:else}
                {#if online.results.albums.length > 0}
                  <div>
                    <h4 class="mb-3 text-xs font-medium text-zinc-400">{t('Albums')}</h4>
                    <div class="grid grid-cols-[repeat(auto-fill,minmax(170px,1fr))] gap-4">
                      {#each online.results.albums as album (album.id)}
                        {@const inLib = isAlbumInLibrary(album, localAlbums)}
                        <button
                          type="button"
                          class="group flex flex-col rounded-xl p-2 text-left transition hover:bg-zinc-200/60 dark:hover:bg-zinc-800/60"
                          onclick={() => void online.openAlbum(album)}
                        >
                          <Cover
                            cover={album.cover ?? album.coverUrl}
                            title={album.title}
                            class="aspect-square w-full rounded-lg text-4xl shadow-sm transition group-hover:brightness-95"
                          />
                          <span class="mt-2 truncate text-sm font-semibold">{album.title}</span>
                          <span class="truncate text-xs text-zinc-500">
                            {album.artist}{album.year ? ` · ${album.year}` : ''}
                          </span>
                          {#if inLib}
                            <span class="mt-1 inline-flex w-fit rounded-full bg-teal-500/15 px-2 py-0.5 text-[10px] font-medium text-teal-700 dark:text-teal-300">
                              {t('In library')}
                            </span>
                          {/if}
                        </button>
                      {/each}
                    </div>
                  </div>
                {/if}

                {#if online.results.tracks.length > 0}
                  <div>
                    <h4 class="mb-2 text-xs font-medium text-zinc-400">{t('Songs')}</h4>
                    <ol class="divide-y divide-zinc-100 dark:divide-zinc-800/70">
                      {#each online.results.tracks as track (track.id)}
                        {@const job = online.jobFor('track', track.title, track.artist)}
                        {@const inLib = isTrackInLibrary(track, localTracks)}
                        {@const isCurrent =
                          online.playingId === track.id ||
                          (player.snapshot.current?.title === track.title &&
                            player.snapshot.current?.path.startsWith('http'))}
                        {@const busy = job?.state === 'queued' || job?.state === 'downloading'}
                        <li class="group flex items-center gap-2 py-0.5">
                          <button
                            type="button"
                            disabled={!ytdlpAvailable || online.playingId !== null}
                            class={[
                              'flex h-12 min-w-0 flex-1 items-center gap-3 rounded-lg px-2 text-left text-sm disabled:cursor-not-allowed disabled:opacity-60',
                              isCurrent
                                ? 'bg-teal-50 text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
                                : 'text-zinc-800 hover:bg-zinc-100/80 dark:text-zinc-200 dark:hover:bg-zinc-800/50',
                            ]}
                            onclick={() => void online.playTrack(track)}
                          >
                            <Cover
                              cover={track.cover ?? track.coverUrl}
                              title={track.title}
                              class="size-9 shrink-0 rounded text-sm"
                            />
                            <div class="min-w-0 flex-1">
                              <p class="truncate font-medium">{track.title}</p>
                              <p class="truncate text-xs text-zinc-500">
                                {track.artist}{track.album ? ` · ${track.album}` : ''}
                              </p>
                            </div>
                            {#if online.playingId === track.id}
                              <span class="text-xs text-teal-600 dark:text-teal-400">
                                {t('Loading…')}
                              </span>
                            {:else if inLib && job?.state !== 'done'}
                              <span class="rounded-full bg-zinc-200/80 px-2 py-0.5 text-[11px] font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300">
                                {t('In library')}
                              </span>
                            {/if}
                            <span class="text-xs tabular-nums text-zinc-500">
                              {formatTime(track.durationSeconds)}
                            </span>
                          </button>

                          <button
                            type="button"
                            disabled={!ytdlpAvailable || busy}
                            title={
                              job?.state === 'failed' && job.error
                                ? job.error
                                : t('Download track')
                            }
                            aria-label={t('Download “{title}”', { title: track.title })}
                            class={[
                              'flex h-8 shrink-0 items-center gap-1.5 rounded-full px-3 text-xs font-medium transition disabled:cursor-not-allowed disabled:opacity-50',
                              job?.state === 'done'
                                ? 'bg-teal-500/15 text-teal-700 dark:text-teal-300'
                                : job?.state === 'failed'
                                  ? 'bg-red-500/15 text-red-600 hover:bg-red-500/25 dark:text-red-400'
                                  : 'text-zinc-600 hover:bg-zinc-200/70 hover:text-zinc-900 dark:text-zinc-300 dark:hover:bg-zinc-800 dark:hover:text-zinc-100',
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
                        </li>
                      {/each}
                    </ol>
                  </div>
                {/if}
              {/if}
            {:else}
              <div class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-zinc-200/80 bg-zinc-100/60 px-4 py-3 text-sm text-zinc-600 dark:border-zinc-800 dark:bg-zinc-900/60 dark:text-zinc-400">
                <span>{t('Press Enter to search online for “{q}”.', { q: trimmedQuery })}</span>
                <button
                  type="button"
                  class="rounded-full bg-teal-500/15 px-3.5 py-1 text-xs font-semibold text-teal-700 hover:bg-teal-500/25 dark:text-teal-300"
                  onclick={() => void online.search()}
                >
                  {t('Online search')}
                </button>
              </div>
            {/if}
          </section>
        </div>
      {/if}
    </div>
  {/if}

  {#if online.downloads.length > 0}
    <section
      aria-label={t('Downloads')}
      class="shrink-0 border-t border-zinc-200 bg-zinc-50/90 px-5 py-3 dark:border-zinc-800 dark:bg-zinc-900/80"
    >
      <div class="mb-2 flex items-center justify-between gap-2">
        <div class="flex items-center gap-2 text-xs font-semibold tracking-wider text-zinc-500 uppercase">
          <Icon name="download" class="size-4 text-teal-600 dark:text-teal-400" />
          <span>{t('Downloads')}</span>
          {#if activeCount > 0}
            <span class="rounded-full bg-teal-500/15 px-2 py-0.5 text-[11px] font-medium text-teal-700 normal-case dark:text-teal-300">
              {count(activeCount, '{n} active', '{n} active')}
            </span>
          {/if}
        </div>
        {#if hasFinishedDownloads}
          <button
            type="button"
            class="text-xs text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-100"
            onclick={() => void online.clearFinished()}
          >
            {t('Clear finished')}
          </button>
        {/if}
      </div>

      <ul class="flex max-h-36 flex-col gap-2 overflow-y-auto">
        {#each online.downloads as job (job.id)}
          <li class="flex flex-col gap-1 rounded-lg bg-white px-3 py-2 text-xs shadow-xs dark:bg-zinc-950/60">
            <div class="flex items-center justify-between gap-3">
              <div class="min-w-0 flex-1">
                <span class="truncate font-medium">{job.title}</span>
                {#if job.artist}
                  <span class="text-zinc-500"> · {job.artist}</span>
                {/if}
                {#if job.currentTrack && job.state === 'downloading'}
                  <p class="truncate text-[11px] text-zinc-500">{job.currentTrack}</p>
                {:else if job.error && job.state === 'failed'}
                  <p class="truncate text-[11px] text-red-500">{job.error}</p>
                {/if}
              </div>

              <span
                class={[
                  'shrink-0 font-medium tabular-nums',
                  job.state === 'done'
                    ? 'text-teal-600 dark:text-teal-400'
                    : job.state === 'failed'
                      ? 'text-red-500'
                      : 'text-zinc-500',
                ]}
              >
                {#if job.state === 'done'}
                  {t('Done')} ({job.doneTracks}/{job.totalTracks})
                {:else if job.state === 'failed'}
                  {t('Failed')}
                {:else if job.state === 'queued'}
                  {t('Queued')}
                {:else}
                  {job.doneTracks}/{job.totalTracks}
                {/if}
              </span>
            </div>

            {#if job.state === 'downloading' || job.state === 'queued'}
              <div class="h-1 w-full overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-800">
                <div
                  class="h-full bg-teal-500 transition-[width] duration-300"
                  style:width="{jobPercent(job)}%"
                ></div>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>
