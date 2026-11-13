<script lang="ts">
  import { t } from './i18n.svelte'
  import { untrack } from 'svelte'
  import { lyrics, findActiveIndex } from './lyrics.svelte'
  import { player } from './player.svelte'

  // Lyrics on a tinted surface: the playing line is white, played lines are
  // faded, upcoming lines use the tint. Follows playback until the user
  // scrolls; tapping a synced line seeks there.
  let {
    big = false,
    dim,
    class: className = '',
  }: {
    /** Full-screen size instead of the card size. */
    big?: boolean
    dim: string
    class?: string
  } = $props()

  let box = $state<HTMLDivElement | null>(null)
  let follow = $state(true)

  const res = $derived(lyrics.result)
  const active = $derived(res?.kind === 'synced' ? findActiveIndex(res.lines, player.position) : -1)

  function scrollToActive(smooth: boolean) {
    if (!box || active < 0) {
      if (box && active < 0) box.scrollTo({ top: 0, behavior: smooth ? 'smooth' : 'instant' })
      return
    }
    const el = box.querySelector<HTMLElement>(`[data-i="${active}"]`)
    if (!el) return
    // Keep the playing line about a third from the top (not scrollIntoView,
    // which would also scroll the page around the card).
    box.scrollTo({ top: Math.max(0, el.offsetTop - box.clientHeight / 3), behavior: smooth ? 'smooth' : 'instant' })
  }

  $effect(() => {
    void active
    if (follow) untrack(() => scrollToActive(true))
  })

  // New track: back to the top, following again.
  $effect(() => {
    void lyrics.trackPath
    untrack(() => {
      follow = true
      box?.scrollTo({ top: 0 })
    })
  })

  // A finger or wheel on the lyrics means the user is reading: stop following.
  function watchManualScroll(el: HTMLElement) {
    const stop = () => {
      if (res?.kind === 'synced') follow = false
    }
    el.addEventListener('touchmove', stop, { passive: true })
    el.addEventListener('wheel', stop, { passive: true })
    return () => {
      el.removeEventListener('touchmove', stop)
      el.removeEventListener('wheel', stop)
    }
  }

  function resume() {
    follow = true
    scrollToActive(true)
  }
</script>

<div class={['relative min-h-0', className]}>
  <div
    bind:this={box}
    class="relative h-full overflow-y-auto overscroll-contain"
    role="region"
    aria-label={t("Lyrics")}
    {@attach watchManualScroll}
  >
    {#if !res && lyrics.loading}
      <p class="py-6 text-base font-semibold" style:color={dim}>{t("Looking for lyrics…")}</p>
    {:else if !res || res.kind === 'notFound'}
      <p class="py-6 text-base font-semibold" style:color={dim}>{t("No lyrics for this song.")}</p>
    {:else if res.kind === 'instrumental'}
      <p class="py-6 text-base font-semibold" style:color={dim}>{t("♪ Instrumental")}</p>
    {:else if res.kind === 'plain'}
      <div class={['pb-8 font-bold', big ? 'space-y-4 text-[26px] leading-snug' : 'space-y-3 text-[22px] leading-snug']}>
        {#each res.lines as line, i (i)}
          {#if line.trim() === ''}
            <div class="h-3" aria-hidden="true"></div>
          {:else}
            <p class="text-white">{line}</p>
          {/if}
        {/each}
      </div>
    {:else}
      <div class={['pb-[40vh] font-bold', big ? 'space-y-5 text-[28px] leading-tight' : 'space-y-4 text-[22px] leading-snug']}>
        {#each res.lines as line, i (i)}
          {@const blank = line.text.trim() === ''}
          <button
            data-i={i}
            class={['block w-full text-left transition-[color,opacity] duration-300', blank && 'text-base']}
            style:color={i === active ? '#fff' : i < active ? 'rgb(255 255 255 / 0.5)' : dim}
            onclick={() => {
              follow = true
              void player.seek(line.timeSeconds)
            }}
          >
            {blank ? '♪' : line.text}
          </button>
        {/each}
      </div>
    {/if}
  </div>
  {#if !follow && res?.kind === 'synced'}
    <button
      class="absolute bottom-3 left-1/2 -translate-x-1/2 rounded-full bg-white px-4 py-2 text-sm font-semibold text-zinc-950 shadow-lg"
      onclick={resume}
    >
      {t("Back to current line")}
    </button>
  {/if}
</div>
