//! Synced and plain lyrics: local `.lrc` sidecar first, then LRCLIB.

pub mod cache;
pub mod lrc;
pub mod lrclib;

use crate::library::{Library, scan};
use cache::{LyricsCache, cache_key, now_unix_secs};
use lrclib::{DEFAULT_BASE_URL, LrclibClient};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LyricsSource {
    LocalLrc,
    Lrclib,
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

pub fn find_local_lrc(audio_path: &Path) -> Option<LyricsResult> {
    let bytes = std::fs::read(audio_path.with_extension("lrc"))
        .or_else(|_| std::fs::read(audio_path.with_extension("LRC")))
        .ok()?;
    let text = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    };
    let parsed = lrc::parse(&text)?;
    Some(match parsed {
        lrc::ParsedLyrics::Synced(lines) => LyricsResult::Synced {
            source: LyricsSource::LocalLrc,
            lines,
        },
        lrc::ParsedLyrics::Plain(lines) => LyricsResult::Plain {
            source: LyricsSource::LocalLrc,
            lines,
        },
    })
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

#[derive(Debug, Clone)]
pub struct LyricsService {
    cache: LyricsCache,
    client: LrclibClient,
}

impl LyricsService {
    pub fn new(cache_dir: &Path) -> Result<Self, LyricsError> {
        Self::with_base_url(cache_dir, DEFAULT_BASE_URL)
    }

    pub fn with_base_url(cache_dir: &Path, base_url: &str) -> Result<Self, LyricsError> {
        Ok(Self {
            cache: LyricsCache::new(cache_dir),
            client: LrclibClient::new(base_url)?,
        })
    }

    pub async fn for_track(
        &self,
        lib: Arc<Library>,
        path: String,
        duration_hint_secs: Option<f64>,
        force: bool,
    ) -> Result<LyricsResult, LyricsError> {
        let path_clone = path.clone();
        let stage = tokio::task::spawn_blocking(move || {
            if let Some(local) = find_local_lrc(Path::new(&path_clone)) {
                return LocalOrQuery::Local(local);
            }
            LocalOrQuery::Query(resolve_query_sync(&lib, &path_clone, duration_hint_secs))
        })
        .await
        .map_err(|e| LyricsError::Task(e.to_string()))?;

        let query = match stage {
            LocalOrQuery::Local(local) => return Ok(local),
            LocalOrQuery::Query(Some(q)) => q,
            LocalOrQuery::Query(None) => return Ok(LyricsResult::NotFound),
        };

        let key = cache_key(&query.artist, &query.title, &query.album, query.duration_secs);
        let now = now_unix_secs();

        if !force {
            let cache = self.cache.clone();
            let key_clone = key.clone();
            if let Some(hit) = tokio::task::spawn_blocking(move || cache.get(&key_clone, now))
                .await
                .map_err(|e| LyricsError::Task(e.to_string()))?
            {
                return Ok(hit);
            }
        }

        let result = match self.client.fetch(&query).await? {
            Some(record) => record.into_result(),
            None => LyricsResult::NotFound,
        };

        let cache = self.cache.clone();
        let res_clone = result.clone();
        tokio::task::spawn_blocking(move || cache.put(&key, &res_clone, now))
            .await
            .map_err(|e| LyricsError::Task(e.to_string()))??;

        Ok(result)
    }
}

#[cfg(test)]
mod tests;
