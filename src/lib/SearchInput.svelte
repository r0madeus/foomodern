<script lang="ts">
  import { t } from './i18n.svelte'
  import Icon from './Icon.svelte'
  import { library } from './library.svelte'

  let {
    value = $bindable<string | undefined>(undefined),
    placeholder = 'Filter…',
    label,
  }: { value?: string; placeholder?: string; label?: string } = $props()

  const text = $derived(value ?? library.query)
  function update(next: string) {
    if (value !== undefined) value = next
    else library.setQuery(next)
  }
</script>

<div class="relative w-64 max-w-full">
  <Icon
    name="search"
    class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-zinc-400"
  />
  <input
    type="search"
    class="w-full rounded-md border border-zinc-200 bg-white py-1 pr-7 pl-8 text-sm text-zinc-800 placeholder:text-zinc-400 focus:border-teal-500 focus:outline-none dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100"
    value={text}
    placeholder={t(placeholder)}
    aria-label={label ?? t('Filter library')}
    oninput={(e) => update(e.currentTarget.value)}
    onkeydown={(e) => {
      if (e.key === 'Escape' && text !== '') {
        e.stopPropagation()
        update('')
      }
    }}
  />
  {#if text !== ''}
    <button
      type="button"
      class="absolute top-1/2 right-1.5 -translate-y-1/2 rounded p-0.5 text-zinc-400 hover:bg-zinc-200/70 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-200"
      onclick={() => update('')}
      aria-label={t("Clear search")}
      title={t("Clear search")}
    >
      <Icon name="close" class="size-3.5" />
    </button>
  {/if}
</div>
