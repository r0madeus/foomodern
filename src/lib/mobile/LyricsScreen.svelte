<script lang="ts">
  import { fly } from 'svelte/transition'
  import Icon from '../Icon.svelte'
  import LyricLines from '../LyricLines.svelte'
  import { player, formatTime } from '../player.svelte'
  import { nowCover } from '../nowCover.svelte'
  import { tintFrom } from '../tint'

  // Full-screen lyrics on the cover's tint, with a compact seek bar.
  let { onClose }: { onClose: () => void } = $props()

  let dragging = $state<number | null>(null)
  const snap = $derived(player.snapshot)
  const duration = $derived(snap.durationSeconds)
  const shown = $derived(dragging ?? player.position)
  const progress = $derived(duration ? Math.min(100, (shown / duration) * 100) : 0)
  const tint = $derived(tintFrom(nowCover.color))
</script>

<div
  class="fixed inset-0 z-50 flex flex-col text-white"
  style:background={tint.surface}
  style:padding-top="var(--inset-top)"
  style:padding-bottom="calc(var(--inset-bottom) + 12px)"
  transition:fly={{ y: 600, duration: 240, opacity: 1 }}
  role="dialog"
  aria-modal="true"
  aria-label="Lyrics"
>
  <header class="flex h-16 shrink-0 items-center gap-1 px-2">
    <button class="rounded-full p-3 active:bg-white/10" aria-label="Close lyrics" onclick={onClose}>
      <Icon name="down" class="size-7" />
    </button>
    {#if snap.current}
      <div class="min-w-0 flex-1 text-center leading-tight">
        <p class="truncate text-[15px] font-bold">{snap.current.title}</p>
        <p class="truncate text-[13px] text-white/70">{snap.current.artist}</p>
      </div>
    {/if}
    <span class="w-12"></span>
  </header>

  <LyricLines big dim={tint.dim} class="flex-1 px-6" />

  <div class="shrink-0 px-6 pt-3">
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
      onchange={() => {
        if (dragging !== null) void player.seek(dragging)
        dragging = null
      }}
    />
    <div class="mt-1 flex items-center justify-between text-xs tabular-nums text-white/60">
      <span class="w-10">{formatTime(shown)}</span>
      <button
        class="flex size-14 items-center justify-center rounded-full bg-white text-zinc-950 active:scale-95"
        aria-label={snap.status === 'playing' ? 'Pause' : 'Play'}
        onclick={player.togglePause}
      >
        <Icon name={snap.status === 'playing' ? 'pause' : 'play'} class="size-7" />
      </button>
      <span class="w-10 text-right">{formatTime(duration)}</span>
    </div>
  </div>
</div>
