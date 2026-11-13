<script lang="ts">
  import { untrack } from 'svelte'
  import { fade, scale } from 'svelte/transition'
  import { appSettings } from './appSettings.svelte'
  import Icon from './Icon.svelte'
  import { t } from './i18n.svelte'
  import { motionDuration } from './motion'
  import PlaylistCover from './PlaylistCover.svelte'
  import {
    buildCoverPreset,
    COVER_ICONS,
    COVER_PALETTES,
    fallbackPalette,
    parseCoverPreset,
  } from './playlistCover'
  import { playlists, type PlaylistInfo } from './playlists.svelte'

  let { playlist, onClose }: { playlist: PlaylistInfo; onClose: () => void } = $props()

  const initial = untrack(
    () =>
      parseCoverPreset(playlist.cover) ?? {
        palette: fallbackPalette(playlist.name),
        icon: 'music' as const,
      },
  )

  let selectedPalette = $state(initial.palette.id)
  let selectedIcon = $state(initial.icon)
  let saving = $state(false)

  const previewPreset = $derived(buildCoverPreset(selectedPalette, selectedIcon))

  async function choosePhoto() {
    saving = true
    try {
      if (await playlists.setCoverFile(playlist.id)) onClose()
    } finally {
      saving = false
    }
  }

  async function resetAuto() {
    saving = true
    try {
      if (await playlists.clearCover(playlist.id)) onClose()
    } finally {
      saving = false
    }
  }

  async function applyPreset() {
    saving = true
    try {
      if (await playlists.setCoverPreset(playlist.id, previewPreset)) onClose()
    } finally {
      saving = false
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onClose()} />

<div
  transition:fade={{ duration: motionDuration(150) }}
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4 backdrop-blur-xs"
  role="dialog"
  aria-modal="true"
  aria-labelledby="playlist-cover-title"
>
  <div
    transition:scale={{ start: 0.96, duration: motionDuration(180) }}
    class="w-full max-w-md rounded-2xl border border-zinc-200 bg-white p-5 shadow-2xl dark:border-zinc-800 dark:bg-zinc-900"
  >
    <div class="mb-4 flex items-center justify-between gap-3">
      <h3 id="playlist-cover-title" class="text-base font-semibold">
        {t('Playlist cover')}
      </h3>
      <button
        type="button"
        aria-label={t('Close')}
        class="rounded-lg p-1 text-zinc-400 hover:bg-zinc-100 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-200"
        onclick={onClose}
      >
        <Icon name="close" class="size-4" />
      </button>
    </div>

    <div class="mb-5 flex items-center gap-4">
      <PlaylistCover
        cover={previewPreset}
        name={playlist.name}
        class="size-24 shrink-0 rounded-xl shadow-md"
        iconClass="size-10"
      />
      <div class="flex min-w-0 flex-1 flex-col gap-2">
        {#if appSettings.platform !== 'android'}
          <button
            type="button"
            disabled={saving}
            class="rounded-lg border border-zinc-200 px-3 py-1.5 text-left text-sm font-medium hover:bg-zinc-100 disabled:opacity-50 dark:border-zinc-700 dark:hover:bg-zinc-800"
            onclick={() => void choosePhoto()}
          >
            {t('Choose photo…')}
          </button>
        {/if}
        <button
          type="button"
          disabled={saving || playlist.cover === null}
          class="rounded-lg px-3 py-1.5 text-left text-xs text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 disabled:opacity-40 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
          onclick={() => void resetAuto()}
        >
          {t('Use automatic cover')}
        </button>
      </div>
    </div>

    <div class="flex flex-col gap-4">
      <div>
        <p class="mb-2 text-xs font-medium uppercase tracking-wide text-zinc-400">
          {t('Gradient')}
        </p>
        <div class="flex flex-wrap gap-2">
          {#each COVER_PALETTES as p (p.id)}
            <button
              type="button"
              aria-label={p.label}
              aria-pressed={selectedPalette === p.id}
              class={[
                'size-8 rounded-full transition',
                selectedPalette === p.id
                  ? 'ring-2 ring-teal-500 ring-offset-2 ring-offset-white dark:ring-offset-zinc-900'
                  : 'opacity-80 hover:opacity-100',
              ]}
              style:background={p.gradient}
              onclick={() => (selectedPalette = p.id)}
            ></button>
          {/each}
        </div>
      </div>

      <div>
        <p class="mb-2 text-xs font-medium uppercase tracking-wide text-zinc-400">
          {t('Icon')}
        </p>
        <div class="flex flex-wrap gap-2">
          {#each COVER_ICONS as icon (icon)}
            <button
              type="button"
              aria-label={icon}
              aria-pressed={selectedIcon === icon}
              class={[
                'flex size-9 items-center justify-center rounded-lg border transition',
                selectedIcon === icon
                  ? 'border-teal-500 bg-teal-500/15 text-teal-600 dark:text-teal-300'
                  : 'border-zinc-200 text-zinc-500 hover:bg-zinc-100 dark:border-zinc-700 dark:hover:bg-zinc-800',
              ]}
              onclick={() => (selectedIcon = icon)}
            >
              <Icon name={icon} class="size-4" />
            </button>
          {/each}
        </div>
      </div>
    </div>

    <div class="mt-6 flex justify-end gap-2">
      <button
        type="button"
        class="rounded-lg px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800"
        onclick={onClose}
      >
        {t('Cancel')}
      </button>
      <button
        type="button"
        disabled={saving}
        class="rounded-lg bg-teal-600 px-4 py-1.5 text-sm font-medium text-white hover:bg-teal-500 disabled:opacity-50"
        onclick={() => void applyPreset()}
      >
        {t('Apply gradient & icon')}
      </button>
    </div>
  </div>
</div>
