<script lang="ts">
  import Cover from './Cover.svelte'
  import Icon from './Icon.svelte'
  import { fallbackPalette, parseCoverPreset } from './playlistCover'

  let {
    cover = null,
    albumCovers = [],
    name = '',
    class: className = 'size-44 rounded-xl',
    iconClass = 'size-16',
  }: {
    cover?: string | null
    albumCovers?: string[]
    name?: string
    class?: string
    iconClass?: string
  } = $props()

  const preset = $derived(parseCoverPreset(cover))
  const fallback = $derived(fallbackPalette(name))
  const isCustomImage = $derived(Boolean(cover && !cover.startsWith('preset:')))
</script>

{#if preset}
  <div
    class={['flex shrink-0 items-center justify-center overflow-hidden text-white shadow-sm', className]}
    style:background={preset.palette.gradient}
  >
    <Icon name={preset.icon} class={iconClass} />
  </div>
{:else if isCustomImage}
  <Cover {cover} title={name} class={className} />
{:else if albumCovers.length >= 4}
  <div class={['grid shrink-0 grid-cols-2 grid-rows-2 overflow-hidden bg-zinc-200 dark:bg-zinc-800', className]}>
    {#each albumCovers.slice(0, 4) as c, idx (`${idx}:${c}`)}
      <Cover cover={c} title={name} class="size-full rounded-none object-cover" />
    {/each}
  </div>
{:else if albumCovers.length >= 1}
  <Cover cover={albumCovers[0]} title={name} class={className} />
{:else}
  <div
    class={['flex shrink-0 items-center justify-center overflow-hidden text-white shadow-sm', className]}
    style:background={fallback.gradient}
  >
    <Icon name="music" class={iconClass} />
  </div>
{/if}
