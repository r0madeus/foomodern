<script lang="ts">
  import { t } from './i18n.svelte'
  import { fly } from 'svelte/transition'
  import AlbumLink from './AlbumLink.svelte'
  import ArtistLinks from './ArtistLinks.svelte'
  import Cover from './Cover.svelte'
  import FavoriteButton from './FavoriteButton.svelte'
  import Icon from './Icon.svelte'
  import LyricLines from './LyricLines.svelte'
  import TintedBackdrop from './TintedBackdrop.svelte'
  import type { TrackRow } from './library.svelte'
  import { motionDuration } from './motion'
  import { player } from './player.svelte'
  import { lyrics } from './lyrics.svelte'
  import { nowCover } from './nowCover.svelte'
  import { tintFrom } from './tint'
  import { trackMenu } from './contextMenu.svelte'

  // Desktop "now playing": the cover large on the cover's tint, the line
  // being sung under it, and the lyrics alongside. The player bar below
  // keeps the controls.
  let { onClose }: { onClose: () => void } = $props()

  const snap = $derived(player.snapshot)
  const tint = $derived(tintFrom(nowCover.color))
  const hasLyrics = $derived(
    lyrics.result !== null && (lyrics.result.kind === 'synced' || lyrics.result.kind === 'plain'),
  )
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onClose()} />

<div
  transition:fly={{ y: 24, duration: motionDuration(220) }}
  class="absolute inset-0 isolate flex flex-col overflow-hidden text-white"
>
  <TintedBackdrop background="linear-gradient(160deg, {tint.top} 0%, {tint.surface} 45%, #09090b 100%)" />
  <header class="flex shrink-0 items-center gap-2 px-6 pt-4">
    <button class="rounded-full p-2 text-white/80 hover:bg-white/10 hover:text-white" aria-label={t("Close now playing")} title={t("Close (Esc)")} onclick={onClose}>
      <Icon name="down" class="size-6" />
    </button>
    <p class="text-xs font-semibold tracking-widest text-white/70 uppercase">{t("Now playing")}</p>
  </header>

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
    <div class={['grid min-h-0 flex-1 gap-10 px-10 pt-4 pb-6', hasLyrics ? 'grid-cols-[minmax(0,1fr)_minmax(0,1.1fr)]' : 'grid-cols-1 place-items-center']}>
      <div class="flex min-h-0 flex-col justify-center">
        <Cover
          cover={nowCover.cover}
          title={cur.title}
          class="aspect-square max-h-[min(56vh,480px)] w-auto max-w-full self-center rounded-lg text-8xl shadow-2xl shadow-black/50"
        />
        <div
          role="region"
          aria-label={t('Now playing')}
          class="mx-auto mt-6 w-full max-w-[480px]"
          oncontextmenu={(e) => trackMenu.fromEvent(e, menuRow)}
        >
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0 flex-1">
              <h2 class="truncate text-3xl leading-tight font-bold">{cur.title}</h2>
              <p class="mt-1 truncate text-lg text-white/70">
                {#if cur.artist}
                  <ArtistLinks artist={cur.artist} onNavigate={onClose} />
                {:else}
                  {t('Unknown artist')}
                {/if}
                {#if nowCover.track?.album}
                  <span class="opacity-60"> · </span>
                  <AlbumLink
                    album={nowCover.track.album}
                    albumId={nowCover.track.albumId}
                    onNavigate={onClose}
                  />
                {/if}
              </p>
            </div>
            {#if !cur.path.startsWith('http://') && !cur.path.startsWith('https://')}
              <FavoriteButton path={cur.path} title={cur.title} size="size-5" light />
            {/if}
          </div>
          {#if !hasLyrics}
            <p class="mt-4 text-sm text-white/50">
              {lyrics.loading ? t('Looking for lyrics…') : lyrics.result?.kind === 'instrumental' ? t('♪ Instrumental') : t('No lyrics for this song.')}
            </p>
          {/if}
        </div>
      </div>
      {#if hasLyrics}
        <LyricLines
          big
          dim={tint.dim}
          class="h-full pr-4"
          scrollClass="[mask-image:linear-gradient(to_bottom,transparent,black_6%,black_88%,transparent)]"
        />
      {/if}
    </div>
  {:else}
    <div class="flex flex-1 items-center justify-center text-white/60">{t("Nothing playing.")}</div>
  {/if}
</div>
