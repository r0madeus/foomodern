//! NetEase Cloud Music as a second, unofficial lyrics source.
//!
//! The API is undocumented and was only verified from third-party sources,
//! so everything here is parsed tolerantly (numbers or strings for ids,
//! several field spellings) and the base URL is configurable for tests.

use super::{LyricLine, LyricsError, LyricsResult, LyricsSource, TrackQuery, lrc};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

pub const DEFAULT_BASE_URL: &str = "https://music.163.com";
const REFERER: &str = "https://music.163.com";
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const DURATION_TOLERANCE_MS: u64 = 3000;
const MIN_TITLE_SIMILARITY: f64 = 0.8;
/// Credit lines (composer, lyricist...) are looked for in the first lines only.
const CREDIT_WINDOW: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub id: i64,
    pub name: String,
    pub duration_ms: Option<u64>,
    pub artists: Vec<String>,
}

fn candidate_from(v: &Value) -> Option<Candidate> {
    let id = match v.get("id")? {
        Value::Number(n) => n.as_i64()?,
        Value::String(s) => s.trim().parse().ok()?,
        _ => return None,
    };
    let name = v.get("name")?.as_str()?.to_string();
    let duration_ms = v
        .get("duration")
        .or_else(|| v.get("dt"))
        .and_then(|d| d.as_u64().or_else(|| d.as_f64().map(|f| f.max(0.0) as u64)))
        .filter(|ms| *ms > 0);
    let artists = v
        .get("artists")
        .or_else(|| v.get("ar"))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|a| a.get("name").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Some(Candidate {
        id,
        name,
        duration_ms,
        artists,
    })
}

/// Songs of a search reply. An error when the reply is not a search result
/// at all (e.g. a rejection), so the caller can try the other HTTP method.
pub fn parse_search(bytes: &[u8]) -> Result<Vec<Candidate>, LyricsError> {
    let v: Value = serde_json::from_slice(bytes)?;
    let Some(result) = v.get("result") else {
        let code = v.get("code").and_then(Value::as_i64).unwrap_or(0);
        if code == 200 {
            return Ok(Vec::new());
        }
        return Err(LyricsError::Task(format!(
            "netease search rejected (code {code})"
        )));
    };
    Ok(result
        .get("songs")
        .and_then(Value::as_array)
        .map(|songs| songs.iter().filter_map(candidate_from).collect())
        .unwrap_or_default())
}

fn normalize(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0u32;
    for c in s.chars() {
        match c {
            '(' | '[' | '（' | '【' | '{' => depth += 1,
            ')' | ']' | '）' | '】' | '}' => depth = depth.saturating_sub(1),
            c if depth == 0 && c.is_alphanumeric() => out.extend(c.to_lowercase()),
            _ => {}
        }
    }
    out
}

fn dice(a: &str, b: &str) -> f64 {
    let bigrams = |s: &str| -> Vec<(char, char)> {
        let c: Vec<char> = s.chars().collect();
        c.windows(2).map(|w| (w[0], w[1])).collect()
    };
    let (mut ba, bb) = (bigrams(a), bigrams(b));
    if ba.is_empty() || bb.is_empty() {
        return 0.0;
    }
    let total = (ba.len() + bb.len()) as f64;
    let mut hits = 0;
    for g in bb {
        if let Some(pos) = ba.iter().position(|x| *x == g) {
            ba.swap_remove(pos);
            hits += 1;
        }
    }
    2.0 * hits as f64 / total
}

fn title_similarity(candidate: &str, wanted: &str) -> f64 {
    let (a, b) = (normalize(candidate), normalize(wanted));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    let (la, lb) = (a.chars().count(), b.chars().count());
    let (short, long) = if la <= lb { (la, lb) } else { (lb, la) };
    if (a.contains(&b) || b.contains(&a)) && short as f64 >= 0.7 * long as f64 {
        return 0.9;
    }
    dice(&a, &b)
}

fn artist_parts(artist: &str) -> Vec<String> {
    let mut parts = vec![normalize(artist)];
    let mut text = artist.to_lowercase();
    for sep in [
        " feat. ",
        " feat ",
        " ft. ",
        " ft ",
        " featuring ",
        " and ",
        " x ",
        " with ",
    ] {
        text = text.replace(sep, ",");
    }
    parts.extend(
        text.split([',', '&', ';', '/', '、', '，', '+'])
            .map(normalize),
    );
    parts.retain(|p| !p.is_empty());
    parts
}

fn artist_matches(candidate: &[String], wanted: &str) -> bool {
    let wanted = artist_parts(wanted);
    let mut have: Vec<String> = candidate.iter().map(|a| normalize(a)).collect();
    have.push(candidate.iter().map(|a| normalize(a)).collect::<String>());
    have.retain(|p| !p.is_empty());
    wanted.iter().any(|w| {
        have.iter().any(|h| {
            h == w
                || (w.chars().count().min(h.chars().count()) >= 2
                    && (h.contains(w.as_str()) || w.contains(h.as_str())))
        })
    })
}

/// The plausible song: a very similar title, a matching artist and, when
/// both durations are known, within 3 seconds. The best of those wins.
pub fn pick_song<'a>(candidates: &'a [Candidate], q: &TrackQuery) -> Option<&'a Candidate> {
    let wanted_ms = q.duration_secs.map(|s| s * 1000);
    candidates
        .iter()
        .filter_map(|c| {
            let sim = title_similarity(&c.name, &q.title);
            if sim < MIN_TITLE_SIMILARITY || !artist_matches(&c.artists, &q.artist) {
                return None;
            }
            let diff = match (wanted_ms, c.duration_ms) {
                (Some(w), Some(d)) => {
                    let diff = w.abs_diff(d);
                    if diff > DURATION_TOLERANCE_MS {
                        return None;
                    }
                    diff
                }
                _ => DURATION_TOLERANCE_MS,
            };
            Some((c, sim, diff))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.2.cmp(&a.2)))
        .map(|(c, _, _)| c)
}

fn is_credit_keyword(text: &str) -> bool {
    ["作词", "作曲", "编曲", "制作人", "作詞", "編曲", "製作人"]
        .iter()
        .any(|k| text.contains(k))
}

/// `^\s*[^:：]{1,12}\s*[:：]`
fn looks_like_credit(text: &str) -> bool {
    let Some(pos) = text.find([':', '：']) else {
        return false;
    };
    let head = text[..pos].trim();
    (1..=12).contains(&head.chars().count())
}

/// Which of the leading lines are credits (`作词 : ...`, `Producer: ...`).
/// Keyword lines in the first lines always count; the looser `label:` shape
/// only while every line before it was a credit too, so a lyric like
/// "Hey: you" further down is safe.
fn credit_flags(texts: &[&str]) -> Vec<bool> {
    let mut flags = vec![false; texts.len()];
    let mut run = true;
    for (i, raw) in texts.iter().take(CREDIT_WINDOW).enumerate() {
        let text = raw.trim();
        if text.is_empty() {
            flags[i] = run;
        } else if is_credit_keyword(text) || (run && looks_like_credit(text)) {
            flags[i] = true;
        } else {
            run = false;
        }
    }
    flags
}

fn strip_credits(parsed: lrc::ParsedLyrics) -> Option<LyricsResult> {
    let source = LyricsSource::Netease;
    match parsed {
        lrc::ParsedLyrics::Synced(lines) => {
            let flags = credit_flags(&lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>());
            let lines: Vec<LyricLine> = lines
                .into_iter()
                .zip(flags)
                .filter_map(|(l, drop)| (!drop).then_some(l))
                .collect();
            (lines.iter().any(|l| !l.text.trim().is_empty()))
                .then_some(LyricsResult::Synced { source, lines })
        }
        lrc::ParsedLyrics::Plain(lines) => {
            let flags = credit_flags(&lines.iter().map(String::as_str).collect::<Vec<_>>());
            let lines = lrc::trim_empty_edges(
                lines
                    .into_iter()
                    .zip(flags)
                    .filter_map(|(l, drop)| (!drop).then_some(l))
                    .collect(),
            );
            (!lines.is_empty()).then_some(LyricsResult::Plain { source, lines })
        }
    }
}

/// Turns a lyric reply into a result: `None` = nothing usable,
/// `Instrumental` for pure music.
pub fn parse_lyric(bytes: &[u8]) -> Result<Option<LyricsResult>, LyricsError> {
    let v: Value = serde_json::from_slice(bytes)?;
    let flag = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
    if flag("pureMusic") {
        return Ok(Some(LyricsResult::Instrumental {
            source: LyricsSource::Netease,
        }));
    }
    if flag("nolyric") {
        return Ok(None);
    }
    let raw = v
        .get("lrc")
        .and_then(|l| l.get("lyric"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if raw.trim().is_empty() {
        return Ok(None);
    }
    let Some(result) = lrc::parse(raw).and_then(strip_credits) else {
        return Ok(None);
    };
    // A lone "pure music, please enjoy" line is how NetEase marks instrumentals.
    let lone_line = match &result {
        LyricsResult::Synced { lines, .. } if lines.len() == 1 => Some(lines[0].text.as_str()),
        LyricsResult::Plain { lines, .. } if lines.len() == 1 => Some(lines[0].as_str()),
        _ => None,
    };
    if lone_line.is_some_and(|t| t.contains("纯音乐")) {
        return Ok(Some(LyricsResult::Instrumental {
            source: LyricsSource::Netease,
        }));
    }
    Ok(Some(result))
}

fn form_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct NeteaseClient {
    http: Client,
    base_url: String,
}

impl NeteaseClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self, LyricsError> {
        let http = crate::net::client_builder()
            .user_agent(BROWSER_UA)
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(8))
            .build()?;
        Ok(Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        })
    }

    async fn search_post(&self, term: &str) -> Result<Vec<Candidate>, LyricsError> {
        let body = format!("s={}&type=1&limit=10&offset=0", form_encode(term));
        let res = self
            .http
            .post(format!("{}/api/search/get", self.base_url))
            .header("Referer", REFERER)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(LyricsError::HttpStatus(res.status().as_u16()));
        }
        parse_search(&res.bytes().await?)
    }

    async fn search_get(&self, term: &str) -> Result<Vec<Candidate>, LyricsError> {
        let res = self
            .http
            .get(format!("{}/api/search/get", self.base_url))
            .header("Referer", REFERER)
            .query(&[("s", term), ("type", "1"), ("limit", "10"), ("offset", "0")])
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(LyricsError::HttpStatus(res.status().as_u16()));
        }
        parse_search(&res.bytes().await?)
    }

    /// POST first; when that fails, GET once (sources disagree on the method).
    pub async fn search(&self, term: &str) -> Result<Vec<Candidate>, LyricsError> {
        match self.search_post(term).await {
            Ok(list) => Ok(list),
            Err(e) => {
                log::debug!("netease POST search failed ({e}); trying GET");
                self.search_get(term).await
            }
        }
    }

    pub async fn lyric(&self, id: i64) -> Result<Option<LyricsResult>, LyricsError> {
        let res = self
            .http
            .get(format!("{}/api/song/lyric", self.base_url))
            .header("Referer", REFERER)
            .query(&[
                ("id", id.to_string().as_str()),
                ("lv", "1"),
                ("kv", "1"),
                ("tv", "-1"),
            ])
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(LyricsError::HttpStatus(res.status().as_u16()));
        }
        parse_lyric(&res.bytes().await?)
    }

    /// `Ok(None)` = no plausible song or no lyrics.
    pub async fn fetch(&self, q: &TrackQuery) -> Result<Option<LyricsResult>, LyricsError> {
        let term = format!("{} {}", q.artist, q.title);
        let candidates = self.search(&term).await?;
        let Some(song) = pick_song(&candidates, q) else {
            return Ok(None);
        };
        self.lyric(song.id).await
    }
}
