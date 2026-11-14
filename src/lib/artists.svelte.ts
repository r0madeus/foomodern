import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { AlbumRow, Page, TrackRow } from './library.svelte'
import type { OnlineAlbum } from './online.svelte'
import { player, type PlayerSnapshot } from './player.svelte'

export interface ArtistRow {
  name: string
  trackCount: number
  albumCount: number
  cover: string | null
  playCount: number
  listenedMs: number
}

export interface ArtistTopTrack {
  track: TrackRow
  playCount: number
  listenedMs: number
}

export interface ArtistProfile {
  name: string
  trackCount: number
  albumCount: number
  totalDurationMs: number
  genres: string[]
  playCount: number
  listenedMs: number
  image: string | null
  bio: string | null
  fetchedAt: number | null
  fallbackCover: string | null
  popularTracks: ArtistTopTrack[]
  albums: AlbumRow[]
  tracks: TrackRow[]
}

export interface ArtistOnlineInfo {
  image: string | null
  bio: string | null
  albums: OnlineAlbum[]
}

export interface ArtistScanProgress {
  scanning: boolean
  done: number
  total: number
  updated: number
  currentArtist: string | null
  cancelled: boolean
  error: string | null
}

export { splitArtistNames } from './artistNames'

const EMPTY_SCAN: ArtistScanProgress = {
  scanning: false,
  done: 0,
  total: 0,
  updated: 0,
  currentArtist: null,
  cancelled: false,
  error: null,
}

class ArtistStore {
  #navigator: ((name: string) => void) | null = null
  #scanInited = false
  scan = $state<ArtistScanProgress>({ ...EMPTY_SCAN })

  registerNavigator(fn: (name: string) => void) {
    this.#navigator = fn
  }

  open(name: string) {
    const clean = name.trim()
    if (!clean) return
    this.#navigator?.(clean)
  }

  async initScan(): Promise<void> {
    if (this.#scanInited) return
    this.#scanInited = true
    try {
      await listen<ArtistScanProgress>('artist://scan', (e) => {
        this.scan = e.payload
      })
      this.scan = await invoke<ArtistScanProgress>('artist_enrich_status')
    } catch {
      // ignore if invoked before backend ready
    }
  }

  async refreshAll(force: boolean, lang: string): Promise<void> {
    try {
      this.scan = await invoke<ArtistScanProgress>('artist_enrich_all', { force, lang })
    } catch (e) {
      this.scan = { ...this.scan, scanning: false, error: String(e) }
    }
  }

  async cancelScan(): Promise<void> {
    try {
      await invoke('artist_enrich_cancel')
    } catch {
      // ignore
    }
  }

  page = (query: string) => (offset: number, limit: number) =>
    invoke<Page<ArtistRow>>('library_artists_page', {
      offset,
      limit,
      query: query === '' ? null : query,
    })

  async profile(name: string): Promise<ArtistProfile> {
    return invoke<ArtistProfile>('library_artist_profile', { name })
  }

  async enrich(name: string, lang: string): Promise<ArtistOnlineInfo> {
    return invoke<ArtistOnlineInfo>('artist_enrich', { name, lang })
  }

  async play(name: string, startIndex = 0, shuffle?: boolean): Promise<void> {
    try {
      const snap = await invoke<PlayerSnapshot>('library_play_artist', { name, startIndex })
      player.applySnapshot(snap)
      if (shuffle !== undefined && snap.shuffle !== shuffle) {
        await player.setShuffle(shuffle)
      }
    } catch (e) {
      player.error = String(e)
    }
  }
}

export const artists = new ArtistStore()
