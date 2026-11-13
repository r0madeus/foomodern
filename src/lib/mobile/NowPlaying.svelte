<script lang="ts">
  import { fly } from 'svelte/transition'
  import Cover from '../Cover.svelte'
  import Icon from '../Icon.svelte'
  import CurrentLine from '../CurrentLine.svelte'
  import LyricLines from '../LyricLines.svelte'
  import { player, formatTime } from '../player.svelte'
  import { lyrics } from '../lyrics.svelte'
  import { nowCover } from '../nowCover.svelte'
  import { tintFrom } from '../tint'

  // Full-screen player tinted with the cover's colour. Scrolling down shows
  // the lyrics card; swipe down at the top (or back) closes it.
  let { onClose, onQueue, onLyrics }: { onClose: () => void; onQueue: () => void; onLyrics: () => void } = $props()

  let dragging = $state<number | null>(null)
  let pull = $state(0)
  let startY = 0
  let scroller = $state<HTMLDivElement | null>(null)

  const snap = $derived(player.snapshot)
  const duration = $derived(snap.durationSeconds)
  const shown = $derived(dragging ?? player.position)
  const progress = $derived(duration ? Math.min(100, (shown / duration) * 100) : 0)
  const tint = $derived(tintFrom(nowCover.color))
  const hasLyrics = $derived(
    lyrics.result !== null && (lyrics.result.kind === 'synced' || lyrics.result.kind === 'plain'),
  )

  function onTouchStart(e: TouchEvent) {
    if ((e.target as HTMLElement).closest('input, button, [data-no-pull]')) return
    if ((scroller?.scrollTop ?? 0) > 0) return
    startY = e.touches[0].clientY
    pull = 0.001
  }
  function onTouchMove(e: TouchEvent) {
    if (pull === 0) return
    const dy = e.touches[0].clientY - startY
    if (dy < 0 && pull <= 0.001) {
      pull = 0 // scrolling up into the lyrics, not a dismiss
      return
    }
    pull = Math.max(0.001, dy)
  }
  function onTouchEnd() {
    if (pull > 120) onClose()
    pull = 0
  }
  function commitSeek() {
    if (dragging !== null) void player.seek(dragging)
    dragging = null
  }
</script>

<div
  class="fixed inset-0 z-40 text-white"
  style:background="linear-gradient(to bottom, {tint.top} 0%, #09090b 92%)"
  style:transform={pull > 0 ? `translateY(${pull}px)` : undefined}
  style:transition={pull > 0 ? 'none' : 'transform 200ms ease-out, background 600ms ease'}
  transition:fly={{ y: 600, duration: 260, opacity: 1 }}
  ontouchstart={onTouchStart}
  ontouchmove={onTouchMove}
  ontouchend={onTouchEnd}
  role="dialog"
  tabindex="-1"
  aria-modal="true"
  aria-label="Now playing"
>
  <div bind:this={scroller} class="h-full overflow-y-auto overscroll-contain">
    <section
      class="flex flex-col px-6"
      style:min-height="100dvh"
      style:padding-top="var(--inset-top)"
      style:padding-bottom="calc(var(--inset-bottom) + 12px)"
    >
      <header class="flex h-14 shrink-0 items-center">
        <button class="-ml-3 rounded-full p-3 active:bg-white/10" aria-label="Close player" onclick={onClose}>
          <Icon name="down" class="size-7" />
        </button>
        <div class="min-w-0 flex-1 text-center leading-tight">
          <p class="text-[11px] tracking-widest text-white/70 uppercase">Now playing</p>
          {#if snap.queueLength > 0}
            <p class="text-sm font-bold">{(snap.currentIndex ?? 0) + 1} of {snap.queueLength}</p>
          {/if}
        </div>
        <span class="w-10"></span>
      </header>

      {#if snap.current}
        <div class="flex min-h-0 flex-1 items-center justify-center pt-3">
          <Cover cover={nowCover.cover} title={snap.current.title} class="aspect-square max-h-full w-full rounded-lg text-7xl shadow-2xl shadow-black/50" />
        </div>

        <div class="shrink-0">
          <CurrentLine />
          <h2 class="mt-1 truncate text-[22px] leading-tight font-bold">{snap.current.title}</h2>
          <p class="mt-0.5 truncate text-base text-white/70">{snap.current.artist || 'Unknown artist'}</p>

          <div class="mt-5" data-no-pull>
            <input
              type="range"
              class="seek touch light w-full"
              style:--progress="{progress}%"
              min="0"
              max={duration ?? 0}
              step="0.1"
              value={shown}
              disabled={!duration}
              aria-label="Seek"
              oninput={(e) => (dragging = e.currentTarget.valueAsNumber)}
              onchange={commitSeek}
            />
            <div class="mt-1.5 flex justify-between text-xs tabular-nums text-white/60">
              <span>{formatTime(shown)}</span>
              <span>{formatTime(duration)}</span>
            </div>
          </div>

          <div class="mt-2 flex items-center justify-between">
            <button
              class={['relative rounded-full p-3 active:bg-white/10', snap.shuffle ? 'text-teal-400' : 'text-white']}
              aria-pressed={snap.shuffle}
              aria-label={snap.shuffle ? 'Shuffle on' : 'Shuffle off'}
              onclick={player.toggleShuffle}
            >
              <Icon name="shuffle" class="size-6" />
              {#if snap.shuffle}<span class="absolute bottom-1 left-1/2 size-1 -translate-x-1/2 rounded-full bg-teal-400"></span>{/if}
            </button>
            <button class="rounded-full p-2 active:bg-white/10" aria-label="Previous" onclick={player.previous}>
              <Icon name="prev" class="size-10" />
            </button>
            <button
              class="flex size-[72px] items-center justify-center rounded-full bg-white text-zinc-950 active:scale-95"
              aria-label={snap.status === 'playing' ? 'Pause' : 'Play'}
              onclick={player.togglePause}
            >
              <Icon name={snap.status === 'playing' ? 'pause' : 'play'} class="size-9" />
            </button>
            <button class="rounded-full p-2 active:bg-white/10" aria-label="Next" onclick={player.next}>
              <Icon name="next" class="size-10" />
            </button>
            <button
              class={['relative rounded-full p-3 active:bg-white/10', snap.repeat !== 'off' ? 'text-teal-400' : 'text-white']}
              aria-pressed={snap.repeat !== 'off'}
              aria-label={snap.repeat === 'one' ? 'Repeat one' : snap.repeat === 'all' ? 'Repeat all' : 'Repeat off'}
              onclick={player.cycleRepeat}
            >
              <Icon name={snap.repeat === 'one' ? 'repeatOne' : 'repeat'} class="size-6" />
              {#if snap.repeat !== 'off'}<span class="absolute bottom-1 left-1/2 size-1 -translate-x-1/2 rounded-full bg-teal-400"></span>{/if}
            </button>
          </div>

          <div class="mt-3 flex items-center justify-between">
            <button class="-ml-3 rounded-full p-3 text-white/80 active:bg-white/10" aria-label="Lyrics" onclick={onLyrics}>
              <Icon name="lyrics" class="size-6" />
            </button>
            <button class="-mr-3 rounded-full p-3 text-white/80 active:bg-white/10" aria-label="Queue" onclick={onQueue}>
              <Icon name="queue" class="size-6" />
            </button>
          </div>
        </div>
      {/if}
    </section>

    <!-- Below the fold: the lyrics card. -->
    <section class="px-4" style:padding-bottom="calc(var(--inset-bottom) + 24px)">
      <div class="overflow-hidden rounded-xl" style:background={tint.surface}>
        <div class="flex items-center justify-between px-4 pt-4 pb-1">
          <h3 class="text-base font-bold">Lyrics</h3>
          {#if hasLyrics}
            <button class="rounded-full bg-black/25 p-2.5 active:bg-black/40" aria-label="Full-screen lyrics" onclick={onLyrics}>
              <Icon name="expand" class="size-4" />
            </button>
          {/if}
        </div>
        <LyricLines dim={tint.dim} class={hasLyrics ? 'h-[420px] px-4' : 'px-4'} />
      </div>
    </section>
  </div>
</div>
