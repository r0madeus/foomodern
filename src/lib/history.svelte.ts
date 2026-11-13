import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export interface StatsSummary {
  totalListenedMs: number
  playCount: number
  rawEventCount: number
  skipCount: number
  completedCount: number
  uniqueTracks: number
  uniqueArtists: number
  uniqueAlbums: number
  activeDays: number
  longestStreakDays: number
  currentStreakDays: number
  topHour: number | null
  topWeekday: number | null
  firstListenAt: number | null
}

export interface TopTrackStat {
  trackId: number | null
  path: string
  title: string
  artist: string
  album: string
  cover: string | null
  playCount: number
  listenedMs: number
  skipCount: number
}

export interface TopArtistStat {
  artist: string
  cover: string | null
  playCount: number
  listenedMs: number
  uniqueTracks: number
  skipCount: number
}

export interface TopAlbumStat {
  album: string
  artist: string
  albumId: number | null
  cover: string | null
  playCount: number
  listenedMs: number
}

export interface TopGenreStat {
  genre: string
  playCount: number
  listenedMs: number
}

export interface BucketStat {
  label: string
  listenedMs: number
  playCount: number
}

export interface RecentListen {
  id: number
  trackId: number | null
  path: string
  title: string
  artist: string
  album: string
  cover: string | null
  startedAt: number
  listenedMs: number
  completed: boolean
  skipped: boolean
  source: string
}

export interface ListeningStats {
  summary: StatsSummary
  topTracksByPlays: TopTrackStat[]
  topTracksByTime: TopTrackStat[]
  topArtistsByTime: TopArtistStat[]
  topAlbumsByTime: TopAlbumStat[]
  topGenres: TopGenreStat[]
  byHour: BucketStat[]
  byWeekday: BucketStat[]
  byMonth: BucketStat[]
  recent: RecentListen[]
  availableYears: number[]
}

export type StatsPreset = '7d' | '30d' | 'year' | 'all' | 'custom'

export function rangeBounds(
  preset: StatsPreset,
  year: number,
  customFrom: string,
  customTo: string,
): { from: number | null; to: number | null } {
  const now = new Date()
  if (preset === '7d' || preset === '30d') {
    const days = preset === '7d' ? 6 : 29
    const start = new Date(now.getFullYear(), now.getMonth(), now.getDate() - days)
    return { from: Math.floor(start.getTime() / 1000), to: null }
  }
  if (preset === 'year') {
    const start = new Date(year, 0, 1)
    const end = new Date(year + 1, 0, 1)
    return {
      from: Math.floor(start.getTime() / 1000),
      to: Math.floor(end.getTime() / 1000),
    }
  }
  if (preset === 'custom') {
    let from: number | null = null
    let to: number | null = null
    if (customFrom) {
      const [y, m, d] = customFrom.split('-').map(Number)
      if (y && m && d) from = Math.floor(new Date(y, m - 1, d).getTime() / 1000)
    }
    if (customTo) {
      const [y, m, d] = customTo.split('-').map(Number)
      if (y && m && d) to = Math.floor(new Date(y, m - 1, d + 1).getTime() / 1000)
    }
    return { from, to }
  }
  return { from: null, to: null }
}

class HistoryStore {
  version = $state(0)
  private ready = false

  async init() {
    if (this.ready) return
    this.ready = true
    await listen('library://history', () => {
      this.version += 1
    })
  }

  async stats(from: number | null, to: number | null): Promise<ListeningStats> {
    return invoke<ListeningStats>('history_stats', { from, to })
  }

  async clear(): Promise<number> {
    return invoke<number>('history_clear')
  }
}

export const history = new HistoryStore()
