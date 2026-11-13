<script lang="ts">
  import { artists } from './artists.svelte'
  import Cover from './Cover.svelte'
  import Icon from './Icon.svelte'
  import { count, i18n, t } from './i18n.svelte'
  import { history, rangeBounds, type ListeningStats, type StatsPreset } from './history.svelte'
  import { player } from './player.svelte'
  import { playlists } from './playlists.svelte'

  let { onOpenAlbum }: { onOpenAlbum?: (albumId: number) => void } = $props()

  const thisYear = new Date().getFullYear()
  const pad2 = (n: number) => String(n).padStart(2, '0')
  const todayStr = () => {
    const d = new Date()
    return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`
  }
  const monthAgoStr = () => {
    const d = new Date()
    d.setDate(d.getDate() - 30)
    return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`
  }

  let preset = $state<StatsPreset>('year')
  let selectedYear = $state(thisYear)
  let customFrom = $state(monthAgoStr())
  let customTo = $state(todayStr())
  let trackSort = $state<'time' | 'plays'>('time')
  let confirmingClear = $state(false)
  let savingPlaylist = $state(false)
  let stats = $state<ListeningStats | null>(null)
  let error = $state<string | null>(null)

  const bounds = $derived(rangeBounds(preset, selectedYear, customFrom, customTo))

  $effect(() => {
    void history.version
    const { from, to } = bounds
    let stale = false
    history
      .stats(from, to)
      .then((res) => {
        if (stale) return
        stats = res
        error = null
      })
      .catch((e) => {
        if (!stale) error = String(e)
      })
    return () => {
      stale = true
    }
  })

  const yearOptions = $derived.by(() => {
    const set = new Set<number>([thisYear, ...(stats?.availableYears ?? [])])
    return [...set].sort((a, b) => b - a)
  })

  const topTracks = $derived(
    trackSort === 'time' ? (stats?.topTracksByTime ?? []) : (stats?.topTracksByPlays ?? []),
  )
  const maxTrackVal = $derived.by(() => {
    const first = topTracks[0]
    if (!first) return 1
    return Math.max(1, trackSort === 'time' ? first.listenedMs : first.playCount)
  })
  const maxArtistMs = $derived(Math.max(1, stats?.topArtistsByTime[0]?.listenedMs ?? 1))
  const maxAlbumMs = $derived(Math.max(1, stats?.topAlbumsByTime[0]?.listenedMs ?? 1))
  const maxGenreMs = $derived(Math.max(1, stats?.topGenres[0]?.listenedMs ?? 1))
  const totalGenreMs = $derived(
    Math.max(1, (stats?.topGenres ?? []).reduce((acc, g) => acc + g.listenedMs, 0)),
  )
  const maxHourMs = $derived(Math.max(1, ...(stats?.byHour.map((h) => h.listenedMs) ?? [1])))
  const maxWdayMs = $derived(Math.max(1, ...(stats?.byWeekday.map((w) => w.listenedMs) ?? [1])))
  const maxMonthMs = $derived(Math.max(1, ...(stats?.byMonth.map((m) => m.listenedMs) ?? [1])))

  const localTopTrackIds = $derived(
    topTracks.map((tr) => tr.trackId).filter((id): id is number => id !== null),
  )

  const WEEKDAYS_SHORT = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']
  const WEEKDAYS_FULL = [
    'Monday',
    'Tuesday',
    'Wednesday',
    'Thursday',
    'Friday',
    'Saturday',
    'Sunday',
  ]

  function formatDurationLong(ms: number): string {
    const totalMinutes = Math.floor(ms / 60_000)
    if (totalMinutes < 1) return t('< 1 min')
    const hours = Math.floor(totalMinutes / 60)
    const mins = totalMinutes % 60
    if (hours === 0) return t('{m} min', { m: mins })
    return t('{h} hr {m} min', { h: hours.toLocaleString(i18n.locale), m: mins })
  }

  function formatShortTime(ms: number): string {
    const s = Math.round(ms / 1000)
    const m = Math.floor(s / 60)
    const r = s % 60
    return `${m}:${pad2(r)}`
  }

  function formatTimestamp(sec: number): string {
    return new Date(sec * 1000).toLocaleString(i18n.locale, {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    })
  }

  function hourPersona(hour: number | null): string {
    if (hour === null) return ''
    if (hour >= 5 && hour < 12) return t('Morning listener')
    if (hour >= 12 && hour < 17) return t('Afternoon listener')
    if (hour >= 17 && hour < 22) return t('Evening listener')
    return t('Night owl')
  }

  function periodLabel(): string {
    if (preset === '7d') return t('Last 7 days')
    if (preset === '30d') return t('Last 30 days')
    if (preset === 'year') return String(selectedYear)
    if (preset === 'custom') return `${customFrom} – ${customTo}`
    return t('All time')
  }

  async function saveTopPlaylist() {
    if (localTopTrackIds.length === 0 || savingPlaylist) return
    savingPlaylist = true
    try {
      const name = t('Top tracks · {period}', { period: periodLabel() })
      const made = await playlists.create(name, localTopTrackIds)
      if (made) playlists.flash(t('Playlist “{name}” created', { name: made.name }))
    } finally {
      savingPlaylist = false
    }
  }

  async function clearAll() {
    confirmingClear = false
    try {
      await history.clear()
      stats = await history.stats(bounds.from, bounds.to)
    } catch (e) {
      error = String(e)
    }
  }

  function playTrack(path: string) {
    if (path.startsWith('http://') || path.startsWith('https://')) return
    void player.openFiles([path])
  }
</script>

<div class="h-full overflow-y-auto px-5 pb-8">
  <div class="mx-auto flex max-w-6xl flex-col gap-6">
    <!-- Wrapped-style hero banner -->
    <section class="rounded-2xl bg-gradient-to-br from-teal-700 via-teal-900 to-zinc-950 p-6 text-white shadow-lg">
      <div class="flex flex-wrap items-start justify-between gap-4">
        <div>
          <p class="flex items-center gap-1.5 text-xs font-semibold tracking-widest text-white/70 uppercase">
            <Icon name="chart" class="size-3.5" />
            {t('Your Wrapped')}
          </p>
          <h2 class="mt-1 text-3xl font-bold tracking-tight sm:text-4xl">{t('Listening stats')}</h2>
        </div>

        <div class="flex flex-wrap items-center gap-1.5">
          {#each [
            ['7d', 'Last 7 days'],
            ['30d', 'Last 30 days'],
            ['year', 'Year'],
            ['all', 'All time'],
            ['custom', 'Custom'],
          ] as [StatsPreset, string][] as [id, label] (id)}
            <button
              type="button"
              class={[
                'rounded-full px-3 py-1.5 text-xs font-medium transition',
                preset === id
                  ? 'bg-teal-400 text-zinc-950'
                  : 'bg-white/10 text-white/85 hover:bg-white/20',
              ]}
              onclick={() => (preset = id)}
            >
              {t(label)}
            </button>
          {/each}

          {#if preset === 'year'}
            <select
              aria-label={t('Year')}
              bind:value={selectedYear}
              class="rounded-full bg-white/15 px-3 py-1.5 text-xs font-medium text-white outline-none hover:bg-white/25 focus-visible:ring-2 focus-visible:ring-teal-400"
            >
              {#each yearOptions as y (y)}
                <option value={y} class="bg-zinc-900 text-white">{y}</option>
              {/each}
            </select>
          {/if}
        </div>
      </div>

      {#if preset === 'custom'}
        <div class="mt-4 flex flex-wrap items-center gap-3 text-xs">
          <label class="flex items-center gap-2 text-white/80">
            <span>{t('From')}</span>
            <input
              type="date"
              bind:value={customFrom}
              class="rounded-lg border border-white/20 bg-black/30 px-2.5 py-1 text-white outline-none focus-visible:ring-2 focus-visible:ring-teal-400"
            />
          </label>
          <label class="flex items-center gap-2 text-white/80">
            <span>{t('To')}</span>
            <input
              type="date"
              bind:value={customTo}
              class="rounded-lg border border-white/20 bg-black/30 px-2.5 py-1 text-white outline-none focus-visible:ring-2 focus-visible:ring-teal-400"
            />
          </label>
        </div>
      {/if}

      {#if stats && stats.summary.rawEventCount > 0}
        {@const sum = stats.summary}
        {@const donePct =
          sum.rawEventCount > 0 ? Math.round((sum.completedCount / sum.rawEventCount) * 100) : 0}
        <div class="mt-6 grid grid-cols-2 gap-4 border-t border-white/15 pt-5 sm:grid-cols-3 lg:grid-cols-6">
          <div>
            <p class="text-[11px] font-medium tracking-wide text-white/60 uppercase">
              {t('Listening time')}
            </p>
            <p class="mt-1 text-xl font-bold tabular-nums sm:text-2xl">
              {formatDurationLong(sum.totalListenedMs)}
            </p>
          </div>
          <div>
            <p class="text-[11px] font-medium tracking-wide text-white/60 uppercase">
              {t('Plays')}
            </p>
            <p class="mt-1 text-xl font-bold tabular-nums sm:text-2xl">
              {sum.playCount.toLocaleString(i18n.locale)}
            </p>
          </div>
          <div>
            <p class="text-[11px] font-medium tracking-wide text-white/60 uppercase">
              {t('Unique tracks')}
            </p>
            <p class="mt-1 text-xl font-bold tabular-nums sm:text-2xl">
              {sum.uniqueTracks.toLocaleString(i18n.locale)}
            </p>
          </div>
          <div>
            <p class="text-[11px] font-medium tracking-wide text-white/60 uppercase">
              {t('Unique artists')}
            </p>
            <p class="mt-1 text-xl font-bold tabular-nums sm:text-2xl">
              {sum.uniqueArtists.toLocaleString(i18n.locale)}
            </p>
          </div>
          <div>
            <p class="text-[11px] font-medium tracking-wide text-white/60 uppercase">
              {t('Longest streak')}
            </p>
            <p class="mt-1 text-xl font-bold tabular-nums sm:text-2xl">
              {count(sum.longestStreakDays, '{n} day', '{n} days')}
            </p>
          </div>
          <div>
            <p class="text-[11px] font-medium tracking-wide text-white/60 uppercase">
              {t('Completion rate')}
            </p>
            <p class="mt-1 text-xl font-bold tabular-nums sm:text-2xl">%{donePct}</p>
          </div>
        </div>
      {/if}
    </section>

    {#if error}
      <p class="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700 dark:bg-red-950/50 dark:text-red-300">
        {error}
      </p>
    {/if}

    {#if stats && stats.summary.rawEventCount === 0}
      <div class="flex flex-col items-center justify-center rounded-2xl border border-zinc-200 bg-white px-6 py-16 text-center dark:border-zinc-800 dark:bg-zinc-900/50">
        <Icon name="history" class="size-10 text-teal-600 dark:text-teal-400" />
        <h3 class="mt-3 text-base font-semibold">
          {stats.availableYears.length === 0
            ? t('No listening history yet')
            : t('No tracks found in this period.')}
        </h3>
        <p class="mt-1.5 max-w-md text-sm text-zinc-500 dark:text-zinc-400">
          {t(
            'Play any song for a few seconds and FooModern will build your listening summary here automatically.',
          )}
        </p>
      </div>
    {:else if stats}
      <!-- Highlight insights row -->
      {@const sum = stats.summary}
      <div class="grid gap-4 sm:grid-cols-3">
        <div class="rounded-2xl border border-zinc-200 bg-white p-4 dark:border-zinc-800 dark:bg-zinc-900/60">
          <p class="text-xs font-medium tracking-wide text-zinc-400 uppercase">{t('Peak hour')}</p>
          <p class="mt-1 text-xl font-bold tabular-nums">
            {sum.topHour !== null ? `${pad2(sum.topHour)}:00 – ${pad2((sum.topHour + 1) % 24)}:00` : '—'}
          </p>
          {#if sum.topHour !== null}
            <p class="mt-0.5 text-xs text-teal-600 dark:text-teal-400">{hourPersona(sum.topHour)}</p>
          {/if}
        </div>
        <div class="rounded-2xl border border-zinc-200 bg-white p-4 dark:border-zinc-800 dark:bg-zinc-900/60">
          <p class="text-xs font-medium tracking-wide text-zinc-400 uppercase">{t('Favorite day')}</p>
          <p class="mt-1 text-xl font-bold">
            {sum.topWeekday !== null ? t(WEEKDAYS_FULL[sum.topWeekday]) : '—'}
          </p>
          <p class="mt-0.5 text-xs text-zinc-500">
            {t('Current streak')}: {count(sum.currentStreakDays, '{n} day', '{n} days')}
          </p>
        </div>
        <div class="rounded-2xl border border-zinc-200 bg-white p-4 dark:border-zinc-800 dark:bg-zinc-900/60">
          <p class="text-xs font-medium tracking-wide text-zinc-400 uppercase">{t('Plays')}</p>
          <p class="mt-1 text-xl font-bold tabular-nums">
            {sum.completedCount.toLocaleString(i18n.locale)}
          </p>
          <p class="mt-0.5 text-xs text-zinc-500">
            {t('{done} finished · {skipped} skipped', {
              done: sum.completedCount.toLocaleString(i18n.locale),
              skipped: sum.skipCount.toLocaleString(i18n.locale),
            })}
          </p>
        </div>
      </div>

      <!-- Top Tracks & Top Artists -->
      <div class="grid gap-6 lg:grid-cols-2">
        <section class="flex flex-col rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900/60">
          <div class="mb-4 flex flex-wrap items-center justify-between gap-2">
            <h3 class="text-base font-semibold">{t('Top tracks')}</h3>
            <div class="flex items-center gap-1.5">
              <div class="inline-flex rounded-lg bg-zinc-100 p-0.5 text-xs dark:bg-zinc-800">
                <button
                  type="button"
                  class={[
                    'rounded-md px-2.5 py-1 transition',
                    trackSort === 'time'
                      ? 'bg-white font-medium text-zinc-900 shadow-sm dark:bg-zinc-700 dark:text-white'
                      : 'text-zinc-500 hover:text-zinc-800 dark:text-zinc-400',
                  ]}
                  onclick={() => (trackSort = 'time')}
                >
                  {t('By time')}
                </button>
                <button
                  type="button"
                  class={[
                    'rounded-md px-2.5 py-1 transition',
                    trackSort === 'plays'
                      ? 'bg-white font-medium text-zinc-900 shadow-sm dark:bg-zinc-700 dark:text-white'
                      : 'text-zinc-500 hover:text-zinc-800 dark:text-zinc-400',
                  ]}
                  onclick={() => (trackSort = 'plays')}
                >
                  {t('By plays')}
                </button>
              </div>
              {#if localTopTrackIds.length > 0}
                <button
                  type="button"
                  disabled={savingPlaylist}
                  class="rounded-lg bg-teal-600 px-2.5 py-1 text-xs font-medium text-white hover:bg-teal-500 disabled:opacity-50"
                  onclick={() => void saveTopPlaylist()}
                >
                  {t('Save as playlist')}
                </button>
              {/if}
            </div>
          </div>

          <div class="flex flex-col gap-1.5">
            {#each topTracks as item, i (`${item.title}:${item.artist}:${i}`)}
              {@const val = trackSort === 'time' ? item.listenedMs : item.playCount}
              {@const pct = Math.max(4, Math.round((val / maxTrackVal) * 100))}
              {@const local = !item.path.startsWith('http://') && !item.path.startsWith('https://')}
              <button
                type="button"
                disabled={!local}
                title={local ? item.path : undefined}
                onclick={() => local && playTrack(item.path)}
                class={[
                  'relative flex w-full items-center gap-3 overflow-hidden rounded-xl px-2.5 py-2 text-left transition',
                  local ? 'hover:bg-zinc-100/80 dark:hover:bg-zinc-800/70' : 'cursor-default',
                ]}
              >
                <span class="w-5 shrink-0 text-right text-xs font-semibold tabular-nums text-zinc-400">
                  {i + 1}
                </span>
                <Cover cover={item.cover} title={item.title} class="size-10 shrink-0 rounded-lg text-xs" />
                <div class="min-w-0 flex-1">
                  <div class="flex items-center justify-between gap-2">
                    <span class="truncate text-sm font-medium">{item.title}</span>
                    <span class="shrink-0 text-xs font-medium tabular-nums text-zinc-700 dark:text-zinc-200">
                      {trackSort === 'time'
                        ? formatDurationLong(item.listenedMs)
                        : count(item.playCount, '{n} play', '{n} plays')}
                    </span>
                  </div>
                  <div class="mt-0.5 flex items-center justify-between gap-2 text-xs text-zinc-500">
                    <span class="truncate">{item.artist || '—'}</span>
                    <span class="shrink-0 tabular-nums">
                      {trackSort === 'time'
                        ? count(item.playCount, '{n} play', '{n} plays')
                        : formatDurationLong(item.listenedMs)}
                    </span>
                  </div>
                  <div class="mt-1.5 h-1 w-full overflow-hidden rounded-full bg-zinc-100 dark:bg-zinc-800">
                    <div
                      class="h-full rounded-full bg-teal-500/80"
                      style:width="{pct}%"
                    ></div>
                  </div>
                </div>
              </button>
            {/each}
          </div>
        </section>

        <section class="flex flex-col rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900/60">
          <h3 class="mb-4 text-base font-semibold">{t('Top artists')}</h3>
          <div class="flex flex-col gap-2">
            {#each stats.topArtistsByTime as artist, i (artist.artist)}
              {@const pct = Math.max(4, Math.round((artist.listenedMs / maxArtistMs) * 100))}
              <button
                class="flex w-full items-center gap-3 rounded-xl px-2.5 py-2 text-left transition-colors hover:bg-zinc-100 dark:hover:bg-zinc-800/70"
                onclick={() => artists.open(artist.artist)}
              >
                <span class="w-5 shrink-0 text-right text-xs font-semibold tabular-nums text-zinc-400">
                  {i + 1}
                </span>
                <Cover
                  cover={artist.cover}
                  title={artist.artist}
                  class="size-10 shrink-0 rounded-full text-xs"
                />
                <div class="min-w-0 flex-1">
                  <div class="flex items-center justify-between gap-2">
                    <span class="truncate text-sm font-medium">{artist.artist}</span>
                    <span class="shrink-0 text-xs font-medium tabular-nums text-zinc-700 dark:text-zinc-200">
                      {formatDurationLong(artist.listenedMs)}
                    </span>
                  </div>
                  <div class="mt-0.5 flex items-center justify-between gap-2 text-xs text-zinc-500">
                    <span>{count(artist.uniqueTracks, '{n} track', '{n} tracks')}</span>
                    <span class="tabular-nums">
                      {count(artist.playCount, '{n} play', '{n} plays')}
                    </span>
                  </div>
                  <div class="mt-1.5 h-1 w-full overflow-hidden rounded-full bg-zinc-100 dark:bg-zinc-800">
                    <div
                      class="h-full rounded-full bg-emerald-500/80"
                      style:width="{pct}%"
                    ></div>
                  </div>
                </div>
              </button>
            {:else}
              <p class="text-sm text-zinc-500">—</p>
            {/each}
          </div>
        </section>
      </div>

      <!-- Top Albums & Top Genres -->
      <div class="grid gap-6 lg:grid-cols-2">
        <section class="flex flex-col rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900/60">
          <h3 class="mb-4 text-base font-semibold">{t('Top albums')}</h3>
          <div class="flex flex-col gap-1.5">
            {#each stats.topAlbumsByTime as album, i (`${album.album}:${i}`)}
              {@const pct = Math.max(4, Math.round((album.listenedMs / maxAlbumMs) * 100))}
              <button
                type="button"
                disabled={album.albumId === null || !onOpenAlbum}
                class="flex w-full items-center gap-3 rounded-xl px-2.5 py-2 text-left transition enabled:hover:bg-zinc-100/80 dark:enabled:hover:bg-zinc-800/70"
                onclick={() => album.albumId !== null && onOpenAlbum?.(album.albumId)}
              >
                <span class="w-5 shrink-0 text-right text-xs font-semibold tabular-nums text-zinc-400">
                  {i + 1}
                </span>
                <Cover cover={album.cover} title={album.album} class="size-10 shrink-0 rounded-lg text-xs" />
                <div class="min-w-0 flex-1">
                  <div class="flex items-center justify-between gap-2">
                    <span class="truncate text-sm font-medium">{album.album}</span>
                    <span class="shrink-0 text-xs font-medium tabular-nums text-zinc-700 dark:text-zinc-200">
                      {formatDurationLong(album.listenedMs)}
                    </span>
                  </div>
                  <div class="mt-0.5 flex items-center justify-between gap-2 text-xs text-zinc-500">
                    <span class="truncate">{album.artist || '—'}</span>
                    <span class="shrink-0 tabular-nums">
                      {count(album.playCount, '{n} play', '{n} plays')}
                    </span>
                  </div>
                  <div class="mt-1.5 h-1 w-full overflow-hidden rounded-full bg-zinc-100 dark:bg-zinc-800">
                    <div class="h-full rounded-full bg-sky-500/80" style:width="{pct}%"></div>
                  </div>
                </div>
              </button>
            {:else}
              <p class="text-sm text-zinc-500">—</p>
            {/each}
          </div>
        </section>

        <section class="flex flex-col rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900/60">
          <h3 class="mb-4 text-base font-semibold">{t('Top genres')}</h3>
          <div class="flex flex-col gap-3">
            {#each stats.topGenres as genre, i (genre.genre)}
              {@const pct = Math.max(4, Math.round((genre.listenedMs / maxGenreMs) * 100))}
              {@const share = Math.max(1, Math.round((genre.listenedMs / totalGenreMs) * 100))}
              <div>
                <div class="flex items-center justify-between text-sm">
                  <span class="font-medium">
                    <span class="mr-2 text-xs text-zinc-400">{i + 1}</span>{genre.genre}
                  </span>
                  <span class="text-xs tabular-nums text-zinc-500">
                    %{share} · {formatDurationLong(genre.listenedMs)}
                  </span>
                </div>
                <div class="mt-1.5 h-2 w-full overflow-hidden rounded-full bg-zinc-100 dark:bg-zinc-800">
                  <div class="h-full rounded-full bg-teal-500" style:width="{pct}%"></div>
                </div>
              </div>
            {:else}
              <p class="text-sm text-zinc-500">—</p>
            {/each}
          </div>
        </section>
      </div>

      <!-- Temporal distribution charts -->
      <section class="rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900/60">
        <h3 class="mb-5 text-base font-semibold">{t('Listening rhythm')}</h3>
        <div class="grid gap-8 lg:grid-cols-2">
          <!-- 24h bar chart -->
          <div>
            <p class="mb-3 text-xs font-medium tracking-wide text-zinc-400 uppercase">
              {t('Hours of the day')}
            </p>
            <div class="flex h-36 items-end gap-1 pt-4">
              {#key stats}
                {#each stats.byHour as bucket, idx (bucket.label)}
                  {@const hPct = bucket.listenedMs > 0 ? Math.max(6, Math.round((bucket.listenedMs / maxHourMs) * 100)) : 3}
                  <div
                    class="group flex h-full flex-1 flex-col items-center justify-end gap-1"
                    title={`${bucket.label}:00 · ${formatDurationLong(bucket.listenedMs)} (${count(bucket.playCount, '{n} play', '{n} plays')})`}
                  >
                    <div class="animate-bar-y h-full w-full" style:animation-delay="{idx * 14}ms">
                      <div
                        class={[
                          'h-full w-full origin-bottom rounded-t transition-[transform,background-color] duration-500 ease-out',
                          bucket.listenedMs > 0
                            ? 'bg-teal-500/80 group-hover:bg-teal-400'
                            : 'bg-zinc-200/70 dark:bg-zinc-800/80',
                        ]}
                        style:transform="scaleY({hPct / 100})"
                      ></div>
                    </div>
                  </div>
                {/each}
              {/key}
            </div>
            <div class="mt-1.5 flex justify-between text-[10px] tabular-nums text-zinc-400">
              <span>00:00</span>
              <span>06:00</span>
              <span>12:00</span>
              <span>18:00</span>
              <span>23:00</span>
            </div>
          </div>

          <!-- Weekdays bar chart -->
          <div>
            <p class="mb-3 text-xs font-medium tracking-wide text-zinc-400 uppercase">
              {t('Days of the week')}
            </p>
            <div class="flex flex-col gap-2">
              {#key stats}
                {#each stats.byWeekday as bucket, idx (bucket.label)}
                  {@const wPct = bucket.listenedMs > 0 ? Math.max(3, Math.round((bucket.listenedMs / maxWdayMs) * 100)) : 0}
                  <div class="flex items-center gap-3 text-xs">
                    <span class="w-8 shrink-0 font-medium text-zinc-500">
                      {t(WEEKDAYS_SHORT[idx])}
                    </span>
                    <div class="h-2.5 flex-1 overflow-hidden rounded-full bg-zinc-100 dark:bg-zinc-800">
                      <div class="animate-bar-x h-full w-full" style:animation-delay="{idx * 35}ms">
                        <div
                          class="h-full w-full origin-left rounded-r-full bg-teal-500/85 transition-transform duration-500 ease-out"
                          style:transform="scaleX({wPct / 100})"
                        ></div>
                      </div>
                    </div>
                    <span class="w-20 shrink-0 text-right tabular-nums text-zinc-500">
                      {bucket.listenedMs > 0 ? formatDurationLong(bucket.listenedMs) : '—'}
                    </span>
                  </div>
                {/each}
              {/key}
            </div>
          </div>
        </div>

        {#if stats.byMonth.length > 1}
          <div class="mt-8 border-t border-zinc-100 pt-5 dark:border-zinc-800">
            <p class="mb-3 text-xs font-medium tracking-wide text-zinc-400 uppercase">
              {t('Monthly trend')}
            </p>
            <div class="flex h-32 items-end gap-2 pt-4">
              {#key stats}
                {#each stats.byMonth as bucket, idx (bucket.label)}
                  {@const mPct = Math.max(8, Math.round((bucket.listenedMs / maxMonthMs) * 100))}
                  <div
                    class="flex h-full min-w-0 flex-1 flex-col items-center justify-end gap-1.5"
                    title={`${bucket.label} · ${formatDurationLong(bucket.listenedMs)} (${count(bucket.playCount, '{n} play', '{n} plays')})`}
                  >
                    <div class="animate-bar-y min-h-0 w-full max-w-10 flex-1" style:animation-delay="{idx * 24}ms">
                      <div
                        class="h-full w-full origin-bottom rounded-t bg-emerald-500/80 transition-[transform,background-color] duration-500 ease-out hover:bg-emerald-400"
                        style:transform="scaleY({mPct / 100})"
                      ></div>
                    </div>
                    <span class="w-full truncate text-center text-[10px] tabular-nums text-zinc-400">
                      {bucket.label.slice(2)}
                    </span>
                  </div>
                {/each}
              {/key}
            </div>
          </div>
        {/if}
      </section>

      <!-- Recent Activity + History Management -->
      <section class="rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900/60">
        <div class="mb-4 flex flex-wrap items-center justify-between gap-3">
          <h3 class="text-base font-semibold">{t('Recent activity')}</h3>
          <div class="flex items-center gap-2">
            {#if confirmingClear}
              <span class="text-xs text-zinc-500" role="alert">
                {t('Clear all listening history?')}
              </span>
              <button
                type="button"
                class="rounded-lg bg-red-600 px-3 py-1 text-xs font-medium text-white hover:bg-red-500"
                onclick={() => void clearAll()}
              >
                {t('Clear')}
              </button>
              <button
                type="button"
                class="rounded-lg px-2.5 py-1 text-xs text-zinc-500 hover:bg-zinc-100 dark:hover:bg-zinc-800"
                onclick={() => (confirmingClear = false)}
              >
                {t('Cancel')}
              </button>
            {:else}
              <button
                type="button"
                class="rounded-lg px-3 py-1 text-xs text-red-600 hover:bg-red-50 dark:text-red-400 dark:hover:bg-red-950/40"
                onclick={() => (confirmingClear = true)}
              >
                {t('Clear history')}
              </button>
            {/if}
          </div>
        </div>

        <div class="divide-y divide-zinc-100 dark:divide-zinc-800/80">
          {#each stats.recent as entry (entry.id)}
            {@const local = !entry.path.startsWith('http://') && !entry.path.startsWith('https://')}
            <button
              type="button"
              disabled={!local}
              onclick={() => local && playTrack(entry.path)}
              class={[
                '-mx-2 flex w-[calc(100%+1rem)] items-center gap-3 rounded-lg px-2 py-2.5 text-left transition',
                local ? 'hover:bg-zinc-100/70 dark:hover:bg-zinc-800/50' : 'cursor-default',
              ]}
            >
              <Cover cover={entry.cover} title={entry.title} class="size-9 shrink-0 rounded text-[10px]" />
              <div class="min-w-0 flex-1">
                <p class="truncate text-sm font-medium">{entry.title}</p>
                <p class="truncate text-xs text-zinc-500">
                  {entry.artist || '—'}{entry.album ? ` · ${entry.album}` : ''}
                </p>
              </div>
              <div class="flex shrink-0 items-center gap-2 text-xs">
                {#if entry.source === 'online'}
                  <span class="rounded bg-sky-500/15 px-1.5 py-0.5 text-[11px] font-medium text-sky-600 dark:text-sky-400">
                    {t('Online')}
                  </span>
                {/if}
                {#if entry.completed}
                  <span class="rounded bg-emerald-500/15 px-1.5 py-0.5 text-[11px] font-medium text-emerald-700 dark:text-emerald-300">
                    {t('Completed')}
                  </span>
                {:else if entry.skipped}
                  <span class="rounded bg-amber-500/15 px-1.5 py-0.5 text-[11px] font-medium text-amber-700 dark:text-amber-300">
                    {t('Skipped')}
                  </span>
                {:else}
                  <span class="rounded bg-zinc-200/70 px-1.5 py-0.5 text-[11px] text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300">
                    {t('Partial')}
                  </span>
                {/if}
                <span class="w-12 text-right tabular-nums text-zinc-600 dark:text-zinc-300">
                  {formatShortTime(entry.listenedMs)}
                </span>
                <span class="hidden w-28 text-right tabular-nums text-zinc-400 sm:inline">
                  {formatTimestamp(entry.startedAt)}
                </span>
              </div>
            </button>
          {/each}
        </div>
      </section>
    {/if}
  </div>
</div>
