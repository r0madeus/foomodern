<script lang="ts">
  import ArtistView from './ArtistView.svelte'
  import Cover from './Cover.svelte'
  import EmptyLibrary from './EmptyLibrary.svelte'
  import SearchInput from './SearchInput.svelte'
  import VirtualList from './VirtualList.svelte'
  import { artists } from './artists.svelte'
  import { count, t } from './i18n.svelte'
  import { library, PagedRows } from './library.svelte'

  const MIN_CARD = 170
  const GAP = 16
  const PAD = 20
  const TEXT = 54

  let {
    selectedArtist = null,
    onSelectArtist,
    onOpenAlbum,
  }: {
    selectedArtist?: string | null
    onSelectArtist?: (name: string | null) => void
    onOpenAlbum?: (albumId: number) => void
  } = $props()

  let localSelected = $state<string | null>(null)
  let width = $state(0)
  const selected = $derived(onSelectArtist ? selectedArtist : localSelected)

  function selectArtist(name: string | null) {
    if (onSelectArtist) onSelectArtist(name)
    else localSelected = name
  }

  const q = $derived(library.debouncedQuery)
  const rows = $derived.by(() => {
    void library.stats.version
    return new PagedRows(artists.page(q))
  })
  $effect(() => {
    const current = rows
    current.ensureRange(0, 1)
    return () => current.dispose()
  })

  const totalCount = $derived(rows.total ?? 0)
  const columns = $derived(Math.max(1, Math.floor((width - PAD * 2 + GAP) / (MIN_CARD + GAP))))
  const cardWidth = $derived((width - PAD * 2 - GAP * (columns - 1)) / columns)
  const rowHeight = $derived(Math.max(1, Math.round(cardWidth + TEXT + GAP)))
  const rowCount = $derived(Math.ceil(totalCount / columns))
</script>

{#if selected !== null}
  <ArtistView
    name={selected}
    onBack={() => selectArtist(null)}
    {onOpenAlbum}
  />
{:else if library.stats.tracks === 0}
  <EmptyLibrary />
{:else}
  <div class="flex h-full flex-col">
    <div class="flex items-center justify-between gap-3 px-5 pb-2">
      <div class="text-xs text-zinc-500">
        {#if q !== '' && rows.total !== null}
          {t('{n} matching', { n: rows.total })}
        {/if}
      </div>
      <SearchInput placeholder={t('Filter artists…')} />
    </div>

    {#if q !== '' && rows.total === 0}
      <div class="flex flex-1 flex-col items-center justify-center gap-2 text-center">
        <p class="text-sm text-zinc-500">{t('No artists match “{q}”.', { q })}</p>
        <button
          type="button"
          class="rounded-lg px-3 py-1.5 text-xs font-medium text-accent-700 hover:bg-accent-50 dark:text-accent-300 dark:hover:bg-accent-950/50"
          onclick={() => library.setQuery('')}
        >
          {t('Clear search')}
        </button>
      </div>
    {:else}
      <div class="min-h-0 flex-1" bind:clientWidth={width}>
        {#key q}
          <VirtualList
            class="h-full"
            count={rowCount}
            {rowHeight}
            overscan={2}
            onRangeChange={(s, e) => rows.ensureRange(s * columns, e * columns)}
          >
            {#snippet row(r)}
              <div
                class="grid items-start px-5"
                style:grid-template-columns="repeat({columns}, minmax(0, 1fr))"
                style:gap="{GAP}px"
              >
                {#each { length: columns } as _, c (c)}
                  {@const idx = r * columns + c}
                  {#if idx < totalCount}
                    {@const a = rows.get(idx)}
                    {#if a}
                      <button
                        type="button"
                        class="group flex flex-col gap-2 rounded-lg p-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-accent-500"
                        onclick={() => selectArtist(a.name)}
                      >
                        <Cover
                          cover={a.cover}
                          title={a.name}
                          class="aspect-square w-full rounded-full shadow-sm transition group-hover:scale-[1.03]"
                        />
                        <div class="min-w-0 text-center">
                          <p class="truncate text-sm font-medium">{a.name}</p>
                          <p class="truncate text-xs text-zinc-500">
                            {count(a.albumCount, '{n} album', '{n} albums')} · {count(a.trackCount, '{n} track', '{n} tracks')}
                          </p>
                        </div>
                      </button>
                    {:else}
                      <div class="flex flex-col gap-2 p-1">
                        <div class="aspect-square w-full animate-pulse rounded-full bg-zinc-200 dark:bg-zinc-800"></div>
                        <div class="mx-auto h-3.5 w-3/4 animate-pulse rounded bg-zinc-200 dark:bg-zinc-800"></div>
                        <div class="mx-auto h-3 w-1/2 animate-pulse rounded bg-zinc-200/70 dark:bg-zinc-800/70"></div>
                      </div>
                    {/if}
                  {/if}
                {/each}
              </div>
            {/snippet}
          </VirtualList>
        {/key}
      </div>
    {/if}
  </div>
{/if}
