// "New version available" notice. Rust asks GitHub for the latest release;
// the UI checks at most once a day and remembers a dismissed version.
import { invoke } from '@tauri-apps/api/core'

export interface UpdateInfo {
  version: string
  url: string
}

const LAST_CHECK = 'foomodern.update.lastCheck'
const DISMISSED = 'foomodern.update.dismissed'
const DAY_MS = 24 * 60 * 60 * 1000

function read(key: string): string | null {
  try {
    return localStorage.getItem(key)
  } catch {
    return null
  }
}

function write(key: string, value: string): void {
  try {
    localStorage.setItem(key, value)
  } catch {
    // ignore
  }
}

class Updates {
  /** Newer release, unless the user dismissed this version. */
  available = $state<UpdateInfo | null>(null)
  checking = $state(false)
  /** Result of the last manual check, for Settings. */
  status = $state<'idle' | 'upToDate' | 'found' | 'failed'>('idle')

  async check(manual = false): Promise<void> {
    if (this.checking) return
    this.checking = true
    try {
      const found = await invoke<UpdateInfo | null>('app_check_update')
      write(LAST_CHECK, String(Date.now()))
      this.status = found ? 'found' : 'upToDate'
      this.available = found && (manual || read(DISMISSED) !== found.version) ? found : null
    } catch {
      this.status = 'failed'
    } finally {
      this.checking = false
    }
  }

  /** Startup: check if the last check is more than a day old. */
  autoCheck(): void {
    const last = Number(read(LAST_CHECK) ?? 0)
    if (Date.now() - last > DAY_MS) void this.check()
  }

  dismiss(): void {
    if (this.available) write(DISMISSED, this.available.version)
    this.available = null
  }

  async open(): Promise<void> {
    if (!this.available) return
    try {
      await invoke('app_open_release', { url: this.available.url })
    } catch {
      // nothing useful to show; the banner stays
    }
  }
}

export const updates = new Updates()
