import { invoke } from '@tauri-apps/api/core'

export type Rgb = [number, number, number]

// Cover and cover colour of the playing track, looked up when it changes.
class NowCover {
  path = $state<string | null>(null)
  cover = $state<string | null>(null)
  color = $state<Rgb | null>(null)

  async update(path: string | null): Promise<void> {
    if (path === this.path) return
    this.path = path
    let cover: string | null = null
    if (path !== null) {
      try {
        cover = await invoke<string | null>('library_track_cover', { path })
      } catch {
        cover = null
      }
    }
    if (this.path !== path) return
    this.cover = cover
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
