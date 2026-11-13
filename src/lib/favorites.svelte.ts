// Favorite ("starred") library tracks mirrored from Rust SQLite.
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { Page, TrackRow } from './library.svelte'
import { player, type PlayerSnapshot } from './player.svelte'

export interface FavoriteRef {
  trackId: number
  path: string
}

class FavoritesStore {
  ids = $state<Set<number>>(new Set())
  paths = $state<Set<string>>(new Set())
  version = $state(0)
  #initialized = false

  async init(): Promise<void> {
    if (!this.#initialized) {
      this.#initialized = true
      await listen('favorites://changed', () => void this.refresh())
      await listen('library://changed', () => void this.refresh())
    }
    await this.refresh()
  }

  async refresh(): Promise<void> {
    try {
      const refs = await invoke<FavoriteRef[]>('favorites_list')
      this.ids = new Set(refs.map((r) => r.trackId))
      this.paths = new Set(refs.map((r) => r.path))
      this.version++
    } catch (e) {
      player.error = String(e)
    }
  }

  has(trackId: number): boolean {
    return this.ids.has(trackId)
  }

  hasPath(path: string | null | undefined): boolean {
    return Boolean(path && this.paths.has(path))
  }

  async toggle(trackId: number): Promise<void> {
    try {
      await invoke<boolean>('favorites_toggle', { trackId })
      await this.refresh()
    } catch (e) {
      player.error = String(e)
    }
  }

  async toggleByPath(path: string): Promise<void> {
    try {
      await invoke<boolean>('favorites_toggle_by_path', { path })
      await this.refresh()
    } catch (e) {
      player.error = String(e)
    }
  }

  page = (offset: number, limit: number): Promise<Page<TrackRow>> =>
    invoke<Page<TrackRow>>('favorites_tracks_page', { offset, limit })

  async play(startIndex: number): Promise<void> {
    try {
      player.applySnapshot(await invoke<PlayerSnapshot>('favorites_play', { startIndex }))
    } catch (e) {
      player.error = String(e)
    }
  }
}

export const favorites = new FavoritesStore()
