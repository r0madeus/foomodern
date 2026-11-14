<script lang="ts">
  import { count, t } from './i18n.svelte'
  import { tick } from 'svelte'
  import { fade, scale } from 'svelte/transition'
  import { invoke } from '@tauri-apps/api/core'
  import Cover from './Cover.svelte'
  import { motionDuration } from './motion'
  import {
    metadataMatch,
    type AlbumCandidate,
    type ApplyOutcome,
    type DiffField,
    type FieldDiff,
    type Proposal,
    type TrackCandidate,
    type TrackDiff,
  } from './metadata.svelte'

  // Album and track candidates are shown the same way.
  type Candidate = {
    key: string
    primary: string
    secondary: string
    meta: string
    score: number
    label: 'recommended' | 'check'
    coverPreview: string | null
    albumFields: FieldDiff[]
    tracks: TrackDiff[]
    proposal: Proposal
  }

  const FIELD_LABEL: Record<DiffField, string> = {
    title: 'Title',
    artist: 'Artist',
    album: 'Album',
    albumArtist: 'Album artist',
    trackNo: 'Track number',
    discNo: 'Disc number',
    year: 'Year',
  }

  type Phase = 'loading' | 'error' | 'list' | 'applying' | 'applied' | 'undoing' | 'undone'

  const request = $derived(metadataMatch.request)

  let phase = $state<Phase>('loading')
  let error = $state<string | null>(null)
  let candidates = $state<Candidate[]>([])
  let selected = $state(0)
  let outcome = $state<ApplyOutcome | null>(null)
  let undoFailed = $state<[string, string][]>([])

  let dialog = $state<HTMLDivElement | null>(null)
  let previousFocus: HTMLElement | null = null

  function meta(c: { year: number | null; country: string | null; format: string | null }, tracks?: number): string {
    return [
      c.year,
      c.country,
      c.format,
      tracks ? count(tracks, '{n} track', '{n} tracks') : null,
    ]
      .filter(Boolean)
      .join(' · ')
  }

  function fromAlbum(c: AlbumCandidate): Candidate {
    return {
      key: c.releaseId,
      primary: c.title,
      secondary: c.artist,
      meta: meta(c, c.trackCount),
      score: c.score,
      label: c.label,
      coverPreview: c.coverPreview,
      albumFields: c.albumFields,
      tracks: c.tracks,
      proposal: c.proposal,
    }
  }

  function fromTrack(c: TrackCandidate): Candidate {
    return {
      key: `${c.recordingId}/${c.releaseId}`,
      primary: c.title,
      secondary: `${c.artist} · ${c.album}`,
      meta: meta(c),
      score: c.score,
      label: c.label,
      coverPreview: c.coverPreview,
      albumFields: [],
      tracks: [c.track],
      proposal: c.proposal,
    }
  }

  // Search whenever the dialog opens for something new.
  $effect(() => {
    const req = request
    if (!req) return
    let stale = false
    previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    phase = 'loading'
    error = null
    candidates = []
    selected = 0
    outcome = null
    undoFailed = []
    const search =
      req.mode === 'album'
        ? invoke<AlbumCandidate[]>('metadata_match_album', { albumId: req.albumId }).then((r) => r.map(fromAlbum))
        : invoke<TrackCandidate[]>('metadata_match_track', { trackId: req.trackId }).then((r) => r.map(fromTrack))
    search
      .then(async (list) => {
        if (stale) return
        candidates = list
        phase = 'list'
        await tick()
        dialog?.focus()
      })
      .catch((e) => {
        if (stale) return
        error = String(e)
        phase = 'error'
      })
    return () => {
      stale = true
    }
  })

  const current = $derived<Candidate | null>(candidates[selected] ?? null)

  async function apply() {
    if (!current || phase !== 'list') return
    phase = 'applying'
    error = null
    try {
      outcome = await invoke<ApplyOutcome>('metadata_apply', { proposal: current.proposal })
      phase = 'applied'
    } catch (e) {
      error = String(e)
      phase = 'list'
    }
  }

  async function undo() {
    if (!outcome || outcome.batchId === 0 || phase !== 'applied') return
    phase = 'undoing'
    error = null
    try {
      const r = await invoke<ApplyOutcome>('metadata_undo', { batchId: outcome.batchId })
      undoFailed = r.failed
      phase = 'undone'
    } catch (e) {
      error = String(e)
      phase = 'applied'
    }
  }

  function close() {
    if (phase === 'applying' || phase === 'undoing') return
    metadataMatch.close()
    const back = previousFocus
    previousFocus = null
    if (back?.isConnected) back.focus()
  }

  function fileName(path: string): string {
    return path.slice(Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\')) + 1)
  }

  function focusables(): HTMLElement[] {
    if (!dialog) return []
    return [
      ...dialog.querySelectorAll<HTMLElement>(
        'button:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ),
    ]
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      close()
    } else if (e.key === 'Tab') {
      const list = focusables()
      if (list.length === 0) return
      const first = list[0]
      const last = list[list.length - 1]
      const active = document.activeElement
      if (e.shiftKey && (active === first || !dialog?.contains(active))) {
        e.preventDefault()
        last.focus()
      } else if (!e.shiftKey && (active === last || !dialog?.contains(active))) {
        e.preventDefault()
        first.focus()
      }
    }
  }

  const errorClass = 'rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700 dark:bg-red-950/50 dark:text-red-300'
</script>

{#snippet diffLine(f: FieldDiff)}
  <li class="flex flex-wrap items-baseline gap-x-1.5 text-xs">
    <span class="w-24 shrink-0 text-zinc-500">{t(FIELD_LABEL[f.field])}</span>
    <span class="min-w-0 break-words text-zinc-500 line-through decoration-zinc-400/60">{f.old ?? '—'}</span>
    <span aria-hidden="true" class="text-zinc-400">→</span>
    <span class="min-w-0 break-words font-medium text-accent-700 dark:text-accent-300">{f.new ?? '—'}</span>
  </li>
{/snippet}

{#if request}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    transition:fade={{ duration: motionDuration(150) }}
    class="fixed inset-0 z-50 flex items-end justify-center bg-black/40 p-0 sm:items-center sm:p-4"
    onclick={(e) => e.target === e.currentTarget && close()}
  >
    <div
      bind:this={dialog}
      transition:scale={{ start: 0.96, duration: motionDuration(180) }}
      role="dialog"
      aria-modal="true"
      aria-labelledby="metadata-title"
      tabindex="-1"
      onkeydown={onKey}
      style:padding-bottom="max(1.25rem, var(--inset-bottom, 0px))"
      class="flex max-h-full w-full max-w-2xl flex-col gap-4 overflow-y-auto rounded-t-2xl border border-zinc-200 bg-white p-4 text-zinc-900 shadow-xl outline-none sm:rounded-2xl sm:p-5 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100"
    >
      <div>
        <h2 id="metadata-title" class="text-lg font-semibold tracking-tight">{request.heading}</h2>
        {#if phase === 'list'}
          <p class="text-sm text-zinc-500">
            {t('Matches from MusicBrainz. Nothing is written until you apply.')}
          </p>
        {/if}
      </div>

      {#if phase === 'loading'}
        <p class="animate-pulse text-sm text-zinc-500" role="status">{t('Searching MusicBrainz…')}</p>
        <div class="flex justify-end">
          <button
            type="button"
            class="rounded-lg px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800"
            onclick={close}
          >
            {t('Cancel')}
          </button>
        </div>
      {:else if phase === 'error'}
        <div class={errorClass} role="alert">
          <p class="font-medium">{t('MusicBrainz could not be reached')}</p>
          <p class="mt-1 text-xs break-words opacity-80">{error}</p>
        </div>
        <div class="flex justify-end">
          <button
            type="button"
            class="rounded-lg px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800"
            onclick={close}
          >
            {t('Close')}
          </button>
        </div>
      {:else if phase === 'applied' || phase === 'undoing' || phase === 'undone'}
        {#if phase === 'undone'}
          <p class="rounded-lg bg-zinc-100 px-3 py-2 text-sm dark:bg-zinc-800" role="status">
            {t('The changes were undone.')}
          </p>
        {:else if outcome && outcome.updated > 0}
          <p
            class="rounded-lg bg-accent-50 px-3 py-2 text-sm text-accent-900 dark:bg-accent-950/40 dark:text-accent-100"
            role="status"
          >
            {t('Updated tags of {count}', { count: count(outcome.updated, '{n} track', '{n} tracks') })}{outcome.cover
              ? ` · ${t('Cover added')}`
              : ''}
          </p>
        {/if}
        {#if error}
          <p class={errorClass} role="alert">{error}</p>
        {/if}
        {@const failed = phase === 'undone' ? undoFailed : (outcome?.failed ?? [])}
        {#if failed.length > 0}
            <div class="rounded-lg bg-amber-50 px-3 py-2 text-sm text-amber-900 dark:bg-amber-950/40 dark:text-amber-200" role="alert">
              <p class="font-medium">
                {t('{count} could not be edited:', { count: count(failed.length, '{n} file', '{n} files') })}
              </p>
              <ul class="mt-1 max-h-32 list-disc overflow-y-auto pl-5">
                {#each failed as [path, message] (path)}
                  <li><span class="font-medium" title={path}>{fileName(path)}</span>: {message}</li>
                {/each}
              </ul>
            </div>
        {/if}
        <div class="flex justify-end gap-2">
          {#if phase !== 'undone' && outcome && outcome.batchId !== 0}
            <button
              type="button"
              class="rounded-lg px-3 py-1.5 text-sm text-zinc-700 hover:bg-zinc-100 disabled:opacity-50 dark:text-zinc-200 dark:hover:bg-zinc-800"
              disabled={phase === 'undoing'}
              onclick={undo}
            >
              {phase === 'undoing' ? t('Undoing…') : t('Undo')}
            </button>
          {/if}
          <button
            type="button"
            class="rounded-lg bg-accent-600 px-4 py-1.5 text-sm font-medium text-white hover:bg-accent-500 disabled:opacity-50"
            disabled={phase === 'undoing'}
            onclick={close}
          >
            {t('Close')}
          </button>
        </div>
      {:else}
        {#if candidates.length === 0}
          <p class="rounded-lg bg-zinc-100 px-3 py-2 text-sm text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300" role="status">
            {t('No matching release was found on MusicBrainz.')}
          </p>
        {:else}
          <div role="radiogroup" aria-label={t('Matches')} class="flex flex-col gap-1.5">
            {#each candidates as c, i (c.key)}
              <button
                type="button"
                role="radio"
                aria-checked={i === selected}
                class={[
                  'flex items-center gap-3 rounded-xl border p-2 text-left outline-none focus-visible:ring-2 focus-visible:ring-accent-500',
                  i === selected
                    ? 'border-accent-500 bg-accent-50/60 dark:bg-accent-950/30'
                    : 'border-zinc-200 hover:bg-zinc-50 dark:border-zinc-700 dark:hover:bg-zinc-800/60',
                ]}
                onclick={() => (selected = i)}
              >
                <Cover cover={c.coverPreview} title={c.primary} class="size-12 shrink-0 rounded-md text-base" />
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-sm font-medium">{c.primary}</span>
                  <span class="block truncate text-xs text-zinc-500">{c.secondary}</span>
                  {#if c.meta}<span class="block truncate text-xs text-zinc-500">{c.meta}</span>{/if}
                </span>
                <span
                  class={[
                    'shrink-0 rounded-full px-2 py-0.5 text-xs font-semibold whitespace-nowrap',
                    c.label === 'recommended'
                      ? 'bg-accent-100 text-accent-800 dark:bg-accent-900/50 dark:text-accent-200'
                      : 'bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-200',
                  ]}
                  title={t('Match score {score}', { score: c.score })}
                >
                  {t(c.label === 'recommended' ? 'Recommended' : 'Check')} · {c.score}
                </span>
              </button>
            {/each}
          </div>

          {#if current}
            <section aria-label={t('Changes')} class="flex flex-col gap-3">
              <h3 class="text-sm font-semibold">{t('Changes')}</h3>
              {#if current.albumFields.length > 0}
                <ul class="flex flex-col gap-1 rounded-lg bg-zinc-50 p-2.5 dark:bg-zinc-800/50">
                  {#each current.albumFields as f (f.field)}{@render diffLine(f)}{/each}
                </ul>
              {/if}
              <ol class="flex flex-col divide-y divide-zinc-100 dark:divide-zinc-800">
                {#each current.tracks as tr (tr.trackId)}
                  <li class={['py-2', !tr.matched || tr.fields.length === 0 ? 'text-zinc-400 dark:text-zinc-500' : '']}>
                    <p class="truncate text-sm font-medium" title={tr.path}>{tr.title || fileName(tr.path)}</p>
                    {#if !tr.matched}
                      <p class="text-xs text-amber-700 dark:text-amber-300">{t('No matching track on this release; left unchanged.')}</p>
                    {:else if tr.fields.length === 0}
                      <p class="text-xs">{t('No visible changes')}</p>
                    {:else}
                      <ul class="mt-1 flex flex-col gap-0.5">
                        {#each tr.fields as f (f.field)}{@render diffLine(f)}{/each}
                      </ul>
                    {/if}
                  </li>
                {/each}
              </ol>
              <p class="text-xs text-zinc-500">
                {t('MusicBrainz ids are saved in the tags too, and a cover is added if the album has none.')}
              </p>
            </section>
          {/if}
        {/if}

        {#if error}
          <p class={errorClass} role="alert">{error}</p>
        {/if}

        <div class="flex justify-end gap-2">
          <button
            type="button"
            class="rounded-lg px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-100 disabled:opacity-50 dark:text-zinc-300 dark:hover:bg-zinc-800"
            disabled={phase === 'applying'}
            onclick={close}
          >
            {t('Cancel')}
          </button>
          {#if candidates.length > 0}
            <button
              type="button"
              class="rounded-lg bg-accent-600 px-4 py-1.5 text-sm font-medium text-white hover:bg-accent-500 disabled:opacity-50"
              disabled={!current || phase === 'applying'}
              onclick={apply}
            >
              {phase === 'applying' ? t('Applying…') : t('Apply')}
            </button>
          {/if}
        </div>
      {/if}
    </div>
  </div>
{/if}
