<script lang="ts">
  import { t } from '../i18n.svelte'
  import Cover from '../Cover.svelte'
  import FavoriteButton from '../FavoriteButton.svelte'
  import AddToPlaylist from '../AddToPlaylist.svelte'
  import { player } from '../player.svelte'

  // One song row on phones: cover, title, artist · album, and a menu.
  let {
    title,
    subtitle,
    path,
    cover = null,
    trackId = null,
    number = null,
    onPlay,
  }: {
    title: string
    subtitle: string
    path: string
    cover?: string | null
    trackId?: number | null
    /** Show a track number instead of the cover (album pages). */
    number?: number | null
    onPlay: () => void
  } = $props()

  const current = $derived(player.snapshot.current?.path === path)
  const playing = $derived(current && player.snapshot.status === 'playing')
</script>

<div class="flex h-16 items-center gap-3 pr-1 pl-4 active:bg-white/5">
  <button class="flex min-w-0 flex-1 items-center gap-3 text-left" onclick={onPlay}>
    {#if number !== null}
      <span class={['w-6 shrink-0 text-center text-sm tabular-nums', current ? 'text-teal-400' : 'text-zinc-500']}>
        {#if playing}<span aria-label={t("Playing")}>▶</span>{:else}{number}{/if}
      </span>
    {:else}
      <span class="relative shrink-0">
        <Cover {cover} {title} class="size-12 rounded text-sm" />
        {#if playing}
          <span class="absolute inset-0 flex items-center justify-center rounded bg-black/50 text-teal-300" aria-label={t("Playing")}>▶</span>
        {/if}
      </span>
    {/if}
    <span class="min-w-0 flex-1">
      <span class={['block truncate text-[15px] leading-tight', current ? 'text-teal-400' : 'text-zinc-50']}>{title}</span>
      <span class="mt-1 block truncate text-[13px] leading-tight text-zinc-400">{subtitle}</span>
    </span>
  </button>
  {#if trackId !== null}
    <FavoriteButton {trackId} {title} size="size-5" light />
    <AddToPlaylist trackIds={[trackId]} editTags label={t('More actions for “{title}”', { title })} class="p-3 text-zinc-400" />
  {/if}
</div>
