<script lang="ts">
  import { count, t } from './i18n.svelte'
  import { tick } from 'svelte'
  import Icon from './Icon.svelte'
  import { playlists, type PlaylistInfo } from './playlists.svelte'
  import { tagEditor } from './tags.svelte'

  // A button that opens a menu of playlists. Give it `trackIds` (library
  // track ids) or an `albumId`. The menu is moved to <body> and positioned
  // with fixed coordinates because it is used inside scrolling, transformed
  // virtual lists that would clip it.
  let {
    trackIds = [],
    albumId = null,
    label = 'Add to playlist',
    text = false,
    editTags = false,
    class: className = '',
  }: {
    trackIds?: number[]
    albumId?: number | null
    label?: string
    /** Show the label next to the icon (header buttons). */
    text?: boolean
    /** Also offer "Edit tags…" for `trackIds`. */
    editTags?: boolean
    class?: string
  } = $props()

  const WIDTH = 240
  const MAX_HEIGHT = 288

  let open = $state(false)
  let creating = $state(false)
  let name = $state('')
  let trigger = $state<HTMLButtonElement | null>(null)
  let menu = $state<HTMLDivElement | null>(null)
  let place = $state({ left: 0, top: 0, flip: false })

  function portal(node: HTMLElement) {
    document.body.appendChild(node)
    return { destroy: () => node.remove() }
  }

  async function show() {
    if (!trigger) return
    const r = trigger.getBoundingClientRect()
    const left = Math.max(8, Math.min(r.right - WIDTH, window.innerWidth - WIDTH - 8))
    const flip = r.bottom + MAX_HEIGHT + 8 > window.innerHeight && r.top > MAX_HEIGHT
    place = { left, top: flip ? window.innerHeight - r.top + 4 : r.bottom + 4, flip }
    creating = false
    name = ''
    open = true
    await tick()
    items()[0]?.focus()
  }

  function close(refocus = false) {
    open = false
    creating = false
    if (refocus) trigger?.focus()
  }

  function items(): HTMLElement[] {
    return menu ? [...menu.querySelectorAll<HTMLElement>('[role="menuitem"]')] : []
  }

  async function chooseExisting(p: PlaylistInfo) {
    close(true)
    if (albumId !== null) await playlists.addAlbum(p, albumId)
    else await playlists.addTracks(p, trackIds)
  }

  function editTagsOfTracks() {
    close()
    tagEditor.open(trackIds, trackIds.length === 1 ? t('Edit tags') : t('Edit tags of {n} tracks', { n: trackIds.length }))
  }

  async function startCreate() {
    creating = true
    await tick()
    menu?.querySelector<HTMLInputElement>('input')?.focus()
  }

  async function create() {
    const trimmed = name.trim()
    if (trimmed === '') return
    const made = await playlists.create(trimmed, albumId === null ? trackIds : [])
    close(true)
    if (!made) return
    if (albumId !== null) await playlists.addAlbum(made, albumId)
    else playlists.flash(t('Added {count} to new playlist “{name}”', { count: count(trackIds.length, '{n} track', '{n} tracks'), name: made.name }))
  }

  function onMenuKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      close(true)
    } else if (e.key === 'Tab') {
      close()
    } else if (!creating && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
      e.preventDefault()
      const list = items()
      const at = list.indexOf(document.activeElement as HTMLElement)
      const next = e.key === 'ArrowDown' ? (at + 1) % list.length : (at - 1 + list.length) % list.length
      list[next]?.focus()
    }
  }

  // Outside click, resize, or scrolling elsewhere closes the menu (the
  // trigger may have moved or been recycled by the virtual list).
  $effect(() => {
    if (!open) return
    const onPointer = (e: PointerEvent) => {
      const t = e.target as Node
      if (!menu?.contains(t) && !trigger?.contains(t)) close()
    }
    const onScroll = (e: Event) => {
      if (!menu?.contains(e.target as Node)) close()
    }
    const onResize = () => close()
    window.addEventListener('pointerdown', onPointer, true)
    window.addEventListener('scroll', onScroll, true)
    window.addEventListener('resize', onResize)
    return () => {
      window.removeEventListener('pointerdown', onPointer, true)
      window.removeEventListener('scroll', onScroll, true)
      window.removeEventListener('resize', onResize)
    }
  })

  // The row that owns this button handles double click / Enter to play;
  // keep those from reaching it.
  const stop = (e: Event) => e.stopPropagation()
</script>

<button
  bind:this={trigger}
  type="button"
  class={[
    text
      ? 'flex items-center gap-2 rounded-lg px-3 py-2 text-sm font-medium text-zinc-700 hover:bg-zinc-200/70 dark:text-zinc-200 dark:hover:bg-zinc-800'
      : 'rounded p-1 text-zinc-400 hover:bg-zinc-200 hover:text-zinc-700 dark:hover:bg-zinc-700 dark:hover:text-zinc-100',
    'outline-none focus-visible:ring-2 focus-visible:ring-teal-500',
    className,
  ]}
  aria-label={text ? undefined : t(label)}
  title={t(label)}
  aria-haspopup="menu"
  aria-expanded={open}
  onclick={(e) => {
    stop(e)
    if (open) close()
    else void show()
  }}
  ondblclick={stop}
  onkeydown={stop}
>
  <Icon name={text ? 'plus' : 'more'} class="size-4" />
  {#if text}{t(label)}{/if}
</button>

{#if open}
  <div
    use:portal
    bind:this={menu}
    role="menu"
    aria-label={t(label)}
    tabindex="-1"
    onkeydown={onMenuKey}
    class="fixed z-50 flex flex-col overflow-hidden rounded-xl border border-zinc-200 bg-white py-1 text-sm text-zinc-800 shadow-lg dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100"
    style:left="{place.left}px"
    style:top={place.flip ? undefined : `${place.top}px`}
    style:bottom={place.flip ? `${place.top}px` : undefined}
    style:width="{WIDTH}px"
    style:max-height="{MAX_HEIGHT}px"
  >
    {#if creating}
      <form
        class="flex flex-col gap-2 p-2"
        onsubmit={(e) => {
          e.preventDefault()
          void create()
        }}
      >
        <input
          class="rounded-md border border-zinc-200 bg-white px-2 py-1 outline-none focus-visible:ring-2 focus-visible:ring-teal-500 dark:border-zinc-700 dark:bg-zinc-950"
          placeholder={t("Playlist name")}
          aria-label={t("New playlist name")}
          bind:value={name}
        />
        <div class="flex justify-end gap-2">
          <button type="button" class="rounded-md px-2 py-1 text-zinc-500 hover:bg-zinc-100 dark:hover:bg-zinc-800" onclick={() => close(true)}>
            {t("Cancel")}
          </button>
          <button
            type="submit"
            class="rounded-md bg-teal-600 px-2 py-1 font-medium text-white hover:bg-teal-500 disabled:opacity-50"
            disabled={name.trim() === ''}
          >
            {t("Create")}
          </button>
        </div>
      </form>
    {:else}
      <p class="px-3 pt-1 pb-0.5 text-xs font-medium uppercase tracking-wide text-zinc-400" aria-hidden="true">
        {t("Add to playlist")}
      </p>
      <div class="min-h-0 overflow-y-auto">
        {#each playlists.list as p (p.id)}
          <button
            type="button"
            role="menuitem"
            class="flex w-full items-center justify-between gap-2 px-3 py-1.5 text-left outline-none hover:bg-zinc-100 focus-visible:bg-zinc-100 dark:hover:bg-zinc-800 dark:focus-visible:bg-zinc-800"
            onclick={() => chooseExisting(p)}
          >
            <span class="truncate">{p.name}</span>
            <span class="text-xs tabular-nums text-zinc-400">{p.trackCount.toLocaleString()}</span>
          </button>
        {:else}
          <p class="px-3 py-1.5 text-xs text-zinc-500">{t("No playlists yet.")}</p>
        {/each}
      </div>
      <button
        type="button"
        role="menuitem"
        class="flex w-full items-center gap-2 border-t border-zinc-100 px-3 py-1.5 text-left text-teal-700 outline-none hover:bg-zinc-100 focus-visible:bg-zinc-100 dark:border-zinc-800 dark:text-teal-300 dark:hover:bg-zinc-800 dark:focus-visible:bg-zinc-800"
        onclick={startCreate}
      >
        <Icon name="plus" class="size-4" /> {t("New playlist…")}
      </button>
      {#if editTags && trackIds.length > 0}
        <button
          type="button"
          role="menuitem"
          class="flex w-full items-center gap-2 border-t border-zinc-100 px-3 py-1.5 text-left outline-none hover:bg-zinc-100 focus-visible:bg-zinc-100 dark:border-zinc-800 dark:hover:bg-zinc-800 dark:focus-visible:bg-zinc-800"
          onclick={editTagsOfTracks}
        >
          {t("Edit tags…")}
        </button>
      {/if}
    {/if}
  </div>
{/if}
