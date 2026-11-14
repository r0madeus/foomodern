// App-level settings owned by Rust (window behaviour). The UI only shows
// them and sends changes (AGENTS.md rule 1).
import { invoke } from '@tauri-apps/api/core'
import { player } from './player.svelte'

interface AppSettingsPayload {
  closeToTray: boolean
  trayAvailable: boolean
  platform: 'android' | 'desktop'
  lyricsSave: LyricsSave
  lyricsNetease: boolean
  offlineMode: boolean
  netLyrics: boolean
  netOnline: boolean
  netArtistInfo: boolean
  netMetadata: boolean
  netUpdates: boolean
  allowLyrics: boolean
  allowOnline: boolean
  allowArtistInfo: boolean
  allowMetadata: boolean
  allowUpdates: boolean
}

/** The internet-related features that can be switched off in Settings. */
export type NetFeature = 'lyrics' | 'online' | 'artistInfo' | 'metadata' | 'updates'

/** Where lyrics found online are kept besides the app's cache. */
export type LyricsSave = 'sidecar' | 'embed' | 'off'

class AppSettings {
  /** Closing the window keeps the app running in the tray. */
  closeToTray = $state(true)
  /** False when no tray icon exists in this run (closing quits anyway). */
  trayAvailable = $state(true)
  /** Android has no tray, no free folder/file picker and a narrow screen. */
  platform = $state<'android' | 'desktop'>('desktop')
  /** Lyrics found online: `.lrc` next to the song, inside its tag, or cache only. */
  lyricsSave = $state<LyricsSave>('sidecar')
  /** Also search NetEase Cloud Music when LRCLIB has nothing. */
  lyricsNetease = $state(true)
  /** Master switch: nothing in the app contacts the internet. */
  offlineMode = $state(false)
  /** The user's own per-feature switches (kept while offline mode is on). */
  netLyrics = $state(true)
  netOnline = $state(true)
  netArtistInfo = $state(true)
  netMetadata = $state(true)
  netUpdates = $state(true)
  /** What Rust really allows now (offline mode applied); controls follow these. */
  allowLyrics = $state(true)
  allowOnline = $state(true)
  allowArtistInfo = $state(true)
  allowMetadata = $state(true)
  allowUpdates = $state(true)
  loaded = $state(false)

  async load(): Promise<void> {
    try {
      this.#apply(await invoke<AppSettingsPayload>('app_settings'))
    } catch (e) {
      player.error = String(e)
    }
  }

  async setCloseToTray(enabled: boolean): Promise<void> {
    try {
      this.#apply(await invoke<AppSettingsPayload>('app_set_close_to_tray', { enabled }))
    } catch (e) {
      player.error = String(e)
      await this.load()
    }
  }

  async setLyricsSave(mode: LyricsSave): Promise<void> {
    try {
      this.#apply(await invoke<AppSettingsPayload>('app_set_lyrics_save', { mode }))
    } catch (e) {
      player.error = String(e)
      await this.load()
    }
  }

  async setLyricsNetease(enabled: boolean): Promise<void> {
    try {
      this.#apply(await invoke<AppSettingsPayload>('app_set_lyrics_netease', { enabled }))
    } catch (e) {
      player.error = String(e)
      await this.load()
    }
  }

  async setOfflineMode(enabled: boolean): Promise<void> {
    try {
      this.#apply(await invoke<AppSettingsPayload>('app_set_offline_mode', { enabled }))
    } catch (e) {
      player.error = String(e)
      await this.load()
    }
  }

  async setNetFeature(feature: NetFeature, enabled: boolean): Promise<void> {
    try {
      this.#apply(await invoke<AppSettingsPayload>('app_set_net_feature', { feature, enabled }))
    } catch (e) {
      player.error = String(e)
      await this.load()
    }
  }

  #apply(s: AppSettingsPayload): void {
    this.lyricsSave = s.lyricsSave
    this.lyricsNetease = s.lyricsNetease
    this.offlineMode = s.offlineMode
    this.netLyrics = s.netLyrics
    this.netOnline = s.netOnline
    this.netArtistInfo = s.netArtistInfo
    this.netMetadata = s.netMetadata
    this.netUpdates = s.netUpdates
    this.allowLyrics = s.allowLyrics
    this.allowOnline = s.allowOnline
    this.allowArtistInfo = s.allowArtistInfo
    this.allowMetadata = s.allowMetadata
    this.allowUpdates = s.allowUpdates
    this.closeToTray = s.closeToTray
    this.trayAvailable = s.trayAvailable
    this.platform = s.platform
    this.loaded = true
  }
}

export const appSettings = new AppSettings()
