//! HTTP client for LRCLIB (https://lrclib.net).

use super::{LyricsError, LyricsResult, LyricsSource, TrackQuery, lrc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const DEFAULT_BASE_URL: &str = "https://lrclib.net";
const USER_AGENT: &str = concat!(
    "FooModern/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/r0madeus/foomodern)"
);
const SEARCH_DURATION_TOLERANCE_SECS: f64 = 5.0;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LrclibRecord {
    pub synced_lyrics: Option<String>,
    pub plain_lyrics: Option<String>,
    #[serde(default)]
    pub instrumental: bool,
    pub duration: Option<f64>,
}

impl LrclibRecord {
    fn has_content(&self) -> bool {
        self.synced_lyrics
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty())
            || self
                .plain_lyrics
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty())
            || self.instrumental
    }

    pub fn into_result(self) -> LyricsResult {
        if let Some(raw) = self.synced_lyrics.filter(|s| !s.trim().is_empty())
            && let Some(parsed) = lrc::parse(&raw)
        {
            return match parsed {
                lrc::ParsedLyrics::Synced(lines) => LyricsResult::Synced {
                    source: LyricsSource::Lrclib,
                    lines,
                },
                lrc::ParsedLyrics::Plain(lines) => LyricsResult::Plain {
                    source: LyricsSource::Lrclib,
                    lines,
                },
            };
        }

        if let Some(raw) = self.plain_lyrics.filter(|s| !s.trim().is_empty()) {
            let lines = lrc::trim_empty_edges(raw.lines().map(|l| l.trim().to_string()).collect());
            if !lines.is_empty() {
                return LyricsResult::Plain {
                    source: LyricsSource::Lrclib,
                    lines,
                };
            }
        }

        if self.instrumental {
            return LyricsResult::Instrumental {
                source: LyricsSource::Lrclib,
            };
        }

        LyricsResult::NotFound
    }
}

pub fn pick_best_match(
    records: Vec<LrclibRecord>,
    target_duration: Option<u64>,
) -> Option<LrclibRecord> {
    let valid: Vec<LrclibRecord> = records
        .into_iter()
        .filter(LrclibRecord::has_content)
        .collect();
    if valid.is_empty() {
        return None;
    }

    let mut candidates: Vec<LrclibRecord> = match target_duration {
        Some(target) => {
            let target = target as f64;
            let exact: Vec<LrclibRecord> = valid
                .iter()
                .filter(|r| {
                    r.duration
                        .is_some_and(|d| (d - target).abs() <= SEARCH_DURATION_TOLERANCE_SECS)
                })
                .cloned()
                .collect();
            if !exact.is_empty() {
                exact
            } else {
                valid.into_iter().filter(|r| r.duration.is_none()).collect()
            }
        }
        None => valid,
    };

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|a, b| {
        let rank = |r: &LrclibRecord| -> u8 {
            if r.synced_lyrics
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty())
            {
                2
            } else if r
                .plain_lyrics
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty())
            {
                1
            } else {
                0
            }
        };
        rank(b).cmp(&rank(a)).then_with(|| match target_duration {
            Some(target) => {
                let target = target as f64;
                let da = a.duration.map_or(f64::INFINITY, |d| (d - target).abs());
                let db = b.duration.map_or(f64::INFINITY, |d| (d - target).abs());
                da.total_cmp(&db)
            }
            None => std::cmp::Ordering::Equal,
        })
    });

    candidates.into_iter().next()
}

#[derive(Debug, Clone)]
pub struct LrclibClient {
    http: Client,
    base_url: String,
}

impl LrclibClient {
    pub fn new(base_url: impl Into<String>) -> std::result::Result<Self, LyricsError> {
        let http = crate::net::client_builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(8))
            .build()?;
        Ok(Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        })
    }

    pub async fn fetch(
        &self,
        query: &TrackQuery,
    ) -> std::result::Result<Option<LrclibRecord>, LyricsError> {
        let mut params: Vec<(&str, String)> = vec![
            ("track_name", query.title.clone()),
            ("artist_name", query.artist.clone()),
        ];
        if !query.album.is_empty() {
            params.push(("album_name", query.album.clone()));
        }
        if let Some(secs) = query.duration_secs {
            params.push(("duration", secs.to_string()));
        }

        let res = self
            .http
            .get(format!("{}/api/get", self.base_url))
            .query(&params)
            .send()
            .await?;

        match res.status() {
            StatusCode::OK => {
                let record: LrclibRecord = res.json().await?;
                if record.has_content() {
                    return Ok(Some(record));
                }
            }
            StatusCode::NOT_FOUND => {}
            status => return Err(LyricsError::HttpStatus(status.as_u16())),
        }

        // Fall back to /api/search without album_name (handles remasters, compilations,
        // or minor duration differences while still checking ±5s tolerance).
        let search_params = [
            ("track_name", query.title.as_str()),
            ("artist_name", query.artist.as_str()),
        ];
        let search_res = self
            .http
            .get(format!("{}/api/search", self.base_url))
            .query(&search_params)
            .send()
            .await?;

        match search_res.status() {
            StatusCode::OK => {
                let records: Vec<LrclibRecord> = search_res.json().await?;
                Ok(pick_best_match(records, query.duration_secs))
            }
            StatusCode::NOT_FOUND => Ok(None),
            status => Err(LyricsError::HttpStatus(status.as_u16())),
        }
    }
}

/// A proof-of-work challenge from `POST /api/request-challenge`.
#[derive(Debug, Clone, Deserialize)]
pub struct Challenge {
    pub prefix: String,
    pub target: String,
}

/// Body of `POST /api/publish`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishBody {
    pub track_name: String,
    pub artist_name: String,
    pub album_name: String,
    pub duration: u64,
    pub plain_lyrics: String,
    pub synced_lyrics: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishReply {
    Created,
    /// The token was refused (wrong or expired challenge): ask for a new one.
    BadToken,
}

impl LrclibClient {
    pub async fn request_challenge(&self) -> std::result::Result<Challenge, LyricsError> {
        let res = self
            .http
            .post(format!("{}/api/request-challenge", self.base_url))
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(LyricsError::HttpStatus(res.status().as_u16()));
        }
        Ok(res.json().await?)
    }

    pub async fn publish(
        &self,
        token: &str,
        body: &PublishBody,
    ) -> std::result::Result<PublishReply, LyricsError> {
        let res = self
            .http
            .post(format!("{}/api/publish", self.base_url))
            .header("X-Publish-Token", token)
            .json(body)
            .send()
            .await?;
        let status = res.status();
        if status.is_success() {
            return Ok(PublishReply::Created);
        }
        if status == StatusCode::BAD_REQUEST {
            let text = res.text().await.unwrap_or_default();
            if text.contains("PublishToken") || text.to_lowercase().contains("token") {
                return Ok(PublishReply::BadToken);
            }
        }
        Err(LyricsError::HttpStatus(status.as_u16()))
    }
}
