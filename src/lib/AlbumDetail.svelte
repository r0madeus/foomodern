<script lang="ts">
  import { count, t } from './i18n.svelte'
  import { invoke } from '@tauri-apps/api/core'
  import Cover from './Cover.svelte'
  import Equalizer from './Equalizer.svelte'
  import Icon from './Icon.svelte'
  import TintedBackdrop from './TintedBackdrop.svelte'
  import AddToPlaylist from './AddToPlaylist.svelte'
  import ArtistLinks from './ArtistLinks.svelte'
  import ExtBadge from './ExtBadge.svelte'
  import FavoriteButton from './FavoriteButton.svelte'
  import { metadataMatch } from './metadata.svelte'
  import { tagEditor } from './tags.svelte'
  import { library, type AlbumRow, type TrackRow } from './library.svelte'
  import { pinned } from './pinned.svelte'
  import { player, formatTime } from './player.svelte'
  import { tintFrom } from './tint'
  import type { Rgb } from './nowCover.svelte'

  let { albumId, onBack }: { albumId: number; onBack: () => void } = $props()

  let data = $state<{ album: AlbumRow; tracks: TrackRow[] } | null>(null)

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

  const currentPath = $derived(player.snapshot.current?.path ?? null)

  // Header on the album cover's colour.
  let color = $state<Rgb | null>(null)
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
</script>

<div class="h-full overflow-y-auto px-5 pb-6">
  <button class="mb-4 rounded-lg px-2 py-1 text-sm text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800" onclick={onBack}>
    {t("← Albums")}
  </button>
  {#if data}
    <div
      class="relative isolate mb-6 flex items-end gap-6 overflow-hidden rounded-2xl p-6 text-white"
    >
      <TintedBackdrop background="linear-gradient(135deg, {tint.top}, {tint.surface})" />
      <Cover cover={data.album.cover} title={data.album.title} class="size-48 shrink-0 rounded-lg text-6xl shadow-2xl shadow-black/40" />
      <div class="min-w-0">
        <p class="text-xs font-semibold tracking-widest text-white/70 uppercase">{t("Album")}</p>
        <h2 class="mt-1 truncate text-4xl font-bold tracking-tight">{data.album.title}</h2>
        <p class="mt-2 truncate font-medium text-white/90">
          <ArtistLinks artist={data.album.albumArtist} onNavigate={onBack} />
        </p>
        <p class="mt-1 text-sm text-white/60">
          {data.album.year ? `${data.album.year} · ` : ''}{count(data.album.trackCount, '{n} track', '{n} tracks')}
        </p>
        <div class="mt-4 flex flex-wrap items-center gap-2">
          <button
            class="flex items-center gap-2 rounded-full bg-teal-500 py-2 pr-5 pl-4 text-sm font-semibold text-zinc-950 hover:scale-[1.03] hover:bg-teal-400"
            onclick={() => library.playAlbum(albumId, 0)}
          >
            <Icon name="play" class="size-5" /> {t("Play")}
          </button>
          <button
            type="button"
            aria-pressed={pinned.isPinned('album', albumId)}
            class={[
              'flex items-center gap-1.5 rounded-full border px-3.5 py-2 text-sm font-medium whitespace-nowrap transition',
              pinned.isPinned('album', albumId)
                ? 'border-teal-400/60 bg-teal-500/20 text-teal-200 hover:bg-teal-500/30'
                : 'border-white/20 bg-white/10 text-white/90 hover:bg-white/20',
            ]}
            onclick={() => void pinned.toggle('album', albumId)}
          >
            <Icon name="pin" class="size-4" />
            {pinned.isPinned('album', albumId) ? t('Pinned') : t('Pin')}
          </button>
          <AddToPlaylist {albumId} label="Add to playlist" text class="whitespace-nowrap text-white! hover:bg-white/10!" />
          <button
            type="button"
            class="rounded-lg px-3 py-2 text-sm font-medium whitespace-nowrap text-white/90 outline-none hover:bg-white/10 focus-visible:ring-2 focus-visible:ring-white/60"
            onclick={() => tagEditor.open(data!.tracks.map((track) => track.id), t('Edit album tags'))}
          >
            {t("Edit tags…")}
          </button>
          <button
            type="button"
            class="rounded-lg px-3 py-2 text-sm font-medium whitespace-nowrap text-white/90 outline-none hover:bg-white/10 focus-visible:ring-2 focus-visible:ring-white/60"
            onclick={() => metadataMatch.openAlbum(albumId)}
          >
            {t("Find info")}
          </button>
        </div>
      </div>
    </div>
    <ol class="divide-y divide-zinc-100 dark:divide-zinc-800/70">
      {#each data.tracks as track, i (track.id)}
        <li class="group flex items-center gap-1">
          <button
            class={[
              'flex h-10 min-w-0 flex-1 items-center gap-3 px-3 text-left text-sm',
              track.path === currentPath
                ? 'bg-teal-50 text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
                : 'text-zinc-700 hover:bg-zinc-100/70 dark:text-zinc-300 dark:hover:bg-zinc-800/50',
            ]}
            onclick={() => library.playAlbum(albumId, i)}
          >
            <span class="w-8 text-right text-xs tabular-nums text-zinc-400">
              {#if track.path === currentPath && player.snapshot.status === 'playing'}
                <Equalizer class="text-teal-600 dark:text-teal-400" />
              {:else}
                {track.trackNo ?? i + 1}
              {/if}
            </span>
            <span class="flex min-w-0 flex-1 items-center">
              <span class="truncate font-medium">{track.title}</span><ExtBadge path={track.path} />
            </span>
            <span class="text-xs tabular-nums text-zinc-500">
              {formatTime(track.durationMs === null ? null : track.durationMs / 1000)}
            </span>
          </button>
          <span class="flex items-center gap-1">
            <FavoriteButton trackId={track.id} title={track.title} revealOnHover />
            <span class="opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 pointer-coarse:opacity-100">
              <AddToPlaylist trackIds={[track.id]} editTags label={t('More actions for “{title}”', { title: track.title })} />
            </span>
          </span>
        </li>
      {/each}
    </ol>
  {:else}
    <p class="text-zinc-500">{t("Loading…")}</p>
  {/if}
</div>
