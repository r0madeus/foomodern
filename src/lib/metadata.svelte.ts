import { popLayer, pushLayer } from './mobile/layers'
import { t } from './i18n.svelte'
// "Find info" (MusicBrainz matching) on the UI side. Rust searches, ranks and
// writes (AGENTS.md rule 1); this module only remembers what to match and
// describes the data the modal renders. Cover previews arrive as cache file
// paths and are shown with convertFileSrc, never as image bytes.

export type DiffField = 'title' | 'artist' | 'album' | 'albumArtist' | 'trackNo' | 'discNo' | 'year'

export interface FieldDiff {
  field: DiffField
  old: string | null
  new: string | null
}

export interface TrackDiff {
  trackId: number
  path: string
  title: string
  /** False when the candidate has no track for this file; it stays untouched. */
  matched: boolean
  /** Only the fields that differ from the current tags. */
  fields: FieldDiff[]
}

/** Opaque for the UI: sent back unchanged to `metadata_apply`. */
export interface Proposal {
  releaseId: string
  releaseGroupId: string
  tracks: Record<string, unknown>[]
  cover: boolean
}

interface CandidateBase {
  releaseId: string
  releaseGroupId: string
  year: number | null
  country: string | null
  format: string | null
  status: string | null
  score: number
  label: 'recommended' | 'check'
  /** Absolute path of a cached preview image. */
  coverPreview: string | null
  proposal: Proposal
}

export interface AlbumCandidate extends CandidateBase {
  title: string
  artist: string
  trackCount: number
  albumFields: FieldDiff[]
  tracks: TrackDiff[]
}

export interface TrackCandidate extends CandidateBase {
  recordingId: string
  title: string
  artist: string
  album: string
  track: TrackDiff
}

export interface ApplyOutcome {
  /** 0 when nothing was written. */
  batchId: number
  updated: number
  failed: [path: string, error: string][]
  cover: string | null
}

export type MatchRequest =
  | { mode: 'album'; albumId: number; heading: string }
  | { mode: 'track'; trackId: number; heading: string }

class MetadataMatchState {
  /** What the dialog is open for; null when closed. */
  request = $state<MatchRequest | null>(null)

  #open(request: MatchRequest): void {
    // A history layer, so the phone's back button closes the dialog rather
    // than the page under it.
    if (this.request === null) pushLayer(() => (this.request = null))
    this.request = request
  }

  openAlbum(albumId: number): void {
    this.#open({ mode: 'album', albumId, heading: t('Find info') })
  }

  openTrack(trackId: number): void {
    this.#open({ mode: 'track', trackId, heading: t('Find info for this track') })
  }

  /** Closes through history, like the back button. */
  close(): void {
    if (this.request !== null) popLayer()
  }
}

export const metadataMatch = new MetadataMatchState()
