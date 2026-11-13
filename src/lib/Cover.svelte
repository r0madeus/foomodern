<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core'

  let { cover, title, class: className = '' }: { cover: string | null; title: string; class?: string } = $props()

  const initials = $derived(
    title
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((w) => Array.from(w)[0])
      .join('')
      .toUpperCase() || '♪',
  )
  // Deterministic tint from the title.
  const hue = $derived.by(() => {
    let h = 0
    for (const ch of title) h = (h * 31 + ch.codePointAt(0)!) % 360
    return h
  })
  let failed = $state(false)
  $effect(() => {
    void cover
    failed = false
  })
</script>

{#if cover && !failed}
  <img
    src={cover.startsWith('http://') || cover.startsWith('https://') ? cover : convertFileSrc(cover)}
    alt="Cover of {title}"
    loading="lazy"
    class={['bg-zinc-200 object-cover dark:bg-zinc-800', className]}
    onerror={() => (failed = true)}
  />
{:else}
  <div
    class={['flex items-center justify-center font-semibold text-white/90 select-none', className]}
    style:background="hsl({hue} 35% 42%)"
    role="img"
    aria-label="No cover for {title}"
  >
    {initials}
  </div>
{/if}
