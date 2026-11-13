<script lang="ts">
  import PageHeader from './PageHeader.svelte'
  import StatsView from '../StatsView.svelte'
  import { t } from '../i18n.svelte'

  let {
    onBack,
    onOpenAlbum,
  }: {
    onBack: () => void
    onOpenAlbum: (albumId: number) => void
  } = $props()
</script>

<div class="absolute inset-0 flex flex-col bg-zinc-950">
  <PageHeader title={t('Listening stats')} {onBack} />
  <div class="min-h-0 flex-1 overflow-y-auto pt-2">
    <StatsView {onOpenAlbum} />
  </div>
</div>
