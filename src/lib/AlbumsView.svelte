<script lang="ts">
  import { t } from './i18n.svelte'
  import VirtualList from './VirtualList.svelte'
  import EmptyLibrary from './EmptyLibrary.svelte'
  import AlbumDetail from './AlbumDetail.svelte'
  import Cover from './Cover.svelte'
  import SearchInput from './SearchInput.svelte'
  import { library, PagedRows } from './library.svelte'

  const MIN_CARD = 180
  const GAP = 16
  const PAD = 20
  const TEXT = 52

  let selected = $state<number | null>(null)
  let width = $state(0)

  const q = $derived(library.debouncedQuery)
  const rows = $derived.by(() => {
    void library.stats.version
    return new PagedRows(library.albumsPage(q))
  })
  $effect(() => {
    // Capture this instance: the cleanup runs after `rows` has already been
    // re-derived, so reading `rows` there would dispose the NEW cache and drop
    // its pages (the list then stayed on skeleton rows after startup).
    const current = rows
    current.ensureRange(0, 1)
    return () => current.dispose()
  })

  const totalCount = $derived(rows.total ?? (q === '' ? library.stats.albums : 0))
  const columns = $derived(Math.max(1, Math.floor((width - PAD * 2 + GAP) / (MIN_CARD + GAP))))
  const cardWidth = $derived((width - PAD * 2 - GAP * (columns - 1)) / columns)
  const rowHeight = $derived(Math.max(1, Math.round(cardWidth + TEXT + GAP)))
  const rowCount = $derived(Math.ceil(totalCount / columns))
</script>

{#if selected !== null}
  <AlbumDetail albumId={selected} onBack={() => (selected = null)} />
{:else if library.stats.albums === 0}
  <EmptyLibrary />
{:else}
  <div class="flex h-full flex-col">
    <div class="flex items-center justify-between gap-3 px-5 pb-2">
      <div class="text-xs text-zinc-500">
        {#if q !== '' && rows.total !== null}
          {t('{n} matching', { n: rows.total })}
        {/if}
      </div>
      <SearchInput placeholder={t("Search albums, artists, songs…")} />
    </div>
    {#if q !== '' && rows.total === 0}
      <div class="flex flex-1 flex-col items-center justify-center gap-2 text-center">
        <p class="text-sm text-zinc-500">{t('No albums match “{q}”.', { q })}</p>
        <button
          type="button"
          class="rounded-lg px-3 py-1.5 text-xs font-medium text-teal-700 hover:bg-teal-50 dark:text-teal-300 dark:hover:bg-teal-950/50"
          onclick={() => library.setQuery('')}
        >
          {t("Clear search")}
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
              <div class="grid items-start px-5" style:grid-template-columns="repeat({columns}, minmax(0, 1fr))" style:gap="{GAP}px">
                {#each { length: columns } as _, c (c)}
                  {@const idx = r * columns + c}
                  {#if idx < totalCount}
                    {@const a = rows.get(idx)}
                    {#if a}
                      <button class="group min-w-0 text-left" onclick={() => (selected = a.id)}>
                        <Cover
                          cover={a.cover}
                          title={a.title}
                          class="aspect-square w-full rounded-lg text-3xl shadow-sm group-hover:ring-2 group-hover:ring-teal-500"
                        />
                        <p class="mt-2 truncate text-sm font-medium">{a.title}</p>
                        <p class="truncate text-xs text-zinc-500">{a.albumArtist}{a.year ? ` · ${a.year}` : ''}</p>
                      </button>
                    {:else}
                      <div class="min-w-0 animate-pulse" aria-hidden="true">
                        <div class="aspect-square w-full rounded-lg bg-zinc-200 dark:bg-zinc-800"></div>
                        <div class="mt-2 h-3 w-3/4 rounded bg-zinc-200 dark:bg-zinc-800"></div>
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
