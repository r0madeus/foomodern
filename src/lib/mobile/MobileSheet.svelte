<script lang="ts">
  import type { Snippet } from 'svelte'
  import { cubicOut } from 'svelte/easing'
  import { fly } from 'svelte/transition'
  import { motionDuration, viewportWidth } from '../motion'

  // Container for pushed mobile sheets: horizontal slide transition plus a
  // left-edge swipe-back gesture matching native Android/iOS navigation.
  let { onBack, children }: { onBack: () => void; children: Snippet } = $props()

  const EDGE_PX = 28
  const TRIGGER_PX = 90

  let pullX = $state(0)
  let tracking = false
  let locked = false
  let startX = 0
  let startY = 0

  function onTouchStart(e: TouchEvent) {
    if (e.touches.length !== 1) return
    const t = e.touches[0]
    if (t.clientX > EDGE_PX) return
    if ((e.target as HTMLElement | null)?.closest('input')) return
    tracking = true
    locked = false
    startX = t.clientX
    startY = t.clientY
  }

  function onTouchMove(e: TouchEvent) {
    if (!tracking || e.touches.length !== 1) return
    const dx = e.touches[0].clientX - startX
    const dy = e.touches[0].clientY - startY
    if (!locked) {
      if (Math.abs(dx) < 8 && Math.abs(dy) < 8) return
      if (dx <= 0 || Math.abs(dy) > dx * 0.75) {
        tracking = false
        pullX = 0
        return
      }
      locked = true
    }
    pullX = Math.max(0, dx)
  }

  function onTouchEnd() {
    if (!tracking) return
    tracking = false
    locked = false
    if (pullX > TRIGGER_PX) {
      onBack()
      return
    }
    pullX = 0
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="absolute inset-0 bg-zinc-950 shadow-2xl shadow-black/50"
  style:transform={pullX > 0 ? `translateX(${pullX}px)` : undefined}
  style:transition={pullX > 0 ? 'none' : 'transform 200ms ease-out'}
  in:fly={{ x: viewportWidth(), duration: motionDuration(230), opacity: 1, easing: cubicOut }}
  out:fly={{ x: viewportWidth(), duration: motionDuration(200), opacity: 1 }}
  ontouchstart={onTouchStart}
  ontouchmove={onTouchMove}
  ontouchend={onTouchEnd}
  ontouchcancel={() => {
    tracking = false
    locked = false
    pullX = 0
  }}
>
  {@render children()}
</div>
