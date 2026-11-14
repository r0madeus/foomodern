<script lang="ts">
  import { t } from './i18n.svelte'
  import Icon from './Icon.svelte'
  import AlbumLink from './AlbumLink.svelte'
  import ArtistLinks from './ArtistLinks.svelte'
  import Cover from './Cover.svelte'
  import FavoriteButton from './FavoriteButton.svelte'
  import type { TrackRow } from './library.svelte'
  import { nowCover } from './nowCover.svelte'
  import { lyrics } from './lyrics.svelte'
  import { player, formatTime } from './player.svelte'
  import { trackMenu } from './contextMenu.svelte'

  let {
    nowPlaying = false,
    onToggleNowPlaying,
    queueOpen = false,
    onToggleQueue,
  }: {
    nowPlaying?: boolean
    onToggleNowPlaying?: () => void
    queueOpen?: boolean
    onToggleQueue?: () => void
  } = $props()

  // While the user drags the seek bar we show the drag value and only send
  // one seek command when they let go.
  let dragging = $state<number | null>(null)

  const snap = $derived(player.snapshot)
  const duration = $derived(snap.durationSeconds)
  const shown = $derived(dragging ?? player.position)
  const hasTrack = $derived(snap.current !== null)
  const progress = $derived(duration ? Math.min(100, (shown / duration) * 100) : 0)
  const repeatLabel = $derived(
    t(snap.repeat === 'one' ? 'Repeat: One' : snap.repeat === 'all' ? 'Repeat: All' : 'Repeat: Off'),
  )
  const ambientGlow = $derived.by(() => {
    const c = nowCover.color
    if (!c) return ''
    const [r, g, b] = c
    return `radial-gradient(420px 96px at 14% 50%, rgba(${r}, ${g}, ${b}, 0.22), transparent 75%)`
  })

  function commitSeek() {
    if (dragging !== null) void player.seek(dragging)
    dragging = null
  }
</script>

<footer
  class="relative grid grid-cols-[minmax(0,1fr)_minmax(0,2fr)_minmax(0,1fr)] items-center gap-6 overflow-hidden border-t border-zinc-200/80 bg-white/85 px-5 py-3 backdrop-blur-xl dark:border-zinc-800/80 dark:bg-zinc-900/85"
>
  {#if ambientGlow}
    <div
      class="pointer-events-none absolute inset-0 transition-opacity duration-500"
      style:background={ambientGlow}
      aria-hidden="true"
    ></div>
  {/if}
  <div class="relative flex min-w-0 items-center gap-2">
    {#if snap.current}
      {@const cur = snap.current}
      {@const menuRow: TrackRow = nowCover.track ?? {
        id: -1,
        title: cur.title,
        artist: cur.artist,
        album: '',
        albumId: null,
        trackNo: null,
        discNo: null,
        year: null,
        durationMs: null,
        path: cur.path,
        cover: nowCover.cover,
      }}
      <div
        role="region"
        aria-label={t('Now playing')}
        class="flex min-w-0 items-center gap-3"
        oncontextmenu={(e) => trackMenu.fromEvent(e, menuRow)}
      >
        <button
          class="group relative shrink-0 rounded-md outline-none focus-visible:ring-2 focus-visible:ring-teal-500"
          onclick={onToggleNowPlaying}
          aria-pressed={nowPlaying}
          title={t(nowPlaying ? 'Close now playing' : 'Open now playing')}
        >
          <Cover cover={nowCover.cover} title={cur.title} class="size-12 rounded-md text-sm shadow" />
          <span class="absolute inset-0 flex items-center justify-center rounded-md bg-black/50 text-white opacity-0 transition-opacity group-hover:opacity-100">
            <Icon name={nowPlaying ? 'down' : 'up'} class="size-6" />
          </span>
        </button>
        <div class="min-w-0 flex-1">
          <button
            class="block max-w-full truncate text-left text-sm font-medium text-zinc-900 hover:underline focus-visible:ring-2 focus-visible:ring-teal-500 dark:text-zinc-100"
            onclick={onToggleNowPlaying}
            aria-pressed={nowPlaying}
            title={t(nowPlaying ? 'Close now playing' : 'Open now playing')}
          >
            {cur.title}
          </button>
          <div class="truncate text-xs text-zinc-500">
            {#if cur.artist}
              <ArtistLinks artist={cur.artist} /> ·
            {/if}{#if nowCover.track?.album}
              <AlbumLink album={nowCover.track.album} albumId={nowCover.track.albumId} /> ·
            {/if}{(snap.currentIndex ?? 0) + 1} / {snap.queueLength}
          </div>
        </div>
      </div>
      {#if !cur.path.startsWith('http://') && !cur.path.startsWith('https://')}
        <FavoriteButton path={cur.path} title={cur.title} />
      {/if}
    {:else}
      <p class="text-sm text-zinc-400">{t("Nothing playing")}</p>
    {/if}
  </div>

  <div class="relative flex flex-col items-center gap-1.5">
    <div class="flex items-center gap-2">
      <button
        class={[
          'rounded-full p-2 transition-colors',
          snap.shuffle
            ? 'text-teal-600 hover:bg-teal-50 dark:text-teal-400 dark:hover:bg-teal-950/60'
            : 'text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-white',
        ]}
        onclick={player.toggleShuffle}
        aria-pressed={snap.shuffle}
        aria-label={t(snap.shuffle ? 'Shuffle on' : 'Shuffle off')}
        title={`${t(snap.shuffle ? 'Shuffle: On' : 'Shuffle: Off')} (S)`}
      >
        <Icon name="shuffle" class="size-4" />
      </button>
      <button
        class="rounded-full p-2 text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 disabled:opacity-40 dark:text-zinc-300 dark:hover:bg-zinc-800 dark:hover:text-white"
        onclick={player.previous}
        disabled={!hasTrack}
        aria-label={t("Previous")}
        title={`${t('Previous')} (Ctrl+←)`}
      >
        <Icon name="prev" />
      </button>
      <button
        class="rounded-full bg-teal-600 p-3 text-white shadow-sm transition hover:scale-105 hover:bg-teal-500 active:scale-95 disabled:opacity-40 disabled:hover:scale-100 dark:bg-teal-500 dark:text-zinc-950 dark:hover:bg-teal-400"
        onclick={player.togglePause}
        disabled={!hasTrack}
        aria-label={t(snap.status === 'playing' ? 'Pause' : 'Play')}
        title={`${t(snap.status === 'playing' ? 'Pause' : 'Play')} (Space)`}
      >
        <Icon name={snap.status === 'playing' ? 'pause' : 'play'} />
      </button>
      <button
        class="rounded-full p-2 text-zinc-600 hover:bg-zinc-100 hover:text-zinc-900 disabled:opacity-40 dark:text-zinc-300 dark:hover:bg-zinc-800 dark:hover:text-white"
        onclick={player.next}
        disabled={!hasTrack}
        aria-label={t("Next")}
        title={`${t('Next')} (Ctrl+→)`}
      >
        <Icon name="next" />
      </button>
      <button
        class={[
          'rounded-full p-2 transition-colors',
          snap.repeat !== 'off'
            ? 'text-teal-600 hover:bg-teal-50 dark:text-teal-400 dark:hover:bg-teal-950/60'
            : 'text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-white',
        ]}
        onclick={player.cycleRepeat}
        aria-pressed={snap.repeat !== 'off'}
        aria-label={repeatLabel}
        title={`${repeatLabel} (R)`}
      >
        <Icon name={snap.repeat === 'one' ? 'repeatOne' : 'repeat'} class="size-4" />
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
        aria-label={t("Seek")}
        oninput={(e) => (dragging = e.currentTarget.valueAsNumber)}
        onchange={commitSeek}
      />
      <span class="w-12">{formatTime(duration)}</span>
    </div>
  </div>

  <div class="relative flex items-center justify-end gap-2">
    <button
      class={[
        'rounded-full p-2 transition-colors',
        lyrics.open
          ? 'bg-teal-50 text-teal-700 hover:bg-teal-100 dark:bg-teal-950/60 dark:text-teal-300 dark:hover:bg-teal-950'
          : 'text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-white',
      ]}
      onclick={() => lyrics.toggle()}
      aria-pressed={lyrics.open}
      aria-label={t("Lyrics")}
      title={t("Lyrics")}
    >
      <Icon name="lyrics" />
    </button>
    {#if onToggleQueue}
      <button
        type="button"
        class={[
          'rounded-full p-2 transition-colors',
          queueOpen
            ? 'bg-teal-50 text-teal-700 hover:bg-teal-100 dark:bg-teal-950/60 dark:text-teal-300 dark:hover:bg-teal-950'
            : 'text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-white',
        ]}
        onclick={onToggleQueue}
        aria-pressed={queueOpen}
        aria-label={t("Queue")}
        title={t("Queue")}
      >
        <Icon name="queue" />
      </button>
    {/if}
    <button
      class="rounded-full p-2 text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-white"
      onclick={player.toggleMute}
      aria-label={t(snap.volume > 0 ? 'Mute' : 'Unmute')}
      title={`${t(snap.volume > 0 ? 'Mute' : 'Unmute')} (M)`}
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
      aria-label={t("Volume")}
      oninput={(e) => player.setVolume(e.currentTarget.valueAsNumber)}
    />
  </div>
</footer>
