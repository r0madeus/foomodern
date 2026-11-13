<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import Cover from '../Cover.svelte'
  import Icon from '../Icon.svelte'
  import OnlineAlbumDetail from '../OnlineAlbumDetail.svelte'
  import TrackItem from './TrackItem.svelte'
  import { artists, type ArtistOnlineInfo, type ArtistProfile } from '../artists.svelte'
  import { history } from '../history.svelte'
  import { count, i18n, t } from '../i18n.svelte'
  import { library } from '../library.svelte'
  import type { Rgb } from '../nowCover.svelte'
  import { online, type OnlineAlbum } from '../online.svelte'
  import { player } from '../player.svelte'
  import { tintFrom } from '../tint'

  let {
    name,
    onBack,
    onOpenAlbum,
  }: {
    name: string
    onBack: () => void
    onOpenAlbum: (albumId: number) => void
  } = $props()

  let profile = $state<ArtistProfile | null>(null)
  let onlineInfo = $state<ArtistOnlineInfo | null>(null)
  let selectedOnlineAlbum = $state<OnlineAlbum | null>(null)
  let color = $state<Rgb | null>(null)
  let scrolled = $state(0)

  $effect(() => {
    void name
    selectedOnlineAlbum = null
    onlineInfo = null
    scrolled = 0
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
  <div class="absolute inset-0 flex flex-col bg-zinc-950 pt-2">
    <OnlineAlbumDetail
      album={selectedOnlineAlbum}
      onBack={() => {
        selectedOnlineAlbum = null
        online.closeAlbum()
      }}
    />
  </div>
{:else}
  <div class="absolute inset-0 flex flex-col bg-zinc-950">
    <header
      class="absolute inset-x-0 top-0 z-10 flex h-14 items-center gap-1 px-1 transition-colors"
      class:bg-zinc-900={scrolled > 220}
    >
      <button
        type="button"
        class="rounded-full bg-black/35 p-2.5 text-white active:bg-black/55"
        aria-label={t('Back')}
        onclick={onBack}
      >
        <Icon name="back" class="size-6" />
      </button>
      {#if scrolled > 220 && profile}
        <h1 class="min-w-0 flex-1 truncate text-base font-semibold">{profile.name}</h1>
      {/if}
    </header>

    <div
      class="min-h-0 flex-1 overflow-y-auto pb-8"
      onscroll={(e) => (scrolled = e.currentTarget.scrollTop)}
    >
      {#if profile}
        <div
          class="relative overflow-hidden px-4 pt-16 pb-4"
          style:background="linear-gradient(180deg, {tint.top} 0%, {tint.surface} 65%, #09090b 100%)"
        >
          <div class="flex flex-col items-center text-center">
            <Cover
              cover={portrait}
              title={profile.name}
              class="size-36 rounded-full text-5xl shadow-2xl shadow-black/50"
            />
            <h2 class="mt-4 text-2xl leading-tight font-bold text-white">{profile.name}</h2>
            <p class="mt-1 text-xs text-white/75">
              {count(profile.albumCount, '{n} album', '{n} albums')} · {count(
                profile.trackCount,
                '{n} track',
                '{n} tracks',
              )}
            </p>
            {#if profile.playCount > 0}
              <p class="mt-1 text-xs font-medium text-teal-300">
                {t('Your listens: {time} · {plays}', {
                  time: formatHoursMins(profile.listenedMs),
                  plays: count(profile.playCount, '{n} play', '{n} plays'),
                })}
              </p>
            {/if}
            {#if profile.genres.length > 0}
              <div class="mt-2.5 flex flex-wrap justify-center gap-1.5">
                {#each profile.genres as g (g)}
                  <span class="rounded-full bg-black/30 px-2.5 py-0.5 text-[11px] font-medium text-white/85">
                    {g}
                  </span>
                {/each}
              </div>
            {/if}
          </div>
        </div>

        <div class="flex items-center justify-end gap-2 px-4 pt-1 pb-2">
          <button
            type="button"
            disabled={profile.tracks.length === 0}
            class="rounded-full p-3 text-zinc-300 active:bg-white/10 disabled:opacity-40"
            aria-label={t('Shuffle play')}
            onclick={() => void artists.play(profile!.name, 0, true)}
          >
            <Icon name="shuffle" class="size-6" />
          </button>
          <button
            type="button"
            disabled={profile.tracks.length === 0}
            class="flex size-14 items-center justify-center rounded-full bg-teal-500 text-zinc-950 shadow-lg active:scale-95 disabled:opacity-40"
            aria-label={t('Play')}
            onclick={() => void artists.play(profile!.name, 0, false)}
          >
            <Icon name="play" class="size-7" />
          </button>
        </div>

        {#if profile.popularTracks.length > 0}
          <section class="mt-2">
            <h3 class="px-4 pb-1 text-base font-bold">{t('Popular')}</h3>
            {#each profile.popularTracks.slice(0, 5) as item, idx (item.track.id)}
              {@const qIdx = trackIndexById.get(item.track.id) ?? 0}
              <TrackItem
                title={item.track.title}
                subtitle={item.playCount > 0
                  ? [count(item.playCount, '{n} play', '{n} plays'), item.track.album]
                      .filter(Boolean)
                      .join(' · ')
                  : item.track.album}
                path={item.track.path}
                cover={item.track.cover}
                trackId={item.track.id}
                number={idx + 1}
                onPlay={() => void artists.play(profile!.name, qIdx)}
              />
            {/each}
          </section>
        {/if}

        {#if bio}
          <section class="mt-5 px-4">
            <h3 class="mb-2 text-base font-bold">{t('About')}</h3>
            <p class="rounded-xl border border-zinc-800 bg-zinc-900/70 p-4 text-sm leading-relaxed text-zinc-300">
              {bio}
            </p>
          </section>
        {/if}

        {#if profile.albums.length > 0}
          <section class="mt-5">
            <h3 class="px-4 pb-2 text-base font-bold">{t('Albums in library')}</h3>
            <div class="flex gap-3 overflow-x-auto px-4 pb-2">
              {#each profile.albums as album (album.id)}
                <button
                  type="button"
                  class="w-32 shrink-0 text-left active:opacity-70"
                  onclick={() => onOpenAlbum(album.id)}
                >
                  <Cover
                    cover={album.cover}
                    title={album.title}
                    class="aspect-square w-full rounded-md text-2xl shadow-lg shadow-black/40"
                  />
                  <span class="mt-1.5 block truncate text-[13px] font-semibold text-zinc-100">
                    {album.title}
                  </span>
                  <span class="block truncate text-xs text-zinc-400">
                    {album.year ? `${album.year} · ` : ''}{count(
                      album.trackCount,
                      '{n} track',
                      '{n} tracks',
                    )}
                  </span>
                </button>
              {/each}
            </div>
          </section>
        {/if}

        {#if missingAlbums.length > 0}
          <section class="mt-5">
            <h3 class="px-4 pb-2 text-base font-bold">{t('More albums (Online)')}</h3>
            <div class="flex gap-3 overflow-x-auto px-4 pb-2">
              {#each missingAlbums as oa (oa.id)}
                <button
                  type="button"
                  class="w-32 shrink-0 text-left active:opacity-70"
                  onclick={() => openOnlineAlbum(oa)}
                >
                  <Cover
                    cover={oa.cover ?? oa.coverUrl}
                    title={oa.title}
                    class="aspect-square w-full rounded-md text-2xl shadow-lg shadow-black/40"
                  />
                  <span class="mt-1.5 block truncate text-[13px] font-semibold text-zinc-100">
                    {oa.title}
                  </span>
                  <span class="block truncate text-xs text-zinc-400">
                    {oa.year ?? ''}
                  </span>
                </button>
              {/each}
            </div>
          </section>
        {/if}

        {#if profile.tracks.length > 0}
          <section class="mt-5">
            <h3 class="px-4 pb-1 text-base font-bold">{t('All tracks')}</h3>
            {#each profile.tracks as track, idx (track.id)}
              <TrackItem
                title={track.title}
                subtitle={[track.album, track.year].filter(Boolean).join(' · ')}
                path={track.path}
                cover={track.cover}
                trackId={track.id}
                onPlay={() => void artists.play(profile!.name, idx)}
              />
            {/each}
          </section>
        {/if}
      {:else}
        <div class="mx-auto mt-24 size-36 animate-pulse rounded-full bg-zinc-800"></div>
      {/if}
    </div>
  </div>
{/if}
