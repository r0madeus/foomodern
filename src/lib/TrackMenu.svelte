<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { tick } from 'svelte'
  import Icon from './Icon.svelte'
  import { appSettings } from './appSettings.svelte'
  import { artists, splitArtistNames } from './artists.svelte'
  import { favorites } from './favorites.svelte'
  import { count, t } from './i18n.svelte'
  import { library } from './library.svelte'
  import { player } from './player.svelte'
  import { playlists, type PlaylistInfo } from './playlists.svelte'
  import { tagEditor } from './tags.svelte'
  import { trackMenu } from './contextMenu.svelte'

  const WIDTH = 228
  const SUB_WIDTH = 220

  let menu = $state<HTMLDivElement | null>(null)
  let submenuEl = $state<HTMLDivElement | null>(null)
  let sub = $state<'playlists' | 'artists' | null>(null)
  let creating = $state(false)
  let playlistName = $state('')
  let pos = $state({ left: 0, top: 0 })
  let subPos = $state({ left: 0, top: 0 })

  const ctx = $derived(trackMenu.current)
  const artistList = $derived(ctx ? splitArtistNames(ctx.track.artist) : [])
  const hasLibId = $derived(ctx ? ctx.track.id > 0 : false)
  const hasAlbum = $derived(ctx ? ctx.track.albumId !== null : false)
  const isLocal = $derived(
    ctx ? !ctx.track.path.startsWith('http://') && !ctx.track.path.startsWith('https://') : false,
  )
  const canReveal = $derived(appSettings.platform !== 'android' && isLocal)
  const starred = $derived(
    ctx ? (ctx.track.id > 0 ? favorites.has(ctx.track.id) : favorites.hasPath(ctx.track.path)) : false,
  )

  function portal(node: HTMLElement) {
    document.body.appendChild(node)
    return { destroy: () => node.remove() }
  }

  function close() {
    sub = null
    creating = false
    playlistName = ''
    trackMenu.close()
  }

  function items(): HTMLElement[] {
    return menu ? [...menu.querySelectorAll<HTMLElement>('[role="menuitem"]')] : []
  }

  $effect(() => {
    const c = ctx
    if (!c) {
      sub = null
      creating = false
      playlistName = ''
      return
    }
    sub = null
    creating = false
    playlistName = ''
    void tick().then(() => {
      if (!menu) return
      const w = menu.offsetWidth || WIDTH
      const h = menu.offsetHeight || 280
      const rawLeft = c.alignRight ? c.x - w : c.x
      const left = Math.max(8, Math.min(rawLeft, window.innerWidth - w - 8))
      const top = Math.max(8, Math.min(c.y, window.innerHeight - h - 8))
      pos = { left, top }
      menu.focus()
    })
  })

  function openSub(kind: 'playlists' | 'artists', rowEl?: HTMLElement | null) {
    sub = kind
    creating = false
    const menuRect = menu?.getBoundingClientRect()
    const rowRect = rowEl?.getBoundingClientRect()
    const rightSpace = window.innerWidth - ((menuRect?.right ?? pos.left + WIDTH) + 4)
    const left =
      rightSpace >= SUB_WIDTH + 8
        ? (menuRect?.right ?? pos.left + WIDTH) + 4
        : Math.max(8, (menuRect?.left ?? pos.left) - SUB_WIDTH - 4)
    const rawTop = rowRect?.top ?? pos.top
    const top = Math.max(8, Math.min(rawTop, window.innerHeight - 280 - 8))
    subPos = { left, top }
  }

  $effect(() => {
    if (!ctx) return
    const onPointer = (e: PointerEvent) => {
      const target = e.target as Node
      if (!menu?.contains(target) && !submenuEl?.contains(target)) close()
    }
    const onScroll = (e: Event) => {
      const target = e.target as Node
      if (!menu?.contains(target) && !submenuEl?.contains(target)) close()
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

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      close()
    } else if (e.key === 'Tab') {
      close()
    } else if (!creating && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
      e.preventDefault()
      const list = items()
      const at = list.indexOf(document.activeElement as HTMLElement)
      const next =
        e.key === 'ArrowDown' ? (at + 1) % list.length : (at - 1 + list.length) % list.length
      list[next]?.focus()
    }
  }

  async function doPlay() {
    const c = ctx
    if (!c) return
    close()
    if (c.onPlay) c.onPlay()
    else await player.openFiles([c.track.path])
  }

  async function doPlayNext() {
    const c = ctx
    if (!c) return
    close()
    await player.playNext(c.track.path)
    playlists.flash(t('Queued to play next'))
  }

  async function doAddQueue() {
    const c = ctx
    if (!c) return
    close()
    await player.appendFiles([c.track.path])
    playlists.flash(t('Added to queue'))
  }

  async function doToggleFavorite() {
    const c = ctx
    if (!c) return
    close()
    if (c.track.id > 0) await favorites.toggle(c.track.id)
    else await favorites.toggleByPath(c.track.path)
  }

  async function addToPlaylist(p: PlaylistInfo) {
    const c = ctx
    if (!c || c.track.id <= 0) return
    close()
    await playlists.addTracks(p, [c.track.id])
  }

  async function startCreate() {
    creating = true
    await tick()
    submenuEl?.querySelector<HTMLInputElement>('input')?.focus()
  }

  async function createPlaylist() {
    const c = ctx
    const trimmed = playlistName.trim()
    if (!c || c.track.id <= 0 || trimmed === '') return
    const made = await playlists.create(trimmed, [c.track.id])
    close()
    if (made) {
      playlists.flash(
        t('Added {count} to new playlist “{name}”', {
          count: count(1, '{n} track', '{n} tracks'),
          name: made.name,
        }),
      )
    }
  }

  function goArtist(name?: string) {
    const target = name ?? artistList[0]
    if (!target) return
    close()
    artists.open(target)
  }

  function goAlbum() {
    const albumId = ctx?.track.albumId
    if (albumId === null || albumId === undefined) return
    close()
    library.openAlbum(albumId)
  }

  function doEditTags() {
    const c = ctx
    if (!c || c.track.id <= 0) return
    close()
    tagEditor.open([c.track.id], t('Edit tags'))
  }

  async function doReveal() {
    const c = ctx
    if (!c) return
    close()
    try {
      await invoke('app_reveal_file', { path: c.track.path })
    } catch (e) {
      player.error = String(e)
    }
  }

  async function doCopy() {
    const c = ctx
    if (!c) return
    const text = c.track.artist ? `${c.track.artist} - ${c.track.title}` : c.track.title
    close()
    try {
      await navigator.clipboard.writeText(text)
      playlists.flash(t('Copied to clipboard'))
    } catch (e) {
      player.error = String(e)
    }
  }

  const itemCls =
    'flex w-full items-center gap-2.5 px-3 py-1.5 text-left outline-none hover:bg-zinc-100 focus-visible:bg-zinc-100 dark:hover:bg-zinc-800 dark:focus-visible:bg-zinc-800'
</script>

{#if ctx}
  <div
    use:portal
    bind:this={menu}
    role="menu"
    aria-label={ctx.track.title}
    tabindex="-1"
    onkeydown={onKey}
    class="fixed z-50 flex flex-col rounded-xl border border-zinc-200 bg-white py-1 text-sm text-zinc-800 shadow-xl dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100"
    style:left="{pos.left}px"
    style:top="{pos.top}px"
    style:width="{WIDTH}px"
  >
    <button type="button" role="menuitem" class={itemCls} onclick={() => void doPlay()}>
      <Icon name="play" class="size-4 shrink-0 text-zinc-500" />
      <span class="truncate">{t('Play')}</span>
    </button>
    <button type="button" role="menuitem" class={itemCls} onclick={() => void doPlayNext()}>
      <Icon name="queue" class="size-4 shrink-0 text-zinc-500" />
      <span class="truncate">{t('Play next')}</span>
    </button>
    <button type="button" role="menuitem" class={itemCls} onclick={() => void doAddQueue()}>
      <Icon name="plus" class="size-4 shrink-0 text-zinc-500" />
      <span class="truncate">{t('Add to queue')}</span>
    </button>

    <div class="my-1 border-t border-zinc-100 dark:border-zinc-800" role="separator"></div>

    <button type="button" role="menuitem" class={itemCls} onclick={() => void doToggleFavorite()}>
      <Icon
        name={starred ? 'star' : 'starOutline'}
        class={['size-4 shrink-0', starred ? 'text-teal-500' : 'text-zinc-500'].join(' ')}
      />
      <span class="truncate">
        {starred ? t('Remove from favorites') : t('Add to favorites')}
      </span>
    </button>

    {#if hasLibId}
      <button
        type="button"
        role="menuitem"
        aria-haspopup="menu"
        aria-expanded={sub === 'playlists'}
        class={[itemCls, 'justify-between', sub === 'playlists' ? 'bg-zinc-100 dark:bg-zinc-800' : '']}
        onmouseenter={(e) => openSub('playlists', e.currentTarget)}
        onclick={(e) => openSub('playlists', e.currentTarget)}
      >
        <span class="flex min-w-0 items-center gap-2.5">
          <Icon name="plus" class="size-4 shrink-0 text-zinc-500" />
          <span class="truncate">{t('Add to playlist')}</span>
        </span>
        <Icon name="chevronRight" class="size-4 shrink-0 text-zinc-400" />
      </button>
    {/if}

    {#if ctx.onRemoveFromPlaylist}
      <button
        type="button"
        role="menuitem"
        class={itemCls}
        onclick={() => {
          const fn = ctx.onRemoveFromPlaylist
          close()
          fn?.()
        }}
      >
        <Icon name="close" class="size-4 shrink-0 text-zinc-500" />
        <span class="truncate">{t('Remove from playlist')}</span>
      </button>
    {/if}

    {#if ctx.onRemoveFromQueue}
      <button
        type="button"
        role="menuitem"
        class={itemCls}
        onclick={() => {
          const fn = ctx.onRemoveFromQueue
          close()
          fn?.()
        }}
      >
        <Icon name="close" class="size-4 shrink-0 text-zinc-500" />
        <span class="truncate">{t('Remove from queue')}</span>
      </button>
    {/if}

    {#if artistList.length > 0 || hasAlbum}
      <div class="my-1 border-t border-zinc-100 dark:border-zinc-800" role="separator"></div>
      {#if artistList.length === 1}
        <button type="button" role="menuitem" class={itemCls} onclick={() => goArtist()}>
          <Icon name="artist" class="size-4 shrink-0 text-zinc-500" />
          <span class="truncate">{t('Go to artist')}</span>
        </button>
      {:else if artistList.length > 1}
        <button
          type="button"
          role="menuitem"
          aria-haspopup="menu"
          aria-expanded={sub === 'artists'}
          class={[itemCls, 'justify-between', sub === 'artists' ? 'bg-zinc-100 dark:bg-zinc-800' : '']}
          onmouseenter={(e) => openSub('artists', e.currentTarget)}
          onclick={(e) => openSub('artists', e.currentTarget)}
        >
          <span class="flex min-w-0 items-center gap-2.5">
            <Icon name="artist" class="size-4 shrink-0 text-zinc-500" />
            <span class="truncate">{t('Go to artist')}</span>
          </span>
          <Icon name="chevronRight" class="size-4 shrink-0 text-zinc-400" />
        </button>
      {/if}
      {#if hasAlbum}
        <button type="button" role="menuitem" class={itemCls} onclick={goAlbum}>
          <Icon name="album" class="size-4 shrink-0 text-zinc-500" />
          <span class="truncate">{t('Go to album')}</span>
        </button>
      {/if}
    {/if}

    {#if hasLibId || canReveal}
      <div class="my-1 border-t border-zinc-100 dark:border-zinc-800" role="separator"></div>
      {#if hasLibId}
        <button type="button" role="menuitem" class={itemCls} onclick={doEditTags}>
          <Icon name="sliders" class="size-4 shrink-0 text-zinc-500" />
          <span class="truncate">{t('Edit tags…')}</span>
        </button>
      {/if}
      {#if canReveal}
        <button type="button" role="menuitem" class={itemCls} onclick={() => void doReveal()}>
          <Icon name="folder" class="size-4 shrink-0 text-zinc-500" />
          <span class="truncate">{t('Show in folder')}</span>
        </button>
      {/if}
    {/if}

    <div class="my-1 border-t border-zinc-100 dark:border-zinc-800" role="separator"></div>
    <button type="button" role="menuitem" class={itemCls} onclick={() => void doCopy()}>
      <Icon name="music" class="size-4 shrink-0 text-zinc-500" />
      <span class="truncate">{t('Copy song info')}</span>
    </button>
  </div>

  {#if sub === 'playlists' && hasLibId}
    <div
      use:portal
      bind:this={submenuEl}
      role="menu"
      aria-label={t('Add to playlist')}
      class="fixed z-50 flex max-h-72 flex-col overflow-hidden rounded-xl border border-zinc-200 bg-white py-1 text-sm text-zinc-800 shadow-xl dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100"
      style:left="{subPos.left}px"
      style:top="{subPos.top}px"
      style:width="{SUB_WIDTH}px"
    >
      {#if creating}
        <form
          class="flex flex-col gap-2 p-2"
          onsubmit={(e) => {
            e.preventDefault()
            void createPlaylist()
          }}
        >
          <input
            class="rounded-md border border-zinc-200 bg-white px-2 py-1 outline-none focus-visible:ring-2 focus-visible:ring-teal-500 dark:border-zinc-700 dark:bg-zinc-950"
            placeholder={t('Playlist name')}
            aria-label={t('New playlist name')}
            bind:value={playlistName}
          />
          <div class="flex justify-end gap-2">
            <button
              type="button"
              class="rounded-md px-2 py-1 text-zinc-500 hover:bg-zinc-100 dark:hover:bg-zinc-800"
              onclick={() => (creating = false)}
            >
              {t('Cancel')}
            </button>
            <button
              type="submit"
              class="rounded-md bg-teal-600 px-2 py-1 font-medium text-white hover:bg-teal-500 disabled:opacity-50"
              disabled={playlistName.trim() === ''}
            >
              {t('Create')}
            </button>
          </div>
        </form>
      {:else}
        <div class="min-h-0 overflow-y-auto">
          {#each playlists.list as p (p.id)}
            <button
              type="button"
              role="menuitem"
              class="flex w-full items-center justify-between gap-2 px-3 py-1.5 text-left outline-none hover:bg-zinc-100 focus-visible:bg-zinc-100 dark:hover:bg-zinc-800 dark:focus-visible:bg-zinc-800"
              onclick={() => void addToPlaylist(p)}
            >
              <span class="truncate">{p.name}</span>
              <span class="text-xs tabular-nums text-zinc-400">{p.trackCount.toLocaleString()}</span>
            </button>
          {:else}
            <p class="px-3 py-1.5 text-xs text-zinc-500">{t('No playlists yet.')}</p>
          {/each}
        </div>
        <button
          type="button"
          role="menuitem"
          class="flex w-full items-center gap-2 border-t border-zinc-100 px-3 py-1.5 text-left text-teal-700 outline-none hover:bg-zinc-100 focus-visible:bg-zinc-100 dark:border-zinc-800 dark:text-teal-300 dark:hover:bg-zinc-800 dark:focus-visible:bg-zinc-800"
          onclick={() => void startCreate()}
        >
          <Icon name="plus" class="size-4" /> {t('New playlist…')}
        </button>
      {/if}
    </div>
  {:else if sub === 'artists' && artistList.length > 1}
    <div
      use:portal
      bind:this={submenuEl}
      role="menu"
      aria-label={t('Go to artist')}
      class="fixed z-50 flex max-h-72 flex-col overflow-y-auto rounded-xl border border-zinc-200 bg-white py-1 text-sm text-zinc-800 shadow-xl dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100"
      style:left="{subPos.left}px"
      style:top="{subPos.top}px"
      style:width="{SUB_WIDTH}px"
    >
      {#each artistList as name, i (`${name}:${i}`)}
        <button
          type="button"
          role="menuitem"
          class={itemCls}
          onclick={() => goArtist(name)}
        >
          <Icon name="artist" class="size-4 shrink-0 text-zinc-500" />
          <span class="truncate">{name}</span>
        </button>
      {/each}
    </div>
  {/if}
{/if}
