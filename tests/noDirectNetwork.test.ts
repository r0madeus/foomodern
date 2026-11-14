import assert from 'node:assert/strict'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { test } from 'node:test'

// Rust enforces the offline mode and the per-feature internet switches
// (src-tauri/src/net.rs). That only holds while the UI never talks to the
// internet itself, so the web code must not use any network API directly.
// Remote cover images are the one exception and go through Cover.svelte,
// which hides them while the online features are off.

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return sources(path)
    return /\.(svelte|ts)$/.test(name) ? [path] : []
  })
}

test('the UI makes no direct network requests', () => {
  const forbidden = /\bfetch\s*\(|XMLHttpRequest|new WebSocket|new EventSource|navigator\.sendBeacon|<audio|<video/
  const offenders = sources('src').filter((f) => forbidden.test(readFileSync(f, 'utf8')))
  assert.deepEqual(offenders, [])
})

test('only Cover.svelte renders a remote image', () => {
  const remoteImg = /<img[^>]*(src=["']https?:|src=\{[^}]*https?:)/
  const offenders = sources('src').filter(
    (f) => !f.endsWith('Cover.svelte') && remoteImg.test(readFileSync(f, 'utf8')),
  )
  assert.deepEqual(offenders, [])
})
