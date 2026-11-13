<script lang="ts">
  import Icon from './Icon.svelte'
  import { player, formatTime } from './player.svelte'

  // While the user drags the seek bar we show the drag value and only send
  // one seek command when they let go.
  let dragging = $state<number | null>(null)
  let volumeBeforeMute = $state(100)

  const snap = $derived(player.snapshot)
  const duration = $derived(snap.durationSeconds)
  const shown = $derived(dragging ?? player.position)
  const hasTrack = $derived(snap.current !== null)
  const progress = $derived(duration ? Math.min(100, (shown / duration) * 100) : 0)

  function commitSeek() {
    if (dragging !== null) void player.seek(dragging)
    dragging = null
  }

  function toggleMute() {
    if (snap.volume > 0) {
      volumeBeforeMute = snap.volume
      void player.setVolume(0)
    } else {
      void player.setVolume(volumeBeforeMute || 100)
    }
  }
</script>

<footer
  class="grid grid-cols-[minmax(0,1fr)_minmax(0,2fr)_minmax(0,1fr)] items-center gap-6 border-t border-zinc-200 bg-white/80 px-5 py-3 backdrop-blur dark:border-zinc-800 dark:bg-zinc-900/80"
>
  <div class="min-w-0">
    {#if snap.current}
      <p class="truncate text-sm font-medium text-zinc-900 dark:text-zinc-100" title={snap.current.path}>
        {snap.current.title}
      </p>
      <p class="truncate text-xs text-zinc-500">
        {(snap.currentIndex ?? 0) + 1} / {snap.queueLength}
      </p>
    {:else}
      <p class="text-sm text-zinc-400">Nothing playing</p>
    {/if}
  </div>

  <div class="flex flex-col items-center gap-1.5">
    <div class="flex items-center gap-2">
      <button
        class="rounded-full p-2 text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 disabled:opacity-40 dark:text-zinc-300 dark:hover:bg-zinc-800 dark:hover:text-white"
        onclick={player.previous}
        disabled={!hasTrack}
        aria-label="Previous"
      >
        <Icon name="prev" />
      </button>
      <button
        class="rounded-full bg-teal-600 p-3 text-white shadow-sm hover:bg-teal-500 disabled:opacity-40"
        onclick={player.togglePause}
        disabled={!hasTrack}
        aria-label={snap.status === 'playing' ? 'Pause' : 'Play'}
      >
        <Icon name={snap.status === 'playing' ? 'pause' : 'play'} />
      </button>
      <button
        class="rounded-full p-2 text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 disabled:opacity-40 dark:text-zinc-300 dark:hover:bg-zinc-800 dark:hover:text-white"
        onclick={player.next}
        disabled={!hasTrack}
        aria-label="Next"
      >
        <Icon name="next" />
      </button>
    </div>
    <div class="flex w-full items-center gap-3 text-xs tabular-nums text-zinc-500">
      <span class="w-12 text-right">{formatTime(hasTrack ? shown : null)}</span>
      <input
        type="range"
        class="seek h-1 w-full"
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
      <span class="w-12">{formatTime(duration)}</span>
    </div>
  </div>

  <div class="flex items-center justify-end gap-2">
    <button
      class="rounded-full p-2 text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-white"
      onclick={toggleMute}
      aria-label={snap.volume > 0 ? 'Mute' : 'Unmute'}
    >
      <Icon name={snap.volume > 0 ? 'volume' : 'mute'} />
    </button>
    <input
      type="range"
      class="seek h-1 w-28"
      style:--progress="{snap.volume}%"
      min="0"
      max="100"
      step="1"
      value={snap.volume}
      aria-label="Volume"
      oninput={(e) => player.setVolume(e.currentTarget.valueAsNumber)}
    />
  </div>
</footer>
