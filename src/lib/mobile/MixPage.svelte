<script lang="ts">
  import Icon from '../Icon.svelte'
  import PlaylistCover from '../PlaylistCover.svelte'
  import PageHeader from './PageHeader.svelte'
  import TrackItem from './TrackItem.svelte'
  import { count, t } from '../i18n.svelte'
  import {
    home,
    mixSubtitle,
    mixTitle,
    type HomeMixDetail,
  } from '../home.svelte'

  let { id, onBack }: { id: string; onBack: () => void } = $props()

  let detail = $state<HomeMixDetail | null>(null)
  let loading = $state(true)
  let saving = $state(false)

  let loadSeq = 0
  async function load(target: string) {
    const seq = ++loadSeq
    loading = true
    let res: HomeMixDetail | null = null
    try {
      res = await home.mixDetail(target)
    } catch {
      res = null
    }
    if (seq !== loadSeq) return
    detail = res
    loading = false
  }

  // Loaded when the page opens or the mix changes, not on every home refresh,
  // so the list does not reorder while playing from it.
  $effect(() => {
    void load(id)
  })

  async function play(startPath: string | null, shuffle?: boolean) {
    if (!detail) return
    const ok = await home.playMix(detail.mix.id, startPath, shuffle)
    if (!ok) await load(detail.mix.id)
  }

  async function savePlaylist() {
    if (!detail || saving) return
    saving = true
    try {
      await home.saveMixAsPlaylist(detail.mix.id, mixTitle(detail.mix))
    } finally {
      saving = false
    }
  }
</script>

<div class="absolute inset-0 flex flex-col bg-zinc-950">
  <PageHeader title={detail ? mixTitle(detail.mix) : t('Dynamic mix')} {onBack} />
  {#if loading && !detail}
    <div class="flex flex-1 items-center justify-center">
      <div class="size-8 animate-spin rounded-full border-2 border-zinc-700 border-t-teal-400"></div>
    </div>
  {:else if !detail}
    <div class="flex flex-1 items-center justify-center px-8 text-center text-sm text-zinc-400">
      {t('Mix could not be loaded.')}
    </div>
  {:else}
    {@const title = mixTitle(detail.mix)}
    <div class="min-h-0 flex-1 overflow-y-auto pb-[var(--mobile-bar-clear,5rem)]">
      <div class="flex flex-col items-center px-4 pt-2 pb-4">
        <PlaylistCover
          cover={null}
          albumCovers={detail.mix.albumCovers}
          name={title}
          class="aspect-square w-1/2 max-w-52 rounded-xl shadow-2xl shadow-black/60"
          iconClass="size-20"
        />
        <h2 class="mt-4 self-start text-2xl font-bold">{title}</h2>
        <p class="mt-1 self-start text-xs text-zinc-400">{mixSubtitle(detail.mix)}</p>
        <p class="mt-1 self-start text-xs text-zinc-500">
          {count(detail.tracks.length, '{n} song', '{n} songs')}
        </p>
      </div>

      <div class="flex items-center justify-between gap-2 px-4 pb-3">
        <button
          type="button"
          disabled={detail.tracks.length === 0 || saving}
          class="flex items-center gap-1.5 rounded-full bg-zinc-800 px-3.5 py-2 text-xs font-medium text-zinc-100 active:bg-zinc-700 disabled:opacity-40"
          onclick={() => void savePlaylist()}
        >
          <Icon name="plus" class="size-4" />
          {t('Save as playlist')}
        </button>

        <div class="flex items-center gap-2">
          <button
            type="button"
            class="rounded-full p-3 text-zinc-300 active:bg-white/10"
            aria-label={t('Shuffle play')}
            onclick={() => void play(null, true)}
          >
            <Icon name="shuffle" class="size-6" />
          </button>
          <button
            type="button"
            class="flex size-14 items-center justify-center rounded-full bg-teal-500 text-zinc-950 shadow-lg active:scale-95"
            aria-label={t('Play')}
            onclick={() => void play(null, false)}
          >
            <Icon name="play" class="size-7" />
          </button>
        </div>
      </div>

      {#each detail.tracks as track, i (track.id)}
        <TrackItem
          title={track.title}
          subtitle={[track.artist, track.album].filter(Boolean).join(' · ')}
          path={track.path}
          cover={track.cover}
          trackId={track.id}
          {track}
          onPlay={() => void play(track.path)}
        />
      {/each}
    </div>
  {/if}
</div>
