// Shared motion utilities honoring OS reduced-motion preferences across
// Windows, Linux and Android WebViews.

export function prefersReducedMotion(): boolean {
  return (
    typeof window !== 'undefined' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  )
}

export function motionDuration(ms: number): number {
  return prefersReducedMotion() ? 0 : ms
}

export function viewportWidth(): number {
  return typeof window !== 'undefined' ? window.innerWidth : 420
}

export function viewportHeight(): number {
  return typeof window !== 'undefined' ? window.innerHeight : 900
}
