// Online search, streaming, and album/track download state on the UI side.
// Rust owns the download queue and tool resolution; this store mirrors
// `online://downloads` events and sends commands back.
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { AlbumRow, TrackRow } from './library.svelte'
import { player, type PlayerSnapshot } from './player.svelte'

export interface OnlineStatus {
  ytdlpAvailable: boolean
  ytdlpVersion: string | null
  ffmpegAvailable: boolean
  downloadDir: string
  ytdlpPath: string | null
}

export interface OnlineAlbum {
  id: string
  title: string
  artist: string
  year: number | null
  trackCount: number
  cover: string | null
  coverUrl: string | null
  sourceUrl: string | null
}

export interface OnlineTrack {
  id: string
  title: string
  artist: string
  album: string
  albumArtist: string
  trackNo: number | null
  discNo: number | null
  year: number | null
  durationSeconds: number | null
  sourceUrl: string | null
  cover: string | null
  coverUrl: string | null
}

export interface SearchResults {
  albums: OnlineAlbum[]
  tracks: OnlineTrack[]
}

export interface OnlineAlbumDetail {
  album: OnlineAlbum
  tracks: OnlineTrack[]
}

export type DownloadKind = 'track' | 'album'
export type DownloadState = 'queued' | 'downloading' | 'done' | 'failed' | 'cancelled'

export interface DownloadJob {
  id: number
  kind: DownloadKind
  title: string
  artist: string
  cover: string | null
  state: DownloadState
  doneTracks: number
  totalTracks: number
  currentTrack: string | null
  error: string | null
  outputDir: string | null
}

class OnlineStore {
  status = $state<OnlineStatus | null>(null)
  query = $state('')
  searchedQuery = $state('')
  searching = $state(false)
  searchError = $state<string | null>(null)
  results = $state<SearchResults | null>(null)

  selectedAlbum = $state<OnlineAlbum | null>(null)
  albumDetail = $state<OnlineAlbumDetail | null>(null)
  loadingAlbum = $state(false)
  albumError = $state<string | null>(null)

  playingId = $state<string | null>(null)
  downloads = $state<DownloadJob[]>([])
  error = $state<string | null>(null)

  #initialized = false
  #searchSeq = 0
  #albumSeq = 0

  async init(): Promise<void> {
    if (!this.#initialized) {
      this.#initialized = true
      await listen<DownloadJob[]>('online://downloads', (e) => {
        this.downloads = e.payload
      })
    }
    await Promise.all([this.refreshStatus(), this.refreshDownloads()])
  }

  async refreshStatus(): Promise<void> {
    try {
      this.status = await invoke<OnlineStatus>('online_status')
      this.error = null
    } catch (e) {
      this.error = String(e)
    }
  }

  async refreshDownloads(): Promise<void> {
    try {
      this.downloads = await invoke<DownloadJob[]>('online_downloads')
    } catch {
      // Desktop only; ignore if unconfigured.
    }
  }

  async setDownloadDir(dir: string | null): Promise<void> {
    try {
      this.status = await invoke<OnlineStatus>('online_set_download_dir', { dir })
      this.error = null
    } catch (e) {
      this.error = String(e)
    }
  }

  async setYtdlpPath(path: string | null): Promise<void> {
    try {
      this.status = await invoke<OnlineStatus>('online_set_ytdlp_path', { path })
      this.error = null
    } catch (e) {
      this.error = String(e)
    }
  }

  async search(nextQuery?: string): Promise<void> {
    const q = (nextQuery ?? this.query).trim()
    this.query = nextQuery ?? this.query
    if (!q) {
      this.clearSearch()
      return
    }
    const seq = ++this.#searchSeq
    this.searchedQuery = q
    this.searching = true
    this.searchError = null
    this.closeAlbum()
    try {
      const res = await invoke<SearchResults>('online_search', { query: q })
      if (seq === this.#searchSeq) {
        this.results = res
      }
    } catch (e) {
      if (seq === this.#searchSeq) {
        this.searchError = String(e)
      }
    } finally {
      if (seq === this.#searchSeq) {
        this.searching = false
      }
    }
  }

  clearSearch(): void {
    this.#searchSeq++
    this.query = ''
    this.searchedQuery = ''
    this.searching = false
    this.searchError = null
    this.results = null
    this.closeAlbum()
  }

  async openAlbum(album: OnlineAlbum): Promise<void> {
    const seq = ++this.#albumSeq
    this.selectedAlbum = album
    this.albumDetail = null
    this.albumError = null
    this.loadingAlbum = true
    try {
      const detail = await invoke<OnlineAlbumDetail>('online_album_detail', { album })
      if (seq === this.#albumSeq) {
        this.albumDetail = detail
      }
    } catch (e) {
      if (seq === this.#albumSeq) {
        this.albumError = String(e)
      }
    } finally {
      if (seq === this.#albumSeq) {
        this.loadingAlbum = false
      }
    }
  }

  closeAlbum(): void {
    this.#albumSeq++
    this.selectedAlbum = null
    this.albumDetail = null
    this.loadingAlbum = false
    this.albumError = null
  }

  /** Plays `tracks[index]`; the tracks after it follow in the queue. */
  async playTracks(tracks: OnlineTrack[], index: number): Promise<void> {
    const track = tracks[index]
    if (!track) return
    this.playingId = track.id
    this.error = null
    try {
      const snap = await invoke<PlayerSnapshot>('online_play_tracks', {
        tracks,
        startIndex: index,
      })
      player.applySnapshot(snap)
    } catch (e) {
      this.error = String(e)
      player.error = String(e)
    } finally {
      if (this.playingId === track.id) {
        this.playingId = null
      }
    }
  }

  async downloadTrack(track: OnlineTrack): Promise<void> {
    this.error = null
    try {
      const job = await invoke<DownloadJob>('online_download_track', { track })
      this.#upsertJob(job)
    } catch (e) {
      this.error = String(e)
    }
  }

  async downloadAlbum(album: OnlineAlbum, tracks: OnlineTrack[]): Promise<void> {
    this.error = null
    try {
      const job = await invoke<DownloadJob>('online_download_album', { album, tracks })
      this.#upsertJob(job)
    } catch (e) {
      this.error = String(e)
    }
  }

  async cancelDownload(id: number): Promise<void> {
    try {
      this.downloads = await invoke<DownloadJob[]>('online_cancel_download', { id })
    } catch (e) {
      this.error = String(e)
    }
  }

  async clearFinished(): Promise<void> {
    try {
      this.downloads = await invoke<DownloadJob[]>('online_clear_finished')
    } catch (e) {
      this.error = String(e)
    }
  }

  jobFor(kind: DownloadKind, title: string, artist: string): DownloadJob | null {
    const matches = this.downloads.filter(
      (j) => j.kind === kind && j.title === title && j.artist === artist,
    )
    return (
      matches.find((j) => j.state === 'downloading' || j.state === 'queued') ??
      matches[0] ??
      null
    )
  }

  #upsertJob(job: DownloadJob): void {
    const rest = this.downloads.filter((j) => j.id !== job.id)
    this.downloads = [job, ...rest]
  }
}

export const online = new OnlineStore()

function normMatch(s: string): string {
  return s.trim().toLowerCase().replace(/\s+/g, ' ')
}

export function isAlbumInLibrary(
  album: { title: string; artist: string },
  localAlbums: AlbumRow[],
): boolean {
  const title = normMatch(album.title)
  const artist = normMatch(album.artist)
  if (!title) return false
  return localAlbums.some((a) => {
    if (normMatch(a.title) !== title) return false
    if (!artist) return true
    const localArtist = normMatch(a.albumArtist)
    return !localArtist || localArtist.includes(artist) || artist.includes(localArtist)
  })
}

export function isTrackInLibrary(
  track: { title: string; artist: string },
  localTracks: TrackRow[],
): boolean {
  const title = normMatch(track.title)
  const artist = normMatch(track.artist)
  if (!title) return false
  return localTracks.some((t) => {
    if (normMatch(t.title) !== title) return false
    if (!artist) return true
    const localArtist = normMatch(t.artist)
    return !localArtist || localArtist.includes(artist) || artist.includes(localArtist)
  })
}
