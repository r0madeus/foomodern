// Pinned albums and playlists mirrored from Rust (`session.json` + SQLite).
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { player } from './player.svelte'

export type PinnedKind = 'album' | 'playlist'

export interface PinnedEntry {
  kind: PinnedKind
  id: number
  title: string
  subtitle: string
  cover: string | null
  albumCovers: string[]
  trackCount: number
}

class PinnedStore {
  entries = $state<PinnedEntry[]>([])
  #initialized = false

  async init(): Promise<void> {
    if (!this.#initialized) {
      this.#initialized = true
      await listen('pinned://changed', () => void this.refresh())
      await listen('library://changed', () => void this.refresh())
      await listen('playlists://changed', () => void this.refresh())
    }
    await this.refresh()
  }

  async refresh(): Promise<void> {
    try {
      this.entries = await invoke<PinnedEntry[]>('pinned_list')
    } catch (e) {
      player.error = String(e)
    }
  }

  async toggle(kind: PinnedKind, id: number): Promise<void> {
    try {
      this.entries = await invoke<PinnedEntry[]>('pinned_toggle', { kind, id })
    } catch (e) {
      player.error = String(e)
    }
  }

  isPinned(kind: PinnedKind, id: number): boolean {
    return this.entries.some((e) => e.kind === kind && e.id === id)
  }
}

export const pinned = new PinnedStore()
