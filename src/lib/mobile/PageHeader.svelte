<script lang="ts">
  import { t } from '../i18n.svelte'
  import type { Snippet } from 'svelte'
  import Icon from '../Icon.svelte'

  // Big tab title, or a back button plus a small title on pushed screens.
  let { title, onBack, actions }: { title: string; onBack?: () => void; actions?: Snippet } = $props()
</script>

{#if onBack}
  <header class="flex h-14 shrink-0 items-center gap-1 px-1">
    <button class="rounded-full p-3 text-zinc-100 active:bg-white/10" aria-label={t("Back")} onclick={onBack}>
      <Icon name="back" class="size-6" />
    </button>
    <h1 class="min-w-0 flex-1 truncate text-base font-semibold">{title}</h1>
    {@render actions?.()}
  </header>
{:else}
  <header class="flex shrink-0 items-center justify-between gap-3 px-4 pt-4 pb-3">
    <h1 class="truncate text-2xl font-bold tracking-tight">{title}</h1>
    {@render actions?.()}
  </header>
{/if}
