<script lang="ts">
  import type { Snippet } from 'svelte'

  // Fixed-row-height virtual list: only the visible window (plus overscan)
  // is in the DOM, inside a spacer sized for all rows.
  let {
    count,
    rowHeight,
    overscan = 6,
    row,
    onRangeChange,
    class: className = '',
  }: {
    count: number
    rowHeight: number
    overscan?: number
    row: Snippet<[index: number]>
    onRangeChange?: (start: number, end: number) => void
    class?: string
  } = $props()

  let scrollTop = $state(0)
  let clientHeight = $state(0)

  const start = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - overscan))
  const end = $derived(Math.min(count, Math.ceil((scrollTop + clientHeight) / rowHeight) + overscan))
  const indexes = $derived(Array.from({ length: Math.max(0, end - start) }, (_, i) => start + i))

  $effect(() => {
    onRangeChange?.(start, end)
  })
</script>

<div
  class={['overflow-y-auto', className]}
  bind:clientHeight
  onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
>
  <div class="relative" style:height="{count * rowHeight}px">
    <div class="absolute inset-x-0 top-0" style:transform="translateY({start * rowHeight}px)">
      {#each indexes as i (i)}
        <div style:height="{rowHeight}px">{@render row(i)}</div>
      {/each}
    </div>
  </div>
</div>
