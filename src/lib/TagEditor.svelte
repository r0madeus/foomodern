<script lang="ts">
  import { count, i18n, t } from './i18n.svelte'
  import { tick } from 'svelte'
  import { fade, scale } from 'svelte/transition'
  import { invoke } from '@tauri-apps/api/core'
  import { motionDuration } from './motion'
  import { playlists } from './playlists.svelte'
  import {
    tagEditor,
    type EditableTags,
    type TagChanges,
    type TagField,
    type TagFailure,
    type TagsRead,
    type TagsWritten,
  } from './tags.svelte'

  type FieldDef = { key: TagField; label: string; numeric?: boolean; wide?: boolean }
  const FIELDS: FieldDef[] = [
    { key: 'title', label: 'Title', wide: true },
    { key: 'artist', label: 'Artist', wide: true },
    { key: 'album', label: 'Album', wide: true },
    { key: 'albumArtist', label: 'Album artist', wide: true },
    { key: 'trackNo', label: 'Track number', numeric: true },
    { key: 'discNo', label: 'Disc number', numeric: true },
    { key: 'year', label: 'Year', numeric: true },
    { key: 'genre', label: 'Genre' },
  ]

  const request = $derived(tagEditor.request)

  let loading = $state(false)
  let loadError = $state<string | null>(null)
  let saving = $state(false)
  let formError = $state<string | null>(null)
  let failures = $state<TagFailure[]>([])
  let saved = $state(0)
  let fileCount = $state(0)
  let initial = $state<Record<TagField, string>>(blank())
  let draft = $state<Record<TagField, string>>(blank())
  let mixed = $state<Set<TagField>>(new Set())
  let cleared = $state<Set<TagField>>(new Set())

  let dialog = $state<HTMLDivElement | null>(null)
  let previousFocus: HTMLElement | null = null

  function blank(): Record<TagField, string> {
    return {
      title: '', artist: '', album: '', albumArtist: '',
      trackNo: '', discNo: '', year: '', genre: '',
    }
  }

  function toDraft(t: EditableTags): Record<TagField, string> {
    const out = blank()
    for (const f of FIELDS) out[f.key] = t[f.key] === null ? '' : String(t[f.key])
    return out
  }

  // Load the tags whenever the dialog is opened for a new set of tracks.
  $effect(() => {
    const req = request
    if (!req) return
    let stale = false
    previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    loading = true
    loadError = null
    formError = null
    failures = []
    saved = 0
    saving = false
    cleared = new Set()
    invoke<TagsRead>('tags_read', { trackIds: req.trackIds })
      .then(async (r) => {
        if (stale) return
        fileCount = r.tracks.length
        mixed = new Set(r.mixed)
        initial = toDraft(r.common)
        draft = { ...initial }
        if (r.tracks.length === 0) {
          loadError = r.failed[0]?.error ?? t('These tracks are no longer in the library.')
        } else if (r.failed.length > 0) {
          failures = r.failed
        }
        loading = false
        await tick()
        dialog?.querySelector<HTMLElement>('input')?.focus()
      })
      .catch((e) => {
        if (stale) return
        loadError = String(e)
        loading = false
      })
    return () => {
      stale = true
    }
  })

  function changed(key: TagField): boolean {
    return cleared.has(key) || draft[key] !== initial[key]
  }

  const dirty = $derived(FIELDS.some((f) => changed(f.key)))

  function buildChanges(): TagChanges | string {
    const out: Record<string, string | number | null> = {}
    for (const f of FIELDS) {
      if (!changed(f.key)) continue
      const raw = cleared.has(f.key) ? '' : draft[f.key].trim()
      if (raw === '') {
        out[f.key] = null
      } else if (f.numeric) {
        const n = Number(raw)
        if (!Number.isInteger(n) || n < 0 || (f.key === 'year' && (n < 1 || n > 9999))) {
          return f.key === 'year' ? t('Year must be between 1 and 9999.') : t('{label} must be a whole number.', { label: t(f.label) })
        }
        out[f.key] = n
      } else {
        out[f.key] = raw
      }
    }
    return out as TagChanges
  }

  async function save() {
    if (!request || saving || !dirty) return
    const changes = buildChanges()
    if (typeof changes === 'string') {
      formError = changes
      return
    }
    formError = null
    saving = true
    try {
      const r = await invoke<TagsWritten>('tags_write', { trackIds: request.trackIds, changes })
      saved = r.updated
      if (r.failed.length === 0) {
        playlists.flash(t('Updated tags of {count}', { count: count(r.updated, '{n} track', '{n} tracks') }))
        // close() refuses while `saving` is set, and `finally` runs too late.
        saving = false
        close()
      } else {
        failures = r.failed
      }
    } catch (e) {
      formError = String(e)
    } finally {
      saving = false
    }
  }

  function close() {
    if (saving) return
    tagEditor.close()
    const back = previousFocus
    previousFocus = null
    // The row that opened the dialog may have been recycled; ignore then.
    if (back?.isConnected) back.focus()
  }

  function fileName(path: string): string {
    return path.slice(Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\')) + 1)
  }

  function focusables(): HTMLElement[] {
    if (!dialog) return []
    return [
      ...dialog.querySelectorAll<HTMLElement>(
        'input:not([disabled]), button:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])',
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

  const inputClass =
    'w-full rounded-md border border-zinc-200 bg-white px-2 py-1.5 text-sm outline-none placeholder:text-zinc-400 focus-visible:ring-2 focus-visible:ring-teal-500 dark:border-zinc-700 dark:bg-zinc-950'
</script>

{#if request}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    transition:fade={{ duration: motionDuration(150) }}
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
    onclick={(e) => e.target === e.currentTarget && close()}
  >
    <div
      bind:this={dialog}
      transition:scale={{ start: 0.96, duration: motionDuration(180) }}
      role="dialog"
      aria-modal="true"
      aria-labelledby="tag-editor-title"
      tabindex="-1"
      onkeydown={onKey}
      class="flex max-h-full w-full max-w-xl flex-col gap-4 overflow-y-auto rounded-2xl border border-zinc-200 bg-white p-5 text-zinc-900 shadow-xl dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100"
    >
      <div>
        <h2 id="tag-editor-title" class="text-lg font-semibold tracking-tight">{request.heading}</h2>
        {#if !loading && !loadError}
          <p class="text-sm text-zinc-500">
            {count(fileCount, '{n} file', '{n} files')}. {t('Only the fields you change are written.')}
          </p>
        {/if}
      </div>

      {#if loading}
        <p class="text-sm text-zinc-500" role="status">{t("Reading tags…")}</p>
      {:else if loadError}
        <p class="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700 dark:bg-red-950/50 dark:text-red-300" role="alert">
          {loadError}
        </p>
        <div class="flex justify-end">
          <button
            type="button"
            class="rounded-lg px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800"
            onclick={close}
          >
            {t("Close")}
          </button>
        </div>
      {:else}
        <form
          class="flex flex-col gap-4"
          onsubmit={(e) => {
            e.preventDefault()
            void save()
          }}
        >
          <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
            {#each FIELDS as f (f.key)}
              {@const isMixed = mixed.has(f.key) && !changed(f.key)}
              <div class={['flex flex-col gap-1', f.wide && 'sm:col-span-2']}>
                <div class="flex items-center justify-between">
                  <label for="tag-{f.key}" class="text-xs font-medium text-zinc-500">{t(f.label)}</label>
                  {#if mixed.has(f.key) && !cleared.has(f.key)}
                    <button
                      type="button"
                      class="rounded px-1 text-xs text-zinc-500 outline-none hover:text-zinc-800 focus-visible:ring-2 focus-visible:ring-teal-500 dark:hover:text-zinc-100"
                      aria-label={t('Clear {label} on all files', { label: t(f.label) })}
                      onclick={() => {
                        draft[f.key] = ''
                        cleared = new Set(cleared).add(f.key)
                      }}
                    >
                      {t("Clear")}
                    </button>
                  {/if}
                </div>
                <input
                  id="tag-{f.key}"
                  class={inputClass}
                  type={f.numeric ? 'number' : 'text'}
                  min={f.numeric ? 0 : undefined}
                  inputmode={f.numeric ? 'numeric' : undefined}
                  placeholder={cleared.has(f.key) ? t('(will be cleared)') : isMixed ? t('(multiple values)') : ''}
                  bind:value={draft[f.key]}
                  oninput={() => {
                    if (cleared.has(f.key)) {
                      const next = new Set(cleared)
                      next.delete(f.key)
                      cleared = next
                    }
                  }}
                />
              </div>
            {/each}
          </div>

          {#if formError}
            <p class="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-700 dark:bg-red-950/50 dark:text-red-300" role="alert">
              {formError}
            </p>
          {/if}

          {#if failures.length > 0}
            <div class="rounded-lg bg-amber-50 px-3 py-2 text-sm text-amber-900 dark:bg-amber-950/40 dark:text-amber-200" role="alert">
              <p class="font-medium">
                {saved > 0 ? t('{n} updated, ', { n: saved.toLocaleString(i18n.locale) }) : ''}{t('{count} could not be edited:', { count: count(failures.length, '{n} file', '{n} files') })}
              </p>
              <ul class="mt-1 max-h-32 list-disc overflow-y-auto pl-5">
                {#each failures as f (f.path)}
                  <li><span class="font-medium" title={f.path}>{fileName(f.path)}</span>: {f.error}</li>
                {/each}
              </ul>
            </div>
          {/if}

          <div class="flex justify-end gap-2">
            <button
              type="button"
              class="rounded-lg px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-100 disabled:opacity-50 dark:text-zinc-300 dark:hover:bg-zinc-800"
              disabled={saving}
              onclick={close}
            >
              {saved > 0 ? t('Close') : t('Cancel')}
            </button>
            <button
              type="submit"
              class="rounded-lg bg-teal-600 px-4 py-1.5 text-sm font-medium text-white hover:bg-teal-500 disabled:opacity-50"
              disabled={!dirty || saving}
            >
              {saving ? t('Saving…') : t('Save')}
            </button>
          </div>
        </form>
      {/if}
    </div>
  </div>
{/if}
