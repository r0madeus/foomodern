<script lang="ts">
  import Icon from './Icon.svelte'
  import LyricLines from './LyricLines.svelte'
  import { lyrics, type LyricsSource } from './lyrics.svelte'
  import { player } from './player.svelte'
  import { nowCover } from './nowCover.svelte'
  import { tintFrom } from './tint'

  // Side panel with the lyrics of the playing track, on the cover's tint.
  // Following, seeking by click and "back to current line" live in
  // LyricLines (shared with the now-playing view and the phone UI).
  const SOURCE_LABELS: Record<LyricsSource, string> = {
    localLrc: 'Local .lrc',
    lrclib: 'LRCLIB',
  }

  const current = $derived(player.snapshot.current)
  const duration = $derived(player.snapshot.durationSeconds)
  const res = $derived(lyrics.result)
  const tint = $derived(tintFrom(nowCover.color))

  function refresh() {
    if (!current) return
    void lyrics.load(current.path, duration, true)
  }

  async function attachLocal() {
    if (!current || lyrics.loading) return
    await lyrics.pickAndAttach(current.path)
  }

  const iconButton =
    'rounded-full p-1.5 text-white/70 hover:bg-white/10 hover:text-white disabled:opacity-40 outline-none focus-visible:ring-2 focus-visible:ring-white/60'
</script>

<aside
  aria-label="Lyrics"
  class="flex w-80 shrink-0 flex-col text-white transition-[background] duration-700 lg:w-96"
  style:background={tint.surface}
>
  <header class="flex items-start justify-between gap-2 px-5 pt-4 pb-2">
    <div class="min-w-0">
      <div class="flex items-center gap-2">
        <h2 class="text-base font-bold">Lyrics</h2>
        {#if res && res.kind !== 'notFound'}
          <span class="rounded bg-black/25 px-1.5 py-0.5 text-[11px] font-medium text-white/80">
            {SOURCE_LABELS[res.source]}
          </span>
        {/if}
      </div>
      {#if current}
        <p class="mt-0.5 truncate text-xs text-white/60" title={current.path}>
          {current.title}{current.artist ? ` · ${current.artist}` : ''}
        </p>
      {/if}
    </div>
    <div class="flex shrink-0 items-center gap-0.5">
      <button class={iconButton} onclick={() => void attachLocal()} disabled={!current || lyrics.loading} aria-label="Load .lrc file" title="Load .lrc file">
        <Icon name="upload" class="size-4" />
      </button>
      <button class={iconButton} onclick={refresh} disabled={!current || lyrics.loading} aria-label="Refresh lyrics" title="Refresh lyrics">
        <Icon name="refresh" class={lyrics.loading ? 'size-4 animate-spin' : 'size-4'} />
      </button>
      <button class={iconButton} onclick={() => lyrics.setOpen(false)} aria-label="Close lyrics" title="Close lyrics">
        <Icon name="close" class="size-4" />
      </button>
    </div>
  </header>

  {#if !current}
    <div class="flex flex-1 items-center justify-center px-6 text-center text-sm text-white/60">
      Play a track to see lyrics.
    </div>
  {:else if (lyrics.error && (!res || res.kind === 'notFound')) || (!lyrics.loading && res?.kind === 'notFound')}
    <div class="flex flex-1 flex-col items-center justify-center gap-3 px-6 text-center">
      <p class="text-sm text-white/70">{lyrics.error ?? 'No lyrics found.'}</p>
      <div class="flex items-center gap-2">
        <button class="rounded-full px-3 py-1.5 text-xs font-semibold text-white/80 hover:bg-white/10" onclick={refresh}>
          Retry
        </button>
        <button class="rounded-full bg-white px-3 py-1.5 text-xs font-semibold text-zinc-950 hover:bg-white/90" onclick={() => void attachLocal()}>
          Load .lrc
        </button>
      </div>
    </div>
  {:else}
    {#if lyrics.error}
      <p class="px-5 pb-2 text-xs text-red-200">{lyrics.error}</p>
    {/if}
    <LyricLines dim={tint.dim} class="flex-1 px-5 [mask-image:linear-gradient(to_bottom,transparent,black_4%,black_90%,transparent)]" />
  {/if}
</aside>
