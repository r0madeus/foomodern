# FooModern

A free, cross-platform music player: foobar2000-level audio (gapless,
ReplayGain, every format FFmpeg plays) with a modern interface.

Status: prototype (v0.8). Windows and Android; Linux works for development.
Progress and plans: [docs/ROADMAP.md](docs/ROADMAP.md).

## Stack

Tauri 2 · Rust · libmpv (LGPL build) · Svelte 5 · Vite · Tailwind 4.
Rules for contributors and AI agents: [AGENTS.md](AGENTS.md).

## Development

### Windows

Prerequisites: Rust (MSVC toolchain), Node.js 22, Visual Studio C++ build
tools, WebView2 (preinstalled on Windows 10/11).

```powershell
powershell -ExecutionPolicy Bypass -File scripts/fetch-libmpv.ps1   # once
npm ci
npm run tauri dev
```

`fetch-libmpv.ps1` downloads the LGPL libmpv build
([zhongfly/mpv-winbuild](https://github.com/zhongfly/mpv-winbuild)) into
`src-tauri/lib/` and creates the import library. The installer
(`npm run tauri build`) ships `libmpv-2.dll` next to the executable.

### Linux

```bash
sudo apt install libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev libmpv-dev
npm ci
npm run tauri dev
```

Optional for online search and download (desktop): `yt-dlp` on PATH (or
its path set in Settings) and `ffmpeg` for m4a conversion.

### Checks

```bash
npm run check
npm test
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

CI runs the same checks on Linux and Windows and uploads a Windows installer
as a build artifact.

## Licensing note

libmpv is used in its LGPL configuration and linked dynamically
(`libmpv-2.dll`), so it can be replaced by the user.
