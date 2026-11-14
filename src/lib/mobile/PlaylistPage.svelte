<script lang="ts">
  import { count, t } from '../i18n.svelte'
  import Icon from '../Icon.svelte'
  import PlaylistCover from '../PlaylistCover.svelte'
  import PlaylistCoverModal from '../PlaylistCoverModal.svelte'
  import PageHeader from './PageHeader.svelte'
  import TrackItem from './TrackItem.svelte'
  import { playlists } from '../playlists.svelte'
  import { player } from '../player.svelte'
  import type { TrackRow } from '../library.svelte'

  let { id, onBack }: { id: number; onBack: () => void } = $props()

  const info = $derived(playlists.byId(id))
  let tracks = $state<TrackRow[]>([])
  let editingCover = $state(false)

  $effect(() => {
    void playlists.version
    const count = info?.trackCount ?? 0
    let stale = false
    void playlists.tracks(id, count).then((rows) => {
      if (!stale) tracks = rows
    })
    return () => (stale = true)
  })

  async function shufflePlay() {
    if (tracks.length === 0) return
    await player.setShuffle(true)
    await playlists.play(id, Math.floor(Math.random() * tracks.length))
  }
</script>

<div class="absolute inset-0 flex flex-col bg-zinc-950">
  <PageHeader title={info?.name ?? t('Playlist')} {onBack}>
    {#snippet actions()}
      {#if info}
        <button class="rounded-full px-3 py-2 text-sm font-medium text-zinc-200 active:bg-white/10" onclick={() => info && void playlists.exportM3u(info)}>
          {t("Export")}
        </button>
      {/if}
    {/snippet}
  </PageHeader>
  <div class="min-h-0 flex-1 overflow-y-auto pb-[var(--mobile-bar-clear,5rem)]">
    {#if info}
      <div class="flex flex-col items-center px-4 pt-2 pb-4">
        <button
          type="button"
          class="group relative aspect-square w-1/2 max-w-56 overflow-hidden rounded-xl shadow-2xl shadow-black/60 active:scale-[0.98]"
          aria-label={t('Playlist cover')}
          onclick={() => (editingCover = true)}
        >
          <PlaylistCover
            cover={info.cover}
            albumCovers={info.albumCovers}
            name={info.name}
            class="size-full rounded-xl text-5xl"
            iconClass="size-20"
          />
          <span class="absolute right-2 bottom-2 flex items-center gap-1 rounded-full bg-black/65 px-2.5 py-1 text-[11px] font-medium text-white backdrop-blur-xs">
            <Icon name="image" class="size-3.5" />
            {t('Cover')}
          </span>
        </button>
        <h2 class="mt-4 self-start text-2xl font-bold">{info.name}</h2>
        <p class="self-start text-xs text-zinc-400">
          {t('Playlist · {count}', { count: count(info.trackCount, '{n} song', '{n} songs') })}
        </p>
      </div>
      {#if tracks.length > 0}
        <div class="flex items-center justify-end gap-2 px-4 pb-2">
          <button class="rounded-full p-3 text-zinc-300 active:bg-white/10" aria-label={t("Shuffle play")} onclick={shufflePlay}>
            <Icon name="shuffle" class="size-6" />
          </button>
          <button
            class="flex size-14 items-center justify-center rounded-full bg-teal-500 text-zinc-950 shadow-lg active:scale-95"
            aria-label={t("Play")}
            onclick={() => playlists.play(id, 0)}
          >
            <Icon name="play" class="size-7" />
          </button>
        </div>
      {:else}
        <p class="px-8 pt-6 text-center text-sm text-zinc-400">
          {t("This playlist is empty. Use the ⋯ menu on a song or album to add tracks.")}
        </p>
      {/if}
      {#each tracks as track, i (`${i}:${track.id}`)}
        <TrackItem
          title={track.title}
          subtitle={[track.artist, track.album].filter(Boolean).join(' · ')}
          path={track.path}
          cover={track.cover}
          trackId={track.id}
          onPlay={() => playlists.play(id, i)}
        />
      {/each}
    {/if}
  </div>
</div>

{#if editingCover && info}
  <PlaylistCoverModal playlist={info} onClose={() => (editingCover = false)} />
{/if}
