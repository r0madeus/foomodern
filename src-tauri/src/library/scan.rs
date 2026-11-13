//! Incremental folder scan: walk the tree, read tags with `lofty`, write
//! covers into the cache folder and upsert rows in batches.

use super::{Library, LibraryError, Result, ScanProgress, db};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::{Picture, PictureType};
use lofty::prelude::{Accessor, ItemKey};
use rusqlite::params;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "oga", "opus", "m4a", "m4b", "aac", "wav", "wv", "ape", "wma", "aif",
    "aiff", "dsf", "dff", "mka", "mpc", "tta", "alac",
];

/// Candidate cover file names next to the audio files, in priority order.
const COVER_FILE_NAMES: &[&str] = &[
    "cover.jpg",
    "cover.png",
    "folder.jpg",
    "folder.png",
    "front.jpg",
    "front.png",
];

const BATCH_SIZE: usize = 500;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanStats {
    /// Audio files seen on disk.
    pub scanned: u64,
    /// Files whose tags were (re-)read and written to the database.
    pub updated: u64,
    /// Tracks deleted because their file is gone.
    pub removed: u64,
}

pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

/// Path as stored in the database (strips the Windows verbatim prefix).
pub fn display_path(p: &Path) -> String {
    let s = p.to_string_lossy();
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
}

/// 64-bit FNV-1a, stable across Rust versions (unlike `DefaultHasher`).
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub fn cover_file_name(album_artist: &str, title: &str, ext: &str) -> String {
    let key = format!("{album_artist}\u{1f}{title}");
    format!("{:016x}.{ext}", fnv1a(key.as_bytes()))
}

pub fn remove_cover_files(paths: &[String]) {
    for p in paths {
        if let Err(e) = std::fs::remove_file(p) {
            log::debug!("could not remove cover {p}: {e}");
        }
    }
}

struct Parsed {
    path: String,
    mtime: i64,
    size: i64,
    title: String,
    artist: String,
    album: String,
    album_artist: String,
    track_no: Option<i64>,
    disc_no: Option<i64>,
    year: Option<i64>,
    genre: Option<String>,
    duration_ms: Option<i64>,
    cover: Option<String>,
}

#[derive(Default)]
struct Tags {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    album_artist: Option<String>,
    track_no: Option<i64>,
    disc_no: Option<i64>,
    year: Option<i64>,
    genre: Option<String>,
    duration_ms: Option<i64>,
    picture: Option<Picture>,
}

fn non_empty(s: Option<impl AsRef<str>>) -> Option<String> {
    s.map(|s| s.as_ref().trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_tags(
    path: &Path,
    want_picture: bool,
) -> std::result::Result<Tags, lofty::error::FileParseError> {
    let tagged = lofty::read_from_path(path)?;
    let mut out = Tags::default();
    let ms = tagged.properties().duration().as_millis();
    out.duration_ms = (ms > 0).then(|| i64::try_from(ms).unwrap_or(i64::MAX));
    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        out.title = non_empty(tag.title());
        out.artist = non_empty(tag.artist());
        out.album = non_empty(tag.album());
        out.album_artist = non_empty(tag.get_string(ItemKey::AlbumArtist));
        out.track_no = tag.track().map(i64::from);
        out.disc_no = tag.disk().map(i64::from);
        out.year = tag.date().map(|d| i64::from(d.year)).filter(|y| *y > 0);
        out.genre = non_empty(tag.genre());
        if want_picture {
            out.picture = tag
                .pictures()
                .iter()
                .find(|p| p.pic_type() == PictureType::CoverFront)
                .or_else(|| tag.pictures().first())
                .cloned();
        }
    }
    Ok(out)
}

fn sniff_image_ext(data: &[u8]) -> &'static str {
    if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else if data.starts_with(b"GIF8") {
        "gif"
    } else if data.starts_with(b"BM") {
        "bmp"
    } else {
        "jpg"
    }
}

fn write_atomic(dest: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = dest.with_extension("tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, dest)
}

/// Finds `cover.jpg` & co. in `dir` (case-insensitive), by priority.
fn find_cover_file(dir: &Path) -> Option<PathBuf> {
    let mut found: Vec<(usize, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if let Some(rank) = COVER_FILE_NAMES.iter().position(|n| *n == name) {
            found.push((rank, entry.path()));
        }
    }
    found.into_iter().min_by_key(|(r, _)| *r).map(|(_, p)| p)
}

struct Scanner<'a> {
    lib: &'a Library,
    folder_id: i64,
    folder: String,
    /// Albums (lowercased title, album artist) that already have a cover file.
    covered: HashSet<(String, String)>,
    dir_cover_cache: HashMap<PathBuf, Option<PathBuf>>,
    now: i64,
    stats: ScanStats,
    last_progress: Instant,
}

impl Scanner<'_> {
    fn album_key(album: &str, album_artist: &str) -> (String, String) {
        (album.to_lowercase(), album_artist.to_lowercase())
    }

    /// Writes the album cover once; returns the cover path if a new file
    /// was written.
    fn ensure_cover(
        &mut self,
        file: &Path,
        album: &str,
        album_artist: &str,
        picture: Option<Picture>,
    ) -> Option<String> {
        let covers_dir = self.lib.covers_dir();
        if let Some(pic) = picture {
            let ext = pic
                .mime_type()
                .and_then(|m| m.ext())
                .unwrap_or_else(|| sniff_image_ext(pic.data()));
            let dest = covers_dir.join(cover_file_name(album_artist, album, ext));
            match write_atomic(&dest, pic.data()) {
                Ok(()) => return Some(display_path(&dest)),
                Err(e) => log::warn!("failed to write cover {}: {e}", dest.display()),
            }
        }
        let dir = file.parent()?.to_path_buf();
        let src = self
            .dir_cover_cache
            .entry(dir.clone())
            .or_insert_with(|| find_cover_file(&dir))
            .clone()?;
        let ext = src
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
            .unwrap_or_else(|| "jpg".into());
        let dest = covers_dir.join(cover_file_name(album_artist, album, &ext));
        match std::fs::copy(&src, &dest) {
            Ok(_) => Some(display_path(&dest)),
            Err(e) => {
                log::warn!("failed to copy cover {}: {e}", src.display());
                None
            }
        }
    }

    fn parse(&mut self, path: &Path, path_str: String, mtime: i64, size: i64) -> Parsed {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        // The picture is cloned out of the tag only; it is written to disk
        // below only if the album has no cover yet.
        let tags = match read_tags(path, true) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("failed to read tags of {}: {e}", path.display());
                Tags::default()
            }
        };
        let artist = tags.artist.clone().unwrap_or_default();
        let album = tags.album.clone().unwrap_or_default();
        let album_artist = tags.album_artist.clone().unwrap_or_else(|| artist.clone());

        let mut cover = None;
        if !album.is_empty() {
            let key = Self::album_key(&album, &album_artist);
            if !self.covered.contains(&key) {
                cover = self.ensure_cover(path, &album, &album_artist, tags.picture);
                if cover.is_some() {
                    self.covered.insert(key);
                }
            }
        }

        Parsed {
            path: path_str,
            mtime,
            size,
            title: tags.title.unwrap_or(stem),
            artist,
            album,
            album_artist,
            track_no: tags.track_no,
            disc_no: tags.disc_no,
            year: tags.year,
            genre: tags.genre,
            duration_ms: tags.duration_ms,
            cover,
        }
    }

    fn flush(&mut self, batch: &mut Vec<Parsed>) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        {
            let mut conn = self.lib.conn();
            let tx = conn.transaction()?;
            {
                let mut upsert_album = tx.prepare_cached(
                    "INSERT INTO albums(title, album_artist, year, cover) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(title, album_artist) DO UPDATE SET
                        year = COALESCE(year, excluded.year),
                        cover = COALESCE(excluded.cover, cover)
                     RETURNING id",
                )?;
                let mut upsert_track = tx.prepare_cached(
                    "INSERT INTO tracks(folder_id, path, mtime, size, title, artist, album_id,
                        track_no, disc_no, year, duration_ms, genre, added_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                     ON CONFLICT(path) DO UPDATE SET
                        folder_id = excluded.folder_id, mtime = excluded.mtime,
                        size = excluded.size, title = excluded.title, artist = excluded.artist,
                        album_id = excluded.album_id, track_no = excluded.track_no,
                        disc_no = excluded.disc_no, year = excluded.year,
                        duration_ms = excluded.duration_ms, genre = excluded.genre",
                )?;
                for t in batch.iter() {
                    let album_id = if t.album.is_empty() {
                        None
                    } else {
                        let id: i64 = upsert_album
                            .query_row(params![t.album, t.album_artist, t.year, t.cover], |r| {
                                r.get(0)
                            })?;
                        Some(id)
                    };
                    upsert_track.execute(params![
                        self.folder_id,
                        t.path,
                        t.mtime,
                        t.size,
                        t.title,
                        t.artist,
                        album_id,
                        t.track_no,
                        t.disc_no,
                        t.year,
                        t.duration_ms,
                        t.genre,
                        self.now
                    ])?;
                }
            }
            tx.commit()?;
        }
        batch.clear();
        self.lib.bump();
        Ok(())
    }

    fn emit_progress(&mut self, force: bool, done: bool) {
        if force || self.last_progress.elapsed() >= PROGRESS_INTERVAL {
            self.last_progress = Instant::now();
            self.lib.progress(ScanProgress {
                folder: self.folder.clone(),
                scanned: self.stats.scanned,
                done,
            });
        }
    }
}

fn unix_secs(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// Scans one folder synchronously. Unchanged files (same mtime and size) are
/// skipped, vanished files are deleted, and albums left without tracks are
/// removed together with their cover files.
pub fn scan_folder(lib: &Library, folder_id: i64, root: &Path) -> Result<ScanStats> {
    if !root.is_dir() {
        // Never delete anything when the folder is unreachable (unplugged drive).
        return Err(LibraryError::Other(format!(
            "folder is not accessible: {}",
            root.display()
        )));
    }

    let (mut existing, covered) = {
        let conn = lib.conn();
        let mut existing: HashMap<String, (i64, i64, i64)> = HashMap::new();
        let mut st =
            conn.prepare("SELECT path, mtime, size, id FROM tracks WHERE folder_id = ?1")?;
        for row in st.query_map([folder_id], |r| {
            Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?, r.get(3)?)))
        })? {
            let (p, v) = row?;
            existing.insert(p, v);
        }
        let mut covered = HashSet::new();
        let mut st =
            conn.prepare("SELECT title, album_artist, cover FROM albums WHERE cover IS NOT NULL")?;
        for row in st.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })? {
            let (title, artist, cover) = row?;
            if Path::new(&cover).is_file() {
                covered.insert(Scanner::album_key(&title, &artist));
            }
        }
        (existing, covered)
    };

    let mut sc = Scanner {
        lib,
        folder_id,
        folder: display_path(root),
        covered,
        dir_cover_cache: HashMap::new(),
        now: unix_secs(SystemTime::now()),
        stats: ScanStats::default(),
        last_progress: Instant::now(),
    };
    let mut batch: Vec<Parsed> = Vec::with_capacity(BATCH_SIZE);
    let mut walk_failed = false;

    for entry in WalkDir::new(root).follow_links(false) {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                log::warn!("walk error: {e}");
                walk_failed = true;
                continue;
            }
        };
        if !entry.file_type().is_file() || !is_audio_file(entry.path()) {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                log::warn!("cannot stat {}: {e}", entry.path().display());
                walk_failed = true;
                continue;
            }
        };
        sc.stats.scanned += 1;
        let path_str = display_path(entry.path());
        let mtime = meta.modified().map(unix_secs).unwrap_or(0);
        let size = i64::try_from(meta.len()).unwrap_or(i64::MAX);
        let unchanged = existing
            .remove(&path_str)
            .is_some_and(|(m, s, _)| m == mtime && s == size);
        if !unchanged {
            let parsed = sc.parse(entry.path(), path_str, mtime, size);
            batch.push(parsed);
            sc.stats.updated += 1;
            if batch.len() >= BATCH_SIZE {
                sc.flush(&mut batch)?;
            }
        }
        sc.emit_progress(false, false);
    }
    sc.flush(&mut batch)?;

    // Whatever is left in `existing` was not seen on disk. Only trust that
    // when the whole walk succeeded.
    let stale: Vec<i64> = if walk_failed {
        log::warn!("walk had errors, not removing missing tracks");
        Vec::new()
    } else {
        existing.values().map(|(_, _, id)| *id).collect()
    };
    let orphan_covers = {
        let mut conn = lib.conn();
        let tx = conn.transaction()?;
        {
            let mut del = tx.prepare("DELETE FROM tracks WHERE id = ?1")?;
            for id in &stale {
                del.execute([id])?;
            }
        }
        let covers = db::delete_empty_albums(&tx)?;
        tx.commit()?;
        covers
    };
    remove_cover_files(&orphan_covers);
    sc.stats.removed = stale.len() as u64;

    lib.bump();
    sc.emit_progress(true, true);
    Ok(sc.stats)
}
