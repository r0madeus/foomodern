<script lang="ts">
  import { t } from '../i18n.svelte'
  import Cover from '../Cover.svelte'
  import Icon from '../Icon.svelte'
  import { player } from '../player.svelte'
  import { nowCover } from '../nowCover.svelte'
  import { tintFrom } from '../tint'

  // Floating bar above the tabs; tapping it opens the full player.
  let { onOpen }: { onOpen: () => void } = $props()

  const snap = $derived(player.snapshot)
  const progress = $derived(
    snap.durationSeconds ? Math.min(100, (player.position / snap.durationSeconds) * 100) : 0,
  )
</script>

{#if snap.current}
  <div
    class="relative mx-2 mb-1.5 overflow-hidden rounded-lg shadow-lg shadow-black/40 transition-[background] duration-500"
    style:background={tintFrom(nowCover.color).surface}
  >
    <div class="flex h-14 items-center gap-3 pr-1 pl-2">
      <button class="flex min-w-0 flex-1 items-center gap-3 text-left" onclick={onOpen} aria-label={t("Open player")}>
        <Cover cover={nowCover.cover} title={snap.current.title} class="size-10 shrink-0 rounded text-xs" />
        <span class="min-w-0 flex-1">
          <span class="block truncate text-sm font-semibold text-zinc-50">{snap.current.title}</span>
          <span class="block truncate text-xs text-zinc-400">{snap.current.artist || t('Unknown artist')}</span>
        </span>
      </button>
      <button class="rounded-full p-3 text-zinc-50 active:bg-white/10" aria-label={t(snap.status === 'playing' ? 'Pause' : 'Play')} onclick={player.togglePause}>
        <Icon name={snap.status === 'playing' ? 'pause' : 'play'} class="size-7" />
      </button>
    </div>
    <div class="absolute inset-x-2 bottom-0 h-0.5 rounded-full bg-white/20">
      <div class="h-full rounded-full bg-white" style:width="{progress}%"></div>
    </div>
  </div>
{/if}
