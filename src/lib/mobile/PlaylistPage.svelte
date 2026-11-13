<script lang="ts">
  import Icon from '../Icon.svelte'
  import Cover from '../Cover.svelte'
  import PageHeader from './PageHeader.svelte'
  import TrackItem from './TrackItem.svelte'
  import { playlists } from '../playlists.svelte'
  import { player } from '../player.svelte'
  import type { TrackRow } from '../library.svelte'

  let { id, onBack }: { id: number; onBack: () => void } = $props()

  const info = $derived(playlists.byId(id))
  let tracks = $state<TrackRow[]>([])

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
  <PageHeader title={info?.name ?? 'Playlist'} {onBack} />
  <div class="min-h-0 flex-1 overflow-y-auto pb-6">
    {#if info}
      <div class="flex flex-col items-center px-4 pt-2 pb-4">
        <Cover cover={tracks[0]?.cover ?? null} title={info.name} class="aspect-square w-1/2 max-w-56 rounded-md text-5xl shadow-2xl shadow-black/60" />
        <h2 class="mt-4 self-start text-2xl font-bold">{info.name}</h2>
        <p class="self-start text-xs text-zinc-400">
          Playlist · {info.trackCount.toLocaleString()} {info.trackCount === 1 ? 'song' : 'songs'}
        </p>
      </div>
      {#if tracks.length > 0}
        <div class="flex items-center justify-end gap-2 px-4 pb-2">
          <button class="rounded-full p-3 text-zinc-300 active:bg-white/10" aria-label="Shuffle play" onclick={shufflePlay}>
            <Icon name="shuffle" class="size-6" />
          </button>
          <button
            class="flex size-14 items-center justify-center rounded-full bg-teal-500 text-zinc-950 shadow-lg active:scale-95"
            aria-label="Play"
            onclick={() => playlists.play(id, 0)}
          >
            <Icon name="play" class="size-7" />
          </button>
        </div>
      {:else}
        <p class="px-8 pt-6 text-center text-sm text-zinc-400">
          This playlist is empty. Use the ⋯ menu on a song or album to add tracks.
        </p>
      {/if}
      {#each tracks as t, i (`${i}:${t.id}`)}
        <TrackItem
          title={t.title}
          subtitle={[t.artist, t.album].filter(Boolean).join(' · ')}
          path={t.path}
          cover={t.cover}
          trackId={t.id}
          onPlay={() => playlists.play(id, i)}
        />
      {/each}
    {/if}
  </div>
</div>
