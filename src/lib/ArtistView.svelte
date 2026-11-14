<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import AddToPlaylist from './AddToPlaylist.svelte'
  import AlbumLink from './AlbumLink.svelte'
  import Cover from './Cover.svelte'
  import Equalizer from './Equalizer.svelte'
  import ExtBadge from './ExtBadge.svelte'
  import FavoriteButton from './FavoriteButton.svelte'
  import Icon from './Icon.svelte'
  import OnlineAlbumDetail from './OnlineAlbumDetail.svelte'
  import TintedBackdrop from './TintedBackdrop.svelte'
  import { artists, type ArtistOnlineInfo, type ArtistProfile } from './artists.svelte'
  import { history } from './history.svelte'
  import { count, i18n, t } from './i18n.svelte'
  import { library } from './library.svelte'
  import type { Rgb } from './nowCover.svelte'
  import { online, type OnlineAlbum } from './online.svelte'
  import { formatTime, player } from './player.svelte'
  import { tintFrom } from './tint'
  import { trackMenu } from './contextMenu.svelte'

  let {
    name,
    onBack,
    onOpenAlbum,
  }: {
    name: string
    onBack: () => void
    onOpenAlbum?: (albumId: number) => void
  } = $props()

  let profile = $state<ArtistProfile | null>(null)
  let onlineInfo = $state<ArtistOnlineInfo | null>(null)
  let selectedOnlineAlbum = $state<OnlineAlbum | null>(null)
  let color = $state<Rgb | null>(null)

  $effect(() => {
    void name
    selectedOnlineAlbum = null
    onlineInfo = null
  })

  $effect(() => {
    void library.stats.version
    void history.version
    const wanted = name
    let stale = false
    artists
      .profile(wanted)
      .then((res) => {
        if (!stale) profile = res
      })
      .catch((e) => (player.error = String(e)))
    return () => {
      stale = true
    }
  })

  $effect(() => {
    const wanted = name
    const lang = i18n.lang
    let stale = false
    artists
      .enrich(wanted, lang)
      .then((res) => {
        if (!stale) onlineInfo = res
      })
      .catch(() => {})
    return () => {
      stale = true
    }
  })

  const portrait = $derived(
    onlineInfo?.image ?? profile?.image ?? profile?.fallbackCover ?? null,
  )
  const bio = $derived(onlineInfo?.bio ?? profile?.bio ?? null)

  $effect(() => {
    const cover = portrait
    color = null
    if (!cover) return
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
  const currentPath = $derived(player.snapshot.current?.path ?? null)

  const trackIndexById = $derived.by(() => {
    const map = new Map<number, number>()
    if (!profile) return map
    profile.tracks.forEach((tr, idx) => map.set(tr.id, idx))
    return map
  })

  const missingAlbums = $derived.by(() => {
    if (!onlineInfo) return []
    const localTitles = new Set(
      (profile?.albums ?? []).map((a) => a.title.trim().toLowerCase()),
    )
    return onlineInfo.albums.filter((a) => !localTitles.has(a.title.trim().toLowerCase()))
  })

  function formatHoursMins(ms: number): string {
    const totalMin = Math.floor(ms / 60_000)
    if (totalMin < 1) return t('< 1 min')
    const h = Math.floor(totalMin / 60)
    const m = totalMin % 60
    return h === 0
      ? t('{m} min', { m })
      : t('{h} hr {m} min', { h: h.toLocaleString(i18n.locale), m })
  }

  function openOnlineAlbum(album: OnlineAlbum) {
    selectedOnlineAlbum = album
    void online.openAlbum(album)
  }
</script>

{#if selectedOnlineAlbum}
  <OnlineAlbumDetail
    album={selectedOnlineAlbum}
    onBack={() => {
      selectedOnlineAlbum = null
      online.closeAlbum()
    }}
  />
{:else}
  <div class="h-full overflow-y-auto px-5 pb-8">
    <button
      type="button"
      class="mb-4 rounded-lg px-2 py-1 text-sm text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800"
      onclick={onBack}
    >
      {t('← Artists')}
    </button>

    {#if profile}
      <div
        class="relative isolate mb-6 flex flex-wrap items-end gap-6 overflow-hidden rounded-2xl p-6 text-white"
      >
        <TintedBackdrop background="linear-gradient(135deg, {tint.top}, {tint.surface})" />
        <Cover
          cover={portrait}
          title={profile.name}
          class="size-40 shrink-0 rounded-full text-5xl shadow-2xl shadow-black/40 sm:size-44"
        />
        <div class="min-w-0 flex-1">
          <p class="text-xs font-semibold tracking-widest text-white/70 uppercase">{t('Artist')}</p>
          <h2 class="mt-1 truncate text-4xl font-bold tracking-tight sm:text-5xl">{profile.name}</h2>
          <p class="mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-white/75">
            <span>{count(profile.albumCount, '{n} album', '{n} albums')}</span>
            <span>·</span>
            <span>{count(profile.trackCount, '{n} track', '{n} tracks')}</span>
            {#if profile.playCount > 0}
              <span>·</span>
              <span>
                {t('Your listens: {time} · {plays}', {
                  time: formatHoursMins(profile.listenedMs),
                  plays: count(profile.playCount, '{n} play', '{n} plays'),
                })}
              </span>
            {/if}
          </p>

          {#if profile.genres.length > 0}
            <div class="mt-2.5 flex flex-wrap gap-1.5">
              {#each profile.genres as g (g)}
                <span class="rounded-full bg-white/10 px-2.5 py-0.5 text-xs text-white/85">{g}</span>
              {/each}
            </div>
          {/if}

          <div class="mt-4 flex flex-wrap items-center gap-2">
            <button
              type="button"
              disabled={profile.trackCount === 0}
              class="flex items-center gap-2 rounded-full bg-teal-500 py-2 pr-5 pl-4 text-sm font-semibold text-zinc-950 transition hover:scale-[1.03] hover:bg-teal-400 disabled:cursor-not-allowed disabled:opacity-50"
              onclick={() => void artists.play(profile!.name, 0, false)}
            >
              <Icon name="play" class="size-5" />
              {t('Play')}
            </button>
            <button
              type="button"
              disabled={profile.trackCount === 0}
              class="flex items-center gap-1.5 rounded-full bg-white/10 px-4 py-2 text-sm font-medium text-white transition hover:bg-white/20 disabled:cursor-not-allowed disabled:opacity-50"
              onclick={() => void artists.play(profile!.name, 0, true)}
            >
              <Icon name="shuffle" class="size-4" />
              {t('Shuffle')}
            </button>
          </div>
        </div>
      </div>

      <div class="flex flex-col gap-8">
        <!-- Popular tracks & About -->
        {#if profile.popularTracks.length > 0 || bio}
          <div class="grid gap-6 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
            {#if profile.popularTracks.length > 0}
              <section>
                <h3 class="mb-3 text-base font-semibold">{t('Popular')}</h3>
                <div class="flex flex-col gap-0.5">
                  {#each profile.popularTracks as entry, i (entry.track.id)}
                    {@const tr = entry.track}
                    {@const active = tr.path === currentPath}
                    {@const idx = trackIndexById.get(tr.id) ?? 0}
                    <div
                      role="button"
                      tabindex="0"
                      title={tr.path}
                      class={[
                        'group grid grid-cols-[1.75rem_2.25rem_minmax(0,1fr)_auto_4.25rem_3.5rem] items-center gap-3 rounded-lg px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-teal-500',
                        active
                          ? 'bg-teal-50 text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
                          : 'text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800/50',
                      ]}
                      ondblclick={() => void artists.play(profile!.name, idx)}
                      oncontextmenu={(e) =>
                        trackMenu.fromEvent(e, tr, {
                          onPlay: () => void artists.play(profile!.name, idx),
                        })}
                      onkeydown={(e) => e.key === 'Enter' && void artists.play(profile!.name, idx)}
                    >
                      <span class="text-right text-xs tabular-nums text-zinc-400">
                        {#if active && player.snapshot.status === 'playing'}
                          <Equalizer class="text-teal-600 dark:text-teal-400" />
                        {:else}
                          {i + 1}
                        {/if}
                      </span>
                      <Cover cover={tr.cover} title={tr.title} class="size-9 rounded text-[10px]" />
                      <span class="min-w-0">
                        <span class="flex items-center truncate font-medium">
                          <span class="truncate">{tr.title}</span>
                          <ExtBadge path={tr.path} />
                        </span>
                        {#if tr.album}
                          <span class="block truncate text-xs text-zinc-500">
                            <AlbumLink album={tr.album} albumId={tr.albumId} />
                          </span>
                        {/if}
                      </span>
                      <span class="text-xs tabular-nums text-zinc-400">
                        {entry.playCount > 0
                          ? count(entry.playCount, '{n} play', '{n} plays')
                          : ''}
                      </span>
                      <span class="flex items-center justify-end gap-0.5">
                        <FavoriteButton trackId={tr.id} title={tr.title} revealOnHover />
                        <span class="opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 pointer-coarse:opacity-100">
                          <AddToPlaylist
                            trackIds={[tr.id]}
                            track={tr}
                            editTags
                            onPlay={() => void artists.play(profile!.name, idx)}
                            label={t('More actions for “{title}”', { title: tr.title })}
                          />
                        </span>
                      </span>
                      <span class="text-right text-xs tabular-nums text-zinc-500">
                        {formatTime(tr.durationMs === null ? null : tr.durationMs / 1000)}
                      </span>
                    </div>
                  {/each}
                </div>
              </section>
            {/if}

            {#if bio}
              <section>
                <h3 class="mb-3 text-base font-semibold">{t('About')}</h3>
                <div class="rounded-2xl border border-zinc-200 bg-white p-4 text-sm leading-relaxed text-zinc-600 dark:border-zinc-800 dark:bg-zinc-900/60 dark:text-zinc-300">
                  {bio}
                </div>
              </section>
            {/if}
          </div>
        {/if}

        <!-- Local albums -->
        {#if profile.albums.length > 0}
          <section>
            <h3 class="mb-3 text-base font-semibold">{t('Albums in library')}</h3>
            <div class="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6">
              {#each profile.albums as a (a.id)}
                <button
                  type="button"
                  class="group flex flex-col gap-2 rounded-lg p-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-teal-500"
                  onclick={() => onOpenAlbum?.(a.id)}
                >
                  <Cover
                    cover={a.cover}
                    title={a.title}
                    class="aspect-square w-full rounded-lg shadow-sm transition group-hover:scale-[1.02]"
                  />
                  <div class="min-w-0">
                    <p class="truncate text-sm font-medium">{a.title}</p>
                    <p class="truncate text-xs text-zinc-500">
                      {a.year ? `${a.year} · ` : ''}{count(a.trackCount, '{n} track', '{n} tracks')}
                    </p>
                  </div>
                </button>
              {/each}
            </div>
          </section>
        {/if}

        <!-- Online missing discography -->
        {#if missingAlbums.length > 0}
          <section>
            <h3 class="mb-3 text-base font-semibold">{t('More albums (Online)')}</h3>
            <div class="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6">
              {#each missingAlbums as a (a.id)}
                <button
                  type="button"
                  class="group flex flex-col gap-2 rounded-lg p-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-teal-500"
                  onclick={() => openOnlineAlbum(a)}
                >
                  <Cover
                    cover={a.cover}
                    title={a.title}
                    class="aspect-square w-full rounded-lg shadow-sm transition group-hover:scale-[1.02]"
                  />
                  <div class="min-w-0">
                    <p class="truncate text-sm font-medium">{a.title}</p>
                    <p class="truncate text-xs text-zinc-500">
                      {a.year ? `${a.year} · ` : ''}{count(a.trackCount, '{n} track', '{n} tracks')}
                    </p>
                  </div>
                </button>
              {/each}
            </div>
          </section>
        {/if}

        <!-- All local tracks -->
        {#if profile.tracks.length > 0}
          <section>
            <h3 class="mb-3 text-base font-semibold">{t('All tracks')}</h3>
            <div class="flex flex-col gap-0.5">
              {#each profile.tracks as tr, i (tr.id)}
                {@const active = tr.path === currentPath}
                <div
                  role="button"
                  tabindex="0"
                  title={tr.path}
                  class={[
                    'group grid grid-cols-[2rem_minmax(0,2fr)_minmax(0,1.5fr)_4.25rem_3.5rem] items-center gap-3 rounded-lg px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-teal-500',
                    active
                      ? 'bg-teal-50 text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
                      : 'text-zinc-700 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800/50',
                  ]}
                  ondblclick={() => void artists.play(profile!.name, i)}
                  oncontextmenu={(e) =>
                    trackMenu.fromEvent(e, tr, {
                      onPlay: () => void artists.play(profile!.name, i),
                    })}
                  onkeydown={(e) => e.key === 'Enter' && void artists.play(profile!.name, i)}
                >
                  <span class="text-right text-xs tabular-nums text-zinc-400">
                    {#if active && player.snapshot.status === 'playing'}
                      <Equalizer class="text-teal-600 dark:text-teal-400" />
                    {:else}
                      {i + 1}
                    {/if}
                  </span>
                  <span class="flex min-w-0 items-center truncate font-medium">
                    <span class="truncate">{tr.title}</span>
                    <ExtBadge path={tr.path} />
                  </span>
                  <span class="truncate text-xs text-zinc-500">
                    <AlbumLink album={tr.album} albumId={tr.albumId} />
                  </span>
                  <span class="flex items-center justify-end gap-0.5">
                    <FavoriteButton trackId={tr.id} title={tr.title} revealOnHover />
                    <span class="opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 pointer-coarse:opacity-100">
                      <AddToPlaylist
                        trackIds={[tr.id]}
                        track={tr}
                        editTags
                        onPlay={() => void artists.play(profile!.name, i)}
                        label={t('More actions for “{title}”', { title: tr.title })}
                      />
                    </span>
                  </span>
                  <span class="text-right text-xs tabular-nums text-zinc-500">
                    {formatTime(tr.durationMs === null ? null : tr.durationMs / 1000)}
                  </span>
                </div>
              {/each}
            </div>
          </section>
        {/if}
      </div>
    {/if}
  </div>
{/if}
