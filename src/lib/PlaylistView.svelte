<script lang="ts">
  import { tick } from 'svelte'
  import VirtualList from './VirtualList.svelte'
  import ExtBadge from './ExtBadge.svelte'
  import Cover from './Cover.svelte'
  import AddToPlaylist from './AddToPlaylist.svelte'
  import Icon from './Icon.svelte'
  import type { TrackRow } from './library.svelte'
  import { playlists } from './playlists.svelte'
  import { player, formatTime } from './player.svelte'

  let { id, onDeleted }: { id: number; onDeleted: () => void } = $props()

  const ROW = 48
  const COLS = 'grid grid-cols-[3rem_minmax(0,2fr)_minmax(0,1.5fr)_minmax(0,1.5fr)_4rem_9rem] items-center gap-3 px-5'

  const info = $derived(playlists.byId(id))
  let tracks = $state<TrackRow[]>([])
  let loadedId = $state<number | null>(null)

  // Reload whenever the playlist or the library changes. The previous rows
  // stay on screen until the new ones arrive, so edits do not flicker.
  $effect(() => {
    void playlists.version
    const wanted = id
    const count = playlists.byId(wanted)?.trackCount ?? 0
    let stale = false
    playlists
      .tracks(wanted, count)
      .then((rows) => {
        if (stale) return
        tracks = rows
        loadedId = wanted
      })
      .catch((e) => (player.error = String(e)))
    return () => (stale = true)
  })

  const visible = $derived(loadedId === id ? tracks : [])
  const currentPath = $derived(player.snapshot.current?.path ?? null)

  let renaming = $state(false)
  let draft = $state('')
  let confirmingDelete = $state(false)
  let nameInput = $state<HTMLInputElement | null>(null)

  $effect(() => {
    void id
    renaming = false
    confirmingDelete = false
  })

  async function startRename() {
    draft = info?.name ?? ''
    renaming = true
    await tick()
    nameInput?.select()
  }

  async function commitRename() {
    const next = draft.trim()
    renaming = false
    if (next === '' || next === info?.name) return
    await playlists.rename(id, next)
  }

  async function remove() {
    confirmingDelete = false
    if (await playlists.delete(id)) onDeleted()
  }

  async function moveRow(from: number, to: number, action: 'up' | 'down') {
    if (to < 0 || to >= visible.length) return
    if (!(await playlists.move(id, from, to))) return
    // Keep keyboard focus on the same entry, which now sits at `to`.
    await tick()
    document
      .querySelector<HTMLElement>(`[data-pl-row="${to}"] [data-action="${action}"]`)
      ?.focus()
  }

  function onRowKey(e: KeyboardEvent, i: number) {
    if (e.target !== e.currentTarget) return
    if (e.key === 'Enter') void playlists.play(id, i)
    else if (e.altKey && e.key === 'ArrowUp') {
      e.preventDefault()
      void moveRow(i, i - 1, 'up')
    } else if (e.altKey && e.key === 'ArrowDown') {
      e.preventDefault()
      void moveRow(i, i + 1, 'down')
    } else if (e.key === 'Delete') void playlists.removeAt(id, [i])
  }

  const btn =
    'rounded p-1 text-zinc-400 outline-none hover:bg-zinc-200 hover:text-zinc-700 focus-visible:ring-2 focus-visible:ring-teal-500 disabled:pointer-events-none disabled:opacity-30 dark:hover:bg-zinc-700 dark:hover:text-zinc-100'
</script>

{#if !info}
  <div class="flex h-full items-center justify-center text-sm text-zinc-500">Playlist not found.</div>
{:else}
  <div class="flex h-full flex-col">
    <div class="flex flex-wrap items-center justify-between gap-3 px-5 pb-3">
      <div class="min-w-0">
        {#if renaming}
          <form
            onsubmit={(e) => {
              e.preventDefault()
              void commitRename()
            }}
          >
            <input
              bind:this={nameInput}
              bind:value={draft}
              aria-label="Playlist name"
              class="w-72 max-w-full rounded-md border border-zinc-200 bg-white px-2 py-1 text-lg font-semibold tracking-tight outline-none focus-visible:ring-2 focus-visible:ring-teal-500 dark:border-zinc-700 dark:bg-zinc-900"
              onkeydown={(e) => e.key === 'Escape' && (renaming = false)}
              onblur={() => renaming && void commitRename()}
            />
          </form>
        {:else}
          <h2 class="truncate text-lg font-semibold tracking-tight">{info.name}</h2>
        {/if}
        <p class="text-xs text-zinc-500">
          {info.trackCount.toLocaleString()}
          {info.trackCount === 1 ? 'track' : 'tracks'}
        </p>
      </div>
      <div class="flex items-center gap-2">
        {#if confirmingDelete}
          <span class="text-sm text-zinc-600 dark:text-zinc-300" role="alert">Delete this playlist?</span>
          <button
            class="rounded-lg bg-red-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-red-500"
            onclick={remove}
          >
            Delete
          </button>
          <button
            class="rounded-lg px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800"
            onclick={() => (confirmingDelete = false)}
          >
            Cancel
          </button>
        {:else}
          <button
            class="rounded-lg px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800"
            onclick={startRename}
          >
            Rename
          </button>
          <button
            class="rounded-lg px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800"
            title="Save as an M3U file, e.g. to open on another device"
            onclick={() => void playlists.exportM3u(info)}
          >
            Export…
          </button>
          <button
            class="rounded-lg px-3 py-1.5 text-sm text-red-600 hover:bg-red-50 dark:text-red-400 dark:hover:bg-red-950/40"
            onclick={() => (confirmingDelete = true)}
          >
            Delete
          </button>
          <button
            class="flex items-center gap-2 rounded-lg bg-teal-600 px-4 py-1.5 text-sm font-medium text-white hover:bg-teal-500 disabled:opacity-50"
            disabled={info.trackCount === 0}
            onclick={() => playlists.play(id, 0)}
          >
            <Icon name="play" class="size-4" /> Play
          </button>
        {/if}
      </div>
    </div>

    <div class={[COLS, 'border-b border-zinc-200 py-1.5 text-xs font-medium uppercase tracking-wide text-zinc-400 dark:border-zinc-800']}>
      <span class="text-right">#</span><span>Title</span><span>Artist</span><span>Album</span><span class="text-right">Time</span><span></span>
    </div>

    {#if info.trackCount === 0}
      <div class="flex flex-1 items-center justify-center px-8 text-center text-sm text-zinc-500">
        This playlist is empty. Use the “…” button on a song or album to add tracks.
      </div>
    {:else}
      <VirtualList class="min-h-0 flex-1" count={visible.length} rowHeight={ROW}>
        {#snippet row(i)}
          {@const t = visible[i]}
          {#if t}
            <div
              role="button"
              tabindex="0"
              data-pl-row={i}
              title={`${t.path}\nDouble-click or Enter to play. Alt+Up/Down moves, Delete removes.`}
              class={[
                COLS,
                'group h-full cursor-default text-sm outline-none focus-visible:ring-2 focus-visible:ring-teal-500 focus-visible:ring-inset',
                t.path === currentPath
                  ? 'bg-teal-50 text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
                  : 'text-zinc-700 hover:bg-zinc-100/70 dark:text-zinc-300 dark:hover:bg-zinc-800/50',
              ]}
              ondblclick={() => playlists.play(id, i)}
              onkeydown={(e) => onRowKey(e, i)}
            >
              <span class="text-right text-xs tabular-nums text-zinc-400">
                {#if t.path === currentPath && player.snapshot.status === 'playing'}
                  <span class="text-teal-600 dark:text-teal-400">▶</span>
                {:else}{i + 1}{/if}
              </span>
              <span class="flex min-w-0 items-center gap-3">
                <Cover cover={t.cover} title={t.title} class="size-9 shrink-0 rounded text-[10px]" />
                <span class="truncate font-medium">{t.title}</span><ExtBadge path={t.path} />
              </span>
              <span class="truncate text-zinc-500">{t.artist}</span>
              <span class="truncate text-zinc-500">{t.album}</span>
              <span class="text-right text-xs tabular-nums text-zinc-500">
                {formatTime(t.durationMs === null ? null : t.durationMs / 1000)}
              </span>
              <span class="flex justify-end opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 pointer-coarse:opacity-100">
                <AddToPlaylist trackIds={[t.id]} editTags label="More actions for “{t.title}”" />
                <button
                  type="button"
                  data-action="up"
                  class={btn}
                  disabled={i === 0}
                  aria-label={`Move “${t.title}” up`}
                  onclick={() => moveRow(i, i - 1, 'up')}
                  ondblclick={(e) => e.stopPropagation()}
                  onkeydown={(e) => e.stopPropagation()}
                >
                  <Icon name="up" class="size-4" />
                </button>
                <button
                  type="button"
                  data-action="down"
                  class={btn}
                  disabled={i === visible.length - 1}
                  aria-label={`Move “${t.title}” down`}
                  onclick={() => moveRow(i, i + 1, 'down')}
                  ondblclick={(e) => e.stopPropagation()}
                  onkeydown={(e) => e.stopPropagation()}
                >
                  <Icon name="down" class="size-4" />
                </button>
                <button
                  type="button"
                  class={btn}
                  aria-label={`Remove “${t.title}” from playlist`}
                  onclick={() => playlists.removeAt(id, [i])}
                  ondblclick={(e) => e.stopPropagation()}
                  onkeydown={(e) => e.stopPropagation()}
                >
                  <Icon name="close" class="size-4" />
                </button>
              </span>
            </div>
          {/if}
        {/snippet}
      </VirtualList>
    {/if}
  </div>
{/if}
