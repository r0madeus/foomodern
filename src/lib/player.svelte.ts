// Player state on the UI side. Rust owns the real state (AGENTS.md rule 1);
// this module only mirrors it from `player://state` / `player://position`
// events and sends commands back.
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export type Status = 'playing' | 'paused' | 'stopped'

export interface TrackInfo {
  id: number
  path: string
  title: string
  /** Empty when unknown (files opened directly, untagged files). */
  artist: string
}

export interface PlayerSnapshot {
  status: Status
  currentIndex: number | null
  current: TrackInfo | null
  queueLength: number
  queueVersion: number
  positionSeconds: number
  durationSeconds: number | null
  volume: number
}

interface PositionPayload {
  seconds: number
  duration: number | null
}

const EMPTY: PlayerSnapshot = {
  status: 'stopped',
  currentIndex: null,
  current: null,
  queueLength: 0,
  queueVersion: 0,
  positionSeconds: 0,
  durationSeconds: null,
  volume: 100,
}

/** Queue rows are fetched in pages; never the whole list in one IPC call. */
const PAGE_SIZE = 500

class PlayerStore {
  snapshot = $state<PlayerSnapshot>(EMPTY)
  queue = $state<TrackInfo[]>([])
  error = $state<string | null>(null)

  // Position arrives a few times per second; in between we interpolate
  // from the last update so the seek bar moves smoothly (rule 5).
  #anchor = $state({ seconds: 0, at: 0 })
  #now = $state(0)
  #frame = 0
  #loadedQueueVersion = -1

  position = $derived.by(() => {
    const { seconds, at } = this.#anchor
    const elapsed = this.snapshot.status === 'playing' ? (this.#now - at) / 1000 : 0
    const duration = this.snapshot.durationSeconds ?? Infinity
    return Math.max(0, Math.min(seconds + Math.max(0, elapsed), duration))
  })

  async init(): Promise<void> {
    await listen<PlayerSnapshot>('player://state', (e) => this.#apply(e.payload))
    await listen<string>('player://error', (e) => (this.error = e.payload))
    await listen<PositionPayload>('player://position', (e) => {
      this.#setAnchor(e.payload.seconds)
      if (e.payload.duration !== null) this.snapshot.durationSeconds = e.payload.duration
    })
    await this.#run('player_snapshot')
  }

  /** Apply a snapshot returned by a command issued outside this store (e.g. library play). */
  applySnapshot = (snapshot: PlayerSnapshot) => {
    this.#apply(snapshot)
    this.error = null
  }
  openFiles = (paths: string[]) => this.#run('queue_replace', { paths, startIndex: 0 })
  appendFiles = (paths: string[]) => this.#run('queue_append', { paths })
  playIndex = (index: number) => this.#run('player_play_index', { index })
  removeIndex = (index: number) => this.#run('queue_remove', { index })
  togglePause = () => this.#run('player_toggle_pause')
  next = () => this.#run('player_next')
  previous = () => this.#run('player_previous')
  setVolume = (percent: number) => this.#run('player_set_volume', { percent })
  seek = (seconds: number) => {
    this.#setAnchor(seconds)
    return this.#run('player_seek', { seconds })
  }

  async #run(command: string, args?: Record<string, unknown>): Promise<void> {
    try {
      this.#apply(await invoke<PlayerSnapshot>(command, args))
      this.error = null
    } catch (e) {
      this.error = String(e)
    }
  }

  #apply(next: PlayerSnapshot): void {
    const wasPlaying = this.snapshot.status === 'playing'
    this.snapshot = next
    this.#setAnchor(next.positionSeconds)
    if (next.queueVersion !== this.#loadedQueueVersion) void this.#loadQueue(next.queueVersion)
    if (next.status === 'playing' && !wasPlaying) this.#startClock()
    if (next.status !== 'playing') cancelAnimationFrame(this.#frame)
  }

  #setAnchor(seconds: number): void {
    const at = performance.now()
    this.#anchor = { seconds, at }
    this.#now = at
  }

  #startClock(): void {
    cancelAnimationFrame(this.#frame)
    const tick = (t: number) => {
      this.#now = t
      if (this.snapshot.status === 'playing') this.#frame = requestAnimationFrame(tick)
    }
    this.#frame = requestAnimationFrame(tick)
  }

  async #loadQueue(version: number): Promise<void> {
    this.#loadedQueueVersion = version
    const rows: TrackInfo[] = []
    for (let offset = 0; offset < this.snapshot.queueLength; offset += PAGE_SIZE) {
      const page = await invoke<TrackInfo[]>('queue_page', { offset, limit: PAGE_SIZE })
      if (this.#loadedQueueVersion !== version) return // a newer load started
      rows.push(...page)
      if (page.length < PAGE_SIZE) break
    }
    this.queue = rows
  }
}

export const player = new PlayerStore()

export function formatTime(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds)) return '–:––'
  const s = Math.floor(seconds)
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const sec = String(s % 60).padStart(2, '0')
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`
}
