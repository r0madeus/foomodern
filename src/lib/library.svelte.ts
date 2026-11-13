// Library state on the UI side. Rust owns the library (AGENTS.md rule 1);
// this module mirrors stats/folders and caches pages of rows.
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { appSettings } from './appSettings.svelte'
import { player, type PlayerSnapshot } from './player.svelte'

export type Sort = 'artist' | 'title' | 'album' | 'added'

const SORT_KEY = 'foomodern.library.sort'
const SORTS: Sort[] = ['artist', 'title', 'album', 'added']

function loadSort(): Sort {
  try {
    const v = localStorage.getItem(SORT_KEY)
    if (SORTS.includes(v as Sort)) return v as Sort
  } catch {
    // storage unavailable: fall through
  }
  return 'artist'
}

export interface FolderInfo {
  id: number
  path: string
}

export interface TrackRow {
  id: number
  title: string
  artist: string
  album: string
  albumId: number | null
  trackNo: number | null
  discNo: number | null
  year: number | null
  durationMs: number | null
  path: string
  /** Cached cover of the track's album. */
  cover: string | null
}

export interface AlbumRow {
  id: number
  title: string
  albumArtist: string
  year: number | null
  trackCount: number
  cover: string | null
}

export interface LibraryStats {
  tracks: number
  albums: number
  version: number
  scanning: boolean
}

export interface Page<T> {
  total: number
  items: T[]
}

interface ScanPayload {
  folder: string
  scanned: number
  done: boolean
}

export const PAGE_SIZE = 200

/**
 * Lazily fetched rows. `get(i)` returns the row or `undefined` (render a
 * skeleton) and starts fetching the page that holds it. Create a new
 * instance when the data set changes (new version or sort).
 */
export class PagedRows<T> {
  /** Bumped whenever a page arrives so readers of `get` re-run. */
  #revision = $state(0)
  total = $state<number | null>(null)
  #pages = new Map<number, T[]>()
  #inflight = new Map<number, Promise<void>>()
  #fetcher: (offset: number, limit: number) => Promise<Page<T>>
  #dead = false

  constructor(fetcher: (offset: number, limit: number) => Promise<Page<T>>) {
    this.#fetcher = fetcher
  }

  get(index: number): T | undefined {
    void this.#revision
    const page = Math.floor(index / PAGE_SIZE)
    const rows = this.#pages.get(page)
    if (rows) return rows[index - page * PAGE_SIZE]
    void this.#load(page)
    return undefined
  }

  /** Prefetch the pages covering [start, end). */
  ensureRange(start: number, end: number): void {
    if (end <= start) return
    for (let p = Math.floor(start / PAGE_SIZE); p <= Math.floor((end - 1) / PAGE_SIZE); p++) {
      if (!this.#pages.has(p)) void this.#load(p)
    }
  }

  dispose(): void {
    this.#dead = true
  }

  #load(page: number): Promise<void> {
    const existing = this.#inflight.get(page)
    if (existing) return existing
    const job = this.#fetcher(page * PAGE_SIZE, PAGE_SIZE)
      .then((res) => {
        if (this.#dead) return
        this.total = res.total
        this.#pages.set(page, res.items)
        this.#revision++
      })
      .catch((e) => {
        player.error = String(e)
      })
      .finally(() => this.#inflight.delete(page))
    this.#inflight.set(page, job)
    return job
  }
}

class LibraryStore {
  stats = $state<LibraryStats>({ tracks: 0, albums: 0, version: 0, scanning: false })
  folders = $state<FolderInfo[]>([])
  scan = $state<{ folder: string; scanned: number } | null>(null)
  #sort = $state<Sort>(loadSort())
  query = $state('')
  debouncedQuery = $state('')

  get sort(): Sort {
    return this.#sort
  }

  set sort(next: Sort) {
    this.#sort = next
    try {
      localStorage.setItem(SORT_KEY, next)
    } catch {
      // ignore
    }
  }

  #searchTimer: ReturnType<typeof setTimeout> | undefined

  scanning = $derived(this.stats.scanning || this.scan !== null)

  setQuery(next: string): void {
    this.query = next
    clearTimeout(this.#searchTimer)
    const trimmed = next.trim()
    if (trimmed === '') {
      this.debouncedQuery = ''
      return
    }
    this.#searchTimer = setTimeout(() => {
      this.debouncedQuery = trimmed
    }, 150)
  }

  async init(): Promise<void> {
    await listen<ScanPayload>('library://scan', (e) => {
      const { folder, scanned, done } = e.payload
      this.scan = done ? null : { folder, scanned }
      if (done) void this.refresh()
    })
    await listen<number>('library://changed', () => void this.refresh())
    await this.refresh()
  }

  async refresh(): Promise<void> {
    try {
      const [stats, folders] = await Promise.all([
        invoke<LibraryStats>('library_stats'),
        invoke<FolderInfo[]>('library_folders'),
      ])
      this.stats = stats
      this.folders = folders
    } catch (e) {
      player.error = String(e)
    }
  }

  async addFolder(): Promise<void> {
    try {
      // Android: the system folder picker, mapped to a file path in Rust
      // (after asking for permission to read music).
      const path =
        appSettings.platform === 'android'
          ? await invoke<string | null>('library_android_pick_folder')
          : await open({ directory: true, multiple: false })
      if (!path) return
      this.folders = await invoke<FolderInfo[]>('library_add_folder', { path })
    } catch (e) {
      player.error = String(e)
    }
  }

  async removeFolder(id: number): Promise<void> {
    try {
      this.folders = await invoke<FolderInfo[]>('library_remove_folder', { id })
    } catch (e) {
      player.error = String(e)
    }
  }

  async rescan(): Promise<void> {
    try {
      await invoke('library_rescan')
    } catch (e) {
      player.error = String(e)
    }
  }

  async playAlbum(albumId: number, startIndex: number): Promise<void> {
    await this.#play('library_play_album', { albumId, startIndex })
  }

  async playAll(startIndex: number): Promise<void> {
    const query = this.debouncedQuery
    await this.#play('library_play_all', {
      sort: this.sort,
      startIndex,
      query: query === '' ? null : query,
    })
  }

  tracksPage = (sort: Sort, query: string) => (offset: number, limit: number) =>
    invoke<Page<TrackRow>>('library_tracks_page', {
      offset,
      limit,
      sort,
      query: query === '' ? null : query,
    })

  albumsPage = (query: string) => (offset: number, limit: number) =>
    invoke<Page<AlbumRow>>('library_albums_page', {
      offset,
      limit,
      query: query === '' ? null : query,
    })

  async #play(command: string, args: Record<string, unknown>): Promise<void> {
    try {
      player.applySnapshot(await invoke<PlayerSnapshot>(command, args))
    } catch (e) {
      player.error = String(e)
    }
  }
}

export const library = new LibraryStore()
