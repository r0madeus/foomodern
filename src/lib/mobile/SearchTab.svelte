<script lang="ts">
  import { t } from '../i18n.svelte'
  import { artists } from '../artists.svelte'
  import VirtualList from '../VirtualList.svelte'
  import Cover from '../Cover.svelte'
  import Icon from '../Icon.svelte'
  import PageHeader from './PageHeader.svelte'
  import TrackItem from './TrackItem.svelte'
  import { library, PagedRows } from '../library.svelte'

  let { onOpenAlbum }: { onOpenAlbum: (albumId: number) => void } = $props()

  const ROW = 64
  const q = $derived(library.debouncedQuery)

  const songs = $derived.by(() => {
    void library.stats.version
    return new PagedRows(library.tracksPage(library.sort, q))
  })
  const albums = $derived.by(() => {
    void library.stats.version
    return new PagedRows(library.albumsPage(q))
  })
  const matchingArtists = $derived.by(() => {
    void library.stats.version
    return new PagedRows(artists.page(q))
  })
  $effect(() => {
    const s = songs
    const a = albums
    const ar = matchingArtists
    if (q !== '') {
      s.ensureRange(0, 1)
      a.ensureRange(0, 12)
      ar.ensureRange(0, 12)
    }
    return () => {
      s.dispose()
      a.dispose()
      ar.dispose()
    }
  })

  const albumList = $derived(
    Array.from({ length: Math.min(12, albums.total ?? 0) }, (_, i) => albums.get(i)).filter((a) => a !== undefined),
  )
  const artistList = $derived(
    Array.from({ length: Math.min(12, matchingArtists.total ?? 0) }, (_, i) => matchingArtists.get(i)).filter((a) => a !== undefined),
  )
</script>

<div class="flex h-full flex-col">
  <PageHeader title={t("Search")} />
  <div class="shrink-0 px-4 pb-3">
    <label class="flex items-center gap-3 rounded-lg bg-zinc-100 px-3 py-2.5 text-zinc-900">
      <Icon name="search" class="size-5 shrink-0 text-zinc-600" />
      <input
        class="min-w-0 flex-1 bg-transparent text-[15px] outline-none placeholder:text-zinc-500"
        placeholder={t("Songs, artists or albums")}
        value={library.query}
        oninput={(e) => library.setQuery(e.currentTarget.value)}
        enterkeyhint="search"
        autocomplete="off"
      />
      {#if library.query !== ''}
        <button class="-m-1 p-1 text-zinc-600" aria-label={t("Clear search")} onclick={() => library.setQuery('')}>
          <Icon name="close" class="size-5" />
        </button>
      {/if}
    </label>
  </div>

  {#if q === ''}
    <div class="flex flex-1 flex-col items-center justify-center gap-2 px-10 text-center">
      <p class="text-lg font-semibold">{t("Search your library")}</p>
      <p class="text-sm text-zinc-400">{t("Find songs, artists and albums by name.")}</p>
    </div>
  {:else if songs.total === 0 && albums.total === 0 && matchingArtists.total === 0}
    <div class="flex flex-1 flex-col items-center justify-center gap-2 px-10 text-center">
      <p class="text-lg font-semibold">{t('No results for “{q}”', { q })}</p>
      <p class="text-sm text-zinc-400">{t("Check the spelling or try fewer words.")}</p>
    </div>
  {:else}
    {#if artistList.length > 0}
      <h2 class="shrink-0 px-4 pb-2 text-lg font-bold">{t("Artists")}</h2>
      <div class="flex shrink-0 gap-3 overflow-x-auto px-4 pb-4">
        {#each artistList as ar (ar.name)}
          <button class="w-24 shrink-0 text-center active:opacity-70" onclick={() => artists.open(ar.name)}>
            <Cover cover={ar.cover} title={ar.name} class="aspect-square w-full rounded-full text-2xl" />
            <span class="mt-1.5 block truncate text-[13px] font-medium">{ar.name}</span>
          </button>
        {/each}
      </div>
    {/if}
    {#if albumList.length > 0}
      <h2 class="shrink-0 px-4 pb-2 text-lg font-bold">{t("Albums")}</h2>
      <div class="flex shrink-0 gap-3 overflow-x-auto px-4 pb-4">
        {#each albumList as a (a.id)}
          <button class="w-28 shrink-0 text-left active:opacity-70" onclick={() => onOpenAlbum(a.id)}>
            <Cover cover={a.cover} title={a.title} class="aspect-square w-full rounded-md text-2xl" />
            <span class="mt-1.5 block truncate text-[13px] font-medium">{a.title}</span>
            <span class="block truncate text-xs text-zinc-400">{a.albumArtist}</span>
          </button>
        {/each}
      </div>
    {/if}
    {#if (songs.total ?? 0) > 0}
      <h2 class="shrink-0 px-4 pb-1 text-lg font-bold">{t("Songs")}</h2>
      <VirtualList class="min-h-0 flex-1 pb-[var(--mobile-bar-clear,5rem)]" count={songs.total ?? 0} rowHeight={ROW} onRangeChange={(s, e) => songs.ensureRange(s, e)}>
        {#snippet row(i)}
          {@const track = songs.get(i)}
          {#if track}
            <TrackItem
              title={track.title}
              subtitle={[track.artist, track.album].filter(Boolean).join(' · ')}
              path={track.path}
              cover={track.cover}
              trackId={track.id}
              onPlay={() => library.playAll(i)}
            />
          {/if}
        {/snippet}
      </VirtualList>
    {/if}
  {/if}
</div>
