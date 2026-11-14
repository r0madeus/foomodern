<script lang="ts">
  import { t } from '../i18n.svelte'
  import VirtualList from '../VirtualList.svelte'
  import Icon from '../Icon.svelte'
  import EmptyLibrary from '../EmptyLibrary.svelte'
  import PageHeader from './PageHeader.svelte'
  import TrackItem from './TrackItem.svelte'
  import { library, PagedRows, type Sort } from '../library.svelte'
  import { player } from '../player.svelte'

  let { onBack }: { onBack?: () => void } = $props()

  const ROW = 64
  const SORTS: { value: Sort; label: string }[] = [
    { value: 'artist', label: 'Artist' },
    { value: 'title', label: 'Title' },
    { value: 'album', label: 'Album' },
    { value: 'added', label: 'Recent' },
    { value: 'duration', label: 'Duration' },
  ]

  const rows = $derived.by(() => {
    void library.stats.version
    return new PagedRows(library.tracksPage(library.sort, '', library.sortDir))
  })
  $effect(() => {
    const current = rows
    current.ensureRange(0, 1)
    return () => current.dispose()
  })
  const total = $derived(rows.total ?? library.stats.tracks)

  async function shuffleAll() {
    if (total === 0) return
    await player.setShuffle(true)
    await library.playAll(Math.floor(Math.random() * total))
  }
</script>

<div class="flex h-full flex-col">
  <PageHeader title={t("Songs")} {onBack}>
    {#snippet actions()}
      {#if total > 0}
        <button
          class="flex items-center gap-2 rounded-full bg-teal-500 py-2 pr-4 pl-3 text-sm font-semibold text-zinc-950 active:scale-95"
          onclick={shuffleAll}
        >
          <Icon name="shuffle" class="size-4" /> {t("Shuffle")}
        </button>
      {/if}
    {/snippet}
  </PageHeader>
  {#if library.stats.tracks === 0}
    <EmptyLibrary />
  {:else}
    <div class="flex shrink-0 gap-2 overflow-x-auto px-4 pb-3">
      {#each SORTS as s (s.value)}
        <button
          class={[
            'shrink-0 rounded-full px-3.5 py-1.5 text-[13px]',
            library.sort === s.value ? 'bg-teal-500 font-medium text-zinc-950' : 'bg-zinc-800 text-zinc-100',
          ]}
          onclick={() => library.toggleColumnSort(s.value)}
        >
          {t(s.label)}{library.sort === s.value ? (library.sortDir === 'asc' ? ' ▲' : ' ▼') : ''}
        </button>
      {/each}
    </div>
    {#key `${library.sort}\0${library.sortDir}`}
      <VirtualList class="min-h-0 flex-1 pb-[var(--mobile-bar-clear,5rem)]" count={total} rowHeight={ROW} onRangeChange={(s, e) => rows.ensureRange(s, e)}>
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
              onPlay={() => library.playAll(i)}
            />
          {:else}
            <div class="flex h-16 items-center gap-3 px-4" aria-hidden="true">
              <span class="size-12 animate-pulse rounded bg-zinc-800"></span>
              <span class="h-3 w-1/2 animate-pulse rounded bg-zinc-800"></span>
            </div>
          {/if}
        {/snippet}
      </VirtualList>
    {/key}
  {/if}
</div>
