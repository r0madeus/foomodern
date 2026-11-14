//! Synced and plain lyrics. Lookup order: `.lrc` sidecar, embedded lyrics
//! tag, app cache, LRCLIB, NetEase (when enabled). Lyrics found online can
//! be saved permanently next to the song or inside its tag.

pub mod cache;
pub mod lrc;
pub mod lrclib;
pub mod netease;
pub mod publish;
pub mod save;

use crate::library::tags::{self, is_playing_path};
use crate::library::{Library, SortOrder, scan};
use cache::{LyricsCache, cache_key, now_unix_secs};
use lrclib::{DEFAULT_BASE_URL, LrclibClient, PublishBody};
use netease::NeteaseClient;
pub use save::SaveMode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum LyricsError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("lrclib returned status {0}")]
    HttpStatus(u16),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("task error: {0}")]
    Task(String),
    #[error("selected file has no readable lyrics")]
    InvalidFile,
    #[error("sharing was cancelled")]
    Cancelled,
    #[error("{0}")]
    Share(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LyricsSource {
    LocalLrc,
    Lrclib,
    /// The lyrics tag inside the audio file.
    Embedded,
    Netease,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub time_seconds: f64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LyricsResult {
    Synced {
        source: LyricsSource,
        lines: Vec<LyricLine>,
    },
    Plain {
        source: LyricsSource,
        lines: Vec<String>,
    },
    Instrumental {
        source: LyricsSource,
    },
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackQuery {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: Option<u64>,
}

fn parse_lrc_text(text: &str, source: LyricsSource) -> Option<LyricsResult> {
    Some(match lrc::parse(text)? {
        lrc::ParsedLyrics::Synced(lines) => LyricsResult::Synced { source, lines },
        lrc::ParsedLyrics::Plain(lines) => LyricsResult::Plain { source, lines },
    })
}

fn parse_lrc_bytes(bytes: &[u8]) -> Option<LyricsResult> {
    parse_lrc_text(&String::from_utf8_lossy(bytes), LyricsSource::LocalLrc)
}

/// Lyrics stored in the audio file's own lyrics tag. LRC text with
/// timestamps is synced, anything else plain.
pub fn read_embedded(audio_path: &Path) -> Option<LyricsResult> {
    parse_lrc_text(&tags::read_lyrics(audio_path)?, LyricsSource::Embedded)
}

/// Sidecar first, then the embedded tag.
fn find_stored(audio_path: &Path) -> Option<LyricsResult> {
    find_local_lrc(audio_path).or_else(|| read_embedded(audio_path))
}

pub fn find_local_lrc(audio_path: &Path) -> Option<LyricsResult> {
    let bytes = std::fs::read(audio_path.with_extension("lrc"))
        .or_else(|_| std::fs::read(audio_path.with_extension("LRC")))
        .ok()?;
    parse_lrc_bytes(&bytes)
}

/// Validates a local `.lrc`/text file and saves it as a sidecar next to `audio_path`.
pub fn attach_local_lrc(audio_path: &Path, lrc_path: &Path) -> Result<LyricsResult, LyricsError> {
    if audio_path.file_name().is_none() {
        let err = std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid track path");
        return Err(err.into());
    }
    let bytes = std::fs::read(lrc_path)?;
    let result = parse_lrc_bytes(&bytes).ok_or(LyricsError::InvalidFile)?;
    let dest = audio_path.with_extension("lrc");
    let same_dest = std::fs::canonicalize(lrc_path)
        .ok()
        .zip(std::fs::canonicalize(&dest).ok())
        .is_some_and(|(a, b)| a == b);
    if !same_dest {
        scan::write_atomic(&dest, &bytes)?;
    }
    Ok(result)
}

fn resolve_query_sync(
    lib: &Library,
    path: &str,
    duration_hint_secs: Option<f64>,
) -> Option<TrackQuery> {
    let audio_path = Path::new(path);
    let meta = lib
        .track_meta(path)
        .ok()
        .flatten()
        .or_else(|| scan::read_track_meta(audio_path))?;

    let title = meta.title.trim().to_string();
    let artist = meta.artist.trim().to_string();
    if title.is_empty() || artist.is_empty() {
        return None;
    }

    let duration_secs = meta
        .duration_ms
        .filter(|ms| *ms > 0)
        .map(|ms| (ms + 500) / 1000)
        .or_else(|| {
            duration_hint_secs
                .filter(|d| d.is_finite() && *d > 0.0)
                .map(|d| d.round() as u64)
        });

    Some(TrackQuery {
        title,
        artist,
        album: meta.album.trim().to_string(),
        duration_secs,
    })
}

enum LocalOrQuery {
    Local(LyricsResult),
    Query(Option<TrackQuery>),
}

/// Per-call settings; the app builds them from the saved settings. The
/// default (no saving, no NetEase) never writes next to the user's files.
#[derive(Debug, Clone)]
pub struct LyricsOptions {
    pub save: SaveMode,
    /// Ask NetEase when LRCLIB has nothing.
    pub netease: bool,
    /// The file the player has loaded: never rewritten while it is current.
    pub current: Option<String>,
}

impl Default for LyricsOptions {
    fn default() -> Self {
        Self::new(SaveMode::Off, false, None)
    }
}

impl LyricsOptions {
    pub fn new(save: SaveMode, netease: bool, current: Option<String>) -> Self {
        Self {
            save,
            netease,
            current,
        }
    }
}

/// How many cached lyrics a bulk save wrote.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct BulkSaveOutcome {
    pub saved: u64,
    pub skipped: u64,
}

/// Library lyrics coverage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    pub with_lyrics: u64,
    pub without_lyrics: u64,
    pub unchecked: u64,
}

/// Why a track's lyrics cannot be shared (the UI shows a translated text).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShareBlock {
    /// No own lyrics (`.lrc` or tag) to share.
    NoLocalLyrics,
    /// Title, artist, album and duration are all needed.
    NeedsMetadata,
    /// LRCLIB already has lyrics for the track.
    OnLrclib,
    /// The lyrics came from another service, not from the user.
    OtherSource,
    /// LRCLIB could not be reached.
    Offline,
    /// Another share is running.
    Busy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareStatus {
    pub available: bool,
    pub reason: Option<ShareBlock>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShareOutcome {
    Published,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct LyricsService {
    cache: LyricsCache,
    client: LrclibClient,
    netease: NeteaseClient,
    /// Embed writes waiting for their track to stop being the current one.
    pending_embeds: Arc<Mutex<HashMap<String, String>>>,
    share_cancel: Arc<AtomicBool>,
    sharing: Arc<AtomicBool>,
    challenge_ttl: Duration,
}

fn blocking_err(e: tokio::task::JoinError) -> LyricsError {
    LyricsError::Task(e.to_string())
}

impl LyricsService {
    pub fn new(cache_dir: &Path) -> Result<Self, LyricsError> {
        Self::with_base_urls(cache_dir, DEFAULT_BASE_URL, netease::DEFAULT_BASE_URL)
    }

    /// Tests: LRCLIB on `base_url`; NetEase stays at its default address and
    /// is only used when [`LyricsOptions::netease`] is set.
    pub fn with_base_url(cache_dir: &Path, base_url: &str) -> Result<Self, LyricsError> {
        Self::with_base_urls(cache_dir, base_url, netease::DEFAULT_BASE_URL)
    }

    pub fn with_base_urls(
        cache_dir: &Path,
        lrclib_url: &str,
        netease_url: &str,
    ) -> Result<Self, LyricsError> {
        Ok(Self {
            cache: LyricsCache::new(cache_dir),
            client: LrclibClient::new(lrclib_url)?,
            netease: NeteaseClient::new(netease_url)?,
            pending_embeds: Arc::new(Mutex::new(HashMap::new())),
            share_cancel: Arc::new(AtomicBool::new(false)),
            sharing: Arc::new(AtomicBool::new(false)),
            challenge_ttl: publish::CHALLENGE_TTL,
        })
    }

    /// Tests: how long one LRCLIB challenge may be worked on.
    pub fn with_challenge_ttl(mut self, ttl: Duration) -> Self {
        self.challenge_ttl = ttl;
        self
    }

    /// Lookup without saving and without NetEase (see [`LyricsOptions`]).
    pub async fn for_track(
        &self,
        lib: Arc<Library>,
        path: String,
        duration_hint_secs: Option<f64>,
        force: bool,
    ) -> Result<LyricsResult, LyricsError> {
        self.for_track_with(
            lib,
            path,
            duration_hint_secs,
            force,
            &LyricsOptions::default(),
        )
        .await
    }

    pub async fn for_track_with(
        &self,
        lib: Arc<Library>,
        path: String,
        duration_hint_secs: Option<f64>,
        force: bool,
        opts: &LyricsOptions,
    ) -> Result<LyricsResult, LyricsError> {
        let (path_clone, svc, current) = (path.clone(), self.clone(), opts.current.clone());
        let stage = tokio::task::spawn_blocking(move || {
            // Embed writes queued while their track was playing.
            svc.flush_pending_embeds(current.as_deref());
            if let Some(local) = find_stored(Path::new(&path_clone)) {
                return LocalOrQuery::Local(local);
            }
            LocalOrQuery::Query(resolve_query_sync(&lib, &path_clone, duration_hint_secs))
        })
        .await
        .map_err(blocking_err)?;

        let query = match stage {
            LocalOrQuery::Local(local) => return Ok(local),
            LocalOrQuery::Query(Some(q)) => q,
            LocalOrQuery::Query(None) => return Ok(LyricsResult::NotFound),
        };

        let key = cache_key(
            &query.artist,
            &query.title,
            &query.album,
            query.duration_secs,
        );
        let now = now_unix_secs();

        if !force {
            let cache = self.cache.clone();
            let key_clone = key.clone();
            if let Some(hit) = tokio::task::spawn_blocking(move || cache.get(&key_clone, now))
                .await
                .map_err(blocking_err)?
            {
                return Ok(hit);
            }
        }

        let mut result = match self.client.fetch(&query).await? {
            Some(record) => record.into_result(),
            None => LyricsResult::NotFound,
        };
        if matches!(result, LyricsResult::NotFound) && opts.netease {
            match self.netease.fetch(&query).await {
                Ok(Some(found)) => result = found,
                Ok(None) => {}
                Err(e) => {
                    // An unreachable unofficial service must not turn into a
                    // cached "not found" or an error for the user.
                    log::warn!("netease lookup failed: {e}");
                    return Ok(LyricsResult::NotFound);
                }
            }
        }

        let (cache, res_clone) = (self.cache.clone(), result.clone());
        tokio::task::spawn_blocking(move || cache.put(&key, &res_clone, now))
            .await
            .map_err(blocking_err)??;

        if opts.save != SaveMode::Off && is_online_hit(&result) {
            let (svc, res, path, opts) = (self.clone(), result.clone(), path, opts.clone());
            let _ = tokio::task::spawn_blocking(move || svc.persist(&path, &res, &opts)).await;
        }

        Ok(result)
    }

    /// Saves lyrics found online (never fails: problems are only logged).
    fn persist(&self, path: &str, result: &LyricsResult, opts: &LyricsOptions) {
        let Some(text) = save::to_lrc_text(result) else {
            return;
        };
        let audio = Path::new(path);
        // Only real files: an online stream's URL has no folder to write to.
        if crate::session::is_stream_url(audio) || !audio.is_file() {
            return;
        }
        match opts.save {
            SaveMode::Off => {}
            SaveMode::Sidecar => match save::write_sidecar(audio, &text) {
                Ok(true) => log::info!("saved lyrics next to {path}"),
                Ok(false) => {}
                Err(e) => log::warn!("could not save lyrics next to {path}: {e}"),
            },
            SaveMode::Embed => {
                if is_playing_path(path, opts.current.as_deref()) {
                    self.pending_embeds
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(path.to_string(), text);
                } else {
                    embed_logged(path, &text);
                }
            }
        }
    }

    /// Number of embed writes still waiting for their track to stop playing.
    pub fn pending_embed_count(&self) -> usize {
        self.pending_embeds
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    /// Writes the queued tags of every track that is no longer the current one.
    pub fn flush_pending_embeds(&self, current: Option<&str>) {
        let ready: Vec<(String, String)> = {
            let mut map = self
                .pending_embeds
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let paths: Vec<String> = map
                .keys()
                .filter(|p| !is_playing_path(p, current))
                .cloned()
                .collect();
            paths
                .into_iter()
                .filter_map(|p| map.remove(&p).map(|t| (p, t)))
                .collect()
        };
        for (path, text) in ready {
            embed_logged(&path, &text);
        }
    }

    /// Writes every cached (found) lyrics of the library with `mode`.
    /// Blocking; the playing file is skipped for embedding.
    pub fn save_cached_blocking(
        &self,
        lib: &Library,
        mode: SaveMode,
        current: Option<&str>,
    ) -> BulkSaveOutcome {
        let mut out = BulkSaveOutcome::default();
        if mode == SaveMode::Off {
            return out;
        }
        let Ok(tracks) = lib.all_tracks(SortOrder::Added) else {
            return out;
        };
        let now = now_unix_secs();
        for track in tracks {
            let path = track.path.to_string_lossy().into_owned();
            let Some(query) = resolve_query_sync(lib, &path, None) else {
                continue;
            };
            let key = cache_key(
                &query.artist,
                &query.title,
                &query.album,
                query.duration_secs,
            );
            let Some(cached) = self.cache.get(&key, now) else {
                continue;
            };
            let Some(text) = save::to_lrc_text(&cached) else {
                continue;
            };
            let written = if mode == SaveMode::Embed && is_playing_path(&path, current) {
                false
            } else {
                match save::save(mode, &track.path, &text) {
                    Ok(w) => w,
                    Err(e) => {
                        log::warn!("could not save lyrics for {path}: {e}");
                        false
                    }
                }
            };
            if written {
                out.saved += 1;
            } else {
                out.skipped += 1;
            }
        }
        out
    }

    /// Counts the library's songs with lyrics (sidecar, embedded or cached
    /// found), checked without lyrics (cached not-found or instrumental) and
    /// never looked up. Blocking.
    pub fn coverage_blocking(&self, lib: &Library) -> Coverage {
        let mut out = Coverage::default();
        let Ok(tracks) = lib.all_tracks(SortOrder::Added) else {
            return out;
        };
        let now = now_unix_secs();
        for track in tracks {
            let audio = track.path.as_path();
            if find_local_lrc(audio).is_some() {
                out.with_lyrics += 1;
                continue;
            }
            let path = audio.to_string_lossy().into_owned();
            let cached = resolve_query_sync(lib, &path, None).and_then(|q| {
                self.cache.get(
                    &cache_key(&q.artist, &q.title, &q.album, q.duration_secs),
                    now,
                )
            });
            let has_text = |r: &LyricsResult| save::to_lrc_text(r).is_some();
            if cached.as_ref().is_some_and(has_text) || read_embedded(audio).is_some() {
                out.with_lyrics += 1;
            } else if cached.is_some() {
                out.without_lyrics += 1;
            } else {
                out.unchecked += 1;
            }
        }
        out
    }

    /// What is needed to share the track's own lyrics, or why not.
    fn prepare_share(
        &self,
        lib: &Library,
        path: &str,
        duration_hint_secs: Option<f64>,
    ) -> Result<(PublishBody, TrackQuery, String), ShareBlock> {
        let own = find_stored(Path::new(path)).ok_or(ShareBlock::NoLocalLyrics)?;
        let query =
            resolve_query_sync(lib, path, duration_hint_secs).ok_or(ShareBlock::NeedsMetadata)?;
        let duration = query
            .duration_secs
            .filter(|d| *d > 0)
            .ok_or(ShareBlock::NeedsMetadata)?;
        if query.album.is_empty() {
            return Err(ShareBlock::NeedsMetadata);
        }
        let plain = save::to_plain_text(&own).ok_or(ShareBlock::NoLocalLyrics)?;
        let synced = match own {
            LyricsResult::Synced { .. } => save::to_lrc_text(&own).unwrap_or_default(),
            _ => String::new(),
        };
        let key = cache_key(
            &query.artist,
            &query.title,
            &query.album,
            query.duration_secs,
        );
        let body = PublishBody {
            track_name: query.title.clone(),
            artist_name: query.artist.clone(),
            album_name: query.album.clone(),
            duration,
            plain_lyrics: plain,
            synced_lyrics: synced,
        };
        Ok((body, query, key))
    }

    async fn prepare_share_async(
        &self,
        lib: Arc<Library>,
        path: String,
        hint: Option<f64>,
    ) -> Result<Result<(PublishBody, TrackQuery, String), ShareBlock>, LyricsError> {
        let svc = self.clone();
        tokio::task::spawn_blocking(move || svc.prepare_share(&lib, &path, hint))
            .await
            .map_err(blocking_err)
    }

    /// Whether "Share on LRCLIB" can be offered for the track.
    pub async fn share_status(
        &self,
        lib: Arc<Library>,
        path: String,
        duration_hint_secs: Option<f64>,
    ) -> Result<ShareStatus, LyricsError> {
        let blocked = |reason| ShareStatus {
            available: false,
            reason: Some(reason),
        };
        if self.sharing.load(Ordering::SeqCst) {
            return Ok(blocked(ShareBlock::Busy));
        }
        let (_, query, key) = match self
            .prepare_share_async(lib, path, duration_hint_secs)
            .await?
        {
            Ok(ready) => ready,
            Err(reason) => return Ok(blocked(reason)),
        };
        if self.came_from_other_service(&key).await? {
            return Ok(blocked(ShareBlock::OtherSource));
        }
        Ok(match self.client.fetch(&query).await {
            Ok(Some(_)) => blocked(ShareBlock::OnLrclib),
            Ok(None) => ShareStatus {
                available: true,
                reason: None,
            },
            Err(_) => blocked(ShareBlock::Offline),
        })
    }

    /// Cached NetEase lyrics are not the user's to publish; a sidecar saved
    /// from them would otherwise look like own lyrics.
    async fn came_from_other_service(&self, key: &str) -> Result<bool, LyricsError> {
        let (cache, key) = (self.cache.clone(), key.to_string());
        let hit = tokio::task::spawn_blocking(move || cache.get(&key, now_unix_secs()))
            .await
            .map_err(blocking_err)?;
        Ok(matches!(
            hit,
            Some(LyricsResult::Synced {
                source: LyricsSource::Netease,
                ..
            }) | Some(LyricsResult::Plain {
                source: LyricsSource::Netease,
                ..
            })
        ))
    }

    /// Publishes the track's own lyrics on LRCLIB. Can take minutes (the
    /// proof-of-work); [`Self::cancel_share`] stops it.
    pub async fn share(
        &self,
        lib: Arc<Library>,
        path: String,
        duration_hint_secs: Option<f64>,
    ) -> Result<ShareOutcome, LyricsError> {
        if self.sharing.swap(true, Ordering::SeqCst) {
            return Err(LyricsError::Share(
                "Another share is already running.".into(),
            ));
        }
        self.share_cancel.store(false, Ordering::SeqCst);
        let result = self.share_inner(lib, path, duration_hint_secs).await;
        self.sharing.store(false, Ordering::SeqCst);
        match result {
            Ok(()) => Ok(ShareOutcome::Published),
            Err(LyricsError::Cancelled) => Ok(ShareOutcome::Cancelled),
            Err(e) => Err(e),
        }
    }

    async fn share_inner(
        &self,
        lib: Arc<Library>,
        path: String,
        hint: Option<f64>,
    ) -> Result<(), LyricsError> {
        let (body, query, key) = self
            .prepare_share_async(lib, path, hint)
            .await?
            .map_err(|reason| LyricsError::Share(format!("cannot share: {reason:?}")))?;
        if self.came_from_other_service(&key).await? {
            return Err(LyricsError::Share(
                "These lyrics came from another service.".into(),
            ));
        }
        if self.client.fetch(&query).await?.is_some() {
            return Err(LyricsError::Share(
                "LRCLIB already has lyrics for this track.".into(),
            ));
        }
        publish::publish(&self.client, &body, &self.share_cancel, self.challenge_ttl).await?;
        let cache = self.cache.clone();
        tokio::task::spawn_blocking(move || cache.remove(&key))
            .await
            .map_err(blocking_err)?;
        Ok(())
    }

    pub fn cancel_share(&self) {
        self.share_cancel.store(true, Ordering::SeqCst);
    }
}

/// Lyrics worth keeping next to the song: found online with real text.
fn is_online_hit(result: &LyricsResult) -> bool {
    matches!(
        result,
        LyricsResult::Synced {
            source: LyricsSource::Lrclib | LyricsSource::Netease,
            ..
        } | LyricsResult::Plain {
            source: LyricsSource::Lrclib | LyricsSource::Netease,
            ..
        }
    )
}

fn embed_logged(path: &str, text: &str) {
    match save::write_embedded(Path::new(path), text) {
        Ok(true) => log::info!("embedded lyrics into {path}"),
        Ok(false) => {}
        Err(e) => log::warn!("could not embed lyrics into {path}: {e}"),
    }
}

#[cfg(test)]
mod tests;
