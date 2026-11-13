<script lang="ts">
  import { fade } from 'svelte/transition'
  import { lyrics, findActiveIndex } from './lyrics.svelte'
  import { player } from './player.svelte'

  // The one lyric line being sung, under the cover. Before the first line
  // and during instrumental breaks the next sung line is previewed, faded.
  // The box keeps a fixed height so the controls below never jump.
  const res = $derived(lyrics.result)
  const line = $derived.by(() => {
    if (res?.kind !== 'synced') return null
    const lines = res.lines
    const at = findActiveIndex(lines, player.position)
    if (at >= 0 && lines[at].text.trim() !== '') return { key: at, text: lines[at].text, next: false }
    for (let j = at + 1; j < lines.length; j++) {
      if (lines[j].text.trim() !== '') return { key: j, text: lines[j].text, next: true }
    }
    return null
  })
</script>

<div class="relative h-14 overflow-hidden" aria-live="off">
  {#key line?.key ?? -1}
    {#if line}
      <p
        class={['absolute inset-x-0 top-1/2 line-clamp-2 -translate-y-1/2 text-[17px] leading-snug font-bold', line.next ? 'text-white/45' : 'text-white']}
        in:fade={{ duration: 220, delay: 80 }}
        out:fade={{ duration: 120 }}
      >
        {line.text}
      </p>
    {/if}
  {/key}
</div>
