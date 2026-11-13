<script lang="ts">
  import VirtualList from './VirtualList.svelte'
  import EmptyLibrary from './EmptyLibrary.svelte'
  import AlbumDetail from './AlbumDetail.svelte'
  import Cover from './Cover.svelte'
  import { library, PagedRows } from './library.svelte'

  const MIN_CARD = 180
  const GAP = 16
  const PAD = 20
  const TEXT = 52

  let selected = $state<number | null>(null)
  let width = $state(0)

  const rows = $derived.by(() => {
    void library.stats.version
    return new PagedRows(library.albumsPage)
  })
  $effect(() => () => rows.dispose())

  const columns = $derived(Math.max(1, Math.floor((width - PAD * 2 + GAP) / (MIN_CARD + GAP))))
  const cardWidth = $derived((width - PAD * 2 - GAP * (columns - 1)) / columns)
  const rowHeight = $derived(Math.max(1, Math.round(cardWidth + TEXT + GAP)))
  const rowCount = $derived(Math.ceil(library.stats.albums / columns))
</script>

{#if selected !== null}
  <AlbumDetail albumId={selected} onBack={() => (selected = null)} />
{:else if library.stats.albums === 0}
  <EmptyLibrary />
{:else}
  <div class="h-full" bind:clientWidth={width}>
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
            {#if idx < library.stats.albums}
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
  </div>
{/if}
