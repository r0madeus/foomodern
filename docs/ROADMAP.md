# FooModern – status and roadmap

Last updated: 2026-11-13. The owner's decision notes live in the 2ncibeyn
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

## Next

1. **Release and test on Windows**: media keys and SMTC flyout, tray and
   close-to-tray, tag editing (also while another program holds the file),
   playlists, ReplayGain setting.
2. **Metadata matching (post-v0, not decided yet)** – proposed design:
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
- Linux desktops without a StatusNotifier host (e.g. GNOME without the
  AppIndicator extension): the tray is created but invisible, so with
  close-to-tray on a closed window can't be brought back. Candidate fix:
  single-instance plugin that shows the window on a second launch.
- Linux MPRIS state lags ~1–2 s (souvlaki's D-Bus loop).
- Albums with the same title and album artist but different years are
  grouped as one album.
- UI text is English only.
