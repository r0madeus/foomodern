<script lang="ts">
  import { onMount } from 'svelte'
  import { getVersion } from '@tauri-apps/api/app'
  import { player, type ReplayGain } from './player.svelte'
  import { prefs } from './prefs.svelte'

  const MODES: { value: ReplayGain; label: string }[] = [
    { value: 'off', label: 'Off' },
    { value: 'track', label: 'Track' },
    { value: 'album', label: 'Album' },
  ]

  let version = $state<string | null>(null)
  onMount(() => {
    getVersion()
      .then((v) => (version = v))
      .catch(() => (version = null))
  })

  const mode = $derived(player.snapshot.replaygain)
</script>

<div class="h-full overflow-y-auto px-5 pb-8">
  <h2 class="mb-4 text-lg font-semibold tracking-tight">Settings</h2>

  <div class="flex max-w-2xl flex-col gap-8">
    <section aria-labelledby="set-playback" class="flex flex-col gap-3">
      <h3 id="set-playback" class="text-xs font-medium uppercase tracking-wide text-zinc-400">Playback</h3>
      <div class="flex items-start justify-between gap-6">
        <div class="min-w-0">
          <p id="rg-label" class="text-sm font-medium">ReplayGain</p>
          <p id="rg-help" class="text-sm text-zinc-500">
            Evens out loudness using ReplayGain tags. Track levels every song; Album keeps the
            loudness differences inside an album.
          </p>
        </div>
        <div
          role="radiogroup"
          aria-labelledby="rg-label"
          aria-describedby="rg-help"
          class="flex shrink-0 rounded-lg border border-zinc-200 bg-white p-0.5 dark:border-zinc-700 dark:bg-zinc-900"
        >
          {#each MODES as m (m.value)}
            <button
              type="button"
              role="radio"
              aria-checked={mode === m.value}
              class={[
                'rounded-md px-3 py-1 text-sm outline-none focus-visible:ring-2 focus-visible:ring-teal-500',
                mode === m.value
                  ? 'bg-teal-600 font-medium text-white'
                  : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-300 dark:hover:bg-zinc-800',
              ]}
              onclick={() => player.setReplayGain(m.value)}
            >
              {m.label}
            </button>
          {/each}
        </div>
      </div>
    </section>

    <section aria-labelledby="set-library" class="flex flex-col gap-3">
      <h3 id="set-library" class="text-xs font-medium uppercase tracking-wide text-zinc-400">Library</h3>
      <label class="flex cursor-pointer items-start justify-between gap-6">
        <span class="min-w-0">
          <span class="block text-sm font-medium">Show file extensions</span>
          <span class="block text-sm text-zinc-500">
            Adds a small format badge (FLAC, MP3…) to song rows and album track lists.
          </span>
        </span>
        <input
          type="checkbox"
          role="switch"
          class="mt-1 size-4 shrink-0 accent-teal-600"
          bind:checked={prefs.showExtensions}
        />
      </label>
    </section>

    <section aria-labelledby="set-about" class="flex flex-col gap-3">
      <h3 id="set-about" class="text-xs font-medium uppercase tracking-wide text-zinc-400">About</h3>
      <p class="text-sm">
        <span class="font-medium">FooModern</span>
        <span class="text-zinc-500">{version ? ` · version ${version}` : ''}</span>
      </p>
    </section>
  </div>
</div>
