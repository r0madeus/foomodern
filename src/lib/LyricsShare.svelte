<script lang="ts">
  import { untrack } from 'svelte'
  import { t } from './i18n.svelte'
  import { isOwnLyrics, lyrics, type ShareBlock } from './lyrics.svelte'
  import { player } from './player.svelte'

  // "Share on LRCLIB" for lyrics the user brought themselves (a .lrc file or
  // the song's tag) when LRCLIB has nothing for the track. Publishing is
  // public, so it asks first. Rust does the work (proof-of-work, can take a
  // few minutes) and can be cancelled.
  let { class: className = '' }: { class?: string } = $props()

  const REASONS: Record<ShareBlock, string> = {
    noLocalLyrics: 'No lyrics of your own to share.',
    needsMetadata: 'Needs title, artist, album and duration in the tags.',
    onLrclib: 'LRCLIB already has these lyrics.',
    otherSource: 'These lyrics came from another service.',
    offline: 'Could not reach LRCLIB.',
    busy: 'Another share is running.',
  }

  const current = $derived(player.snapshot.current)
  const duration = $derived(player.snapshot.durationSeconds)
  const own = $derived(isOwnLyrics(lyrics.result))
  const here = $derived(current !== null && lyrics.shareFor === current.path)
  const status = $derived(here ? lyrics.shareStatus : null)
  const phase = $derived(here ? lyrics.sharePhase : 'idle')
  let confirming = $state(false)

  // Check once per track and lyrics shown (not while the lookup is running).
  $effect(() => {
    const path = current?.path
    if (!path || !own || lyrics.loading || lyrics.trackPath !== path) return
    untrack(() => {
      confirming = false
      void lyrics.checkShare(path, duration)
    })
  })

  const visible = $derived(
    own &&
      (phase !== 'idle' ||
        status?.available === true ||
        status?.reason === 'needsMetadata'),
  )

  const link =
    'rounded-full px-3 py-1.5 text-xs font-semibold text-white/80 hover:bg-white/10 active:bg-white/10 disabled:cursor-not-allowed disabled:opacity-50'
</script>

{#if visible && current}
  <div class={['flex flex-col gap-1.5 text-xs text-white/80', className]} role="group" aria-label={t('Share on LRCLIB')}>
    {#if phase === 'sharing'}
      <p role="status">{t('Sharing… (this can take a few minutes)')}</p>
      <div>
        <button class={link} onclick={() => void lyrics.cancelShare()}>{t('Cancel')}</button>
      </div>
    {:else if confirming}
      <p>{t('Your lyrics will be published publicly on lrclib.net. Sending takes a few minutes of processor work (LRCLIB requires it against spam).')}</p>
      <div class="flex items-center gap-2">
        <button
          class="rounded-full bg-white px-3 py-1.5 text-xs font-semibold text-zinc-950 hover:bg-white/90"
          onclick={() => {
            confirming = false
            void lyrics.share(current.path, duration)
          }}
        >
          {t('Publish')}
        </button>
        <button class={link} onclick={() => (confirming = false)}>{t('Cancel')}</button>
      </div>
    {:else}
      {#if phase === 'done'}
        <p role="status">{t('Shared on LRCLIB. Thank you!')}</p>
      {:else if phase === 'cancelled'}
        <p role="status">{t('Sharing cancelled.')}</p>
      {:else if phase === 'failed'}
        <p role="alert" class="text-red-200">{t('Could not share: {error}', { error: lyrics.shareError ?? '' })}</p>
      {/if}
      {#if status?.available}
        <div>
          <button class={link} onclick={() => (confirming = true)}>{t('Share on LRCLIB')}</button>
        </div>
      {:else if status?.reason === 'needsMetadata'}
        <div>
          <button class={link} disabled title={t(REASONS.needsMetadata)}>{t('Share on LRCLIB')}</button>
          <span class="ml-1 text-white/60">{t(REASONS.needsMetadata)}</span>
        </div>
      {/if}
    {/if}
  </div>
{/if}
