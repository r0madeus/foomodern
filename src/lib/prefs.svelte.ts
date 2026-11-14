// UI-only preferences, kept per viewer in localStorage. Storage can be
// unavailable, so every access is wrapped.

const SHOW_EXT_KEY = 'neomodern.showExtensions'

function loadBool(key: string, fallback: boolean): boolean {
  try {
    const v = localStorage.getItem(key)
    if (v === 'true') return true
    if (v === 'false') return false
  } catch {
    // storage unavailable: fall through
  }
  return fallback
}

class Prefs {
  #showExtensions = $state(loadBool(SHOW_EXT_KEY, false))

  get showExtensions(): boolean {
    return this.#showExtensions
  }

  set showExtensions(next: boolean) {
    this.#showExtensions = next
    try {
      localStorage.setItem(SHOW_EXT_KEY, String(next))
    } catch {
      // ignore
    }
  }
}

export const prefs = new Prefs()

/** Upper-case file extension of `path` ("FLAC"), or '' when it has none. */
export function fileExtension(path: string): string {
  const name = path.slice(Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\')) + 1)
  const dot = name.lastIndexOf('.')
  if (dot <= 0 || dot === name.length - 1) return ''
  return name.slice(dot + 1).toUpperCase()
}
