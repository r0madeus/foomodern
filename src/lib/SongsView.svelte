<script lang="ts">
  import VirtualList from './VirtualList.svelte'
  import EmptyLibrary from './EmptyLibrary.svelte'
  import SearchInput from './SearchInput.svelte'
  import AddToPlaylist from './AddToPlaylist.svelte'
  import ExtBadge from './ExtBadge.svelte'
  import { library, PagedRows, type Sort } from './library.svelte'
  import { player, formatTime } from './player.svelte'

  const ROW = 40
  const SORTS: { value: Sort; label: string }[] = [
    { value: 'artist', label: 'Artist' },
    { value: 'title', label: 'Title' },
    { value: 'album', label: 'Album' },
    { value: 'added', label: 'Recently added' },
  ]
  const COLS = 'grid grid-cols-[3rem_minmax(0,2fr)_minmax(0,1.5fr)_minmax(0,1.5fr)_4rem_2rem] items-center gap-3 px-5'

  const q = $derived(library.debouncedQuery)
  // A fresh cache whenever the library version, sort, or search query changes.
  const rows = $derived.by(() => {
    void library.stats.version
    return new PagedRows(library.tracksPage(library.sort, q))
  })
  $effect(() => {
    // Capture this instance: the cleanup runs after `rows` has already been
    // re-derived, so reading `rows` there would dispose the NEW cache and drop
    // its pages (the list then stayed on skeleton rows after startup).
    const current = rows
    current.ensureRange(0, 1)
    return () => current.dispose()
  })

  const totalCount = $derived(rows.total ?? (q === '' ? library.stats.tracks : 0))
  const currentPath = $derived(player.snapshot.current?.path ?? null)
</script>

{#if library.stats.tracks === 0}
  <EmptyLibrary />
{:else}
  <div class="flex h-full flex-col">
    <div class="flex items-center justify-between gap-3 px-5 pb-2">
      <div class="flex items-center gap-3">
        <label class="flex items-center gap-2 text-sm text-zinc-500">
          Sort by
          <select
            class="rounded-md border border-zinc-200 bg-white px-2 py-1 text-zinc-800 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100"
            bind:value={library.sort}
          >
            {#each SORTS as s (s.value)}
              <option value={s.value}>{s.label}</option>
            {/each}
          </select>
        </label>
        {#if q !== '' && rows.total !== null}
          <span class="text-xs text-zinc-500">{rows.total} matching</span>
        {/if}
      </div>
      <SearchInput placeholder="Search songs, artists, albums…" />
    </div>
    <div class={[COLS, 'border-b border-zinc-200 py-1.5 text-xs font-medium uppercase tracking-wide text-zinc-400 dark:border-zinc-800']}>
      <span class="text-right">#</span><span>Title</span><span>Artist</span><span>Album</span><span class="text-right">Time</span><span></span>
    </div>
    {#if q !== '' && rows.total === 0}
      <div class="flex flex-1 flex-col items-center justify-center gap-2 text-center">
        <p class="text-sm text-zinc-500">No songs match “{q}”.</p>
        <button
          type="button"
          class="rounded-lg px-3 py-1.5 text-xs font-medium text-teal-700 hover:bg-teal-50 dark:text-teal-300 dark:hover:bg-teal-950/50"
          onclick={() => library.setQuery('')}
        >
          Clear search
        </button>
      </div>
    {:else}
      {#key `${library.sort}\0${q}`}
        <VirtualList
          class="min-h-0 flex-1"
          count={totalCount}
          rowHeight={ROW}
          onRangeChange={(s, e) => rows.ensureRange(s, e)}
        >
        {#snippet row(i)}
          {@const t = rows.get(i)}
          {#if t}
            <div
              role="button"
              tabindex="0"
              title={t.path}
              class={[
                COLS,
                'group h-full cursor-default text-sm outline-none focus-visible:ring-2 focus-visible:ring-teal-500 focus-visible:ring-inset',
                t.path === currentPath
                  ? 'bg-teal-50 text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
                  : 'text-zinc-700 hover:bg-zinc-100/70 dark:text-zinc-300 dark:hover:bg-zinc-800/50',
              ]}
              ondblclick={() => library.playAll(i)}
              onkeydown={(e) => e.key === 'Enter' && library.playAll(i)}
            >
              <span class="text-right text-xs tabular-nums text-zinc-400">
                {#if t.path === currentPath && player.snapshot.status === 'playing'}
                  <span class="text-teal-600 dark:text-teal-400">▶</span>
                {:else}{i + 1}{/if}
              </span>
              <span class="flex min-w-0 items-center">
                <span class="truncate font-medium">{t.title}</span><ExtBadge path={t.path} />
              </span>
              <span class="truncate text-zinc-500">{t.artist}</span>
              <span class="truncate text-zinc-500">{t.album}</span>
              <span class="text-right text-xs tabular-nums text-zinc-500">
                {formatTime(t.durationMs === null ? null : t.durationMs / 1000)}
              </span>
              <span class="flex justify-end opacity-0 group-focus-within:opacity-100 group-hover:opacity-100">
                <AddToPlaylist trackIds={[t.id]} editTags label="More actions for “{t.title}”" />
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
    {/key}
    {/if}
  </div>
{/if}
