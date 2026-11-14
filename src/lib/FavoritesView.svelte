<script lang="ts">
  import AddToPlaylist from './AddToPlaylist.svelte'
  import AlbumLink from './AlbumLink.svelte'
  import Cover from './Cover.svelte'
  import Equalizer from './Equalizer.svelte'
  import ExtBadge from './ExtBadge.svelte'
  import ArtistLinks from './ArtistLinks.svelte'
  import FavoriteButton from './FavoriteButton.svelte'
  import Icon from './Icon.svelte'
  import VirtualList from './VirtualList.svelte'
  import { favorites } from './favorites.svelte'
  import { count, t } from './i18n.svelte'
  import { PagedRows } from './library.svelte'
  import { formatTime, player } from './player.svelte'
  import { trackMenu } from './contextMenu.svelte'

  const ROW = 48
  const COLS =
    'grid grid-cols-[3rem_minmax(0,2fr)_minmax(0,1.5fr)_minmax(0,1.5fr)_4rem_4.25rem] items-center gap-3 px-5'

  const rows = $derived.by(() => {
    void favorites.version
    return new PagedRows(favorites.page)
  })
  $effect(() => {
    const current = rows
    current.ensureRange(0, 1)
    return () => current.dispose()
  })

  const totalCount = $derived(rows.total ?? favorites.ids.size)
  const currentPath = $derived(player.snapshot.current?.path ?? null)
</script>

<div class="flex h-full flex-col">
  <div class="mx-5 mb-5 flex items-end gap-6 rounded-2xl bg-gradient-to-br from-teal-700 via-teal-900 to-zinc-950 p-6 text-white">
    <div class="flex size-40 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-teal-400 to-indigo-600 shadow-2xl shadow-black/40 sm:size-44">
      <Icon name="star" class="size-20 text-white" />
    </div>
    <div class="min-w-0">
      <p class="text-xs font-semibold tracking-widest text-white/70 uppercase">{t('Playlist')}</p>
      <h2 class="mt-1 truncate text-4xl font-bold tracking-tight">{t('Favorites')}</h2>
      <p class="mt-2 text-sm text-white/70">
        {count(totalCount, '{n} track', '{n} tracks')}
      </p>
      <div class="mt-4">
        <button
          type="button"
          disabled={totalCount === 0}
          class="flex items-center gap-2 rounded-full bg-teal-500 py-2 pr-5 pl-4 text-sm font-semibold text-zinc-950 hover:scale-[1.03] hover:bg-teal-400 disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:scale-100"
          onclick={() => void favorites.play(0)}
        >
          <Icon name="play" class="size-5" /> {t('Play')}
        </button>
      </div>
    </div>
  </div>

  {#if totalCount === 0}
    <div class="flex flex-1 flex-col items-center justify-center gap-2 px-8 text-center text-zinc-500">
      <Icon name="starOutline" class="size-8 text-zinc-400" />
      <p class="max-w-sm text-sm">
        {t('No favorite tracks yet. Click the star on any song to save it here.')}
      </p>
    </div>
  {:else}
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
    <VirtualList
      class="min-h-0 flex-1"
      count={totalCount}
      rowHeight={ROW}
      onRangeChange={(s, e) => rows.ensureRange(s, e)}
    >
      {#snippet row(i)}
        {@const track = rows.get(i)}
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
            ondblclick={() => void favorites.play(i)}
            oncontextmenu={(e) =>
              trackMenu.fromEvent(e, track, { onPlay: () => void favorites.play(i) })}
            onkeydown={(e) => e.target === e.currentTarget && e.key === 'Enter' && void favorites.play(i)}
          >
            <span class="text-right text-xs tabular-nums text-zinc-400">
              {#if track.path === currentPath && player.snapshot.status === 'playing'}
                <Equalizer class="text-teal-600 dark:text-teal-400" />
              {:else}{i + 1}{/if}
            </span>
            <span class="flex min-w-0 items-center gap-3">
              <Cover
                cover={track.cover}
                title={track.title}
                class="size-9 shrink-0 rounded text-[10px]"
              />
              <span class="truncate font-medium">{track.title}</span><ExtBadge path={track.path} />
            </span>
            <span class="truncate text-zinc-500"><ArtistLinks artist={track.artist} /></span>
            <span class="truncate text-zinc-500">
              <AlbumLink album={track.album} albumId={track.albumId} />
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
                  onPlay={() => void favorites.play(i)}
                  label={t('More actions for “{title}”', { title: track.title })}
                />
              </span>
            </span>
          </div>
        {:else}
          <div class={[COLS, 'h-full animate-pulse']} aria-hidden="true">
            <span></span>
            <span class="h-3 w-3/4 rounded bg-zinc-200 dark:bg-zinc-800"></span>
            <span class="h-3 w-1/2 rounded bg-zinc-200 dark:bg-zinc-800"></span>
            <span class="h-3 w-1/2 rounded bg-zinc-200 dark:bg-zinc-800"></span>
            <span></span>
            <span></span>
          </div>
        {/if}
      {/snippet}
    </VirtualList>
  {/if}
</div>
