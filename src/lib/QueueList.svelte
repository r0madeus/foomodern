<script lang="ts">
  import ArtistLinks from './ArtistLinks.svelte'
  import Equalizer from './Equalizer.svelte'
  import { t } from './i18n.svelte'
  import Icon from './Icon.svelte'
  import { library, type TrackRow } from './library.svelte'
  import { player, type TrackInfo } from './player.svelte'
  import { trackMenu } from './contextMenu.svelte'

  // Plain list for now; the virtualised list arrives with the library view
  // (roadmap step 4).
  const current = $derived(player.snapshot.currentIndex)
  const status = $derived(player.snapshot.status)

  function fallbackRow(track: TrackInfo): TrackRow {
    return {
      id: -1,
      title: track.title,
      artist: track.artist,
      album: '',
      albumId: null,
      trackNo: null,
      discNo: null,
      year: null,
      durationMs: null,
      path: track.path,
      cover: null,
    }
  }

  async function openMenu(e: MouseEvent, track: TrackInfo, index: number) {
    e.preventDefault()
    e.stopPropagation()
    const x = e.clientX
    const y = e.clientY
    const row = (await library.trackByPath(track.path)) ?? fallbackRow(track)
    trackMenu.open({
      track: row,
      x,
      y,
      onPlay: () => void player.playIndex(index),
      onRemoveFromQueue: () => void player.removeIndex(index),
    })
  }
</script>

<ol class="divide-y divide-zinc-100 dark:divide-zinc-800/70">
  {#each player.queue as track, i (track.id)}
    <li
      class={[
        'group flex h-11 items-center gap-3 px-5 text-sm',
        i === current
          ? 'bg-teal-50 text-teal-900 dark:bg-teal-950/40 dark:text-teal-100'
          : 'text-zinc-700 hover:bg-zinc-50 dark:text-zinc-300 dark:hover:bg-zinc-800/50',
      ]}
      ondblclick={() => player.playIndex(i)}
      oncontextmenu={(e) => void openMenu(e, track, i)}
    >
      <span class="w-8 shrink-0 text-right text-xs tabular-nums text-zinc-400">
        {#if i === current && status === 'playing'}
          <Equalizer class="text-teal-600 dark:text-teal-400" />
        {:else}
          {i + 1}
        {/if}
      </span>
      <div class="flex min-w-0 flex-1 items-baseline gap-2 truncate">
        <button class="truncate text-left font-medium hover:underline" onclick={() => player.playIndex(i)} title={track.path}>
          {track.title}
        </button>
        {#if track.artist}
          <span class="truncate text-xs text-zinc-400"><ArtistLinks artist={track.artist} /></span>
        {:else}
          <span class="truncate text-xs text-zinc-400">{track.path}</span>
        {/if}
      </div>
      <button
        class="invisible rounded p-1 text-zinc-400 hover:bg-zinc-200 hover:text-zinc-700 group-hover:visible dark:hover:bg-zinc-700 dark:hover:text-zinc-100"
        onclick={() => player.removeIndex(i)}
        aria-label={t("Remove from queue")}
      >
        <Icon name="close" class="size-4" />
      </button>
    </li>
  {/each}
</ol>
