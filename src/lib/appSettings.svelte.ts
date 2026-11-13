// App-level settings owned by Rust (window behaviour). The UI only shows
// them and sends changes (AGENTS.md rule 1).
import { invoke } from '@tauri-apps/api/core'
import { player } from './player.svelte'

interface AppSettingsPayload {
  closeToTray: boolean
  trayAvailable: boolean
  platform: 'android' | 'desktop'
}

class AppSettings {
  /** Closing the window keeps the app running in the tray. */
  closeToTray = $state(true)
  /** False when no tray icon exists in this run (closing quits anyway). */
  trayAvailable = $state(true)
  /** Android has no tray, no free folder/file picker and a narrow screen. */
  platform = $state<'android' | 'desktop'>('desktop')
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

  #apply(s: AppSettingsPayload): void {
    this.closeToTray = s.closeToTray
    this.trayAvailable = s.trayAvailable
    this.platform = s.platform
    this.loaded = true
  }
}

export const appSettings = new AppSettings()
