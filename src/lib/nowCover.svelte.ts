import { invoke } from '@tauri-apps/api/core'
import type { TrackRow } from './library.svelte'

export type Rgb = [number, number, number]

// Cover, cover colour and library metadata of the playing track, looked up when it changes.
class NowCover {
  path = $state<string | null>(null)
  cover = $state<string | null>(null)
  color = $state<Rgb | null>(null)
  track = $state<TrackRow | null>(null)

  async update(path: string | null): Promise<void> {
    if (path === this.path) return
    this.path = path
    if (path === null) {
      this.cover = null
      this.color = null
      this.track = null
      return
    }
    const [coverRes, trackRes] = await Promise.allSettled([
      invoke<string | null>('library_track_cover', { path }),
      invoke<TrackRow | null>('library_track_by_path', { path }),
    ])
    if (this.path !== path) return
    const cover = coverRes.status === 'fulfilled' ? coverRes.value : null
    this.cover = cover
    this.track = trackRes.status === 'fulfilled' ? trackRes.value : null
    let color: Rgb | null = null
    if (cover !== null) {
      try {
        color = await invoke<Rgb | null>('library_cover_color', { cover })
      } catch {
        color = null
      }
    }
    if (this.path === path) this.color = color
  }
}

export const nowCover = new NowCover()
