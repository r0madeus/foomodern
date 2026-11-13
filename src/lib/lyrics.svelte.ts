// Lyrics state for the currently playing track (AGENTS.md rule 1: Rust handles
// local .lrc lookup, LRCLIB requests and disk caching).
import { invoke } from '@tauri-apps/api/core'

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
    if (!this.open || path === null) {
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
