<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { count, t } from './i18n.svelte'
  import { tick } from 'svelte'
  import VirtualList from './VirtualList.svelte'
  import ExtBadge from './ExtBadge.svelte'
  import Cover from './Cover.svelte'
  import PlaylistCover from './PlaylistCover.svelte'
  import ArtistLinks from './ArtistLinks.svelte'
  import PlaylistCoverModal from './PlaylistCoverModal.svelte'
  import AddToPlaylist from './AddToPlaylist.svelte'
  import FavoriteButton from './FavoriteButton.svelte'
  import Icon from './Icon.svelte'
  import type { TrackRow } from './library.svelte'
  import type { Rgb } from './nowCover.svelte'
  import { pinned } from './pinned.svelte'
  import { playlists } from './playlists.svelte'
  import { player, formatTime } from './player.svelte'
  import {
    fallbackPalette,
    heroImageForPlaylist,
    parseCoverPreset,
  } from './playlistCover'
  import { tintFrom } from './tint'

  let { id, onDeleted }: { id: number; onDeleted: () => void } = $props()

  const ROW = 48
  const COLS = 'grid grid-cols-[3rem_minmax(0,2fr)_minmax(0,1.5fr)_minmax(0,1.5fr)_4rem_11rem] items-center gap-3 px-5'

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
  let editingCover = $state(false)
  let nameInput = $state<HTMLInputElement | null>(null)
  let heroColor = $state<Rgb | null>(null)

  const heroImage = $derived(info ? heroImageForPlaylist(info.cover, info.albumCovers) : null)

  $effect(() => {
    const target = heroImage
    if (!target) {
      heroColor = null
      return
    }
    let stale = false
    invoke<Rgb | null>('library_cover_color', { cover: target })
      .then((c) => {
        if (!stale) heroColor = c
      })
      .catch(() => {
        if (!stale) heroColor = null
      })
    return () => {
      stale = true
    }
  })

  const heroBg = $derived.by(() => {
    if (!info) return ''
    if (heroImage) {
      const tint = tintFrom(heroColor)
      return `linear-gradient(135deg, ${tint.top} 0%, ${tint.surface} 100%)`
    }
    const preset = parseCoverPreset(info.cover)?.palette ?? fallbackPalette(info.name)
    return `linear-gradient(135deg, ${preset.heroTop} 0%, ${preset.heroSurface} 100%)`
  })

  $effect(() => {
    void id
    renaming = false
    confirmingDelete = false
    editingCover = false
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
  <div class="flex h-full items-center justify-center text-sm text-zinc-500">{t("Playlist not found.")}</div>
{:else}
  <div class="flex h-full flex-col">
    <div
      class="mx-5 mb-5 flex flex-wrap items-end gap-6 rounded-2xl p-6 text-white"
      style:background={heroBg}
    >
      <button
        type="button"
        aria-label={t('Change cover')}
        class="group relative shrink-0 overflow-hidden rounded-xl shadow-2xl shadow-black/40 outline-none focus-visible:ring-2 focus-visible:ring-white"
        onclick={() => (editingCover = true)}
      >
        <PlaylistCover
          cover={info.cover}
          albumCovers={info.albumCovers}
          name={info.name}
          class="size-40 rounded-xl sm:size-44"
          iconClass="size-16"
        />
        <span class="pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-1.5 bg-black/55 text-xs font-medium text-white opacity-0 transition group-hover:opacity-100 group-focus-visible:opacity-100">
          <Icon name="upload" class="size-6" />
          {t('Change cover')}
        </span>
      </button>

      <div class="min-w-0 flex-1">
        <p class="text-xs font-semibold tracking-widest text-white/70 uppercase">{t('Playlist')}</p>
        {#if renaming}
          <form
            class="mt-1"
            onsubmit={(e) => {
              e.preventDefault()
              void commitRename()
            }}
          >
            <input
              bind:this={nameInput}
              bind:value={draft}
              aria-label={t('Playlist name')}
              class="w-80 max-w-full rounded-lg border border-white/25 bg-black/35 px-3 py-1.5 text-2xl font-bold tracking-tight text-white outline-none focus-visible:ring-2 focus-visible:ring-teal-400"
              onkeydown={(e) => e.key === 'Escape' && (renaming = false)}
              onblur={() => renaming && void commitRename()}
            />
          </form>
        {:else}
          <h2 class="mt-1 truncate text-4xl font-bold tracking-tight">{info.name}</h2>
        {/if}
        <p class="mt-2 text-sm text-white/70">
          {count(info.trackCount, '{n} track', '{n} tracks')}
        </p>

        <div class="mt-4 flex flex-wrap items-center gap-2">
          {#if confirmingDelete}
            <span class="text-sm text-white/90" role="alert">{t('Delete this playlist?')}</span>
            <button
              type="button"
              class="rounded-full bg-red-600 px-3.5 py-1.5 text-sm font-medium text-white hover:bg-red-500"
              onclick={remove}
            >
              {t('Delete')}
            </button>
            <button
              type="button"
              class="rounded-full bg-white/10 px-3.5 py-1.5 text-sm text-white hover:bg-white/20"
              onclick={() => (confirmingDelete = false)}
            >
              {t('Cancel')}
            </button>
          {:else}
            <button
              type="button"
              disabled={info.trackCount === 0}
              class="flex items-center gap-2 rounded-full bg-teal-500 py-2 pr-5 pl-4 text-sm font-semibold text-zinc-950 transition hover:scale-[1.03] hover:bg-teal-400 disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:scale-100"
              onclick={() => playlists.play(id, 0)}
            >
              <Icon name="play" class="size-5" /> {t('Play')}
            </button>
            <button
              type="button"
              aria-pressed={pinned.isPinned('playlist', id)}
              class={[
                'flex items-center gap-1.5 rounded-full px-3.5 py-1.5 text-sm transition',
                pinned.isPinned('playlist', id)
                  ? 'bg-teal-400/20 font-medium text-teal-200 hover:bg-teal-400/30'
                  : 'bg-white/10 text-white/90 hover:bg-white/20',
              ]}
              onclick={() => void pinned.toggle('playlist', id)}
            >
              <Icon name="pin" class="size-4" />
              {pinned.isPinned('playlist', id) ? t('Pinned') : t('Pin')}
            </button>
            <button
              type="button"
              class="rounded-full bg-white/10 px-3.5 py-1.5 text-sm text-white/90 transition hover:bg-white/20"
              onclick={() => (editingCover = true)}
            >
              {t('Cover')}
            </button>
            <button
              type="button"
              class="rounded-full bg-white/10 px-3.5 py-1.5 text-sm text-white/90 transition hover:bg-white/20"
              onclick={startRename}
            >
              {t('Rename')}
            </button>
            <button
              type="button"
              class="rounded-full bg-white/10 px-3.5 py-1.5 text-sm text-white/90 transition hover:bg-white/20"
              title={t('Save as an M3U file, e.g. to open on another device')}
              onclick={() => void playlists.exportM3u(info)}
            >
              {t('Export…')}
            </button>
            <button
              type="button"
              class="rounded-full bg-red-500/20 px-3.5 py-1.5 text-sm text-red-200 transition hover:bg-red-500/30"
              onclick={() => (confirmingDelete = true)}
            >
              {t('Delete')}
            </button>
          {/if}
        </div>
      </div>
    </div>

    <div class={[COLS, 'border-b border-zinc-200 py-1.5 text-xs font-medium uppercase tracking-wide text-zinc-400 dark:border-zinc-800']}>
      <span class="text-right">#</span><span>{t("Title")}</span><span>{t("Artist")}</span><span>{t("Album")}</span><span class="text-right">{t("Time")}</span><span></span>
    </div>

    {#if info.trackCount === 0}
      <div class="flex flex-1 items-center justify-center px-8 text-center text-sm text-zinc-500">
        {t("This playlist is empty. Use the “…” button on a song or album to add tracks.")}
      </div>
    {:else}
      <VirtualList class="min-h-0 flex-1" count={visible.length} rowHeight={ROW}>
        {#snippet row(i)}
          {@const track = visible[i]}
          {#if track}
            <div
              role="button"
              tabindex="0"
              data-pl-row={i}
              title={t('{path}\nDouble-click or Enter to play. Alt+Up/Down moves, Delete removes.', { path: track.path })}
              class={[
                COLS,
                'group h-full cursor-default text-sm outline-none focus-visible:ring-2 focus-visible:ring-teal-500 focus-visible:ring-inset',
                track.path === currentPath
                  ? 'bg-teal-50 text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
                  : 'text-zinc-700 hover:bg-zinc-100/70 dark:text-zinc-300 dark:hover:bg-zinc-800/50',
              ]}
              ondblclick={() => playlists.play(id, i)}
              onkeydown={(e) => onRowKey(e, i)}
            >
              <span class="text-right text-xs tabular-nums text-zinc-400">
                {#if track.path === currentPath && player.snapshot.status === 'playing'}
                  <span class="text-teal-600 dark:text-teal-400">▶</span>
                {:else}{i + 1}{/if}
              </span>
              <span class="flex min-w-0 items-center gap-3">
                <Cover cover={track.cover} title={track.title} class="size-9 shrink-0 rounded text-[10px]" />
                <span class="truncate font-medium">{track.title}</span><ExtBadge path={track.path} />
              </span>
              <span class="truncate text-zinc-500"><ArtistLinks artist={track.artist} /></span>
              <span class="truncate text-zinc-500">{track.album}</span>
              <span class="text-right text-xs tabular-nums text-zinc-500">
                {formatTime(track.durationMs === null ? null : track.durationMs / 1000)}
              </span>
              <span class="flex items-center justify-end gap-0.5">
                <FavoriteButton trackId={track.id} title={track.title} revealOnHover />
                <span class="flex justify-end opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 pointer-coarse:opacity-100">
                  <AddToPlaylist trackIds={[track.id]} editTags label={t('More actions for “{title}”', { title: track.title })} />
                <button
                  type="button"
                  data-action="up"
                  class={btn}
                  disabled={i === 0}
                  aria-label={t('Move “{title}” up', { title: track.title })}
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
                  aria-label={t('Move “{title}” down', { title: track.title })}
                  onclick={() => moveRow(i, i + 1, 'down')}
                  ondblclick={(e) => e.stopPropagation()}
                  onkeydown={(e) => e.stopPropagation()}
                >
                  <Icon name="down" class="size-4" />
                </button>
                <button
                  type="button"
                  class={btn}
                  aria-label={t('Remove “{title}” from playlist', { title: track.title })}
                  onclick={() => playlists.removeAt(id, [i])}
                  ondblclick={(e) => e.stopPropagation()}
                  onkeydown={(e) => e.stopPropagation()}
                >
                  <Icon name="close" class="size-4" />
                </button>
                </span>
              </span>
            </div>
          {/if}
        {/snippet}
      </VirtualList>
    {/if}
  </div>

  {#if editingCover}
    <PlaylistCoverModal playlist={info} onClose={() => (editingCover = false)} />
  {/if}
{/if}
