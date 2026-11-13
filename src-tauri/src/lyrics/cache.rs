//! Disk cache for LRCLIB lookups under the application cache directory.

use super::{LyricsError, LyricsResult};
use crate::library::scan::{fnv1a, write_atomic};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const NOT_FOUND_TTL_SECS: u64 = 7 * 24 * 60 * 60;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedEntry {
    fetched_at: u64,
    result: LyricsResult,
}

#[derive(Debug, Clone)]
pub struct LyricsCache {
    dir: PathBuf,
}

pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn cache_key(artist: &str, title: &str, album: &str, duration_secs: Option<u64>) -> String {
    let raw = format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}",
        artist.trim().to_lowercase(),
        title.trim().to_lowercase(),
        album.trim().to_lowercase(),
        duration_secs.unwrap_or(0),
    );
    format!("{:016x}.json", fnv1a(raw.as_bytes()))
}

impl LyricsCache {
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }

    pub fn get(&self, key: &str, now_secs: u64) -> Option<LyricsResult> {
        let bytes = std::fs::read(self.dir.join(key)).ok()?;
        let entry: CachedEntry = serde_json::from_slice(&bytes).ok()?;
        if matches!(entry.result, LyricsResult::NotFound)
            && now_secs.saturating_sub(entry.fetched_at) >= NOT_FOUND_TTL_SECS
        {
            return None;
        }
        Some(entry.result)
    }

    pub fn put(
        &self,
        key: &str,
        result: &LyricsResult,
        now_secs: u64,
    ) -> std::result::Result<(), LyricsError> {
        std::fs::create_dir_all(&self.dir)?;
        let entry = CachedEntry {
            fetched_at: now_secs,
            result: result.clone(),
        };
        let json = serde_json::to_vec(&entry)?;
        write_atomic(&self.dir.join(key), &json)?;
        Ok(())
    }
}
