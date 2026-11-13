//! Desktop online search, streaming resolution and downloading via `yt-dlp`
//! (with album lookup via the public iTunes Search API). Android keeps stub
//! types and commands cleanly disabled so shared IPC handlers stay uniform.

use crate::library::Library;
use crate::library::scan::display_path;
use crate::session::SessionStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(not(target_os = "android"))]
use crate::library::scan::fnv1a;
#[cfg(not(target_os = "android"))]
use crate::net::client_builder;
#[cfg(not(target_os = "android"))]
use image::ImageFormat;
#[cfg(not(target_os = "android"))]
use lofty::config::WriteOptions;
#[cfg(not(target_os = "android"))]
use lofty::file::{AudioFile, TaggedFileExt};
#[cfg(not(target_os = "android"))]
use lofty::picture::{MimeType, Picture, PictureType};
#[cfg(not(target_os = "android"))]
use lofty::prelude::{Accessor, ItemKey};
#[cfg(not(target_os = "android"))]
use lofty::tag::Tag;
#[cfg(not(target_os = "android"))]
use lofty::tag::items::Timestamp;
#[cfg(not(target_os = "android"))]
use std::io::Cursor;
#[cfg(not(target_os = "android"))]
use std::process::Command;
#[cfg(not(target_os = "android"))]
use std::sync::mpsc;
#[cfg(not(target_os = "android"))]
use std::time::Duration;

#[cfg(not(target_os = "android"))]
const HTTP_TIMEOUT: Duration = Duration::from_secs(12);
#[cfg(not(target_os = "android"))]
const USER_AGENT: &str = concat!("FooModern/", env!("CARGO_PKG_VERSION"));
#[cfg(not(target_os = "android"))]
const MAX_KEPT_JOBS: usize = 25;

#[derive(Debug, thiserror::Error)]
pub enum OnlineError {
    #[error("{0}")]
    Message(String),
    #[cfg(not(target_os = "android"))]
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[cfg(target_os = "android")]
    #[error("online search and download are only available on desktop")]
    Unsupported,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OnlineStatus {
    pub ytdlp_available: bool,
    pub ytdlp_version: Option<String>,
    pub ffmpeg_available: bool,
    pub download_dir: String,
    pub ytdlp_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OnlineAlbum {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<i32>,
    pub track_count: u32,
    /// Local cached cover file inside `covers/online`, if downloaded.
    pub cover: Option<String>,
    pub cover_url: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OnlineTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<i32>,
    pub duration_seconds: Option<f64>,
    pub source_url: Option<String>,
    /// Local cached cover file inside `covers/online`, if downloaded.
    pub cover: Option<String>,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub albums: Vec<OnlineAlbum>,
    pub tracks: Vec<OnlineTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OnlineAlbumDetail {
    pub album: OnlineAlbum,
    pub tracks: Vec<OnlineTrack>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadKind {
    Track,
    Album,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadState {
    Queued,
    Downloading,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadJob {
    pub id: u64,
    pub kind: DownloadKind,
    pub title: String,
    pub artist: String,
    pub cover: Option<String>,
    pub state: DownloadState,
    pub done_tracks: usize,
    pub total_tracks: usize,
    pub current_track: Option<String>,
    pub error: Option<String>,
    pub output_dir: Option<String>,
}

pub type JobsObserver = Arc<dyn Fn(&[DownloadJob]) + Send + Sync>;

#[cfg(not(target_os = "android"))]
struct DownloadWork {
    job_id: u64,
    tracks: Vec<OnlineTrack>,
    download_dir: PathBuf,
    ytdlp_bin: String,
    lib: Arc<Library>,
}

pub struct OnlineService {
    #[cfg(not(target_os = "android"))]
    covers_dir: PathBuf,
    default_download_dir: PathBuf,
    jobs: Arc<Mutex<Vec<DownloadJob>>>,
    next_job_id: AtomicU64,
    stream_covers: Mutex<HashMap<String, String>>,
    #[cfg(not(target_os = "android"))]
    on_jobs: JobsObserver,
    #[cfg(not(target_os = "android"))]
    client: reqwest::Client,
    #[cfg(not(target_os = "android"))]
    worker_tx: mpsc::Sender<DownloadWork>,
}

impl OnlineService {
    pub fn new(covers_dir: PathBuf, default_download_dir: PathBuf, on_jobs: JobsObserver) -> Self {
        #[cfg(target_os = "android")]
        let _ = (covers_dir, on_jobs);
        #[cfg(not(target_os = "android"))]
        let (worker_tx, worker_rx) = mpsc::channel::<DownloadWork>();
        let jobs = Arc::new(Mutex::new(Vec::new()));

        #[cfg(not(target_os = "android"))]
        let client = client_builder()
            .timeout(HTTP_TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        #[cfg(not(target_os = "android"))]
        {
            let worker_jobs = Arc::clone(&jobs);
            let worker_observer = Arc::clone(&on_jobs);
            let worker_covers = covers_dir.clone();
            let worker_client = client.clone();
            let spawned = std::thread::Builder::new()
                .name("online-downloads".into())
                .spawn(move || {
                    download_worker(
                        worker_rx,
                        worker_jobs,
                        worker_observer,
                        worker_covers,
                        worker_client,
                    );
                });
            if let Err(e) = spawned {
                log::error!("failed to start online download worker: {e}");
            }
        }

        Self {
            #[cfg(not(target_os = "android"))]
            covers_dir,
            default_download_dir,
            jobs,
            next_job_id: AtomicU64::new(1),
            stream_covers: Mutex::new(HashMap::new()),
            #[cfg(not(target_os = "android"))]
            on_jobs,
            #[cfg(not(target_os = "android"))]
            client,
            #[cfg(not(target_os = "android"))]
            worker_tx,
        }
    }

    pub fn resolve_download_dir(&self, store: &SessionStore, lib: &Library) -> PathBuf {
        if let Some(dir) = store.download_dir() {
            return dir;
        }
        if let Ok(folders) = lib.folders()
            && let Some(first) = folders.first()
        {
            return PathBuf::from(&first.path);
        }
        self.default_download_dir.clone()
    }

    pub fn resolve_ytdlp_bin(&self, store: &SessionStore) -> String {
        store
            .ytdlp_path()
            .map(|p| display_path(&p))
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "yt-dlp".into())
    }

    pub fn remember_stream_cover(&self, url: &str, cover: Option<String>) {
        let Some(cover) = cover else { return };
        let mut map = self
            .stream_covers
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if map.len() > 64 {
            map.clear();
        }
        map.insert(url.to_owned(), cover);
    }

    pub fn stream_cover(&self, path: &str) -> Option<String> {
        self.stream_covers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(path)
            .cloned()
    }

    pub fn downloads(&self) -> Vec<DownloadJob> {
        self.jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn clear_finished(&self) -> Vec<DownloadJob> {
        let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        jobs.retain(|j| {
            matches!(
                j.state,
                DownloadState::Queued | DownloadState::Downloading
            )
        });
        jobs.clone()
    }

    pub fn next_id(&self) -> u64 {
        self.next_job_id.fetch_add(1, Ordering::Relaxed)
    }

    #[cfg(target_os = "android")]
    pub fn status(&self, store: &SessionStore, lib: &Library) -> OnlineStatus {
        OnlineStatus {
            ytdlp_available: false,
            ytdlp_version: None,
            ffmpeg_available: false,
            download_dir: display_path(&self.resolve_download_dir(store, lib)),
            ytdlp_path: store.ytdlp_path().map(|p| display_path(&p)),
        }
    }

    #[cfg(not(target_os = "android"))]
    pub fn status(&self, store: &SessionStore, lib: &Library) -> OnlineStatus {
        let ytdlp_bin = self.resolve_ytdlp_bin(store);
        let ytdlp_version = command_first_line(&ytdlp_bin, &["--version"]);
        let ffmpeg_available = command_first_line("ffmpeg", &["-version"]).is_some();
        OnlineStatus {
            ytdlp_available: ytdlp_version.is_some(),
            ytdlp_version,
            ffmpeg_available,
            download_dir: display_path(&self.resolve_download_dir(store, lib)),
            ytdlp_path: store.ytdlp_path().map(|p| display_path(&p)),
        }
    }

    #[cfg(target_os = "android")]
    pub async fn search(
        &self,
        _query: &str,
        _ytdlp_bin: &str,
    ) -> Result<SearchResults, OnlineError> {
        Err(OnlineError::Unsupported)
    }

    #[cfg(not(target_os = "android"))]
    pub async fn search(
        &self,
        query: &str,
        ytdlp_bin: &str,
    ) -> Result<SearchResults, OnlineError> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(SearchResults {
                albums: Vec::new(),
                tracks: Vec::new(),
            });
        }
        if is_http_url(q) {
            let url = q.to_owned();
            let bin = ytdlp_bin.to_owned();
            let mut results = tokio::task::spawn_blocking(move || {
                let raw = run_ytdlp_json(&bin, &["--flat-playlist", "--dump-single-json", &url])?;
                parse_ytdlp_flat_json(&raw, Some(&url))
            })
            .await
            .map_err(|e| OnlineError::Message(e.to_string()))??;
            self.cache_search_covers(&mut results).await;
            return Ok(results);
        }

        let client = self.client.clone();
        let q_albums = q.to_owned();
        let albums_handle = tokio::task::spawn(async move {
            fetch_itunes_albums(&client, &q_albums).await
        });

        let bin = ytdlp_bin.to_owned();
        let q_tracks = q.to_owned();
        let tracks_handle = tokio::task::spawn_blocking(move || {
            let target = format!("ytsearch12:{q_tracks}");
            let raw = run_ytdlp_json(
                &bin,
                &[
                    "--flat-playlist",
                    "--dump-single-json",
                    "--default-search",
                    "ytsearch",
                    &target,
                ],
            )?;
            parse_ytdlp_flat_json(&raw, None)
        });

        let tracks_res = tracks_handle
            .await
            .map_err(|e| OnlineError::Message(e.to_string()))?;
        let albums_res = albums_handle
            .await
            .map_err(|e| OnlineError::Message(e.to_string()))?;

        let (albums, tracks) = match (albums_res, tracks_res) {
            (Ok(a), Ok(t)) => (a, t.tracks),
            (Ok(a), Err(e)) => {
                log::warn!("yt-dlp search failed, returning album results only: {e}");
                (a, Vec::new())
            }
            (Err(e), Ok(t)) => {
                log::warn!("iTunes album search failed, returning track results only: {e}");
                (Vec::new(), t.tracks)
            }
            (Err(_), Err(e)) => return Err(e),
        };

        let mut results = SearchResults { albums, tracks };
        self.cache_search_covers(&mut results).await;
        Ok(results)
    }

    #[cfg(target_os = "android")]
    pub async fn album_detail(
        &self,
        _album: &OnlineAlbum,
        _ytdlp_bin: &str,
    ) -> Result<OnlineAlbumDetail, OnlineError> {
        Err(OnlineError::Unsupported)
    }

    #[cfg(not(target_os = "android"))]
    pub async fn album_detail(
        &self,
        album: &OnlineAlbum,
        ytdlp_bin: &str,
    ) -> Result<OnlineAlbumDetail, OnlineError> {
        let mut detail = if let Some(cid) = album.id.strip_prefix("itunes:") {
            fetch_itunes_album_detail(&self.client, cid, album).await?
        } else {
            let url = album
                .source_url
                .clone()
                .or_else(|| album.id.strip_prefix("ytpl:").map(str::to_owned))
                .ok_or_else(|| OnlineError::Message("missing album source".into()))?;
            let bin = ytdlp_bin.to_owned();
            let raw = tokio::task::spawn_blocking(move || {
                run_ytdlp_json(&bin, &["--flat-playlist", "--dump-single-json", &url])
            })
            .await
            .map_err(|e| OnlineError::Message(e.to_string()))??;
            let parsed = parse_ytdlp_flat_json(&raw, album.source_url.as_deref())?;
            let resolved_album = parsed.albums.into_iter().next().unwrap_or_else(|| album.clone());
            OnlineAlbumDetail {
                album: resolved_album,
                tracks: parsed.tracks,
            }
        };

        if detail.album.cover.is_none()
            && let Some(url) = detail.album.cover_url.clone()
        {
            detail.album.cover = ensure_cached_cover(&self.client, &self.covers_dir, &url).await;
        }
        let album_cover = detail.album.cover.clone();
        let album_cover_url = detail.album.cover_url.clone();
        for t in &mut detail.tracks {
            if t.cover.is_none() {
                t.cover = album_cover.clone();
            }
            if t.cover_url.is_none() {
                t.cover_url = album_cover_url.clone();
            }
        }
        Ok(detail)
    }

    #[cfg(target_os = "android")]
    pub async fn resolve_stream(
        &self,
        _track: &OnlineTrack,
        _ytdlp_bin: &str,
    ) -> Result<(String, Option<String>), OnlineError> {
        Err(OnlineError::Unsupported)
    }

    #[cfg(not(target_os = "android"))]
    pub async fn resolve_stream(
        &self,
        track: &OnlineTrack,
        ytdlp_bin: &str,
    ) -> Result<(String, Option<String>), OnlineError> {
        let cover = match (&track.cover, &track.cover_url) {
            (Some(c), _) => Some(c.clone()),
            (None, Some(url)) => ensure_cached_cover(&self.client, &self.covers_dir, url).await,
            (None, None) => None,
        };
        let target = yt_search_query(track);
        let bin = ytdlp_bin.to_owned();
        let stream_url = tokio::task::spawn_blocking(move || {
            let args = ["--no-playlist", "-f", "bestaudio/best", "-g", &target];
            let out = run_ytdlp_lines(&bin, &args)?;
            out.into_iter()
                .find(|l| !l.trim().is_empty())
                .ok_or_else(|| OnlineError::Message("yt-dlp did not return a stream URL".into()))
        })
        .await
        .map_err(|e| OnlineError::Message(e.to_string()))??;
        self.remember_stream_cover(&stream_url, cover.clone());
        Ok((stream_url, cover))
    }

    #[cfg(target_os = "android")]
    pub fn enqueue_download(
        &self,
        _kind: DownloadKind,
        _title: String,
        _artist: String,
        _cover: Option<String>,
        _tracks: Vec<OnlineTrack>,
        _store: &SessionStore,
        _lib: &Arc<Library>,
    ) -> Result<DownloadJob, OnlineError> {
        Err(OnlineError::Unsupported)
    }

    #[cfg(not(target_os = "android"))]
    pub fn enqueue_download(
        &self,
        kind: DownloadKind,
        title: String,
        artist: String,
        cover: Option<String>,
        tracks: Vec<OnlineTrack>,
        store: &SessionStore,
        lib: &Arc<Library>,
    ) -> Result<DownloadJob, OnlineError> {
        if tracks.is_empty() {
            return Err(OnlineError::Message("no tracks to download".into()));
        }
        let download_dir = self.resolve_download_dir(store, lib);
        let ytdlp_bin = self.resolve_ytdlp_bin(store);
        let job_id = self.next_id();
        let job = DownloadJob {
            id: job_id,
            kind,
            title,
            artist,
            cover,
            state: DownloadState::Queued,
            done_tracks: 0,
            total_tracks: tracks.len(),
            current_track: None,
            error: None,
            output_dir: Some(display_path(&download_dir)),
        };

        let snapshot = {
            let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
            jobs.insert(0, job.clone());
            trim_finished_jobs(&mut jobs);
            jobs.clone()
        };
        (self.on_jobs)(&snapshot);

        self.worker_tx
            .send(DownloadWork {
                job_id,
                tracks,
                download_dir,
                ytdlp_bin,
                lib: Arc::clone(lib),
            })
            .map_err(|_| OnlineError::Message("download worker is not running".into()))?;
        Ok(job)
    }

    #[cfg(not(target_os = "android"))]
    async fn cache_search_covers(&self, results: &mut SearchResults) {
        let mut handles = Vec::new();
        for a in results.albums.iter().take(12) {
            if a.cover.is_none()
                && let Some(url) = a.cover_url.clone()
            {
                let client = self.client.clone();
                let dir = self.covers_dir.clone();
                handles.push(tokio::task::spawn(async move {
                    let cached = ensure_cached_cover(&client, &dir, &url).await;
                    (url, cached)
                }));
            }
        }
        let mut resolved = HashMap::new();
        for h in handles {
            if let Ok((url, Some(path))) = h.await {
                resolved.insert(url, path);
            }
        }
        for a in &mut results.albums {
            if a.cover.is_none()
                && let Some(url) = &a.cover_url
                && let Some(path) = resolved.get(url)
            {
                a.cover = Some(path.clone());
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
fn trim_finished_jobs(jobs: &mut Vec<DownloadJob>) {
    while jobs.len() > MAX_KEPT_JOBS {
        if let Some(pos) = jobs.iter().rposition(|j| {
            matches!(j.state, DownloadState::Done | DownloadState::Failed)
        }) {
            jobs.remove(pos);
        } else {
            break;
        }
    }
}

#[cfg(not(target_os = "android"))]
fn update_job(
    jobs: &Mutex<Vec<DownloadJob>>,
    on_jobs: &JobsObserver,
    job_id: u64,
    f: impl FnOnce(&mut DownloadJob),
) {
    let snapshot = {
        let mut list = jobs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = list.iter_mut().find(|j| j.id == job_id) {
            f(job);
        }
        list.clone()
    };
    on_jobs(&snapshot);
}

#[cfg(not(target_os = "android"))]
fn download_worker(
    rx: mpsc::Receiver<DownloadWork>,
    jobs: Arc<Mutex<Vec<DownloadJob>>>,
    on_jobs: JobsObserver,
    covers_dir: PathBuf,
    client: reqwest::Client,
) {
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        log::error!("failed to build HTTP runtime for online downloads");
        return;
    };

    while let Ok(work) = rx.recv() {
        let ffmpeg_ok = command_first_line("ffmpeg", &["-version"]).is_some();
        update_job(&jobs, &on_jobs, work.job_id, |j| {
            j.state = DownloadState::Downloading;
            j.current_track = work.tracks.first().map(|t| t.title.clone());
        });

        let mut succeeded = 0usize;
        let mut last_error: Option<String> = None;
        let mut cached_covers: HashMap<String, Option<Vec<u8>>> = HashMap::new();

        for track in &work.tracks {
            update_job(&jobs, &on_jobs, work.job_id, |j| {
                j.current_track = Some(track.title.clone());
            });
            let cover_bytes = resolve_cover_bytes(
                track,
                &covers_dir,
                &client,
                &rt,
                &mut cached_covers,
            );
            match download_one_track(
                track,
                &work.download_dir,
                &work.ytdlp_bin,
                ffmpeg_ok,
                cover_bytes.as_deref(),
            ) {
                Ok(_path) => {
                    succeeded += 1;
                    update_job(&jobs, &on_jobs, work.job_id, |j| {
                        j.done_tracks = succeeded;
                    });
                }
                Err(e) => {
                    log::warn!("failed to download '{}': {e}", track.title);
                    last_error = Some(e.to_string());
                }
            }
        }

        if succeeded > 0 {
            register_and_scan_dir(&work.lib, &work.download_dir);
        }

        update_job(&jobs, &on_jobs, work.job_id, |j| {
            j.current_track = None;
            j.done_tracks = succeeded;
            if succeeded == 0 {
                j.state = DownloadState::Failed;
                j.error = last_error.or_else(|| Some("download failed".into()));
            } else if succeeded < j.total_tracks {
                j.state = DownloadState::Done;
                j.error = last_error;
            } else {
                j.state = DownloadState::Done;
                j.error = None;
            }
        });
    }
}

#[cfg(not(target_os = "android"))]
fn resolve_cover_bytes(
    track: &OnlineTrack,
    covers_dir: &Path,
    client: &reqwest::Client,
    rt: &tokio::runtime::Runtime,
    cache: &mut HashMap<String, Option<Vec<u8>>>,
) -> Option<Vec<u8>> {
    if let Some(path) = &track.cover
        && let Ok(bytes) = std::fs::read(path)
    {
        return Some(bytes);
    }
    let url = track.cover_url.as_ref()?;
    if let Some(entry) = cache.get(url) {
        return entry.clone();
    }
    let downloaded = rt.block_on(async {
        let cached = ensure_cached_cover(client, covers_dir, url).await?;
        std::fs::read(cached).ok()
    });
    cache.insert(url.clone(), downloaded.clone());
    downloaded
}

#[cfg(not(target_os = "android"))]
fn register_and_scan_dir(lib: &Arc<Library>, dir: &Path) {
    if let Err(e) = std::fs::create_dir_all(dir) {
        log::warn!("could not create download dir {}: {e}", dir.display());
        return;
    }
    let canonical = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let key = display_path(&canonical);
    let folder_id = match lib.add_folder(&key) {
        Ok(id) => id,
        Err(e) => {
            log::warn!("could not add download dir {key} to library: {e}");
            return;
        }
    };
    if let Ok(folders) = lib.folders() {
        let matched: Vec<_> = folders.into_iter().filter(|f| f.id == folder_id).collect();
        if !matched.is_empty() {
            lib.start_scan(matched);
        }
    }
}

#[cfg(not(target_os = "android"))]
fn download_one_track(
    track: &OnlineTrack,
    download_dir: &Path,
    ytdlp_bin: &str,
    ffmpeg_available: bool,
    cover_bytes: Option<&[u8]>,
) -> Result<PathBuf, OnlineError> {
    let (subdirs, stem) = target_track_stem(track);
    let mut target_dir = download_dir.to_path_buf();
    for seg in subdirs {
        target_dir.push(seg);
    }
    std::fs::create_dir_all(&target_dir)?;

    let out_tpl = target_dir.join(format!("{stem}.%(ext)s"));
    let out_tpl_str = display_path(&out_tpl);
    let query = yt_search_query(track);

    let mut args: Vec<&str> = vec![
        "--no-playlist",
        "--no-warnings",
        "--no-progress",
        "--force-overwrites",
        "--print",
        "after_move:filepath",
    ];
    if ffmpeg_available {
        args.extend(["-x", "--audio-format", "m4a", "-f", "bestaudio/best"]);
    } else {
        args.extend(["-f", "bestaudio[ext=m4a]/bestaudio"]);
    }
    args.extend(["-o", &out_tpl_str, &query]);

    let lines = run_ytdlp_lines(ytdlp_bin, &args)?;
    let produced = lines
        .into_iter()
        .rev()
        .map(|l| l.trim().to_owned())
        .find(|l| !l.is_empty())
        .ok_or_else(|| OnlineError::Message("yt-dlp produced no output file".into()))?;
    let path = PathBuf::from(&produced);
    if !path.is_file() {
        return Err(OnlineError::Message(format!(
            "downloaded file missing: {produced}"
        )));
    }
    if let Err(e) = write_downloaded_tags(&path, track, cover_bytes) {
        log::warn!("failed to write tags on {}: {e}", path.display());
    }
    Ok(path)
}

#[cfg(not(target_os = "android"))]
fn no_window_cmd(bin: &str) -> Command {
    let mut cmd = Command::new(bin);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[cfg(not(target_os = "android"))]
fn command_first_line(bin: &str, args: &[&str]) -> Option<String> {
    let out = no_window_cmd(bin).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_owned)
}

#[cfg(not(target_os = "android"))]
fn run_ytdlp_bytes(bin: &str, args: &[&str]) -> Result<Vec<u8>, OnlineError> {
    let out = no_window_cmd(bin).args(args).output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            OnlineError::Message(
                "yt-dlp was not found. Install yt-dlp or set its path in Settings.".into(),
            )
        } else {
            OnlineError::Io(e)
        }
    })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        let msg = if stderr.is_empty() {
            format!("yt-dlp exited with {}", out.status)
        } else {
            stderr
        };
        return Err(OnlineError::Message(msg));
    }
    Ok(out.stdout)
}

#[cfg(not(target_os = "android"))]
fn run_ytdlp_json(bin: &str, args: &[&str]) -> Result<String, OnlineError> {
    let mut full = Vec::with_capacity(args.len() + 2);
    full.extend(["--no-warnings", "--encoding", "utf-8"]);
    full.extend_from_slice(args);
    let bytes = run_ytdlp_bytes(bin, &full)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(not(target_os = "android"))]
fn run_ytdlp_lines(bin: &str, args: &[&str]) -> Result<Vec<String>, OnlineError> {
    let bytes = run_ytdlp_bytes(bin, args)?;
    Ok(String::from_utf8_lossy(&bytes)
        .lines()
        .map(str::to_owned)
        .collect())
}

#[cfg(not(target_os = "android"))]
async fn ensure_cached_cover(
    client: &reqwest::Client,
    covers_dir: &Path,
    url: &str,
) -> Option<String> {
    let file = covers_dir.join(format!("{:016x}.jpg", fnv1a(url.as_bytes())));
    if file.is_file() {
        return Some(display_path(&file));
    }
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = resp.bytes().await.ok()?;
    if std::fs::create_dir_all(covers_dir).is_err() {
        return None;
    }
    let normalized = normalize_cover_jpeg(&bytes)?;
    let tmp = file.with_extension("tmp");
    std::fs::write(&tmp, &normalized).ok()?;
    std::fs::rename(&tmp, &file).ok()?;
    Some(display_path(&file))
}

#[cfg(not(target_os = "android"))]
fn normalize_cover_jpeg(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) || bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Some(bytes.to_vec());
    }
    let img = image::load_from_memory(bytes).ok()?;
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Jpeg)
        .ok()?;
    Some(buf)
}

#[cfg(not(target_os = "android"))]
async fn fetch_itunes_albums(
    client: &reqwest::Client,
    query: &str,
) -> Result<Vec<OnlineAlbum>, OnlineError> {
    let resp = client
        .get("https://itunes.apple.com/search")
        .query(&[
            ("term", query),
            ("entity", "album"),
            ("media", "music"),
            ("limit", "12"),
        ])
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(OnlineError::Message(format!(
            "album search returned status {}",
            resp.status()
        )));
    }
    let text = resp.text().await?;
    parse_itunes_albums(&text)
}

#[cfg(not(target_os = "android"))]
async fn fetch_itunes_album_detail(
    client: &reqwest::Client,
    collection_id: &str,
    fallback: &OnlineAlbum,
) -> Result<OnlineAlbumDetail, OnlineError> {
    let resp = client
        .get("https://itunes.apple.com/lookup")
        .query(&[("id", collection_id), ("entity", "song"), ("limit", "200")])
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(OnlineError::Message(format!(
            "album lookup returned status {}",
            resp.status()
        )));
    }
    let text = resp.text().await?;
    parse_itunes_album_detail(&text, fallback)
}

pub fn is_http_url(s: &str) -> bool {
    let t = s.trim();
    t.starts_with("http://") || t.starts_with("https://")
}

pub fn yt_search_query(track: &OnlineTrack) -> String {
    if let Some(url) = track.source_url.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        return url.to_owned();
    }
    if track.artist.trim().is_empty() {
        format!("ytsearch1:{}", track.title.trim())
    } else {
        format!("ytsearch1:{} - {}", track.artist.trim(), track.title.trim())
    }
}

pub fn safe_segment(raw: &str) -> String {
    let replaced: String = raw
        .chars()
        .map(|c| {
            if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control()
            {
                '_'
            } else {
                c
            }
        })
        .collect();
    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_matches([' ', '.']);
    if trimmed.is_empty() {
        return "Unknown".into();
    }
    trimmed.chars().take(80).collect::<String>().trim_end_matches([' ', '.']).to_owned()
}

pub fn target_track_stem(track: &OnlineTrack) -> (Vec<String>, String) {
    let title = safe_segment(&track.title);
    if track.album.trim().is_empty() {
        let artist = if track.artist.trim().is_empty() {
            "Singles".into()
        } else {
            safe_segment(&track.artist)
        };
        return (vec![artist], title);
    }

    let artist = if !track.album_artist.trim().is_empty() {
        safe_segment(&track.album_artist)
    } else if !track.artist.trim().is_empty() {
        safe_segment(&track.artist)
    } else {
        "Unknown".into()
    };
    let album_name = safe_segment(&track.album);
    let album_dir = match track.year {
        Some(y) if (1..=9999).contains(&y) => format!("{album_name} ({y})"),
        _ => album_name,
    };
    let stem = match (track.disc_no, track.track_no) {
        (Some(d), Some(n)) if d > 1 => format!("{d}-{n:02} - {title}"),
        (_, Some(n)) => format!("{n:02} - {title}"),
        _ => title,
    };
    (vec![artist, album_dir], stem)
}

#[cfg(not(target_os = "android"))]
pub fn write_downloaded_tags(
    path: &Path,
    track: &OnlineTrack,
    cover_bytes: Option<&[u8]>,
) -> Result<(), OnlineError> {
    let mut tagged = lofty::read_from_path(path)
        .map_err(|e| OnlineError::Message(format!("cannot open audio file for tagging: {e}")))?;
    let primary = tagged.primary_tag_type();
    if tagged.tag(primary).is_none() {
        let seeded = match tagged.first_tag() {
            Some(other) => {
                let mut clone = other.clone();
                clone.re_map(primary);
                clone
            }
            None => Tag::new(primary),
        };
        tagged.insert_tag(seeded);
    }
    let Some(tag) = tagged.tag_mut(primary) else {
        return Err(OnlineError::Message(
            "audio format does not support tags".into(),
        ));
    };
    if !track.title.trim().is_empty() {
        tag.set_title(track.title.trim().to_owned());
    }
    if !track.artist.trim().is_empty() {
        tag.set_artist(track.artist.trim().to_owned());
    }
    if !track.album.trim().is_empty() {
        tag.set_album(track.album.trim().to_owned());
    }
    let aa = if track.album_artist.trim().is_empty() {
        track.artist.trim()
    } else {
        track.album_artist.trim()
    };
    if !aa.is_empty() && !track.album.trim().is_empty() {
        tag.insert_text(ItemKey::AlbumArtist, aa.to_owned());
    }
    if let Some(n) = track.track_no {
        tag.set_track(n);
    }
    if let Some(d) = track.disc_no {
        tag.set_disk(d);
    }
    if let Some(y) = track.year.and_then(|y| u16::try_from(y).ok()).filter(|&y| y > 0) {
        tag.set_date(Timestamp {
            year: y,
            ..Timestamp::default()
        });
    }
    if let Some(bytes) = cover_bytes
        && let Some((mime, data)) = prepare_picture_bytes(bytes)
    {
        tag.remove_picture_type(PictureType::CoverFront);
        tag.push_picture(
            Picture::unchecked(data)
                .pic_type(PictureType::CoverFront)
                .mime_type(mime)
                .build(),
        );
    }

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.foomodern-dl.tmp"));
    let res = (|| -> Result<(), OnlineError> {
        std::fs::copy(path, &tmp)?;
        tagged
            .save_to_path(&tmp, WriteOptions::default())
            .map_err(|e| OnlineError::Message(format!("cannot save tags: {e}")))?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

#[cfg(not(target_os = "android"))]
fn prepare_picture_bytes(bytes: &[u8]) -> Option<(MimeType, Vec<u8>)> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Some((MimeType::Png, bytes.to_vec()));
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some((MimeType::Jpeg, bytes.to_vec()));
    }
    let img = image::load_from_memory(bytes).ok()?;
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Jpeg)
        .ok()?;
    Some((MimeType::Jpeg, buf))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesEnvelope {
    #[serde(default)]
    results: Vec<ItunesItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesItem {
    #[serde(default)]
    wrapper_type: String,
    collection_id: Option<u64>,
    track_id: Option<u64>,
    collection_name: Option<String>,
    artist_name: Option<String>,
    track_name: Option<String>,
    track_number: Option<u32>,
    disc_number: Option<u32>,
    track_count: Option<u32>,
    track_time_millis: Option<u64>,
    release_date: Option<String>,
    artwork_url100: Option<String>,
}

fn parse_year(iso: Option<&str>) -> Option<i32> {
    iso.and_then(|s| s.get(..4))
        .and_then(|y| y.parse::<i32>().ok())
        .filter(|&y| (1..=9999).contains(&y))
}

fn highres_itunes_cover(url: Option<&str>) -> Option<String> {
    url.map(|u| u.replace("100x100bb", "600x600bb"))
}

pub fn parse_itunes_albums(json: &str) -> Result<Vec<OnlineAlbum>, OnlineError> {
    let env: ItunesEnvelope = serde_json::from_str(json)?;
    let mut out = Vec::new();
    for item in env.results {
        let Some(id) = item.collection_id else { continue };
        let Some(title) = item.collection_name.filter(|s| !s.trim().is_empty()) else {
            continue;
        };
        let artist = item.artist_name.unwrap_or_default();
        out.push(OnlineAlbum {
            id: format!("itunes:{id}"),
            title,
            artist,
            year: parse_year(item.release_date.as_deref()),
            track_count: item.track_count.unwrap_or(0),
            cover: None,
            cover_url: highres_itunes_cover(item.artwork_url100.as_deref()),
            source_url: None,
        });
    }
    Ok(out)
}

pub fn parse_itunes_album_detail(
    json: &str,
    fallback: &OnlineAlbum,
) -> Result<OnlineAlbumDetail, OnlineError> {
    let env: ItunesEnvelope = serde_json::from_str(json)?;
    let mut album = fallback.clone();
    if let Some(col) = env.results.iter().find(|i| i.wrapper_type == "collection") {
        if let Some(title) = col.collection_name.as_ref().filter(|s| !s.trim().is_empty()) {
            album.title = title.clone();
        }
        if let Some(artist) = col.artist_name.as_ref().filter(|s| !s.trim().is_empty()) {
            album.artist = artist.clone();
        }
        if album.year.is_none() {
            album.year = parse_year(col.release_date.as_deref());
        }
        if album.cover_url.is_none() {
            album.cover_url = highres_itunes_cover(col.artwork_url100.as_deref());
        }
        if let Some(tc) = col.track_count {
            album.track_count = tc;
        }
    }

    let mut tracks = Vec::new();
    for item in env.results {
        if item.wrapper_type != "track" {
            continue;
        }
        let Some(title) = item.track_name.filter(|s| !s.trim().is_empty()) else {
            continue;
        };
        let artist = item
            .artist_name
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| album.artist.clone());
        let id = item
            .track_id
            .map(|n| format!("itunes:{n}"))
            .unwrap_or_else(|| format!("{}:{}", album.id, tracks.len() + 1));
        tracks.push(OnlineTrack {
            id,
            title,
            artist,
            album: album.title.clone(),
            album_artist: album.artist.clone(),
            track_no: item.track_number,
            disc_no: item.disc_number,
            year: album.year.or_else(|| parse_year(item.release_date.as_deref())),
            duration_seconds: item.track_time_millis.map(|ms| ms as f64 / 1000.0),
            source_url: None,
            cover: album.cover.clone(),
            cover_url: album.cover_url.clone(),
        });
    }
    tracks.sort_by_key(|t| (t.disc_no.unwrap_or(1), t.track_no.unwrap_or(u32::MAX)));
    if album.track_count == 0 {
        album.track_count = tracks.len() as u32;
    }
    Ok(OnlineAlbumDetail { album, tracks })
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct YtThumb {
    url: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct YtNode {
    #[serde(rename = "_type")]
    kind: Option<String>,
    id: Option<String>,
    title: Option<String>,
    url: Option<String>,
    webpage_url: Option<String>,
    uploader: Option<String>,
    channel: Option<String>,
    artist: Option<String>,
    track: Option<String>,
    album: Option<String>,
    release_year: Option<i32>,
    duration: Option<f64>,
    thumbnail: Option<String>,
    thumbnails: Vec<YtThumb>,
    entries: Option<Vec<YtNode>>,
}

fn clean_yt_artist(raw: Option<String>) -> String {
    raw.map(|s| s.trim().strip_suffix(" - Topic").unwrap_or(s.trim()).to_owned())
        .unwrap_or_default()
}

fn pick_yt_thumb(node: &YtNode) -> Option<String> {
    node.thumbnail
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            node.thumbnails
                .iter()
                .rev()
                .filter_map(|t| t.url.clone())
                .find(|s| !s.is_empty())
        })
}

fn resolve_yt_url(node: &YtNode) -> Option<String> {
    if let Some(u) = node.webpage_url.as_deref().filter(|s| is_http_url(s)) {
        return Some(u.to_owned());
    }
    if let Some(u) = node.url.as_deref().filter(|s| is_http_url(s)) {
        return Some(u.to_owned());
    }
    node.id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|id| format!("https://www.youtube.com/watch?v={id}"))
}

pub fn parse_ytdlp_flat_json(
    json: &str,
    pasted_url: Option<&str>,
) -> Result<SearchResults, OnlineError> {
    let root: YtNode = serde_json::from_str(json)?;
    if let Some(entries) = root.entries {
        let is_playlist = pasted_url.is_some()
            && (root.kind.as_deref() == Some("playlist") || entries.len() > 1);
        let playlist_title = root
            .title
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Playlist".into());
        let playlist_artist = clean_yt_artist(root.artist.or(root.channel).or(root.uploader));
        let playlist_thumb = root
            .thumbnail
            .clone()
            .or_else(|| root.thumbnails.last().and_then(|t| t.url.clone()));

        let mut tracks = Vec::new();
        for (i, e) in entries.into_iter().enumerate() {
            let Some(source_url) = resolve_yt_url(&e) else {
                continue;
            };
            let title = e
                .track
                .clone()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| e.title.clone())
                .unwrap_or_else(|| "Untitled".into());
            let artist = clean_yt_artist(
                e.artist
                    .clone()
                    .or_else(|| e.channel.clone())
                    .or_else(|| e.uploader.clone()),
            );
            let thumb = pick_yt_thumb(&e).or_else(|| playlist_thumb.clone());
            tracks.push(OnlineTrack {
                id: e.id.unwrap_or_else(|| source_url.clone()),
                title,
                artist: if artist.is_empty() {
                    playlist_artist.clone()
                } else {
                    artist
                },
                album: if is_playlist {
                    e.album.unwrap_or_else(|| playlist_title.clone())
                } else {
                    e.album.unwrap_or_default()
                },
                album_artist: if is_playlist {
                    playlist_artist.clone()
                } else {
                    String::new()
                },
                track_no: is_playlist.then_some((i + 1) as u32),
                disc_no: None,
                year: e.release_year,
                duration_seconds: e.duration,
                source_url: Some(source_url),
                cover: None,
                cover_url: thumb,
            });
        }

        let albums = match pasted_url {
            Some(url) if is_playlist => vec![OnlineAlbum {
                id: format!("ytpl:{url}"),
                title: playlist_title,
                artist: playlist_artist,
                year: root.release_year,
                track_count: tracks.len() as u32,
                cover: None,
                cover_url: playlist_thumb.or_else(|| {
                    tracks.first().and_then(|t| t.cover_url.clone())
                }),
                source_url: Some(url.to_owned()),
            }],
            _ => Vec::new(),
        };

        return Ok(SearchResults { albums, tracks });
    }

    let source_url = resolve_yt_url(&root)
        .or_else(|| pasted_url.map(str::to_owned))
        .ok_or_else(|| OnlineError::Message("no playable track found".into()))?;
    let title = root
        .track
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| root.title.clone())
        .unwrap_or_else(|| "Untitled".into());
    let artist = clean_yt_artist(
        root.artist
            .clone()
            .or_else(|| root.channel.clone())
            .or_else(|| root.uploader.clone()),
    );
    let thumb = pick_yt_thumb(&root);
    Ok(SearchResults {
        albums: Vec::new(),
        tracks: vec![OnlineTrack {
            id: root.id.unwrap_or_else(|| source_url.clone()),
            title,
            artist,
            album: root.album.unwrap_or_default(),
            album_artist: String::new(),
            track_no: None,
            disc_no: None,
            year: root.release_year,
            duration_seconds: root.duration,
            source_url: Some(source_url),
            cover: None,
            cover_url: thumb,
        }],
    })
}

#[cfg(test)]
mod tests;
