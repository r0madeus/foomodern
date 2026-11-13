<script lang="ts">
  import { t } from '../i18n.svelte'
  import { fly } from 'svelte/transition'
  import Icon from '../Icon.svelte'
  import { player } from '../player.svelte'

  let { onClose }: { onClose: () => void } = $props()

  const current = $derived(player.snapshot.currentIndex)
  const upNext = $derived(
    current === null
      ? player.queue.map((track, i) => ({ track, i }))
      : player.queue.map((track, i) => ({ track, i })).slice(current + 1),
  )
</script>

<div
  class="fixed inset-0 z-50 flex flex-col bg-zinc-950 text-white"
  style:padding-top="var(--inset-top)"
  style:padding-bottom="var(--inset-bottom)"
  transition:fly={{ y: 600, duration: 220, opacity: 1 }}
  role="dialog"
  aria-modal="true"
  aria-label={t("Queue")}
>
  <header class="flex h-14 shrink-0 items-center px-1">
    <button class="rounded-full p-3 active:bg-white/10" aria-label={t("Close queue")} onclick={onClose}>
      <Icon name="down" class="size-7" />
    </button>
    <h1 class="flex-1 text-base font-semibold">{t("Queue")}</h1>
  </header>
  <div class="min-h-0 flex-1 overflow-y-auto pb-4">
    {#if current !== null && player.queue[current]}
      <h2 class="px-4 pt-2 pb-1 text-sm font-bold">{t("Now playing")}</h2>
      <div class="flex h-14 items-center px-4">
        <span class="min-w-0">
          <span class="block truncate text-[15px] text-teal-400">{player.queue[current].title}</span>
          <span class="block truncate text-[13px] text-zinc-400">{player.queue[current].artist}</span>
        </span>
      </div>
    {/if}
    {#if upNext.length > 0}
      <h2 class="px-4 pt-4 pb-1 text-sm font-bold">{t("Next up")}</h2>
      {#each upNext as { track, i } (track.id)}
        <div class="flex h-14 items-center active:bg-white/5">
          <button class="min-w-0 flex-1 px-4 text-left" onclick={() => player.playIndex(i)}>
            <span class="block truncate text-[15px]">{track.title}</span>
            <span class="block truncate text-[13px] text-zinc-400">{track.artist}</span>
          </button>
          <button class="p-4 text-zinc-500 active:text-zinc-200" aria-label={t('Remove “{title}” from queue', { title: track.title })} onclick={() => player.removeIndex(i)}>
            <Icon name="close" class="size-5" />
          </button>
        </div>
      {/each}
    {:else}
      <p class="px-4 pt-6 text-sm text-zinc-400">{t("Nothing after this song.")}</p>
    {/if}
  </div>
</div>
