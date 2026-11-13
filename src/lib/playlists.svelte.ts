import { count, t } from './i18n.svelte'
// Playlist state on the UI side. Rust owns the playlists (AGENTS.md rule 1);
// this module mirrors the list and sends commands.
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { appSettings } from './appSettings.svelte'
import type { TrackRow } from './library.svelte'
import { player, type PlayerSnapshot } from './player.svelte'

export interface PlaylistInfo {
  id: number
  name: string
  trackCount: number
  cover: string | null
  albumCovers: string[]
}

interface SavedPlaylist {
  playlist: PlaylistInfo
  skipped: number
}

interface ImportedPlaylist {
  playlist: PlaylistInfo
  matched: number
  missing: number
}

const M3U_FILTERS = [{ name: 'Playlist', extensions: ['m3u8', 'm3u'] }]

/** Playlist rows are fetched in pages; never the whole list in one IPC call. */
const PAGE_SIZE = 500

const tracks = (n: number) => count(n, '{n} track', '{n} tracks')

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

  async setCoverFile(id: number): Promise<boolean> {
    const picked = await open({
      multiple: false,
      directory: false,
      title: t('Choose playlist cover'),
      filters: [{ name: 'Image', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
    })
    if (typeof picked !== 'string') return false
    return (await this.#try(() => invoke('playlist_set_cover_file', { id, path: picked }))) !== null
  }

  async setCoverPreset(id: number, preset: string): Promise<boolean> {
    return (await this.#try(() => invoke('playlist_set_cover_preset', { id, preset }))) !== null
  }

  async clearCover(id: number): Promise<boolean> {
    return (await this.#try(() => invoke('playlist_clear_cover', { id }))) !== null
  }

  async addTracks(playlist: PlaylistInfo, trackIds: number[]): Promise<void> {
    const ok = await this.#try(() => invoke('playlist_add_tracks', { id: playlist.id, trackIds }))
    if (ok !== null) this.flash(t('Added {count} to “{name}”', { count: tracks(trackIds.length), name: playlist.name }))
  }

  async addAlbum(playlist: PlaylistInfo, albumId: number): Promise<void> {
    const ok = await this.#try(() => invoke('playlist_add_album', { id: playlist.id, albumId }))
    if (ok !== null) this.flash(t('Added album to “{name}”', { name: playlist.name }))
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
      t('Saved “{name}” with {count}', { name: playlist.name, count: tracks(playlist.trackCount) }) +
        (skipped > 0 ? t(' ({n} not in the library, skipped)', { n: skipped }) : ''),
    )
    return playlist
  }

  /** Saves a playlist as an M3U file (to move it to another device). */
  async exportM3u(p: PlaylistInfo): Promise<void> {
    const fileName = `${p.name.replace(/[\\/:*?"<>|]/g, '_')}.m3u8`
    const res = await this.#try(async () => {
      if (appSettings.platform === 'android') {
        const text = await invoke<string>('playlist_export_m3u', { id: p.id })
        return invoke<boolean>('android_save_text', { name: fileName, text })
      }
      const path = await save({ defaultPath: fileName, filters: M3U_FILTERS })
      if (!path) return false
      await invoke('playlist_export_m3u_file', { id: p.id, path })
      return true
    })
    if (res?.value) this.flash(t('Exported “{name}”', { name: p.name }))
  }

  /** Creates a playlist from an M3U file; songs are matched to this library. */
  async importM3u(): Promise<PlaylistInfo | null> {
    const res = await this.#try(async () => {
      if (appSettings.platform === 'android') {
        const file = await invoke<{ name: string | null; text: string | null }>('android_open_text')
        if (file.text === null) return null
        const name = (file.name ?? t('Imported')).replace(/\.m3u8?$/i, '') || t('Imported')
        return invoke<ImportedPlaylist>('playlist_import_m3u', { name, text: file.text })
      }
      const path = await open({ multiple: false, directory: false, filters: M3U_FILTERS })
      if (!path || typeof path !== 'string') return null
      return invoke<ImportedPlaylist>('playlist_import_m3u_file', { path })
    })
    if (!res?.value) return null
    const { playlist, matched, missing } = res.value
    this.flash(
      t('Imported “{name}” with {count}', { name: playlist.name, count: tracks(matched) }) +
        (missing > 0 ? t(' ({n} not found in this library)', { n: missing }) : ''),
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
