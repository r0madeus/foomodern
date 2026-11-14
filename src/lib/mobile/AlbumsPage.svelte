<script lang="ts">
  import EmptyLibrary from '../EmptyLibrary.svelte'
  import { t } from '../i18n.svelte'
  import { library } from '../library.svelte'
  import AlbumGrid from './AlbumGrid.svelte'
  import PageHeader from './PageHeader.svelte'

  let {
    onBack,
    onOpenAlbum,
  }: {
    onBack: () => void
    onOpenAlbum: (albumId: number) => void
  } = $props()
</script>

<div class="absolute inset-0 flex flex-col bg-zinc-950">
  <PageHeader title={t('Albums')} {onBack} />
  {#if library.stats.albums === 0}
    <EmptyLibrary />
  {:else}
    <AlbumGrid onOpen={onOpenAlbum} />
  {/if}
</div>
