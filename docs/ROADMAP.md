# FooModern – status and roadmap

Last updated: 2026-11-14. The owner's decision notes live in the 2ncibeyn
vault (`projects/muzik-calar/`); this file is the in-repo summary for
contributors and agents.

## Done

### v0 scope (decisions document, section 6)
- [x] Folder selection and scanning: tags and covers (SQLite, incremental, cover cache)
- [x] Song list with virtual scrolling
- [x] Album grid with covers and album detail
- [x] Play / pause / seek / next / previous
- [x] Queue (single queue owned by Rust; mpv holds only current + next)
- [x] Gapless playback and ReplayGain
- [x] Synced lyrics panel: local `.lrc` first, then LRCLIB
- [x] Media keys, Windows SMTC, Linux MPRIS

### Beyond v0
- Shuffle and repeat (off / all / one)
- Library search (songs and albums)
- Session persistence: volume, shuffle/repeat, ReplayGain, queue, current track and position
- Settings view: ReplayGain mode, file extension badge, keep playing in the tray, version
- Playlists (schema v2): create, rename, delete, add tracks/albums, reorder, play, save queue as playlist
- Tag editing: single and multiple tracks, only changed fields are written, the loaded file is refused
- System tray: Show, Play/Pause, Next, Previous, Quit; close-to-tray
- Attach a local `.lrc` file manually
- Windows release workflow (`v*` tags) building the NSIS installer
- Turkish UI (`t()` dictionary; follows the system or Settings)
- Single instance: a second launch shows the existing window

### v0.5 (desktop unless noted)
- **Online search and download** (`src-tauri/src/online`): one Search
  view for the library and online results. Albums come from the iTunes
  Search API, tracks and streams from `yt-dlp` (YouTube). Album tracks
  are matched to a recording by ranking the top five hits (length,
  official "Artist - Topic" uploads, no live/cover/remix variants).
  Playing a track queues the rest of its list; stream URLs are resolved
  just in time and never saved with the session. Downloads are tagged,
  get the album cover, land in the library and can be cancelled; every
  `yt-dlp` run has a time limit. Needs `yt-dlp` on PATH (or set in
  Settings); `ffmpeg` is optional (converts to m4a).
- **Favourites** (schema v4): star tracks, a Favourites list.
- **Playlist covers** (schema v5): automatic 2×2 collage, gradient+icon
  presets or a custom image.
- **Listening history and stats** (schema v6): one row per finished,
  skipped or interrupted listen (≥ 3 s); a stats view with periods, top
  tracks/artists/albums/genres and listening rhythm.
- **Artists** (schema v7): compound tags are split into artists
  (`feat.`/`ft.`, `,` and `;`; `&`/`+`, "Tyler, The Creator" and
  "Earth, Wind & Fire" stay whole; rules shared by Rust and the UI via
  `fixtures/artist-split-cases.json`), an artist directory and profiles
  with portrait (Deezer), bio (Wikipedia) and missing albums (iTunes),
  cached locally.
- **Albums** (schema v8): an album is one (folder, album, album artist)
  group; its year is the most common track year (latest on a tie).
  Same-titled albums in different folders with different years stay
  separate; disc folders of one album merge.
- Sidebar pins for albums and playlists; queue toggle in the player bar.
- Phone UI parity: favourites, artists, stats; swipe-back sheets;
  `prefers-reduced-motion` aware animations.

### Android (same app, decision K7)
- Audio through Media3/ExoPlayer behind `AudioEngine`, in a local Tauri
  plugin (`src-tauri/plugins/android-audio`). ExoPlayer holds only
  current + next, like mpv; the queue stays in Rust.
- Background playback via a `MediaSessionService` with a media
  notification; its next/previous go to the Rust queue.
- Engine commands go through a worker thread: mobile plugin calls run on
  the Android main thread and block.
- Desktop-only parts are gated (libmpv, souvlaki, tray menu); lyrics use
  rustls (ring + webpki roots) on Android.
- Library: asks for `READ_MEDIA_AUDIO` and adds the shared `Music` folder.
- CI builds an aarch64 debug APK (artifact `foomodern-android-debug-apk`).

### Look and feel (phone and desktop)
- Phone UI (`src/lib/mobile`, below 640 px): Albums / Songs / Search /
  Library tabs, a mini player, a full-screen player tinted with the
  cover's colour, the line being sung under the cover, a lyrics card and
  full-screen lyrics (follow playback, tap a line to seek), album and
  playlist pages, cover thumbnails, Android back button closes screens,
  always dark, edge-to-edge insets.
- Desktop: a now-playing view (large cover + synced lyrics) opened from
  the player bar, the lyrics panel in the same tinted style, covers in
  song/playlist rows and in the player bar, sidebar icons, album headers
  on the cover's colour.
- The cover's dominant colour is computed in Rust (`library::cover_color`).

## Next

1. **Release and test on Windows**: media keys and SMTC flyout, tray and
   close-to-tray, tag editing (also while another program holds the file),
   playlists, ReplayGain setting, the new now-playing view.
2. **Try the APK on a phone**: permission prompt and Music folder scan,
   playback with the screen off, notification controls, cover tint and
   the lyric line, back button, status/navigation bar insets.
3. **Android follow-ups**: ReplayGain (ExoPlayer has none), picking
   folders other than `Music`, a signed release APK, tag writing on shared
   storage, attaching `.lrc` files (the picker returns `content://` URIs).
4. **Metadata matching (post-v0, not decided yet)** – proposed design:
   - Match per **album**, not per track: compare a folder's track count and
     durations with MusicBrainz releases (Picard's approach).
   - **Preview and confirm**: old and new values side by side; nothing is
     written without confirmation; low-score matches are not offered.
   - **Undo**: keep the previous tags in the database before writing.
   - Write through the existing tag-editing path (temp file + replace,
     loaded file refused).
   - First version: album search + cover from the Cover Art Archive.
     AcoustID / `fpcalc` fingerprinting only if untagged libraries turn out
     to be a real problem (extra binary + API key).
   - Limits: MusicBrainz allows about one request per second and requires
     a descriptive client identification; album-level matching keeps large
     libraries feasible.
   - Scope: "find missing info and covers", not a replacement for
     MusicBrainz Picard.

## Known limitations
- Linux MPRIS state lags ~1–2 s (souvlaki's D-Bus loop).
- Online search/download is desktop only and depends on `yt-dlp`
  (and YouTube); a wrong recording is still possible when no hit is
  close to the album track.
- Artist splitting is heuristic: a duet tagged "A, B & C" splits into
  "A" and "B & C"; three single-word rappers joined by `&` stay one name.
- Android: not yet tried on a device; ReplayGain is not applied; only the
  `Music` folder can be added; "Open files" is hidden.
