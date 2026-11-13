<script lang="ts">
  import { artists, splitArtistNames } from './artists.svelte'

  let {
    artist,
    class: className = '',
    linkClass = 'hover:text-teal-600 hover:underline dark:hover:text-teal-400',
    onNavigate,
  }: {
    artist: string
    class?: string
    linkClass?: string
    onNavigate?: (name: string) => void
  } = $props()

  const names = $derived(splitArtistNames(artist))

  function open(e: MouseEvent, name: string) {
    e.stopPropagation()
    onNavigate?.(name)
    artists.open(name)
  }
</script>

{#if names.length === 0}
  <span class={className}>—</span>
{:else}
  <span class={className}>
    {#each names as name, i (`${name}:${i}`)}
      {#if i > 0}<span class="opacity-60">, </span>{/if}
      <button
        type="button"
        class={['inline text-left outline-none focus-visible:underline', linkClass]}
        onclick={(e) => open(e, name)}
      >
        {name}
      </button>
    {/each}
  </span>
{/if}
