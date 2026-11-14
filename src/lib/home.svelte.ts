import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { t } from './i18n.svelte'
import type { AlbumRow, TrackRow } from './library.svelte'
import { player, type PlayerSnapshot } from './player.svelte'
import { playlists, type PlaylistInfo } from './playlists.svelte'

export interface DynamicMix {
  id: string
  kind: string
  titleKey: string
  subject: string | null
  subtitleKey: string
  trackCount: number
  albumCovers: string[]
}

export interface RecentTrack {
  track: TrackRow
  startedAt: number
}

export interface HomeFeed {
  recentlyPlayedAlbums: AlbumRow[]
  recentlyPlayedTracks: RecentTrack[]
  recentlyAddedAlbums: AlbumRow[]
  favoriteTracks: TrackRow[]
  favoriteCount: number
  mixes: DynamicMix[]
  hasHistory: boolean
}

export interface HomeMixDetail {
  mix: DynamicMix
  tracks: TrackRow[]
}

export function mixTitle(mix: Pick<DynamicMix, 'titleKey' | 'subject'>): string {
  return mix.subject ? t(mix.titleKey, { name: mix.subject }) : t(mix.titleKey)
}

export function mixSubtitle(mix: Pick<DynamicMix, 'subtitleKey'>): string {
  return t(mix.subtitleKey)
}

export function mixAccent(kind: string): string {
  switch (kind) {
    case 'weekly-favorites':
      return 'from-accent-500/80 via-accent-800/70 to-zinc-900'
    case 'on-repeat':
      return 'from-indigo-500/80 via-violet-700/70 to-zinc-900'
    case 'rediscover':
      return 'from-amber-500/80 via-orange-700/70 to-zinc-900'
    default:
      return 'from-rose-500/80 via-fuchsia-700/70 to-zinc-900'
  }
}

export function greetingKey(hour = new Date().getHours()): string {
  if (hour < 12) return 'Good morning'
  if (hour < 18) return 'Good afternoon'
  return 'Good evening'
}

class HomeStore {
  feed = $state<HomeFeed | null>(null)
  loading = $state(false)
  error = $state<string | null>(null)
  version = $state(0)
  #initialized = false
  #inFlight: Promise<void> | null = null
  #queued = false

  async init(): Promise<void> {
    if (this.#initialized) return
    this.#initialized = true
    await Promise.all([
      listen('library://changed', () => void this.refresh()),
      listen('favorites://changed', () => void this.refresh()),
      listen('playlists://changed', () => void this.refresh()),
      listen('library://history', () => void this.refresh()),
    ])
    await this.refresh()
  }

  // Bursts of change events are coalesced: one query in flight, and at most
  // one more queued to run after it.
  refresh(): Promise<void> {
    if (this.#inFlight) {
      this.#queued = true
      return this.#inFlight
    }
    const run = async () => {
      do {
        this.#queued = false
        await this.#load()
      } while (this.#queued)
    }
    this.#inFlight = run().finally(() => {
      this.#inFlight = null
    })
    return this.#inFlight
  }

  async #load(): Promise<void> {
    this.loading = true
    try {
      this.feed = await invoke<HomeFeed>('library_home')
      this.error = null
      this.version++
    } catch (e) {
      this.error = String(e)
    } finally {
      this.loading = false
    }
  }

  async mixDetail(mixId: string): Promise<HomeMixDetail | null> {
    return invoke<HomeMixDetail | null>('library_home_mix', { mixId })
  }

  /** `startPath` is the clicked track's path, or null to play from the start. Returns false on failure. */
  async playMix(mixId: string, startPath: string | null = null, shuffle?: boolean): Promise<boolean> {
    try {
      if (shuffle !== undefined) await player.setShuffle(shuffle)
      const snap = await invoke<PlayerSnapshot>('library_play_mix', { mixId, startPath })
      player.applySnapshot(snap)
      return true
    } catch (e) {
      player.error = String(e)
      return false
    }
  }

  async saveMixAsPlaylist(mixId: string, name: string): Promise<PlaylistInfo | null> {
    try {
      const info = await invoke<PlaylistInfo>('library_save_mix_playlist', { mixId, name })
      playlists.flash(t('Mix saved as playlist “{name}”', { name: info.name }))
      await playlists.refresh()
      return info
    } catch (e) {
      player.error = String(e)
      return null
    }
  }
}

export const home = new HomeStore()
