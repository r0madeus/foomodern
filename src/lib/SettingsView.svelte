<script lang="ts">
  import { onMount } from 'svelte'
  import { getVersion } from '@tauri-apps/api/app'
  import { player, type ReplayGain } from './player.svelte'
  import { prefs } from './prefs.svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { appSettings } from './appSettings.svelte'
  import { updates } from './updates.svelte'

  const MODES: { value: ReplayGain; label: string }[] = [
    { value: 'off', label: 'Off' },
    { value: 'track', label: 'Track' },
    { value: 'album', label: 'Album' },
  ]

  let version = $state<string | null>(null)
  // Android: "all files access", needed to write tags and to read or write
  // .lrc files next to songs. Re-checked when the app comes back from the
  // system settings screen.
  let fileAccess = $state<boolean | null>(null)
  function checkFileAccess() {
    invoke<boolean>('app_file_access')
      .then((g) => (fileAccess = g))
      .catch(() => (fileAccess = null))
  }
  $effect(() => {
    if (appSettings.platform !== 'android') return
    checkFileAccess()
    const onVisible = () => document.visibilityState === 'visible' && checkFileAccess()
    document.addEventListener('visibilitychange', onVisible)
    return () => document.removeEventListener('visibilitychange', onVisible)
  })
  let logDir = $state<string | null>(null)
  let copied = $state(false)

  async function copyLogDir() {
    if (!logDir) return
    try {
      await navigator.clipboard.writeText(logDir)
      copied = true
      setTimeout(() => (copied = false), 1500)
    } catch {
      // clipboard unavailable: the path is still selectable
    }
  }

  onMount(() => {
    void appSettings.load()
    invoke<string>('app_log_dir')
      .then((d) => (logDir = d))
      .catch(() => (logDir = null))
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

    {#if appSettings.platform === 'android'}
      <section aria-labelledby="set-storage" class="flex flex-col gap-3">
        <h3 id="set-storage" class="text-xs font-medium uppercase tracking-wide text-zinc-400">Storage</h3>
        <div class="flex items-start justify-between gap-4">
          <span class="min-w-0 text-sm">
            <span class="block font-medium">Access to all files</span>
            <span class="block text-zinc-500">
              Needed to edit tags and to attach .lrc lyrics next to your songs. Playing music works without it.
            </span>
          </span>
          {#if fileAccess}
            <span class="shrink-0 text-sm font-medium text-teal-600 dark:text-teal-400">Allowed</span>
          {:else}
            <button
              class="shrink-0 rounded-lg bg-teal-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-teal-500"
              onclick={() => void invoke('app_request_file_access').catch((e) => (player.error = String(e)))}
            >
              Allow
            </button>
          {/if}
        </div>
      </section>
    {/if}

    {#if appSettings.platform !== 'android'}
    <section aria-labelledby="set-window" class="flex flex-col gap-3">
      <h3 id="set-window" class="text-xs font-medium uppercase tracking-wide text-zinc-400">Window</h3>
      <label class="flex cursor-pointer items-start justify-between gap-6">
        <span class="min-w-0">
          <span class="block text-sm font-medium">Keep playing in the tray when the window is closed</span>
          <span id="tray-help" class="block text-sm text-zinc-500">
            Closing the window hides FooModern to the system tray and playback continues. Use the
            tray menu to show the window again or to quit.
            {#if appSettings.loaded && !appSettings.trayAvailable}
              <span class="mt-1 block text-amber-700 dark:text-amber-400" role="status">
                No system tray is available here, so closing the window always quits.
              </span>
            {/if}
          </span>
        </span>
        <input
          type="checkbox"
          role="switch"
          class="mt-1 size-4 shrink-0 accent-teal-600"
          aria-describedby="tray-help"
          checked={appSettings.closeToTray}
          disabled={!appSettings.loaded}
          onchange={(e) => void appSettings.setCloseToTray(e.currentTarget.checked)}
        />
      </label>
    </section>
    {/if}

    <section aria-labelledby="set-about" class="flex flex-col gap-3">
      <h3 id="set-about" class="text-xs font-medium uppercase tracking-wide text-zinc-400">About</h3>
      <div class="flex flex-wrap items-center gap-3">
        <p class="text-sm">
          <span class="font-medium">FooModern</span>
          <span class="text-zinc-500">{version ? ` · version ${version}` : ''}</span>
        </p>
        <button
          class="rounded-lg px-3 py-1.5 text-sm font-medium text-teal-700 hover:bg-teal-50 disabled:opacity-50 dark:text-teal-300 dark:hover:bg-teal-950/50"
          disabled={updates.checking}
          onclick={() => void updates.check(true)}
        >
          {updates.checking ? 'Checking…' : 'Check for updates'}
        </button>
        <span class="text-sm text-zinc-500" role="status">
          {#if updates.status === 'upToDate'}You have the latest version.
          {:else if updates.status === 'failed'}Could not reach GitHub.
          {:else if updates.status === 'found' && updates.available}Version {updates.available.version} is available.{/if}
        </span>
      </div>
      {#if logDir}
        <div class="text-sm">
          <p class="font-medium">Log files</p>
          <p class="mt-0.5 text-zinc-500">Attach these when reporting a problem.</p>
          <div class="mt-1.5 flex items-center gap-2">
            <code class="min-w-0 flex-1 truncate rounded bg-zinc-100 px-2 py-1 text-xs select-all dark:bg-zinc-800" title={logDir}>{logDir}</code>
            <button class="shrink-0 rounded-lg px-2.5 py-1 text-xs font-medium hover:bg-zinc-200/70 dark:hover:bg-zinc-800" onclick={copyLogDir}>
              {copied ? 'Copied' : 'Copy'}
            </button>
          </div>
        </div>
      {/if}
    </section>
  </div>
</div>
