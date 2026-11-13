<script lang="ts">
  import { fly } from 'svelte/transition'
  import Cover from '../Cover.svelte'
  import Icon from '../Icon.svelte'
  import { player, formatTime } from '../player.svelte'
  import { nowCover } from './nowCover.svelte'

  // Full-screen player. Swipe down (or back) closes it.
  let { onClose, onQueue, onLyrics }: { onClose: () => void; onQueue: () => void; onLyrics: () => void } = $props()

  let dragging = $state<number | null>(null)
  let pull = $state(0)
  let startY = 0

  const snap = $derived(player.snapshot)
  const duration = $derived(snap.durationSeconds)
  const shown = $derived(dragging ?? player.position)
  const progress = $derived(duration ? Math.min(100, (shown / duration) * 100) : 0)

  function onTouchStart(e: TouchEvent) {
    if ((e.target as HTMLElement).closest('input, button')) return
    startY = e.touches[0].clientY
    pull = 0.001
  }
  function onTouchMove(e: TouchEvent) {
    if (pull === 0) return
    pull = Math.max(0.001, e.touches[0].clientY - startY)
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
  class="fixed inset-0 z-40 overflow-hidden bg-zinc-950 text-white"
  style:transform={pull > 0 ? `translateY(${pull}px)` : undefined}
  style:transition={pull > 0 ? 'none' : 'transform 200ms ease-out'}
  transition:fly={{ y: 600, duration: 260, opacity: 1 }}
  ontouchstart={onTouchStart}
  ontouchmove={onTouchMove}
  ontouchend={onTouchEnd}
  role="dialog"
  tabindex="-1"
  aria-modal="true"
  aria-label="Now playing"
>
  <!-- Backdrop: the cover, blurred, fading to black. -->
  {#if snap.current}
    <Cover cover={nowCover.cover} title={snap.current.title} class="absolute inset-0 size-full scale-150 opacity-60 blur-3xl" />
  {/if}
  <div class="absolute inset-0 bg-gradient-to-b from-black/20 via-black/50 to-zinc-950"></div>

  <div class="relative flex h-full flex-col px-6" style:padding-top="var(--inset-top)" style:padding-bottom="calc(var(--inset-bottom) + 16px)">
    <header class="flex h-14 shrink-0 items-center">
      <button class="-ml-3 rounded-full p-3 active:bg-white/10" aria-label="Close player" onclick={onClose}>
        <Icon name="down" class="size-7" />
      </button>
      <p class="flex-1 text-center text-xs font-semibold tracking-widest text-white/80 uppercase">Now playing</p>
      <button class="-mr-3 rounded-full p-3 active:bg-white/10" aria-label="Queue" onclick={onQueue}>
        <Icon name="queue" class="size-6" />
      </button>
    </header>

    {#if snap.current}
      <div class="flex min-h-0 flex-1 items-center justify-center py-4">
        <Cover cover={nowCover.cover} title={snap.current.title} class="aspect-square max-h-full w-full rounded-lg text-7xl shadow-2xl shadow-black/60" />
      </div>

      <div class="shrink-0">
        <h2 class="truncate text-[22px] leading-tight font-bold">{snap.current.title}</h2>
        <p class="mt-1 truncate text-base text-white/70">{snap.current.artist || 'Unknown artist'}</p>

        <div class="mt-5">
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

        <div class="mt-3 flex items-center justify-between">
          <button
            class={['relative rounded-full p-3 active:bg-white/10', snap.shuffle ? 'text-teal-400' : 'text-white/80']}
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
            class={['relative rounded-full p-3 active:bg-white/10', snap.repeat !== 'off' ? 'text-teal-400' : 'text-white/80']}
            aria-pressed={snap.repeat !== 'off'}
            aria-label={snap.repeat === 'one' ? 'Repeat one' : snap.repeat === 'all' ? 'Repeat all' : 'Repeat off'}
            onclick={player.cycleRepeat}
          >
            <Icon name={snap.repeat === 'one' ? 'repeatOne' : 'repeat'} class="size-6" />
            {#if snap.repeat !== 'off'}<span class="absolute bottom-1 left-1/2 size-1 -translate-x-1/2 rounded-full bg-teal-400"></span>{/if}
          </button>
        </div>

        <div class="mt-4 flex items-center justify-between">
          <button class="flex items-center gap-2 rounded-full bg-white/10 px-4 py-2 text-sm font-medium active:bg-white/20" onclick={onLyrics}>
            <Icon name="lyrics" class="size-4" /> Lyrics
          </button>
          <span class="text-xs text-white/50">{(snap.currentIndex ?? 0) + 1} of {snap.queueLength}</span>
        </div>
      </div>
    {/if}
  </div>
</div>
