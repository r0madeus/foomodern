// Lyrics state for the currently playing track (AGENTS.md rule 1: Rust handles
// local .lrc lookup, LRCLIB requests and disk caching).
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { appSettings } from './appSettings.svelte'

export type LyricsSource = 'localLrc' | 'lrclib'

export interface LyricLine {
  timeSeconds: number
  text: string
}

export type LyricsResult =
  | { kind: 'synced'; source: LyricsSource; lines: LyricLine[] }
  | { kind: 'plain'; source: LyricsSource; lines: string[] }
  | { kind: 'instrumental'; source: LyricsSource }
  | { kind: 'notFound' }

/** Index of the synced line playing at `pos` seconds; -1 before the first. */
export function findActiveIndex(lines: LyricLine[], pos: number): number {
  const t = pos + 0.05
  let lo = 0
  let hi = lines.length - 1
  let ans = -1
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    if (lines[mid].timeSeconds <= t) {
      ans = mid
      lo = mid + 1
    } else {
      hi = mid - 1
    }
  }
  return ans
}

const KEY = 'foomodern.lyricsOpen'

function loadOpen(): boolean {
  try {
    return localStorage.getItem(KEY) === '1'
  } catch {
    return false
  }
}

class LyricsStore {
  open = $state<boolean>(loadOpen())
  loading = $state(false)
  error = $state<string | null>(null)
  result = $state<LyricsResult | null>(null)
  trackPath = $state<string | null>(null)
  /** Load lyrics even while the panel is closed (the phone player shows them inline). */
  always = false

  #requestSeq = 0

  toggle(): void {
    this.setOpen(!this.open)
  }

  setOpen(next: boolean): void {
    this.open = next
    try {
      localStorage.setItem(KEY, next ? '1' : '0')
    } catch {
      // ignore
    }
  }

  async load(path: string | null, durationSeconds: number | null, force = false): Promise<void> {
    if ((!this.open && !this.always) || path === null) {
      if (path === null) {
        this.#requestSeq++
        this.trackPath = null
        this.result = null
        this.error = null
        this.loading = false
      }
      return
    }

    if (!force && path === this.trackPath && (this.loading || this.result !== null)) {
      return
    }

    const seq = ++this.#requestSeq
    this.trackPath = path
    this.loading = true
    this.error = null
    if (!force) this.result = null

    try {
      const res = await invoke<LyricsResult>('lyrics_for_track', {
        path,
        durationSeconds,
        force,
      })
      if (seq !== this.#requestSeq) return
      this.result = res
    } catch (e) {
      if (seq !== this.#requestSeq) return
      this.error = String(e)
      if (!force) this.result = null
    } finally {
      if (seq === this.#requestSeq) this.loading = false
    }
  }

  /** Lets the user pick a .lrc file for the track at `path` and attaches it. */
  async pickAndAttach(path: string): Promise<void> {
    if (this.loading) return
    let picked: string | null = null
    try {
      picked =
        appSettings.platform === 'android'
          ? await invoke<string | null>('android_pick_file')
          : ((await open({
              multiple: false,
              directory: false,
              filters: [{ name: 'Lyrics', extensions: ['lrc', 'txt'] }],
            })) as string | null)
    } catch (e) {
      this.error = String(e)
      return
    }
    if (!picked) return
    await this.attachFile(path, picked)
  }

  async attachFile(path: string, lrcPath: string): Promise<void> {
    const seq = ++this.#requestSeq
    this.trackPath = path
    this.loading = true
    this.error = null

    try {
      const res = await invoke<LyricsResult>('lyrics_attach_file', {
        audioPath: path,
        lrcPath,
      })
      if (seq !== this.#requestSeq) return
      this.result = res
    } catch (e) {
      if (seq !== this.#requestSeq) return
      this.error = String(e)
    } finally {
      if (seq === this.#requestSeq) this.loading = false
    }
  }
}

export const lyrics = new LyricsStore()
