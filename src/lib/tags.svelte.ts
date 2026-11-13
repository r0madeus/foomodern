import { t } from './i18n.svelte'
// Tag editor state on the UI side. Rust reads and writes the tags
// (AGENTS.md rule 1); this module only remembers which tracks to edit.

export interface EditableTags {
  title: string | null
  artist: string | null
  album: string | null
  albumArtist: string | null
  trackNo: number | null
  discNo: number | null
  year: number | null
  genre: string | null
}

export type TagField = keyof EditableTags

export interface TagFailure {
  path: string
  error: string
}

export interface TagsRead {
  tracks: (EditableTags & { id: number; path: string })[]
  /** Shared value per field; null when empty or mixed. */
  common: EditableTags
  /** Fields whose values differ between the tracks. */
  mixed: TagField[]
  failed: TagFailure[]
}

/** Per field: omitted = keep, null = clear, value = set. */
export type TagChanges = { [K in TagField]?: EditableTags[K] }

export interface TagsWritten {
  updated: number
  failed: TagFailure[]
}

class TagEditorState {
  /** The tracks being edited; null when the dialog is closed. */
  request = $state<{ trackIds: number[]; heading: string } | null>(null)

  open(trackIds: number[], heading = t('Edit tags')): void {
    if (trackIds.length === 0) return
    this.request = { trackIds: [...trackIds], heading }
  }

  close(): void {
    this.request = null
  }
}

export const tagEditor = new TagEditorState()
