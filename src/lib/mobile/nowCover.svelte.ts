import { invoke } from '@tauri-apps/api/core'

// Cover of the playing track, looked up when the track changes.
class NowCover {
  path = $state<string | null>(null)
  cover = $state<string | null>(null)

  async update(path: string | null): Promise<void> {
    if (path === this.path) return
    this.path = path
    if (path === null) {
      this.cover = null
      return
    }
    try {
      const cover = await invoke<string | null>('library_track_cover', { path })
      if (this.path === path) this.cover = cover
    } catch {
      if (this.path === path) this.cover = null
    }
  }
}

export const nowCover = new NowCover()
