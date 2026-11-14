<script lang="ts">
  import AddToPlaylist from './AddToPlaylist.svelte'
  import AlbumLink from './AlbumLink.svelte'
  import ArtistLinks from './ArtistLinks.svelte'
  import Cover from './Cover.svelte'
  import Equalizer from './Equalizer.svelte'
  import ExtBadge from './ExtBadge.svelte'
  import FavoriteButton from './FavoriteButton.svelte'
  import Icon from './Icon.svelte'
  import PlaylistCover from './PlaylistCover.svelte'
  import VirtualList from './VirtualList.svelte'
  import { count, t } from './i18n.svelte'
  import {
    home,
    mixAccent,
    mixSubtitle,
    mixTitle,
    type HomeMixDetail,
  } from './home.svelte'
  import { formatTime, player } from './player.svelte'
  import { trackMenu } from './contextMenu.svelte'

  let {
    id,
    onBack,
    onOpenAlbum,
    onSavedPlaylist,
  }: {
    id: string
    onBack: () => void
    onOpenAlbum: (albumId: number) => void
    onSavedPlaylist?: (playlistId: number) => void
  } = $props()

  const ROW = 48
  const COLS =
    'grid grid-cols-[3rem_minmax(0,2fr)_minmax(0,1.5fr)_minmax(0,1.5fr)_4rem_4.25rem] items-center gap-3 px-5'

  let detail = $state<HomeMixDetail | null>(null)
  let loading = $state(true)
  let saving = $state(false)

  $effect(() => {
    const target = id
    void home.version
    let stale = false
    loading = true
    home
      .mixDetail(target)
      .then((res) => {
        if (!stale) {
          detail = res
          loading = false
        }
      })
      .catch(() => {
        if (!stale) {
          detail = null
          loading = false
        }
      })
    return () => {
      stale = true
    }
  })

  const currentPath = $derived(player.snapshot.current?.path ?? null)

  async function savePlaylist() {
    if (!detail || saving) return
    saving = true
    try {
      const made = await home.saveMixAsPlaylist(detail.mix.id, mixTitle(detail.mix))
      if (made) onSavedPlaylist?.(made.id)
    } finally {
      saving = false
    }
  }
</script>

<div class="flex h-full flex-col">
  <div class="flex items-center px-5 pb-3">
    <button
      type="button"
      class="flex items-center gap-1.5 rounded-lg px-2 py-1 text-sm text-zinc-600 transition hover:bg-zinc-200/70 hover:text-zinc-900 dark:text-zinc-300 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
      onclick={onBack}
    >
      <Icon name="back" class="size-4" />
      {t('Home')}
    </button>
  </div>

  {#if loading && !detail}
    <div class="mx-5 h-52 animate-pulse rounded-2xl bg-zinc-200 dark:bg-zinc-800"></div>
  {:else if !detail}
    <div class="flex flex-1 flex-col items-center justify-center gap-2 px-8 text-center text-zinc-500">
      <p class="text-sm">{t('Mix could not be loaded.')}</p>
      <button
        type="button"
        class="rounded-lg bg-zinc-200 px-3.5 py-1.5 text-sm font-medium text-zinc-800 hover:bg-zinc-300 dark:bg-zinc-800 dark:text-zinc-100 dark:hover:bg-zinc-700"
        onclick={onBack}
      >
        {t('Back')}
      </button>
    </div>
  {:else}
    {@const title = mixTitle(detail.mix)}
    <div
      class={[
        'mx-5 mb-5 flex flex-wrap items-end gap-6 rounded-2xl bg-gradient-to-br p-6 text-white shadow-lg',
        mixAccent(detail.mix.kind),
      ]}
    >
      <PlaylistCover
        cover={null}
        albumCovers={detail.mix.albumCovers}
        name={title}
        class="size-40 shrink-0 rounded-xl shadow-2xl shadow-black/40 sm:size-44"
        iconClass="size-16"
      />
      <div class="min-w-0 flex-1">
        <p class="text-xs font-semibold tracking-widest text-white/75 uppercase">
          {t('Dynamic mix')}
        </p>
        <h2 class="mt-1 truncate text-3xl font-bold tracking-tight sm:text-4xl">{title}</h2>
        <p class="mt-1.5 text-sm text-white/80">{mixSubtitle(detail.mix)}</p>
        <p class="mt-1 text-xs text-white/65">
          {count(detail.tracks.length, '{n} song', '{n} songs')}
        </p>
        <div class="mt-4 flex flex-wrap items-center gap-2.5">
          <button
            type="button"
            disabled={detail.tracks.length === 0}
            class="flex items-center gap-2 rounded-full bg-teal-500 py-2 pr-5 pl-4 text-sm font-semibold text-zinc-950 transition hover:scale-[1.03] hover:bg-teal-400 disabled:cursor-not-allowed disabled:opacity-50"
            onclick={() => void home.playMix(detail!.mix.id, 0, false)}
          >
            <Icon name="play" class="size-5" />
            {t('Play')}
          </button>
          <button
            type="button"
            disabled={detail.tracks.length === 0}
            class="flex items-center gap-2 rounded-full bg-white/15 px-4 py-2 text-sm font-medium text-white transition hover:bg-white/25 disabled:cursor-not-allowed disabled:opacity-50"
            onclick={() => void home.playMix(detail!.mix.id, 0, true)}
          >
            <Icon name="shuffle" class="size-4" />
            {t('Shuffle')}
          </button>
          <button
            type="button"
            disabled={detail.tracks.length === 0 || saving}
            class="flex items-center gap-2 rounded-full bg-white/15 px-4 py-2 text-sm font-medium text-white transition hover:bg-white/25 disabled:cursor-not-allowed disabled:opacity-50"
            onclick={() => void savePlaylist()}
          >
            <Icon name="plus" class="size-4" />
            {t('Save as playlist')}
          </button>
        </div>
      </div>
    </div>

    <div
      class={[
        COLS,
        'border-b border-zinc-200 py-1.5 text-xs font-medium uppercase tracking-wide text-zinc-400 dark:border-zinc-800',
      ]}
    >
      <span class="text-right">#</span>
      <span>{t('Title')}</span>
      <span>{t('Artist')}</span>
      <span>{t('Album')}</span>
      <span class="text-right">{t('Time')}</span>
      <span></span>
    </div>

    <VirtualList class="min-h-0 flex-1" count={detail.tracks.length} rowHeight={ROW}>
      {#snippet row(i)}
        {@const track = detail?.tracks[i]}
        {#if track}
          <div
            role="button"
            tabindex="0"
            title={track.path}
            class={[
              COLS,
              'group h-full cursor-default text-sm outline-none focus-visible:ring-2 focus-visible:ring-teal-500 focus-visible:ring-inset',
              track.path === currentPath
                ? 'bg-teal-50 text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
                : 'text-zinc-700 hover:bg-zinc-100/70 dark:text-zinc-300 dark:hover:bg-zinc-800/50',
            ]}
            ondblclick={() => void home.playMix(detail!.mix.id, i)}
            oncontextmenu={(e) =>
              trackMenu.fromEvent(e, track, { onPlay: () => void home.playMix(detail!.mix.id, i) })}
            onkeydown={(e) => e.key === 'Enter' && void home.playMix(detail!.mix.id, i)}
          >
            <span class="text-right text-xs tabular-nums text-zinc-400">
              {#if track.path === currentPath && player.snapshot.status === 'playing'}
                <Equalizer class="text-teal-600 dark:text-teal-400" />
              {:else}
                {i + 1}
              {/if}
            </span>
            <span class="flex min-w-0 items-center gap-3">
              <Cover
                cover={track.cover}
                title={track.title}
                class="size-9 shrink-0 rounded text-[10px]"
              />
              <span class="truncate font-medium">{track.title}</span>
              <ExtBadge path={track.path} />
            </span>
            <span class="truncate text-zinc-500"><ArtistLinks artist={track.artist} /></span>
            <span class="min-w-0 truncate text-zinc-500">
              <AlbumLink album={track.album} albumId={track.albumId} onNavigate={onOpenAlbum} />
            </span>
            <span class="text-right text-xs tabular-nums text-zinc-500">
              {formatTime(track.durationMs === null ? null : track.durationMs / 1000)}
            </span>
            <span class="flex items-center justify-end gap-1">
              <FavoriteButton trackId={track.id} title={track.title} />
              <span class="opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 pointer-coarse:opacity-100">
                <AddToPlaylist
                  trackIds={[track.id]}
                  {track}
                  editTags
                  onPlay={() => void home.playMix(detail!.mix.id, i)}
                  label={t('More actions for “{title}”', { title: track.title })}
                />
              </span>
            </span>
          </div>
        {/if}
      {/snippet}
    </VirtualList>
  {/if}
</div>
