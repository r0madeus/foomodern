//! Music library: folder scanning (`walkdir`), tags and embedded covers
//! (`lofty`), SQLite storage (`rusqlite`). Covers are written to a cache
//! folder and shown through the asset protocol, never sent over IPC as
//! base64 (architecture rule 4). Song lists are paged over IPC (rule 4).
//!
//! * [`db`] – schema (versioned with `PRAGMA user_version`) and queries.
//! * [`scan`] – incremental folder scan: walk, read tags, extract covers.
//! * [`Library`] – thread-safe facade: one SQLite connection behind a
//!   mutex, a change `version` counter and a `scanning` flag.
//!
//! Scans run on a background thread, one at a time. Folders requested while
//! a scan is running are queued and processed by the same worker thread.
//! The database lock is only held while a batch (~500 files) is written, so
//! queries stay responsive during a scan.

pub mod db;
pub mod scan;
pub mod tags;

use crate::queue::Track;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

pub use db::SortOrder;
pub use scan::ScanStats;

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, LibraryError>;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FolderInfo {
    pub id: i64,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStats {
    pub tracks: u64,
    pub albums: u64,
    pub version: u64,
    pub scanning: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackMeta {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrackRow {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_id: Option<i64>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<i32>,
    pub duration_ms: Option<u64>,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AlbumRow {
    pub id: i64,
    pub title: String,
    pub album_artist: String,
    pub year: Option<i32>,
    pub track_count: u32,
    /// Absolute path of the cover file in the cache folder, if any.
    pub cover: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistInfo {
    pub id: i64,
    pub name: String,
    pub track_count: u32,
}

/// Result of saving the queue as a playlist.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SavedPlaylist {
    pub playlist: PlaylistInfo,
    /// Queue entries that are not in the library and were left out.
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumDetail {
    pub album: AlbumRow,
    pub tracks: Vec<TrackRow>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub total: u64,
    pub items: Vec<T>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub folder: String,
    pub scanned: u64,
    pub done: bool,
}

/// Callbacks into the host application (Tauri events in the app, no-ops in
/// tests).
pub struct Hooks {
    pub on_progress: Box<dyn Fn(ScanProgress) + Send + Sync>,
    /// Called with the new version after every change.
    pub on_changed: Box<dyn Fn(u64) + Send + Sync>,
}

impl Hooks {
    pub fn none() -> Self {
        Self {
            on_progress: Box::new(|_| {}),
            on_changed: Box::new(|_| {}),
        }
    }
}

pub struct Library {
    conn: Mutex<Connection>,
    covers_dir: PathBuf,
    version: AtomicU64,
    scanning: AtomicBool,
    queue: Mutex<VecDeque<(i64, PathBuf)>>,
    hooks: Hooks,
}

impl Library {
    pub fn open(db_path: &Path, covers_dir: &Path, hooks: Hooks) -> Result<Self> {
        if let Some(dir) = db_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::create_dir_all(covers_dir)?;
        let conn = Connection::open(db_path)?;
        db::init(&conn)?;
        Ok(Self::with_connection(conn, covers_dir, hooks))
    }

    fn with_connection(conn: Connection, covers_dir: &Path, hooks: Hooks) -> Self {
        Self {
            conn: Mutex::new(conn),
            covers_dir: covers_dir.to_path_buf(),
            version: AtomicU64::new(1),
            scanning: AtomicBool::new(false),
            queue: Mutex::new(VecDeque::new()),
            hooks,
        }
    }

    pub(crate) fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn covers_dir(&self) -> &Path {
        &self.covers_dir
    }

    pub fn version(&self) -> u64 {
        self.version.load(Ordering::SeqCst)
    }

    pub fn is_scanning(&self) -> bool {
        self.scanning.load(Ordering::SeqCst)
    }

    /// Bumps the version and notifies the host.
    pub(crate) fn bump(&self) -> u64 {
        let v = self.version.fetch_add(1, Ordering::SeqCst) + 1;
        (self.hooks.on_changed)(v);
        v
    }

    pub(crate) fn progress(&self, p: ScanProgress) {
        (self.hooks.on_progress)(p);
    }

    pub fn folders(&self) -> Result<Vec<FolderInfo>> {
        db::folders(&self.conn())
    }

    /// Registers a folder (idempotent) and returns its id. Does not scan.
    pub fn add_folder(&self, path: &str) -> Result<i64> {
        let p = Path::new(path);
        if !p.is_dir() {
            return Err(LibraryError::Other(format!("not a folder: {path}")));
        }
        let canonical = std::fs::canonicalize(p)?;
        let canonical = scan::display_path(&canonical);
        let id = db::add_folder(&self.conn(), &canonical)?;
        self.bump();
        Ok(id)
    }

    pub fn remove_folder(&self, id: i64) -> Result<()> {
        self.queue
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .retain(|(fid, _)| *fid != id);
        let covers = {
            let mut conn = self.conn();
            db::remove_folder(&mut conn, id)?
        };
        scan::remove_cover_files(&covers);
        self.bump();
        Ok(())
    }

    pub fn stats(&self) -> Result<LibraryStats> {
        let (tracks, albums) = db::counts(&self.conn())?;
        Ok(LibraryStats {
            tracks,
            albums,
            version: self.version(),
            scanning: self.is_scanning(),
        })
    }

    pub fn tracks_page(&self, offset: u64, limit: u64, sort: SortOrder) -> Result<Vec<TrackRow>> {
        db::tracks_page(&self.conn(), offset, limit.min(1000), sort)
    }

    pub fn search_tracks_page(
        &self,
        offset: u64,
        limit: u64,
        sort: SortOrder,
        query: &str,
    ) -> Result<Page<TrackRow>> {
        db::search_tracks_page(&self.conn(), offset, limit.min(1000), sort, query)
    }

    pub fn track_meta(&self, path: &str) -> Result<Option<TrackMeta>> {
        let key = scan::display_path(Path::new(path));
        db::track_meta_by_path(&self.conn(), &key)
    }

    /// Absolute cover path of the album containing the track at `path`.
    pub fn track_cover(&self, path: &str) -> Result<Option<String>> {
        let key = scan::display_path(Path::new(path));
        db::track_cover_by_path(&self.conn(), &key)
    }

    pub fn albums_page(&self, offset: u64, limit: u64) -> Result<Vec<AlbumRow>> {
        db::albums_page(&self.conn(), offset, limit.min(1000))
    }

    pub fn search_albums_page(
        &self,
        offset: u64,
        limit: u64,
        query: &str,
    ) -> Result<Page<AlbumRow>> {
        db::search_albums_page(&self.conn(), offset, limit.min(1000), query)
    }

    pub fn album(&self, album_id: i64) -> Result<AlbumDetail> {
        db::album_detail(&self.conn(), album_id)
    }

    /// Tracks of an album in play order, ready for the queue.
    pub fn album_tracks(&self, album_id: i64) -> Result<Vec<Track>> {
        db::album_tracks(&self.conn(), album_id)
    }

    /// The whole library in `sort` order, ready for the queue.
    pub fn all_tracks(&self, sort: SortOrder) -> Result<Vec<Track>> {
        db::all_tracks(&self.conn(), sort)
    }

    /// Tracks matching `query` in `sort` order, ready for the queue.
    pub fn matching_tracks(&self, sort: SortOrder, query: &str) -> Result<Vec<Track>> {
        db::matching_tracks(&self.conn(), sort, query)
    }

    pub fn playlists(&self) -> Result<Vec<PlaylistInfo>> {
        db::playlists(&self.conn())
    }

    pub fn create_playlist(&self, name: &str, track_ids: &[i64]) -> Result<PlaylistInfo> {
        db::create_playlist(&mut self.conn(), name, track_ids, unix_now())
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<()> {
        db::rename_playlist(&self.conn(), id, name)
    }

    pub fn delete_playlist(&self, id: i64) -> Result<()> {
        db::delete_playlist(&self.conn(), id)
    }

    pub fn playlist_tracks(&self, id: i64, offset: u64, limit: u64) -> Result<Vec<TrackRow>> {
        db::playlist_tracks(&self.conn(), id, offset, limit.min(1000))
    }

    /// Playlist contents in order, ready for the queue.
    pub fn playlist_queue_tracks(&self, id: i64) -> Result<Vec<Track>> {
        db::playlist_queue_tracks(&self.conn(), id)
    }

    /// Appends tracks; ids that are not in the library are skipped.
    /// Returns the number of entries added.
    pub fn playlist_add_tracks(&self, id: i64, track_ids: &[i64]) -> Result<usize> {
        db::playlist_add_tracks(&mut self.conn(), id, track_ids)
    }

    pub fn playlist_add_album(&self, id: i64, album_id: i64) -> Result<usize> {
        db::playlist_add_album(&mut self.conn(), id, album_id)
    }

    /// Removes the entries at the given indexes.
    pub fn playlist_remove(&self, id: i64, indexes: &[usize]) -> Result<()> {
        db::playlist_remove(&mut self.conn(), id, indexes)
    }

    pub fn playlist_move(&self, id: i64, from: usize, to: usize) -> Result<()> {
        db::playlist_move(&mut self.conn(), id, from, to)
    }

    /// Creates a playlist from file paths (e.g. the queue). Files that are
    /// not in the library are skipped and counted.
    pub fn playlist_from_paths(&self, name: &str, paths: &[String]) -> Result<SavedPlaylist> {
        // Library paths are canonical (folders are canonicalized when added),
        // so canonicalize here too: on Windows a path can differ only by
        // 8.3 short names (e.g. `RUNNER~1`) or case and would not match.
        let keys: Vec<String> = paths
            .iter()
            .map(|p| {
                let p = Path::new(p);
                let canonical = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
                scan::display_path(&canonical)
            })
            .collect();
        let mut conn = self.conn();
        let found = db::track_ids_by_paths(&conn, &keys)?;
        let ids: Vec<i64> = found.iter().flatten().copied().collect();
        let playlist = db::create_playlist(&mut conn, name, &ids, unix_now())?;
        Ok(SavedPlaylist {
            playlist,
            skipped: found.len() - ids.len(),
        })
    }

    /// Library file paths (and their folder ids) for the given track ids, in
    /// the same order. Unknown ids are skipped.
    pub fn track_files(&self, ids: &[i64]) -> Result<Vec<(i64, String, i64)>> {
        db::track_files_by_ids(&self.conn(), ids)
    }

    /// Re-reads the tags of the given files into the database (after a tag
    /// edit) and notifies the host. `files` are `(path, folder id)` pairs.
    pub fn rescan_files(&self, files: &[(String, i64)]) -> Result<usize> {
        scan::rescan_files(self, files)
    }

    /// Queues the given folders for scanning and starts the worker thread if
    /// none is running. Folders already queued are not queued twice.
    pub fn start_scan(self: &Arc<Self>, folders: Vec<FolderInfo>) {
        {
            let mut q = self.queue.lock().unwrap_or_else(|p| p.into_inner());
            for f in folders {
                if !q.iter().any(|(id, _)| *id == f.id) {
                    q.push_back((f.id, PathBuf::from(f.path)));
                }
            }
        }
        if self
            .scanning
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return; // the running worker will pick the queued folders up
        }
        let lib = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("library-scan".into())
            .spawn(move || lib.scan_worker());
        if let Err(e) = spawned {
            log::error!("failed to start scan thread: {e}");
            self.scanning.store(false, Ordering::SeqCst);
        }
    }

    fn scan_worker(self: Arc<Self>) {
        loop {
            let next = {
                let mut q = self.queue.lock().unwrap_or_else(|p| p.into_inner());
                let next = q.pop_front();
                if next.is_none() {
                    // Clear the flag while holding the queue lock so that a
                    // concurrent start_scan cannot be lost.
                    self.scanning.store(false, Ordering::SeqCst);
                }
                next
            };
            let Some((id, path)) = next else { break };
            match scan::scan_folder(&self, id, &path) {
                Ok(stats) => log::info!("scanned {}: {stats:?}", path.display()),
                Err(e) => {
                    log::warn!("scan of {} failed: {e}", path.display());
                    self.progress(ScanProgress {
                        folder: path.to_string_lossy().into_owned(),
                        scanned: 0,
                        done: true,
                    });
                }
            }
        }
        self.bump();
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
