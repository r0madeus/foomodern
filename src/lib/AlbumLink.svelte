<script lang="ts">
  import { library } from './library.svelte'

  let {
    album,
    albumId,
    class: className = '',
    linkClass = 'hover:text-teal-600 hover:underline dark:hover:text-teal-400',
    onNavigate,
  }: {
    album: string
    albumId: number | null | undefined
    class?: string
    linkClass?: string
    onNavigate?: (albumId: number) => void
  } = $props()

  function open(e: MouseEvent) {
    if (albumId === null || albumId === undefined) return
    e.stopPropagation()
    onNavigate?.(albumId)
    library.openAlbum(albumId)
  }
</script>

{#if !album}
  <span class={className}>—</span>
{:else if albumId === null || albumId === undefined}
  <span class={className}>{album}</span>
{:else}
  <span class={className}>
    <button
      type="button"
      class={['inline text-left outline-none focus-visible:underline', linkClass]}
      onclick={open}
    >
      {album}
    </button>
  </span>
{/if}
