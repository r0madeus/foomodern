<script lang="ts">
  import Icon from './Icon.svelte'
  import { favorites } from './favorites.svelte'
  import { t } from './i18n.svelte'

  let {
    trackId = null,
    path = null,
    title = '',
    size = 'size-4',
    light = false,
    revealOnHover = false,
  }: {
    trackId?: number | null
    path?: string | null
    title?: string
    size?: string
    light?: boolean
    /** Hide inactive star until the row is hovered or focused. */
    revealOnHover?: boolean
  } = $props()

  const active = $derived(
    trackId !== null ? favorites.has(trackId) : favorites.hasPath(path),
  )
  const label = $derived(
    active
      ? t('Remove “{title}” from favorites', { title })
      : t('Add “{title}” to favorites', { title }),
  )

  let popping = $state(false)
  let popTimer: ReturnType<typeof setTimeout> | undefined

  function toggle(e: MouseEvent) {
    e.stopPropagation()
    if (!active) {
      popping = false
      clearTimeout(popTimer)
      queueMicrotask(() => {
        popping = true
        popTimer = setTimeout(() => (popping = false), 300)
      })
    }
    if (trackId !== null) void favorites.toggle(trackId)
    else if (path) void favorites.toggleByPath(path)
  }
</script>

<button
  type="button"
  aria-pressed={active}
  aria-label={label}
  title={label}
  class={[
    'shrink-0 rounded p-1 outline-none transition focus-visible:ring-2 focus-visible:ring-accent-500 active:scale-90',
    active
      ? light
        ? 'text-accent-300 opacity-100 hover:bg-white/10'
        : 'text-accent-500 opacity-100 hover:bg-zinc-200/70 dark:text-accent-400 dark:hover:bg-zinc-800'
      : [
          light
            ? 'text-white/60 hover:bg-white/10 hover:text-white'
            : 'text-zinc-400 hover:bg-zinc-200/70 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-100',
          revealOnHover && 'opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 pointer-coarse:opacity-100',
        ],
  ]}
  onclick={toggle}
  ondblclick={(e) => e.stopPropagation()}
  onkeydown={(e) => e.stopPropagation()}
>
  <Icon name={active ? 'star' : 'starOutline'} class={popping ? `${size} animate-star-pop` : size} />
</button>
