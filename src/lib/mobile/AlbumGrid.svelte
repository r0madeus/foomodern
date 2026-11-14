<script lang="ts">
  import VirtualList from '../VirtualList.svelte'
  import Cover from '../Cover.svelte'
  import { library, PagedRows } from '../library.svelte'

  // Two-column album grid (three on wider phones), virtualised by rows.
  let { query = '', onOpen }: { query?: string; onOpen: (albumId: number) => void } = $props()

  const GAP = 16
  const PAD = 16
  const TEXT = 48

  let width = $state(0)
  const rows = $derived.by(() => {
    void library.stats.version
    return new PagedRows(library.albumsPage(query))
  })
  $effect(() => {
    const current = rows
    current.ensureRange(0, 1)
    return () => current.dispose()
  })

  const total = $derived(rows.total ?? (query === '' ? library.stats.albums : 0))
  const columns = $derived(width >= 560 ? 3 : 2)
  const card = $derived(Math.max(1, (width - PAD * 2 - GAP * (columns - 1)) / columns))
  const rowHeight = $derived(Math.round(card + TEXT + GAP))
</script>

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
        <div class="grid items-start" style:padding="0 {PAD}px" style:grid-template-columns="repeat({columns}, minmax(0, 1fr))" style:gap="{GAP}px">
          {#each { length: columns } as _, c (c)}
            {@const idx = r * columns + c}
            {@const a = idx < total ? rows.get(idx) : undefined}
            {#if a}
              <button class="min-w-0 text-left active:opacity-70" onclick={() => onOpen(a.id)}>
                <Cover cover={a.cover} title={a.title} class="aspect-square w-full rounded-md text-3xl shadow-lg shadow-black/40" />
                <span class="mt-2 block truncate text-sm font-semibold text-zinc-50">{a.title}</span>
                <span class="block truncate text-xs text-zinc-400">{a.albumArtist}{a.year ? ` · ${a.year}` : ''}</span>
              </button>
            {:else if idx < total}
              <div class="aspect-square w-full animate-pulse rounded-md bg-zinc-800"></div>
            {/if}
          {/each}
        </div>
      {/snippet}
    </VirtualList>
  {/if}
</div>
