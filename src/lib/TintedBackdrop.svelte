<script lang="ts">
  // Cross-fades gradient or solid backgrounds on the GPU compositor (`opacity`)
  // instead of animating CSS `background`, avoiding per-frame gradient repaints.
  let { background, duration = 650 }: { background: string; duration?: number } = $props()

  let bottomBg = $state('')
  let topBg = $state('')
  let showTop = $state(false)
  let initialized = false

  $effect(() => {
    const next = background
    if (!initialized) {
      bottomBg = next
      topBg = next
      initialized = true
      return
    }
    if (!showTop) {
      if (next !== bottomBg) {
        topBg = next
        showTop = true
      }
    } else if (next !== topBg) {
      bottomBg = next
      showTop = false
    }
  })
</script>

<div class="pointer-events-none absolute inset-0 -z-10 overflow-hidden rounded-[inherit]" aria-hidden="true">
  <div class="absolute inset-0" style:background={bottomBg}></div>
  <div
    class="absolute inset-0"
    style:background={topBg}
    style:opacity={showTop ? 1 : 0}
    style:transition="opacity {duration}ms ease"
  ></div>
</div>
