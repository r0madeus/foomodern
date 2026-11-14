<script lang="ts">
  import { i18n, t, type LangChoice } from './i18n.svelte'
  import { onMount } from 'svelte'
  import { getVersion } from '@tauri-apps/api/app'
  import { open } from '@tauri-apps/plugin-dialog'
  import { invoke } from '@tauri-apps/api/core'
  import Icon, { type Name as IconName } from './Icon.svelte'
  import ToggleSwitch from './ToggleSwitch.svelte'
  import FoldersView from './FoldersView.svelte'
  import { player, type ReplayGain } from './player.svelte'
  import { artists } from './artists.svelte'
  import { prefs } from './prefs.svelte'
  import { appSettings, type LyricsSave, type NetFeature } from './appSettings.svelte'
  import { lyrics, type Coverage } from './lyrics.svelte'
  import { online } from './online.svelte'
  import { updates } from './updates.svelte'
  import { theme, type ThemeChoice } from './theme.svelte'

  type Category = 'all' | 'general' | 'playback' | 'library' | 'internet' | 'about'

  const CATEGORIES: { id: Category; label: string; icon: IconName }[] = [
    { id: 'all', label: 'All', icon: 'sliders' },
    { id: 'general', label: 'General', icon: 'settings' },
    { id: 'playback', label: 'Playback', icon: 'music' },
    { id: 'library', label: 'Library', icon: 'library' },
    { id: 'internet', label: 'Internet', icon: 'globe' },
    { id: 'about', label: 'About', icon: 'info' },
  ]

  let category = $state<Category>('all')
  const show = (c: Exclude<Category, 'all'>) => category === 'all' || category === c

  async function pickDownloadDir() {
    const picked = await open({
      directory: true,
      multiple: false,
      title: t('Choose download folder'),
    })
    if (typeof picked === 'string') {
      await online.setDownloadDir(picked)
    }
  }

  async function pickYtdlpPath() {
    const picked = await open({
      directory: false,
      multiple: false,
      title: t('Locate yt-dlp executable'),
    })
    if (typeof picked === 'string') {
      await online.setYtdlpPath(picked)
    }
  }

  const THEMES: { value: ThemeChoice; label: string }[] = [
    { value: 'system', label: 'System default' },
    { value: 'dark', label: 'Dark' },
    { value: 'light', label: 'Light' },
  ]

  const LANGS: { value: LangChoice; label: string }[] = [
    { value: 'system', label: 'System default' },
    { value: 'en', label: 'English' },
    { value: 'tr', label: 'Türkçe' },
  ]

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

  const cardClass =
    'divide-y divide-zinc-200/70 rounded-2xl border border-zinc-200/80 bg-white/80 shadow-xs dark:divide-zinc-800/80 dark:border-zinc-800/80 dark:bg-zinc-900/60'
  const pillGroup =
    'inline-flex shrink-0 flex-wrap rounded-xl border border-zinc-200/80 bg-zinc-100/80 p-1 dark:border-zinc-800 dark:bg-zinc-950/60'
  const pillBtn = (active: boolean) => [
    'rounded-lg px-3 py-1.5 text-xs font-medium transition-colors outline-none focus-visible:ring-2 focus-visible:ring-teal-500',
    active
      ? 'bg-teal-600 text-white shadow-xs dark:bg-teal-500 dark:text-zinc-950'
      : 'text-zinc-600 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100',
  ]
  const actionBtn =
    'shrink-0 rounded-full border border-zinc-200 bg-white px-3.5 py-1.5 text-xs font-medium text-zinc-700 transition hover:bg-zinc-100 disabled:opacity-50 dark:border-zinc-700 dark:bg-zinc-800/80 dark:text-zinc-200 dark:hover:bg-zinc-700'
</script>

<div class="h-full overflow-y-auto px-4 pb-10 sm:px-6">
  <div class="mx-auto flex max-w-3xl flex-col gap-6">
    <header class="flex flex-col gap-3 pt-1">
      <h2 class="text-2xl font-bold tracking-tight max-sm:hidden">{t('Settings')}</h2>
      <div class="flex gap-1.5 overflow-x-auto pb-1" role="tablist" aria-label={t('Settings')}>
        {#each CATEGORIES as cat (cat.id)}
          <button
            type="button"
            role="tab"
            aria-selected={category === cat.id}
            class={[
              'flex shrink-0 items-center gap-1.5 rounded-full px-3.5 py-1.5 text-xs font-medium transition-colors outline-none focus-visible:ring-2 focus-visible:ring-teal-500',
              category === cat.id
                ? 'bg-teal-600 text-white shadow-xs dark:bg-teal-500 dark:text-zinc-950'
                : 'bg-zinc-200/70 text-zinc-700 hover:bg-zinc-200 dark:bg-zinc-800/80 dark:text-zinc-300 dark:hover:bg-zinc-800',
            ]}
            onclick={() => (category = cat.id)}
          >
            <Icon name={cat.icon} class="size-3.5" />
            {t(cat.label)}
          </button>
        {/each}
      </div>
    </header>

    {#if show('general')}
      <section aria-labelledby="set-general" class="flex flex-col gap-2.5">
        <h3 id="set-general" class="px-1 text-xs font-semibold uppercase tracking-wider text-zinc-400">
          {t('General')}
        </h3>
        <div class={cardClass}>
          {#if appSettings.platform !== 'android'}
            <div class="flex flex-col gap-3 p-4 sm:flex-row sm:items-center sm:justify-between">
              <div class="min-w-0">
                <p id="theme-label" class="text-sm font-medium">{t('Appearance')}</p>
                <p class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
                  {t('Choose whether FooModern follows your system theme or stays dark or light.')}
                </p>
              </div>
              <div role="radiogroup" aria-labelledby="theme-label" class={pillGroup}>
                {#each THEMES as opt (opt.value)}
                  <button
                    type="button"
                    role="radio"
                    aria-checked={theme.choice === opt.value}
                    class={pillBtn(theme.choice === opt.value)}
                    onclick={() => theme.setChoice(opt.value)}
                  >
                    {t(opt.label)}
                  </button>
                {/each}
              </div>
            </div>
          {/if}

          <div class="flex flex-col gap-3 p-4 sm:flex-row sm:items-center sm:justify-between">
            <div class="min-w-0">
              <p id="lang-label" class="text-sm font-medium">{t('Language')}</p>
              <p class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
                {t('Language of the app. "System default" follows the device language.')}
              </p>
            </div>
            <div role="radiogroup" aria-labelledby="lang-label" class={pillGroup}>
              {#each LANGS as opt (opt.value)}
                <button
                  type="button"
                  role="radio"
                  aria-checked={i18n.choice === opt.value}
                  class={pillBtn(i18n.choice === opt.value)}
                  onclick={() => i18n.setChoice(opt.value)}
                >
                  {opt.value === 'system' ? t(opt.label) : opt.label}
                </button>
              {/each}
            </div>
          </div>

          {#if appSettings.platform !== 'android'}
            <div class="flex items-start justify-between gap-6 p-4">
              <div class="min-w-0">
                <p id="tray-label" class="text-sm font-medium">
                  {t('Keep playing in the tray when the window is closed')}
                </p>
                <p id="tray-help" class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
                  {t('Closing the window hides FooModern to the system tray and playback continues. Use the tray menu to show the window again or to quit.')}
                </p>
                {#if appSettings.loaded && !appSettings.trayAvailable}
                  <p class="mt-1.5 inline-flex items-center gap-1.5 rounded-full bg-amber-500/15 px-2.5 py-0.5 text-xs text-amber-700 dark:text-amber-300" role="status">
                    {t('No system tray is available here, so closing the window always quits.')}
                  </p>
                {/if}
              </div>
              <ToggleSwitch
                ariaLabelledby="tray-label"
                ariaDescribedby="tray-help"
                checked={appSettings.closeToTray}
                disabled={!appSettings.loaded}
                onchange={(next) => void appSettings.setCloseToTray(next)}
              />
            </div>
          {/if}
        </div>
      </section>
    {/if}

    {#if show('playback')}
      <section aria-labelledby="set-playback" class="flex flex-col gap-2.5">
        <h3 id="set-playback" class="px-1 text-xs font-semibold uppercase tracking-wider text-zinc-400">
          {t('Playback')}
        </h3>
        <div class={cardClass}>
          <div class="flex flex-col gap-3 p-4 sm:flex-row sm:items-center sm:justify-between">
            <div class="min-w-0">
              <p id="rg-label" class="text-sm font-medium">{t('ReplayGain')}</p>
              <p id="rg-help" class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
                {t('Evens out loudness using ReplayGain tags. Track levels every song; Album keeps the loudness differences inside an album.')}
              </p>
            </div>
            <div role="radiogroup" aria-labelledby="rg-label" aria-describedby="rg-help" class={pillGroup}>
              {#each MODES as m (m.value)}
                <button
                  type="button"
                  role="radio"
                  aria-checked={mode === m.value}
                  class={pillBtn(mode === m.value)}
                  onclick={() => player.setReplayGain(m.value)}
                >
                  {t(m.label)}
                </button>
              {/each}
            </div>
          </div>

          <div class="flex items-start justify-between gap-6 p-4">
            <div class="min-w-0">
              <p id="gapless-label" class="text-sm font-medium">{t('Gapless playback')}</p>
              <p id="gapless-help" class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
                {t('Plays consecutive tracks without gaps or silence. Turn off to reset playback between tracks.')}
              </p>
            </div>
            <ToggleSwitch
              ariaLabelledby="gapless-label"
              ariaDescribedby="gapless-help"
              checked={player.snapshot.gapless}
              onchange={(next) => void player.setGapless(next)}
            />
          </div>

          {#if appSettings.wasapiAvailable}
            <div class="flex items-start justify-between gap-6 p-4">
              <div class="min-w-0">
                <p id="wasapi-label" class="text-sm font-medium">{t('WASAPI exclusive mode')}</p>
                <p id="wasapi-help" class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
                  {t('Bypasses the Windows mixer for direct output at each track’s native format. Other apps cannot play sound while active.')}
                </p>
              </div>
              <ToggleSwitch
                ariaLabelledby="wasapi-label"
                ariaDescribedby="wasapi-help"
                checked={player.snapshot.wasapiExclusive}
                onchange={(next) => void player.setWasapiExclusive(next)}
              />
            </div>
          {/if}
        </div>
      </section>
    {/if}

    {#if show('library')}
      <section aria-labelledby="set-library" class="flex flex-col gap-2.5">
        <h3 id="set-library" class="px-1 text-xs font-semibold uppercase tracking-wider text-zinc-400">
          {t('Library')}
        </h3>
        <div class={cardClass}>
          <div class="flex items-start justify-between gap-6 p-4">
            <div class="min-w-0">
              <p id="ext-label" class="text-sm font-medium">{t('Show file extensions')}</p>
              <p id="ext-help" class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
                {t('Adds a small format badge (FLAC, MP3…) to song rows and album track lists.')}
              </p>
            </div>
            <ToggleSwitch
              ariaLabelledby="ext-label"
              ariaDescribedby="ext-help"
              bind:checked={prefs.showExtensions}
            />
          </div>

          <div class="p-4">
            <FoldersView embedded />
          </div>

          {#if appSettings.platform === 'android'}
            <div class="flex items-start justify-between gap-4 p-4">
              <div class="min-w-0">
                <p class="text-sm font-medium">{t('Access to all files')}</p>
                <p class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
                  {t('Needed to edit tags and to attach .lrc lyrics next to your songs. Playing music works without it.')}
                </p>
              </div>
              {#if fileAccess}
                <span class="inline-flex shrink-0 items-center gap-1.5 rounded-full bg-emerald-500/15 px-3 py-1 text-xs font-medium text-emerald-600 dark:text-emerald-400">
                  <span class="size-1.5 rounded-full bg-emerald-500"></span>
                  {t('Allowed')}
                </span>
              {:else}
                <button
                  type="button"
                  class="shrink-0 rounded-full bg-teal-600 px-3.5 py-1.5 text-xs font-semibold text-white transition hover:bg-teal-500"
                  onclick={() => void invoke('app_request_file_access').catch((e) => (player.error = String(e)))}
                >
                  {t('Allow')}
                </button>
              {/if}
            </div>
          {/if}
        </div>
      </section>
    {/if}

    {#if show('internet')}
      <section aria-labelledby="set-internet" class="flex flex-col gap-2.5">
        <h3 id="set-internet" class="px-1 text-xs font-semibold uppercase tracking-wider text-zinc-400">
          {t('Internet')}
        </h3>
        <div class={cardClass}>
          <div class="flex items-start justify-between gap-6 p-4">
            <div class="min-w-0">
              <p id="offline-label" class="text-sm font-medium">{t('Offline mode')}</p>
              <p id="offline-help" class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
                {t('Nothing connects to the internet')}
              </p>
            </div>
            <ToggleSwitch
              ariaLabelledby="offline-label"
              ariaDescribedby="offline-help"
              checked={appSettings.offlineMode}
              disabled={!appSettings.loaded}
              onchange={(next) => void appSettings.setOfflineMode(next)}
            />
          </div>

          {#each NET_FEATURES.filter((f) => !f.desktopOnly || appSettings.platform !== 'android') as f (f.feature)}
            <div
              class={[
                'flex items-start justify-between gap-6 p-4 transition-opacity',
                appSettings.offlineMode && 'opacity-55',
              ]}
            >
              <div class="min-w-0">
                <p id={`net-${f.feature}-label`} class="text-sm font-medium">{t(f.label)}</p>
                <p id={`net-${f.feature}-help`} class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
                  {t(f.help)}
                </p>
              </div>
              <ToggleSwitch
                ariaLabelledby={`net-${f.feature}-label`}
                ariaDescribedby={`net-${f.feature}-help`}
                checked={NET_VALUE[f.feature]()}
                disabled={!appSettings.loaded || appSettings.offlineMode}
                onchange={(next) => void appSettings.setNetFeature(f.feature, next)}
              />
            </div>
          {/each}

          {#if appSettings.offlineMode}
            <p class="px-4 py-2.5 text-xs text-zinc-500 dark:text-zinc-400" role="status">
              {t('Offline mode is on, so the switches above are off.')}
            </p>
          {/if}

          <div class="flex flex-col gap-3 p-4">
            <div class="flex flex-wrap items-start justify-between gap-4">
              <div class="min-w-0 flex-1">
                <p class="text-sm font-medium">{t('Artist profiles')}</p>
                <p class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
                  {t(
                    'Download artist photos and biographies from Deezer and Wikipedia for artists in your library.',
                  )}
                </p>
              </div>
              <div class="flex shrink-0 flex-wrap items-center gap-2">
                {#if artists.scan.scanning}
                  <button
                    type="button"
                    class={actionBtn}
                    onclick={() => void artists.cancelScan()}
                  >
                    {t('Cancel')}
                  </button>
                {:else}
                  <button
                    type="button"
                    class={actionBtn}
                    disabled={!appSettings.loaded || !appSettings.allowArtistInfo}
                    onclick={() => void artists.refreshAll(false, i18n.locale)}
                  >
                    {t('Refresh missing')}
                  </button>
                  <button
                    type="button"
                    class={actionBtn}
                    disabled={!appSettings.loaded || !appSettings.allowArtistInfo}
                    onclick={() => void artists.refreshAll(true, i18n.locale)}
                  >
                    {t('Rescan all')}
                  </button>
                {/if}
              </div>
            </div>

            {#if artists.scan.scanning}
              <div class="flex flex-col gap-1.5" role="status">
                <div class="flex items-center justify-between gap-2 text-xs text-zinc-500 dark:text-zinc-400">
                  <span class="truncate">
                    {artists.scan.currentArtist ?? ''}
                  </span>
                  <span class="shrink-0 tabular-nums">
                    {artists.scan.done} / {artists.scan.total}
                  </span>
                </div>
                <div class="h-1.5 overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-800">
                  <div
                    class="h-full rounded-full bg-teal-500 transition-[width]"
                    style:width="{artists.scan.total > 0 ? Math.round((artists.scan.done / artists.scan.total) * 100) : 0}%"
                  ></div>
                </div>
              </div>
            {:else if artists.scan.error}
              <p class="text-xs text-red-600 dark:text-red-400" role="alert">
                {artists.scan.error}
              </p>
            {:else if artists.scan.cancelled}
              <p class="text-xs text-zinc-500 dark:text-zinc-400" role="status">
                {t('Artist scan cancelled ({done}/{total}).', {
                  done: artists.scan.done,
                  total: artists.scan.total,
                })}
              </p>
            {:else if artists.scan.total > 0}
              <p class="text-xs text-teal-700 dark:text-teal-400" role="status">
                {t('Updated {updated} of {total} artists.', {
                  updated: artists.scan.updated,
                  total: artists.scan.total,
                })}
              </p>
            {:else if !appSettings.allowArtistInfo && appSettings.loaded}
              <p class="text-xs text-zinc-500 dark:text-zinc-400">
                {t('Turn on “Artist profiles & albums” above to fetch artist info online.')}
              </p>
            {/if}
          </div>
        </div>
      </section>

      <section aria-labelledby="set-lyrics" class="flex flex-col gap-2.5">
        <h3 id="set-lyrics" class="px-1 text-xs font-semibold uppercase tracking-wider text-zinc-400">
          {t('Lyrics')}
        </h3>
        <div class={cardClass}>
          <div class="flex flex-col gap-3 p-4">
            <div class="min-w-0">
              <p id="lyr-save-label" class="text-sm font-medium">{t('Save lyrics')}</p>
              <p id="lyr-save-help" class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
                {t("Lyrics found online are also saved permanently, so they work offline and in other players. An existing .lrc file or lyrics tag is never replaced. Lyrics in a song's tag are written once the song is no longer the one playing.")}
              </p>
            </div>
            <div
              role="radiogroup"
              aria-labelledby="lyr-save-label"
              aria-describedby="lyr-save-help"
              class="grid grid-cols-1 gap-2 sm:grid-cols-3"
            >
              {#each LYRICS_SAVE as m (m.value)}
                {@const active = appSettings.lyricsSave === m.value}
                <button
                  type="button"
                  role="radio"
                  aria-checked={active}
                  disabled={!appSettings.loaded}
                  class={[
                    'flex items-center gap-2.5 rounded-xl border px-3 py-2.5 text-left text-xs font-medium transition-colors outline-none focus-visible:ring-2 focus-visible:ring-teal-500 disabled:opacity-50',
                    active
                      ? 'border-teal-500/70 bg-teal-50/70 text-teal-900 dark:border-teal-500/60 dark:bg-teal-950/40 dark:text-teal-200'
                      : 'border-zinc-200/80 text-zinc-700 hover:bg-zinc-100/70 dark:border-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-800/50',
                  ]}
                  onclick={() => void appSettings.setLyricsSave(m.value)}
                >
                  <span
                    class={[
                      'flex size-4 shrink-0 items-center justify-center rounded-full border',
                      active
                        ? 'border-teal-600 bg-teal-600 dark:border-teal-400 dark:bg-teal-400'
                        : 'border-zinc-400 dark:border-zinc-600',
                    ]}
                  >
                    {#if active}
                      <span class="size-1.5 rounded-full bg-white dark:bg-zinc-950"></span>
                    {/if}
                  </span>
                  <span class="min-w-0 leading-snug">{t(m.label)}</span>
                </button>
              {/each}
            </div>
          </div>

          <div
            class={[
              'flex items-start justify-between gap-6 p-4 transition-opacity',
              !appSettings.allowLyrics && 'opacity-55',
            ]}
          >
            <div class="min-w-0">
              <p id="netease-label" class="text-sm font-medium">{t('NetEase Cloud Music')}</p>
              <p id="netease-help" class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
                {t('Also search NetEase Cloud Music (unofficial; used when LRCLIB has nothing)')}
              </p>
              {#if !appSettings.allowLyrics}
                <span class="mt-1 inline-block text-xs text-zinc-500" role="status">
                  {t('Turned off in Settings')}
                </span>
              {/if}
            </div>
            <ToggleSwitch
              ariaLabelledby="netease-label"
              ariaDescribedby="netease-help"
              checked={appSettings.lyricsNetease}
              disabled={!appSettings.loaded || !appSettings.allowLyrics}
              onchange={(next) => void appSettings.setLyricsNetease(next)}
            />
          </div>

          <div class="flex flex-wrap items-center justify-between gap-3 p-4">
            <div class="min-w-0 flex-1">
              <p class="text-sm font-medium">{t('Save cached lyrics to songs')}</p>
              <p class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
                {t('Writes the lyrics already found for your library with the setting above. Songs that already have lyrics are skipped.')}
              </p>
              {#if lyricsSaved}
                <p class="mt-1 text-xs font-medium text-teal-600 dark:text-teal-400" role="status">
                  {lyricsSaved}
                </p>
              {/if}
            </div>
            <button
              type="button"
              class={actionBtn}
              disabled={lyricsBusy !== null || appSettings.lyricsSave === 'off'}
              onclick={() => void saveCachedLyrics()}
            >
              {lyricsBusy === 'save' ? t('Saving…') : t('Save')}
            </button>
          </div>

          <div class="flex flex-wrap items-center justify-between gap-3 p-4">
            <div class="min-w-0 flex-1">
              <p class="text-sm font-medium">{t('Lyrics coverage')}</p>
              <p class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400" role="status">
                {#if coverage}
                  {t('{with} with lyrics · {without} checked, none found · {unchecked} not looked up yet', {
                    with: coverage.withLyrics.toLocaleString(i18n.locale),
                    without: coverage.withoutLyrics.toLocaleString(i18n.locale),
                    unchecked: coverage.unchecked.toLocaleString(i18n.locale),
                  })}
                {:else}
                  {t('How many songs of your library have lyrics.')}
                {/if}
              </p>
            </div>
            <button
              type="button"
              class={actionBtn}
              disabled={lyricsBusy !== null}
              onclick={() => void countLyrics()}
            >
              {lyricsBusy === 'count' ? t('Counting…') : t('Count')}
            </button>
          </div>

          {#if lyricsToolError}
            <p class="px-4 py-2.5 text-xs text-red-600 dark:text-red-400" role="alert">
              {lyricsToolError}
            </p>
          {/if}
        </div>
      </section>

      {#if appSettings.platform !== 'android'}
        <section aria-labelledby="set-online" class="flex flex-col gap-2.5">
          <h3 id="set-online" class="px-1 text-xs font-semibold uppercase tracking-wider text-zinc-400">
            {t('Search & Downloads')}
          </h3>
          <div class={cardClass}>
            {#if !appSettings.allowOnline}
              <p class="px-4 py-2.5 text-xs text-zinc-500 dark:text-zinc-400" role="status">
                {t('Turned off in Settings')}
              </p>
            {/if}

            <div class="flex flex-wrap items-center justify-between gap-3 p-4">
              <div class="min-w-0 flex-1">
                <p class="text-sm font-medium">{t('Download folder')}</p>
                <p class="mt-0.5 truncate text-xs text-zinc-500 dark:text-zinc-400">
                  {online.status?.downloadDir ?? t('Loading…')}
                </p>
              </div>
              <div class="flex shrink-0 items-center gap-2">
                <button type="button" class={actionBtn} onclick={() => void pickDownloadDir()}>
                  {t('Choose…')}
                </button>
                <button
                  type="button"
                  class="rounded-full px-3 py-1.5 text-xs text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
                  onclick={() => void online.setDownloadDir(null)}
                >
                  {t('Reset')}
                </button>
              </div>
            </div>

            <div class="flex flex-wrap items-center justify-between gap-3 p-4">
              <div class="min-w-0 flex-1">
                <div class="flex flex-wrap items-center gap-2">
                  <p class="text-sm font-medium">yt-dlp</p>
                  {#if !appSettings.allowOnline}
                    <span class="inline-flex items-center gap-1 rounded-full bg-zinc-200/80 px-2 py-0.5 text-[11px] font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400">
                      {t('Not checked')}
                    </span>
                  {:else if online.status?.ytdlpAvailable}
                    <span class="inline-flex items-center gap-1 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[11px] font-medium text-emerald-600 dark:text-emerald-400">
                      <span class="size-1.5 rounded-full bg-emerald-500"></span>
                      {t('Ready')}{online.status.ytdlpVersion ? ` (${online.status.ytdlpVersion})` : ''}
                    </span>
                  {:else}
                    <span class="inline-flex items-center gap-1 rounded-full bg-amber-500/15 px-2 py-0.5 text-[11px] font-medium text-amber-600 dark:text-amber-400">
                      <span class="size-1.5 rounded-full bg-amber-500"></span>
                      {t('Not found')}
                    </span>
                  {/if}
                  {#if appSettings.allowOnline && online.status}
                    <span
                      class={[
                        'inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium',
                        online.status.ffmpegAvailable
                          ? 'bg-emerald-500/15 text-emerald-600 dark:text-emerald-400'
                          : 'bg-amber-500/15 text-amber-600 dark:text-amber-400',
                      ]}
                    >
                      {online.status.ffmpegAvailable ? t('ffmpeg available') : t('ffmpeg missing')}
                    </span>
                  {/if}
                </div>
                {#if online.status?.ytdlpPath}
                  <p class="mt-1 truncate text-xs text-zinc-400">{online.status.ytdlpPath}</p>
                {/if}
              </div>
              <div class="flex shrink-0 items-center gap-2">
                <button type="button" class={actionBtn} onclick={() => void pickYtdlpPath()}>
                  {t('Choose…')}
                </button>
                {#if online.status?.ytdlpPath}
                  <button
                    type="button"
                    class="rounded-full px-3 py-1.5 text-xs text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
                    onclick={() => void online.setYtdlpPath(null)}
                  >
                    {t('Reset')}
                  </button>
                {/if}
                <button
                  type="button"
                  class="rounded-full px-3 py-1.5 text-xs text-zinc-500 hover:bg-zinc-100 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
                  onclick={() => void online.refreshStatus()}
                >
                  {t('Refresh')}
                </button>
              </div>
            </div>
          </div>
        </section>
      {/if}
    {/if}

    {#if show('about')}
      <section aria-labelledby="set-about" class="flex flex-col gap-2.5">
        <h3 id="set-about" class="px-1 text-xs font-semibold uppercase tracking-wider text-zinc-400">
          {t('About')}
        </h3>
        <div class={cardClass}>
          <div class="flex flex-wrap items-center justify-between gap-3 p-4">
            <div class="flex items-center gap-3">
              <span class="flex size-10 items-center justify-center rounded-xl bg-gradient-to-br from-teal-400 to-emerald-600 text-white shadow-sm">
                <Icon name="music" class="size-5" />
              </span>
              <div>
                <div class="flex items-center gap-2">
                  <p class="text-sm font-semibold">FooModern</p>
                  {#if version}
                    <span class="rounded-full bg-zinc-200/80 px-2 py-0.5 text-[11px] font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300">
                      v{version}
                    </span>
                  {/if}
                </div>
                <p class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400" role="status">
                  {#if updates.status === 'upToDate'}
                    {t('You have the latest version.')}
                  {:else if !appSettings.allowUpdates}
                    {t('Turned off in Settings')}
                  {:else if updates.status === 'failed'}
                    {t('Could not reach GitHub.')}
                  {:else if updates.status === 'found' && updates.available}
                    {t('Version {v} is available.', { v: updates.available.version })}
                  {/if}
                </p>
              </div>
            </div>

            <button
              type="button"
              class="rounded-full bg-teal-500/15 px-4 py-1.5 text-xs font-semibold text-teal-700 transition hover:bg-teal-500/25 disabled:opacity-50 dark:text-teal-300"
              disabled={updates.checking || !appSettings.allowUpdates}
              title={appSettings.allowUpdates ? undefined : t('Turned off in Settings')}
              onclick={() => void updates.check(true)}
            >
              {updates.checking ? t('Checking…') : t('Check for updates')}
            </button>
          </div>

          {#if logDir}
            <div class="p-4 text-sm">
              <p class="font-medium">{t('Log files')}</p>
              <p class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
                {t('Attach these when reporting a problem.')}
              </p>
              <div class="mt-2 flex items-center gap-2">
                <code
                  class="min-w-0 flex-1 truncate rounded-xl bg-zinc-100 px-3 py-2 text-xs select-all dark:bg-zinc-950/70"
                  title={logDir}
                >
                  {logDir}
                </code>
                <button type="button" class={actionBtn} onclick={copyLogDir}>
                  {copied ? t('Copied') : t('Copy')}
                </button>
              </div>
            </div>
          {/if}
        </div>
      </section>
    {/if}
  </div>
</div>
