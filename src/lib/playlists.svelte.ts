// Playlist state on the UI side. Rust owns the playlists (AGENTS.md rule 1);
// this module mirrors the list and sends commands.
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { TrackRow } from './library.svelte'
import { player, type PlayerSnapshot } from './player.svelte'

export interface PlaylistInfo {
  id: number
  name: string
  trackCount: number
}

interface SavedPlaylist {
  playlist: PlaylistInfo
  skipped: number
}

/** Playlist rows are fetched in pages; never the whole list in one IPC call. */
const PAGE_SIZE = 500

function plural(n: number, word: string): string {
  return `${n.toLocaleString()} ${word}${n === 1 ? '' : 's'}`
}

class PlaylistStore {
  list = $state<PlaylistInfo[]>([])
  /** True once the first list has arrived. */
  loaded = $state(false)
  /** Bumped on every change event so open playlist views reload. */
  version = $state(0)
  /** Short confirmation message shown by the app shell. */
  notice = $state<string | null>(null)

  #noticeTimer: ReturnType<typeof setTimeout> | undefined

  async init(): Promise<void> {
    // Playlist edits, and library changes (a rescan can drop tracks).
    await listen('playlists://changed', () => void this.refresh())
    await listen('library://changed', () => void this.refresh())
    await this.refresh()
  }

  async refresh(): Promise<void> {
    try {
      this.list = await invoke<PlaylistInfo[]>('playlists_list')
      this.loaded = true
      this.version++
    } catch (e) {
      player.error = String(e)
    }
  }

  flash(message: string): void {
    this.notice = message
    clearTimeout(this.#noticeTimer)
    this.#noticeTimer = setTimeout(() => (this.notice = null), 4000)
  }

  byId(id: number | null): PlaylistInfo | undefined {
    return id === null ? undefined : this.list.find((p) => p.id === id)
  }

  async create(name: string, trackIds?: number[]): Promise<PlaylistInfo | null> {
    return (await this.#try(() => invoke<PlaylistInfo>('playlist_create', { name, trackIds })))?.value ?? null
  }

  async rename(id: number, name: string): Promise<boolean> {
    return (await this.#try(() => invoke('playlist_rename', { id, name }))) !== null
  }

  async delete(id: number): Promise<boolean> {
    return (await this.#try(() => invoke('playlist_delete', { id }))) !== null
  }

  async addTracks(playlist: PlaylistInfo, trackIds: number[]): Promise<void> {
    const ok = await this.#try(() => invoke('playlist_add_tracks', { id: playlist.id, trackIds }))
    if (ok !== null) this.flash(`Added ${plural(trackIds.length, 'track')} to “${playlist.name}”`)
  }

  async addAlbum(playlist: PlaylistInfo, albumId: number): Promise<void> {
    const ok = await this.#try(() => invoke('playlist_add_album', { id: playlist.id, albumId }))
    if (ok !== null) this.flash(`Added album to “${playlist.name}”`)
  }

  async removeAt(id: number, positions: number[]): Promise<boolean> {
    return (await this.#try(() => invoke('playlist_remove', { id, positions }))) !== null
  }

  async move(id: number, from: number, to: number): Promise<boolean> {
    return (await this.#try(() => invoke('playlist_move', { id, from, to }))) !== null
  }

  async play(id: number, startIndex: number): Promise<void> {
    const res = await this.#try(() => invoke<PlayerSnapshot>('playlist_play', { id, startIndex }))
    if (res) player.applySnapshot(res.value)
  }

  async saveQueue(name: string): Promise<PlaylistInfo | null> {
    const res = await this.#try(() => invoke<SavedPlaylist>('playlist_from_queue', { name }))
    if (!res) return null
    const { playlist, skipped } = res.value
    this.flash(
      `Saved “${playlist.name}” with ${plural(playlist.trackCount, 'track')}` +
        (skipped > 0 ? ` (${skipped.toLocaleString()} not in the library, skipped)` : ''),
    )
    return playlist
  }

  /** Every track of a playlist, fetched page by page. */
  async tracks(id: number, count: number): Promise<TrackRow[]> {
    const rows: TrackRow[] = []
    for (let offset = 0; offset < count; offset += PAGE_SIZE) {
      const page = await invoke<TrackRow[]>('playlist_tracks', { id, offset, limit: PAGE_SIZE })
      rows.push(...page)
      if (page.length < PAGE_SIZE) break
    }
    return rows
  }

  /** Runs a command; null (and `player.error` set) when it failed. Commands that return nothing resolve to null on success too, hence the wrapper. */
  async #try<T>(job: () => Promise<T>): Promise<{ value: T } | null> {
    try {
      return { value: await job() }
    } catch (e) {
      player.error = String(e)
      return null
    }
  }
}

export const playlists = new PlaylistStore()
