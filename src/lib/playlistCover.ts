import type { Name as IconName } from './Icon.svelte'

export interface CoverPalette {
  id: string
  label: string
  gradient: string
  heroTop: string
  heroSurface: string
  swatch: string
}

export const COVER_PALETTES: CoverPalette[] = [
  {
    id: 'teal',
    label: 'Teal',
    gradient: 'linear-gradient(135deg, #14b8a6, #0f766e, #134e4a)',
    heroTop: '#0d9488',
    heroSurface: '#09090b',
    swatch: '#14b8a6',
  },
  {
    id: 'ocean',
    label: 'Ocean',
    gradient: 'linear-gradient(135deg, #38bdf8, #1d4ed8, #1e1b4b)',
    heroTop: '#0284c7',
    heroSurface: '#09090b',
    swatch: '#38bdf8',
  },
  {
    id: 'indigo',
    label: 'Indigo',
    gradient: 'linear-gradient(135deg, #6366f1, #4338ca, #1e1b4b)',
    heroTop: '#4f46e5',
    heroSurface: '#09090b',
    swatch: '#6366f1',
  },
  {
    id: 'violet',
    label: 'Violet',
    gradient: 'linear-gradient(135deg, #c084fc, #7e22ce, #3b0764)',
    heroTop: '#9333ea',
    heroSurface: '#09090b',
    swatch: '#a855f7',
  },
  {
    id: 'rose',
    label: 'Rose',
    gradient: 'linear-gradient(135deg, #fb7185, #be123c, #4c0519)',
    heroTop: '#e11d48',
    heroSurface: '#09090b',
    swatch: '#f43f5e',
  },
  {
    id: 'sunset',
    label: 'Sunset',
    gradient: 'linear-gradient(135deg, #fb923c, #ea580c, #7c2d12)',
    heroTop: '#ea580c',
    heroSurface: '#09090b',
    swatch: '#f97316',
  },
  {
    id: 'emerald',
    label: 'Emerald',
    gradient: 'linear-gradient(135deg, #34d399, #059669, #064e3b)',
    heroTop: '#059669',
    heroSurface: '#09090b',
    swatch: '#10b981',
  },
  {
    id: 'slate',
    label: 'Slate',
    gradient: 'linear-gradient(135deg, #64748b, #334155, #0f172a)',
    heroTop: '#475569',
    heroSurface: '#09090b',
    swatch: '#64748b',
  },
]

export const COVER_ICONS: IconName[] = ['music', 'star', 'album', 'lyrics', 'play', 'queue']

export function findPalette(id: string): CoverPalette {
  return COVER_PALETTES.find((p) => p.id === id) ?? COVER_PALETTES[0]
}

export function fallbackPalette(name: string): CoverPalette {
  let h = 0
  for (let i = 0; i < name.length; i++) {
    h = (h * 31 + name.charCodeAt(i)) >>> 0
  }
  return COVER_PALETTES[h % COVER_PALETTES.length]
}

export function parseCoverPreset(
  cover: string | null | undefined,
): { palette: CoverPalette; icon: IconName } | null {
  if (!cover || !cover.startsWith('preset:')) return null
  const [, paletteId = 'teal', iconRaw = 'music'] = cover.split(':')
  const icon = COVER_ICONS.includes(iconRaw as IconName) ? (iconRaw as IconName) : 'music'
  return { palette: findPalette(paletteId), icon }
}

export function buildCoverPreset(paletteId: string, icon: IconName): string {
  return `preset:${findPalette(paletteId).id}:${icon}`
}

export function heroImageForPlaylist(
  cover: string | null | undefined,
  albumCovers: string[],
): string | null {
  if (cover && !cover.startsWith('preset:')) return cover
  if (!cover && albumCovers.length > 0) return albumCovers[0]
  return null
}
