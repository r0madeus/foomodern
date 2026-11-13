<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import { untrack } from 'svelte'
  import Icon from './Icon.svelte'
  import { lyrics, type LyricLine, type LyricsSource } from './lyrics.svelte'
  import { player } from './player.svelte'

  const SOURCE_LABELS: Record<LyricsSource, string> = {
    localLrc: 'Local .lrc',
    lrclib: 'LRCLIB',
  }

  // Keys that scroll the focused lyrics region by hand.
  const SCROLL_KEYS = new Set(['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End', ' '])

  let follow = $state(true)
  let container = $state<HTMLDivElement | null>(null)

  const current = $derived(player.snapshot.current)
  const duration = $derived(player.snapshot.durationSeconds)
  const res = $derived(lyrics.result)

  function findActiveIndex(lines: LyricLine[], pos: number): number {
    const t = pos + 0.05
    let lo = 0
    let hi = lines.length - 1
    let ans = -1
    while (lo <= hi) {
      const mid = (lo + hi) >> 1
      if (lines[mid].timeSeconds <= t) {
        ans = mid
        lo = mid + 1
      } else {
        hi = mid - 1
      }
    }
    return ans
  }

  const activeIndex = $derived.by(() => {
    if (!res || res.kind !== 'synced') return -1
    return findActiveIndex(res.lines, player.position)
  })

  // Reset scroll follow whenever the active track changes.
  $effect(() => {
    void current?.path
    untrack(() => {
      follow = true
      if (container) container.scrollTop = 0
    })
  })

  // Keep the active synced line centered while auto-follow is active.
  $effect(() => {
    const idx = activeIndex
    const shouldFollow = follow
    if (!shouldFollow || idx < 0) return
    untrack(() => {
      const el = container?.querySelector(`[data-lyric-index="${idx}"]`)
      if (el instanceof HTMLElement) {
        el.scrollIntoView({ block: 'center', behavior: 'smooth' })
      }
    })
  })

  // Manual scrolling (wheel, drag, or keys on the focused region) turns
  // auto-follow off. These listeners only observe the user's intent, so they
  // are attached directly instead of making the scroll area an interactive
  // element. Browsers make scrollable regions keyboard-focusable themselves.
  function watchManualScroll(el: HTMLElement) {
    const stopFollowing = () => {
      if (res?.kind === 'synced') follow = false
    }
    const onPointerDown = (e: PointerEvent) => {
      if (!(e.target instanceof HTMLButtonElement)) stopFollowing()
    }
    const onKeyDown = (e: KeyboardEvent) => {
      if (SCROLL_KEYS.has(e.key)) stopFollowing()
    }
    el.addEventListener('wheel', stopFollowing, { passive: true })
    el.addEventListener('pointerdown', onPointerDown)
    el.addEventListener('keydown', onKeyDown)
    return () => {
      el.removeEventListener('wheel', stopFollowing)
      el.removeEventListener('pointerdown', onPointerDown)
      el.removeEventListener('keydown', onKeyDown)
    }
  }

  function refresh() {
    if (!current) return
    follow = true
    void lyrics.load(current.path, duration, true)
  }

  async function attachLocal() {
    if (!current || lyrics.loading) return
    const targetPath = current.path
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Lyrics', extensions: ['lrc', 'txt'] }],
    })
    if (!picked || typeof picked !== 'string' || current?.path !== targetPath) return
    follow = true
    await lyrics.attachFile(targetPath, picked)
  }

  function seekTo(seconds: number) {
    follow = true
    void player.seek(seconds)
  }

  function resumeSync() {
    follow = true
    if (activeIndex < 0) return
    const el = container?.querySelector(`[data-lyric-index="${activeIndex}"]`)
    if (el instanceof HTMLElement) {
      el.scrollIntoView({ block: 'center', behavior: 'smooth' })
    }
  }
</script>

<aside
  aria-label="Lyrics"
  class="flex w-80 shrink-0 flex-col border-l border-zinc-200 bg-zinc-50/70 lg:w-96 dark:border-zinc-800 dark:bg-zinc-950/70"
>
  <header class="flex items-start justify-between gap-2 border-b border-zinc-200 px-4 py-3 dark:border-zinc-800">
    <div class="min-w-0">
      <div class="flex items-center gap-2">
        <h2 class="text-sm font-semibold tracking-tight">Lyrics</h2>
        {#if res && res.kind !== 'notFound'}
          <span
            class="rounded bg-zinc-200/80 px-1.5 py-0.5 text-[11px] font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300"
          >
            {SOURCE_LABELS[res.source]}
          </span>
        {/if}
      </div>
      {#if current}
        <p class="mt-0.5 truncate text-xs text-zinc-500" title={current.path}>
          {current.title}{current.artist ? ` · ${current.artist}` : ''}
        </p>
      {/if}
    </div>

    <div class="flex shrink-0 items-center gap-1">
      <button
        class="rounded-md p-1.5 text-zinc-500 hover:bg-zinc-200/70 hover:text-zinc-900 disabled:opacity-40 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
        onclick={() => void attachLocal()}
        disabled={!current || lyrics.loading}
        aria-label="Load .lrc file"
        title="Load .lrc file"
      >
        <Icon name="upload" class="size-4" />
      </button>
      <button
        class="rounded-md p-1.5 text-zinc-500 hover:bg-zinc-200/70 hover:text-zinc-900 disabled:opacity-40 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
        onclick={refresh}
        disabled={!current || lyrics.loading}
        aria-label="Refresh lyrics"
        title="Refresh lyrics"
      >
        <Icon name="refresh" class={lyrics.loading ? 'size-4 animate-spin' : 'size-4'} />
      </button>
      <button
        class="rounded-md p-1.5 text-zinc-500 hover:bg-zinc-200/70 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
        onclick={() => lyrics.setOpen(false)}
        aria-label="Close lyrics"
        title="Close lyrics"
      >
        <Icon name="close" class="size-4" />
      </button>
    </div>
  </header>

  <div class="relative min-h-0 flex-1">
    <div
      bind:this={container}
      role="region"
      aria-label="Lyrics text"
      {@attach watchManualScroll}
      class="h-full overflow-y-auto px-4 py-4"
    >
      {#if !current}
        <div class="flex h-full items-center justify-center text-center text-sm text-zinc-400">
          Play a track to see lyrics.
        </div>
      {:else if lyrics.loading && !res}
        <div class="flex h-full items-center justify-center text-sm text-zinc-400">
          Looking up lyrics…
        </div>
      {:else if lyrics.error && (!res || res.kind === 'notFound')}
        <div class="flex h-full flex-col items-center justify-center gap-3 text-center">
          <p class="text-sm text-red-600 dark:text-red-400">{lyrics.error}</p>
          <div class="flex items-center gap-2">
            <button
              class="rounded-lg bg-zinc-900 px-3 py-1.5 text-xs font-medium text-white hover:bg-zinc-700 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-white"
              onclick={refresh}
            >
              Retry
            </button>
            <button
              class="rounded-lg px-3 py-1.5 text-xs font-medium text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800"
              onclick={() => void attachLocal()}
            >
              Load .lrc
            </button>
          </div>
        </div>
      {:else if !res || res.kind === 'notFound'}
        <div class="flex h-full flex-col items-center justify-center gap-3 text-center">
          <p class="text-sm text-zinc-500">No lyrics found.</p>
          <div class="flex items-center gap-2">
            <button
              class="rounded-lg px-3 py-1.5 text-xs font-medium text-zinc-600 hover:bg-zinc-200/70 dark:text-zinc-300 dark:hover:bg-zinc-800"
              onclick={refresh}
            >
              Retry
            </button>
            <button
              class="rounded-lg bg-zinc-900 px-3 py-1.5 text-xs font-medium text-white hover:bg-zinc-700 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-white"
              onclick={() => void attachLocal()}
            >
              Load .lrc
            </button>
          </div>
        </div>
      {:else}
        {#if lyrics.error}
          <p class="mb-3 text-xs text-red-600 dark:text-red-400">{lyrics.error}</p>
        {/if}
        {#if res.kind === 'instrumental'}
          <div class="flex h-full items-center justify-center text-sm text-zinc-500">
            Instrumental track.
          </div>
        {:else if res.kind === 'synced'}
        <div class="flex flex-col gap-1.5 pb-16">
          {#each res.lines as line, i (i)}
            {#if line.text === ''}
              <div
                data-lyric-index={i}
                class={[
                  'py-1.5 text-xs tracking-widest',
                  i === activeIndex ? 'text-teal-600 dark:text-teal-400' : 'text-zinc-400/60',
                ]}
                aria-hidden="true"
              >
                ···
              </div>
            {:else}
              <button
                type="button"
                data-lyric-index={i}
                onclick={() => seekTo(line.timeSeconds)}
                class={[
                  'rounded-lg px-2 py-1.5 text-left text-sm leading-relaxed transition-colors',
                  i === activeIndex
                    ? 'bg-teal-50/90 font-semibold text-teal-900 dark:bg-teal-950/50 dark:text-teal-100'
                    : i < activeIndex
                      ? 'text-zinc-400 hover:bg-zinc-200/60 hover:text-zinc-700 dark:text-zinc-500 dark:hover:bg-zinc-800/60 dark:hover:text-zinc-200'
                      : 'text-zinc-700 hover:bg-zinc-200/60 dark:text-zinc-300 dark:hover:bg-zinc-800/60',
                ]}
              >
                {line.text}
              </button>
            {/if}
          {/each}
        </div>
      {:else}
        <div class="select-text space-y-2 pb-8 text-sm leading-relaxed text-zinc-700 dark:text-zinc-300">
          {#each res.lines as line, i (i)}
            {#if line === ''}
              <div class="h-3" aria-hidden="true"></div>
            {:else}
              <p>{line}</p>
            {/if}
          {/each}
        </div>
        {/if}
      {/if}
    </div>

    {#if res?.kind === 'synced' && !follow}
      <button
        type="button"
        onclick={resumeSync}
        class="absolute right-4 bottom-4 rounded-full bg-teal-600 px-3 py-1 text-xs font-medium text-white shadow-md hover:bg-teal-500"
      >
        Sync
      </button>
    {/if}
  </div>
</aside>
