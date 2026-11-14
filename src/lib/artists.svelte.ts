import { invoke } from '@tauri-apps/api/core'
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

export { splitArtistNames } from './artistNames'

class ArtistStore {
  #navigator: ((name: string) => void) | null = null

  registerNavigator(fn: (name: string) => void) {
    this.#navigator = fn
  }

  open(name: string) {
    const clean = name.trim()
    if (!clean) return
    this.#navigator?.(clean)
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
