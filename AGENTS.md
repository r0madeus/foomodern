# FooModern – rules for AI agents

A free, cross-platform music player: foobar2000-level audio features with a
modern UI. Decisions and their reasons live in the owner's notes
(`projects/muzik-calar/Kararlar.md` in the 2ncibeyn vault); the rules below
are the ones every coding session must follow.

## Stack (versions are pinned – do not bump casually)

- Tauri 2 (`tauri` 2.12.x), Rust edition 2024
- Audio: libmpv through `libmpv2` **7.0.x** (not 4.x/5.x examples from the web)
- UI: **Svelte 5 with runes** (`$state`, `$derived`, `$effect`, `$props`),
  Vite 8, Tailwind 4 (`@tailwindcss/vite`, no `tailwind.config.js`)
- Before writing code against a crate or npm package, check the docs for the
  exact pinned version (docs.rs / package docs). Do not write Svelte 4
  syntax (`export let`, `$:`, `on:click`, stores for local state).

## Architecture rules

1. **Rust is the single source of truth.** Queue, playback state and library
   live in Rust. The UI only renders and sends commands.
2. **One queue only.** The queue is owned by `src-tauri/src/queue`. mpv is
   given exactly two entries: the current track and the next one
   (`AudioEngine::load` / `AudioEngine::set_next`). Never use mpv's playlist
   as a queue, never add more than one "next" entry.
3. **libmpv is only called inside `src-tauri/src/audio/`.** Everything else
   uses the `AudioEngine` trait.
4. **No big payloads over IPC.** Lists are paged. Cover art is written to the
   cache folder and loaded with `convertFileSrc` – never base64 over IPC.
5. **Throttle chatty data.** Playback position is sent at most ~4×/s; the UI
   interpolates between updates.
6. Audio plays in Rust, never through an HTML `<audio>` element.

## Scope

v0 scope is locked (see the decisions document, section 6). New ideas go to a
"later" list, not into the code.

## Conventions

- Code, comments, identifiers, logs, commits and PRs are in English.
- Rust: `cargo fmt`, `cargo clippy --all-targets -- -D warnings` clean.
- Frontend: `npm run check` clean.
- Tests must not need a sound card: mpv tests use `ao=null`.

## Commands

```bash
npm ci
npm run tauri dev          # run the app
npm run check              # svelte-check + tsc
cd src-tauri && cargo test # Rust tests (needs libmpv)
```

Linux: `libmpv-dev` and the Tauri system packages. Windows: run
`scripts/fetch-libmpv.ps1` once; it downloads the LGPL libmpv build into
`src-tauri/lib/`.
