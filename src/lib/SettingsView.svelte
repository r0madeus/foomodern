<script lang="ts">
  import { i18n, t, type LangChoice } from './i18n.svelte'
  import { onMount } from 'svelte'
  import { getVersion } from '@tauri-apps/api/app'
  import { open } from '@tauri-apps/plugin-dialog'
  import { player, type ReplayGain } from './player.svelte'
  import { prefs } from './prefs.svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { appSettings, type LyricsSave, type NetFeature } from './appSettings.svelte'
  import { lyrics, type Coverage } from './lyrics.svelte'
  import FoldersView from './FoldersView.svelte'
  import { online } from './online.svelte'
  import { updates } from './updates.svelte'

  async function pickDownloadDir() {
    const picked = await open({ directory: true, multiple: false, title: t('Choose download folder') })
    if (typeof picked === 'string') {
      await online.setDownloadDir(picked)
    }
  }

  async function pickYtdlpPath() {
    const picked = await open({ directory: false, multiple: false, title: t('Locate yt-dlp executable') })
    if (typeof picked === 'string') {
      await online.setYtdlpPath(picked)
    }
  }

  const MODES: { value: ReplayGain; label: string }[] = [
    { value: 'off', label: 'Off' },
    { value: 'track', label: 'Track' },
    { value: 'album', label: 'Album' },
  ]

  const LYRICS_SAVE: { value: LyricsSave; label: string }[] = [
    { value: 'sidecar', label: 'Next to the song (.lrc file)' },
    { value: 'embed', label: 'Inside the song file (tag)' },
    { value: 'off', label: "Only in the app's cache" },
  ]

  // Bulk save and coverage count run in Rust; only small numbers come back.
  let lyricsBusy = $state<'save' | 'count' | null>(null)
  let lyricsSaved = $state<string | null>(null)
  let coverage = $state<Coverage | null>(null)
  let lyricsToolError = $state<string | null>(null)

  async function saveCachedLyrics() {
    lyricsBusy = 'save'
    lyricsSaved = null
    lyricsToolError = null
    try {
      const r = await lyrics.saveCached()
      lyricsSaved = t('{saved} saved, {skipped} skipped', { saved: r.saved, skipped: r.skipped })
    } catch (e) {
      lyricsToolError = String(e)
    } finally {
      lyricsBusy = null
    }
  }

  async function countLyrics() {
    lyricsBusy = 'count'
    lyricsToolError = null
    try {
      coverage = await lyrics.coverage()
    } catch (e) {
      lyricsToolError = String(e)
    } finally {
      lyricsBusy = null
    }
  }

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

  /** One switch per internet feature, in the order they are shown. */
  const NET_FEATURES: { feature: NetFeature; label: string; help: string; desktopOnly?: boolean }[] = [
    {
      feature: 'lyrics',
      label: 'Lyrics (LRCLIB, NetEase)',
      help: 'Looks up lyrics online and lets you share your own on LRCLIB. Lyrics next to your songs and in their tags keep working.',
    },
    {
      feature: 'online',
      label: 'Online search and downloads (YouTube, iTunes)',
      help: 'Online search, streaming and downloads, including album search and covers through iTunes.',
      desktopOnly: true,
    },
    {
      feature: 'artistInfo',
      label: 'Artist info (Deezer, Wikipedia, iTunes)',
      help: 'Artist portraits, biographies and missing albums. Anything already saved is still shown.',
    },
    {
      feature: 'metadata',
      label: 'Find info (MusicBrainz, Cover Art Archive)',
      help: 'Finds album and track tags and covers on MusicBrainz.',
    },
    {
      feature: 'updates',
      label: 'Update check (GitHub)',
      help: 'Checks once a day whether a new version of FooModern is available.',
    },
  ]
  const NET_VALUE: Record<NetFeature, () => boolean> = {
    lyrics: () => appSettings.netLyrics,
    online: () => appSettings.netOnline,
    artistInfo: () => appSettings.netArtistInfo,
    metadata: () => appSettings.netMetadata,
    updates: () => appSettings.netUpdates,
  }
</script>

<div class="h-full overflow-y-auto px-5 pb-8">
  <h2 class="mb-4 text-lg font-semibold tracking-tight max-sm:hidden">{t("Settings")}</h2>

  <div class="flex max-w-2xl flex-col gap-8">
    <section aria-labelledby="set-language" class="flex flex-col gap-3">
      <h3 id="set-language" class="text-xs font-medium uppercase tracking-wide text-zinc-400">{t('Language')}</h3>
      <label class="flex items-start justify-between gap-6">
        <span class="min-w-0 text-sm text-zinc-500">
          {t('Language of the app. "System default" follows the device language.')}
        </span>
        <select
          class="shrink-0 rounded-md border border-zinc-200 bg-white px-2 py-1 text-sm dark:border-zinc-700 dark:bg-zinc-900"
          value={i18n.choice}
          onchange={(e) => i18n.setChoice(e.currentTarget.value as LangChoice)}
        >
          <option value="system">{t('System default')}</option>
          <option value="en">English</option>
          <option value="tr">Türkçe</option>
        </select>
      </label>
    </section>

    <section aria-labelledby="set-playback" class="flex flex-col gap-3">
      <h3 id="set-playback" class="text-xs font-medium uppercase tracking-wide text-zinc-400">{t("Playback")}</h3>
      <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between sm:gap-6">
        <div class="min-w-0">
          <p id="rg-label" class="text-sm font-medium">{t("ReplayGain")}</p>
          <p id="rg-help" class="text-sm text-zinc-500">
            {t("Evens out loudness using ReplayGain tags. Track levels every song; Album keeps the loudness differences inside an album.")}
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
              {t(m.label)}
            </button>
          {/each}
        </div>
      </div>
    </section>

    <section aria-labelledby="set-library" class="flex flex-col gap-3">
      <h3 id="set-library" class="text-xs font-medium uppercase tracking-wide text-zinc-400">{t("Library")}</h3>
      <label class="flex cursor-pointer items-start justify-between gap-6">
        <span class="min-w-0">
          <span class="block text-sm font-medium">{t("Show file extensions")}</span>
          <span class="block text-sm text-zinc-500">
            {t("Adds a small format badge (FLAC, MP3…) to song rows and album track lists.")}
          </span>
        </span>
        <input
          type="checkbox"
          role="switch"
          class="mt-1 size-4 shrink-0 accent-teal-600"
          bind:checked={prefs.showExtensions}
        />
      </label>
      <div class="mt-1 border-t border-zinc-200/70 pt-3 dark:border-zinc-800/70">
        <FoldersView embedded />
      </div>
    </section>

    <section aria-labelledby="set-internet" class="flex flex-col gap-3">
      <h3 id="set-internet" class="text-xs font-medium uppercase tracking-wide text-zinc-400">{t("Internet")}</h3>
      <label class="flex cursor-pointer items-start justify-between gap-6">
        <span class="min-w-0">
          <span class="block text-sm font-medium">{t("Offline mode")}</span>
          <span id="offline-help" class="block text-sm text-zinc-500">
            {t("Nothing connects to the internet")}
          </span>
        </span>
        <input
          type="checkbox"
          role="switch"
          class="mt-1 size-4 shrink-0 accent-teal-600"
          aria-describedby="offline-help"
          checked={appSettings.offlineMode}
          disabled={!appSettings.loaded}
          onchange={(e) => void appSettings.setOfflineMode(e.currentTarget.checked)}
        />
      </label>
      {#each NET_FEATURES.filter((f) => !f.desktopOnly || appSettings.platform !== 'android') as f (f.feature)}
        <label class={['flex items-start justify-between gap-6', appSettings.offlineMode ? 'opacity-60' : 'cursor-pointer']}>
          <span class="min-w-0">
            <span class="block text-sm font-medium">{t(f.label)}</span>
            <span class="block text-sm text-zinc-500">{t(f.help)}</span>
          </span>
          <input
            type="checkbox"
            role="switch"
            class="mt-1 size-4 shrink-0 accent-teal-600"
            checked={NET_VALUE[f.feature]()}
            disabled={!appSettings.loaded || appSettings.offlineMode}
            onchange={(e) => void appSettings.setNetFeature(f.feature, e.currentTarget.checked)}
          />
        </label>
      {/each}
      {#if appSettings.offlineMode}
        <p class="text-xs text-zinc-500" role="status">{t("Offline mode is on, so the switches above are off.")}</p>
      {/if}
    </section>

    <section aria-labelledby="set-lyrics" class="flex flex-col gap-3">
      <h3 id="set-lyrics" class="text-xs font-medium uppercase tracking-wide text-zinc-400">{t("Lyrics")}</h3>
      <div class="flex flex-col gap-2">
        <div class="min-w-0">
          <p id="lyr-save-label" class="text-sm font-medium">{t("Save lyrics")}</p>
          <p id="lyr-save-help" class="text-sm text-zinc-500">
            {t("Lyrics found online are also saved permanently, so they work offline and in other players. An existing .lrc file or lyrics tag is never replaced. Lyrics in a song's tag are written once the song is no longer the one playing.")}
          </p>
        </div>
        <div role="radiogroup" aria-labelledby="lyr-save-label" aria-describedby="lyr-save-help" class="flex flex-col gap-1">
          {#each LYRICS_SAVE as m (m.value)}
            <label class="flex cursor-pointer items-center gap-2 text-sm">
              <input
                type="radio"
                name="lyrics-save"
                class="size-4 accent-teal-600"
                value={m.value}
                checked={appSettings.lyricsSave === m.value}
                disabled={!appSettings.loaded}
                onchange={() => void appSettings.setLyricsSave(m.value)}
              />
              {t(m.label)}
            </label>
          {/each}
        </div>
      </div>

      <label class={['flex items-start justify-between gap-6', appSettings.allowLyrics ? 'cursor-pointer' : 'opacity-60']}>
        <span class="min-w-0">
          <span class="block text-sm font-medium">{t("NetEase Cloud Music")}</span>
          <span class="block text-sm text-zinc-500">
            {t("Also search NetEase Cloud Music (unofficial; used when LRCLIB has nothing)")}
            {#if !appSettings.allowLyrics}
              <span class="block text-xs" role="status">{t("Turned off in Settings")}</span>
            {/if}
          </span>
        </span>
        <input
          type="checkbox"
          role="switch"
          class="mt-1 size-4 shrink-0 accent-teal-600"
          checked={appSettings.lyricsNetease}
          disabled={!appSettings.loaded || !appSettings.allowLyrics}
          onchange={(e) => void appSettings.setLyricsNetease(e.currentTarget.checked)}
        />
      </label>

      <div class="flex flex-wrap items-start justify-between gap-3">
        <div class="min-w-0 flex-1">
          <p class="text-sm font-medium">{t("Save cached lyrics to songs")}</p>
          <p class="text-sm text-zinc-500">
            {t("Writes the lyrics already found for your library with the setting above. Songs that already have lyrics are skipped.")}
          </p>
          {#if lyricsSaved}<p class="mt-0.5 text-xs text-zinc-500" role="status">{lyricsSaved}</p>{/if}
        </div>
        <button
          type="button"
          class="shrink-0 rounded-lg border border-zinc-200 px-3 py-1.5 text-xs font-medium hover:bg-zinc-100 disabled:opacity-50 dark:border-zinc-700 dark:hover:bg-zinc-800"
          disabled={lyricsBusy !== null || appSettings.lyricsSave === 'off'}
          onclick={() => void saveCachedLyrics()}
        >
          {lyricsBusy === 'save' ? t('Saving…') : t('Save')}
        </button>
      </div>

      <div class="flex flex-wrap items-start justify-between gap-3">
        <div class="min-w-0 flex-1">
          <p class="text-sm font-medium">{t("Lyrics coverage")}</p>
          <p class="text-sm text-zinc-500" role="status">
            {#if coverage}
              {t('{with} with lyrics · {without} checked, none found · {unchecked} not looked up yet', {
                with: coverage.withLyrics.toLocaleString(i18n.locale),
                without: coverage.withoutLyrics.toLocaleString(i18n.locale),
                unchecked: coverage.unchecked.toLocaleString(i18n.locale),
              })}
            {:else}
              {t("How many songs of your library have lyrics.")}
            {/if}
          </p>
        </div>
        <button
          type="button"
          class="shrink-0 rounded-lg border border-zinc-200 px-3 py-1.5 text-xs font-medium hover:bg-zinc-100 disabled:opacity-50 dark:border-zinc-700 dark:hover:bg-zinc-800"
          disabled={lyricsBusy !== null}
          onclick={() => void countLyrics()}
        >
          {lyricsBusy === 'count' ? t('Counting…') : t('Count')}
        </button>
      </div>
      {#if lyricsToolError}
        <p class="text-xs text-red-600 dark:text-red-400" role="alert">{lyricsToolError}</p>
      {/if}
    </section>

    {#if appSettings.platform === 'android'}
      <section aria-labelledby="set-storage" class="flex flex-col gap-3">
        <h3 id="set-storage" class="text-xs font-medium uppercase tracking-wide text-zinc-400">{t("Storage")}</h3>
        <div class="flex items-start justify-between gap-4">
          <span class="min-w-0 text-sm">
            <span class="block font-medium">{t("Access to all files")}</span>
            <span class="block text-zinc-500">
              {t("Needed to edit tags and to attach .lrc lyrics next to your songs. Playing music works without it.")}
            </span>
          </span>
          {#if fileAccess}
            <span class="shrink-0 text-sm font-medium text-teal-600 dark:text-teal-400">{t("Allowed")}</span>
          {:else}
            <button
              class="shrink-0 rounded-lg bg-teal-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-teal-500"
              onclick={() => void invoke('app_request_file_access').catch((e) => (player.error = String(e)))}
            >
              {t("Allow")}
            </button>
          {/if}
        </div>
      </section>
    {/if}

    {#if appSettings.platform !== 'android'}
    <section aria-labelledby="set-window" class="flex flex-col gap-3">
      <h3 id="set-window" class="text-xs font-medium uppercase tracking-wide text-zinc-400">{t("Window")}</h3>
      <label class="flex cursor-pointer items-start justify-between gap-6">
        <span class="min-w-0">
          <span class="block text-sm font-medium">{t("Keep playing in the tray when the window is closed")}</span>
          <span id="tray-help" class="block text-sm text-zinc-500">
            {t('Closing the window hides FooModern to the system tray and playback continues. Use the tray menu to show the window again or to quit.')}
            {#if appSettings.loaded && !appSettings.trayAvailable}
              <span class="mt-1 block text-amber-700 dark:text-amber-400" role="status">
                {t("No system tray is available here, so closing the window always quits.")}
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

    <section aria-labelledby="set-online" class="flex flex-col gap-3">
      <h3 id="set-online" class="text-xs font-medium uppercase tracking-wide text-zinc-400">
        {t('Search & Downloads')}
      </h3>
      {#if !appSettings.allowOnline}
        <p class="text-xs text-zinc-500" role="status">{t("Turned off in Settings")}</p>
      {/if}
      <div class="flex flex-col gap-4">
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="min-w-0 flex-1">
            <p class="text-sm font-medium">{t('Download folder')}</p>
            <p class="mt-0.5 truncate text-xs text-zinc-500">
              {online.status?.downloadDir ?? t('Loading…')}
            </p>
          </div>
          <div class="flex shrink-0 items-center gap-2">
            <button
              type="button"
              class="rounded-lg border border-zinc-200 px-3 py-1.5 text-xs font-medium hover:bg-zinc-100 dark:border-zinc-700 dark:hover:bg-zinc-800"
              onclick={() => void pickDownloadDir()}
            >
              {t('Choose…')}
            </button>
            <button
              type="button"
              class="rounded-lg px-2.5 py-1.5 text-xs text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
              onclick={() => void online.setDownloadDir(null)}
            >
              {t('Reset')}
            </button>
          </div>
        </div>

        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="min-w-0 flex-1">
            <p class="text-sm font-medium">yt-dlp</p>
            <p class="mt-0.5 text-xs text-zinc-500">
              {#if !appSettings.allowOnline}
                {t('Not checked')}
              {:else if online.status?.ytdlpAvailable}
                {t('Ready')}
                {online.status.ytdlpVersion ? ` (${online.status.ytdlpVersion})` : ''} ·
                {online.status.ffmpegAvailable ? t('ffmpeg available') : t('ffmpeg missing')}
              {:else}
                {t('Not found')} ·
                {online.status?.ffmpegAvailable ? t('ffmpeg available') : t('ffmpeg missing')}
              {/if}
            </p>
            {#if online.status?.ytdlpPath}
              <p class="mt-0.5 truncate text-xs text-zinc-400">{online.status.ytdlpPath}</p>
            {/if}
          </div>
          <div class="flex shrink-0 items-center gap-2">
            <button
              type="button"
              class="rounded-lg border border-zinc-200 px-3 py-1.5 text-xs font-medium hover:bg-zinc-100 dark:border-zinc-700 dark:hover:bg-zinc-800"
              onclick={() => void pickYtdlpPath()}
            >
              {t('Choose…')}
            </button>
            {#if online.status?.ytdlpPath}
              <button
                type="button"
                class="rounded-lg px-2.5 py-1.5 text-xs text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
                onclick={() => void online.setYtdlpPath(null)}
              >
                {t('Reset')}
              </button>
            {/if}
            <button
              type="button"
              class="rounded-lg px-2.5 py-1.5 text-xs text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
              onclick={() => void online.refreshStatus()}
            >
              {t('Refresh')}
            </button>
          </div>
        </div>
      </div>
    </section>
    {/if}

    <section aria-labelledby="set-about" class="flex flex-col gap-3">
      <h3 id="set-about" class="text-xs font-medium uppercase tracking-wide text-zinc-400">{t("About")}</h3>
      <div class="flex flex-wrap items-center gap-3">
        <p class="text-sm">
          <span class="font-medium">FooModern</span>
          <span class="text-zinc-500">{version ? t(' · version {v}', { v: version }) : ''}</span>
        </p>
        <button
          class="rounded-lg px-3 py-1.5 text-sm font-medium text-teal-700 hover:bg-teal-50 disabled:opacity-50 dark:text-teal-300 dark:hover:bg-teal-950/50"
          disabled={updates.checking || !appSettings.allowUpdates}
          title={appSettings.allowUpdates ? undefined : t('Turned off in Settings')}
          onclick={() => void updates.check(true)}
        >
          {updates.checking ? t('Checking…') : t('Check for updates')}
        </button>
        <span class="text-sm text-zinc-500" role="status">
          {#if updates.status === 'upToDate'}{t('You have the latest version.')}
          {:else if !appSettings.allowUpdates}{t('Turned off in Settings')}
          {:else if updates.status === 'failed'}{t('Could not reach GitHub.')}
          {:else if updates.status === 'found' && updates.available}{t('Version {v} is available.', { v: updates.available.version })}{/if}
        </span>
      </div>
      {#if logDir}
        <div class="text-sm">
          <p class="font-medium">{t("Log files")}</p>
          <p class="mt-0.5 text-zinc-500">{t("Attach these when reporting a problem.")}</p>
          <div class="mt-1.5 flex items-center gap-2">
            <code class="min-w-0 flex-1 truncate rounded bg-zinc-100 px-2 py-1 text-xs select-all dark:bg-zinc-800" title={logDir}>{logDir}</code>
            <button class="shrink-0 rounded-lg px-2.5 py-1 text-xs font-medium hover:bg-zinc-200/70 dark:hover:bg-zinc-800" onclick={copyLogDir}>
              {copied ? t('Copied') : t('Copy')}
            </button>
          </div>
        </div>
      {/if}
    </section>
  </div>
</div>
