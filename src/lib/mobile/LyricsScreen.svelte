<script lang="ts">
  import { t } from '../i18n.svelte'
  import { fly } from 'svelte/transition'
  import Icon from '../Icon.svelte'
  import LyricLines from '../LyricLines.svelte'
  import LyricsShare from '../LyricsShare.svelte'
  import TintedBackdrop from '../TintedBackdrop.svelte'
  import { motionDuration, viewportHeight } from '../motion'
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
  class="fixed inset-0 z-50 isolate flex flex-col overflow-hidden text-white"
  style:padding-top="var(--inset-top)"
  style:padding-bottom="calc(var(--inset-bottom) + 12px)"
  transition:fly={{ y: viewportHeight(), duration: motionDuration(240), opacity: 1 }}
  role="dialog"
  aria-modal="true"
  aria-label={t("Lyrics")}
>
  <TintedBackdrop background={tint.surface} duration={600} />
  <header class="flex h-16 shrink-0 items-center gap-1 px-2">
    <button class="rounded-full p-3 active:bg-white/10" aria-label={t("Close lyrics")} onclick={onClose}>
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
  <LyricsShare class="shrink-0 px-6 pt-2" />

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
      aria-label={t("Seek")}
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
        aria-label={t(snap.status === 'playing' ? 'Pause' : 'Play')}
        onclick={player.togglePause}
      >
        <Icon name={snap.status === 'playing' ? 'pause' : 'play'} class="size-7" />
      </button>
      <span class="w-10 text-right">{formatTime(duration)}</span>
    </div>
  </div>
</div>
