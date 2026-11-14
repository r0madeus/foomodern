import type { Rgb } from './nowCover.svelte'

// Colours derived from the cover's dominant colour. Saturation and
// lightness are clamped so white text always reads well on them.

function toHsl([r, g, b]: Rgb): [number, number, number] {
  const [rr, gg, bb] = [r / 255, g / 255, b / 255]
  const max = Math.max(rr, gg, bb)
  const min = Math.min(rr, gg, bb)
  const l = (max + min) / 2
  if (max === min) return [0, 0, l]
  const d = max - min
  const s = d / (1 - Math.abs(2 * l - 1))
  let h: number
  if (max === rr) h = ((gg - bb) / d) % 6
  else if (max === gg) h = (bb - rr) / d + 2
  else h = (rr - gg) / d + 4
  return [(h * 60 + 360) % 360, s, l]
}

const hsl = (h: number, s: number, l: number) => `hsl(${h.toFixed(0)} ${(s * 100).toFixed(0)}% ${(l * 100).toFixed(0)}%)`

export interface Tint {
  /** Top of the player background. */
  top: string
  /** Mini player and lyrics card. */
  surface: string
  /** Lyric lines that are not playing. */
  dim: string
}

export const NEUTRAL: Tint = { top: '#3d414e', surface: '#262933', dim: '#9fa3b2' }

export function tintFrom(color: Rgb | null): Tint {
  if (!color) return NEUTRAL
  const [h, s] = toHsl(color)
  const sat = Math.min(s, 0.5)
  if (sat < 0.08) return NEUTRAL
  return {
    top: hsl(h, sat, 0.3),
    surface: hsl(h, sat * 0.9, 0.24),
    dim: hsl(h, Math.min(sat + 0.15, 0.6), 0.72),
  }
}
