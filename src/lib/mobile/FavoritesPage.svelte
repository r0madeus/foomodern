<script lang="ts">
  import Icon from '../Icon.svelte'
  import PageHeader from './PageHeader.svelte'
  import TrackItem from './TrackItem.svelte'
  import VirtualList from '../VirtualList.svelte'
  import { favorites } from '../favorites.svelte'
  import { count, t } from '../i18n.svelte'
  import { PagedRows } from '../library.svelte'
  import { player } from '../player.svelte'

  let { onBack }: { onBack: () => void } = $props()

  const ROW = 64

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

  async function shufflePlay() {
    if (totalCount === 0) return
    await player.setShuffle(true)
    await favorites.play(Math.floor(Math.random() * totalCount))
  }
</script>

<div class="absolute inset-0 flex flex-col bg-zinc-950">
  <PageHeader title={t('Favorites')} {onBack} />
  <div class="flex shrink-0 items-center gap-4 px-4 pt-1 pb-4">
    <div class="flex size-20 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-teal-400 to-indigo-600 shadow-lg shadow-black/40">
      <Icon name="star" class="size-10 text-white" />
    </div>
    <div class="min-w-0 flex-1">
      <h2 class="truncate text-xl font-bold">{t('Favorites')}</h2>
      <p class="mt-0.5 text-xs text-zinc-400">
        {count(totalCount, '{n} track', '{n} tracks')}
      </p>
    </div>
    {#if totalCount > 0}
      <button
        type="button"
        class="rounded-full p-3 text-zinc-300 active:bg-white/10"
        aria-label={t('Shuffle play')}
        onclick={shufflePlay}
      >
        <Icon name="shuffle" class="size-6" />
      </button>
      <button
        type="button"
        class="flex size-12 items-center justify-center rounded-full bg-teal-500 text-zinc-950 shadow-lg active:scale-95"
        aria-label={t('Play')}
        onclick={() => void favorites.play(0)}
      >
        <Icon name="play" class="size-6" />
      </button>
    {/if}
  </div>

  {#if totalCount === 0}
    <div class="flex flex-1 flex-col items-center justify-center gap-2 px-8 text-center text-zinc-400">
      <Icon name="starOutline" class="size-8 text-zinc-500" />
      <p class="text-sm">
        {t('No favorite tracks yet. Click the star on any song to save it here.')}
      </p>
    </div>
  {:else}
    <VirtualList
      class="min-h-0 flex-1 pb-[var(--mobile-bar-clear,5rem)]"
      count={totalCount}
      rowHeight={ROW}
      onRangeChange={(s, e) => rows.ensureRange(s, e)}
    >
      {#snippet row(i)}
        {@const track = rows.get(i)}
        {#if track}
          <TrackItem
            title={track.title}
            subtitle={[track.artist, track.album].filter(Boolean).join(' · ')}
            path={track.path}
            cover={track.cover}
            trackId={track.id}
            {track}
            onPlay={() => void favorites.play(i)}
          />
        {:else}
          <div class="flex h-16 items-center gap-3 px-4" aria-hidden="true">
            <span class="size-12 animate-pulse rounded bg-zinc-800"></span>
            <span class="h-3 w-1/2 animate-pulse rounded bg-zinc-800"></span>
          </div>
        {/if}
      {/snippet}
    </VirtualList>
  {/if}
</div>
