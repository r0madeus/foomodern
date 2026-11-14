<script lang="ts">
  import Cover from '../Cover.svelte'
  import EmptyLibrary from '../EmptyLibrary.svelte'
  import SearchInput from '../SearchInput.svelte'
  import VirtualList from '../VirtualList.svelte'
  import PageHeader from './PageHeader.svelte'
  import { artists } from '../artists.svelte'
  import { count, t } from '../i18n.svelte'
  import { library, PagedRows } from '../library.svelte'

  let {
    onBack,
    onOpenArtist,
  }: {
    onBack: () => void
    onOpenArtist: (name: string) => void
  } = $props()

  const GAP = 16
  const PAD = 16
  const TEXT = 48

  let query = $state('')
  let width = $state(0)

  const rows = $derived.by(() => {
    void library.stats.version
    return new PagedRows(artists.page(query.trim()))
  })
  $effect(() => {
    const current = rows
    current.ensureRange(0, 1)
    return () => current.dispose()
  })

  const total = $derived(rows.total ?? 0)
  const columns = $derived(width >= 560 ? 3 : 2)
  const card = $derived(Math.max(1, (width - PAD * 2 - GAP * (columns - 1)) / columns))
  const rowHeight = $derived(Math.round(card + TEXT + GAP))
</script>

<div class="absolute inset-0 flex flex-col bg-zinc-950">
  <PageHeader title={t('Artists')} {onBack} />
  <div class="shrink-0 px-4 pb-3">
    <SearchInput
      bind:value={query}
      placeholder={t('Filter artists…')}
      label={t('Filter artists…')}
    />
  </div>

  {#if rows.total === 0}
    {#if query.trim() !== ''}
      <div class="flex flex-1 flex-col items-center justify-center px-8 text-center text-sm text-zinc-400">
        {t('No artists match “{q}”.', { q: query.trim() })}
      </div>
    {:else}
      <EmptyLibrary />
    {/if}
  {:else}
    <div class="min-h-0 flex-1" bind:clientWidth={width}>
      {#if width > 0}
        <VirtualList
          class="h-full pb-[var(--mobile-bar-clear,5rem)]"
          count={Math.ceil(total / columns)}
          {rowHeight}
          overscan={2}
          onRangeChange={(s, e) => rows.ensureRange(s * columns, e * columns)}
        >
          {#snippet row(r)}
            <div
              class="grid items-start"
              style:padding="0 {PAD}px"
              style:grid-template-columns="repeat({columns}, minmax(0, 1fr))"
              style:gap="{GAP}px"
            >
              {#each { length: columns } as _, c (c)}
                {@const idx = r * columns + c}
                {@const artist = idx < total ? rows.get(idx) : undefined}
                {#if artist}
                  <button
                    type="button"
                    class="min-w-0 text-center active:opacity-70"
                    onclick={() => onOpenArtist(artist.name)}
                  >
                    <Cover
                      cover={artist.cover}
                      title={artist.name}
                      class="aspect-square w-full rounded-full text-3xl shadow-lg shadow-black/40"
                    />
                    <span class="mt-2 block truncate text-sm font-semibold text-zinc-50">
                      {artist.name}
                    </span>
                    <span class="block truncate text-xs text-zinc-400">
                      {count(artist.albumCount, '{n} album', '{n} albums')} · {count(artist.trackCount, '{n} track', '{n} tracks')}
                    </span>
                  </button>
                {:else if idx < total}
                  <div class="aspect-square w-full animate-pulse rounded-full bg-zinc-800"></div>
                {/if}
              {/each}
            </div>
          {/snippet}
        </VirtualList>
      {/if}
    </div>
  {/if}
</div>
