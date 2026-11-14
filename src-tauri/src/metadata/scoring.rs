//! Pure matching logic: text similarity, album scoring, release choice.
//! Nothing here touches the network, the database or the disk.

use super::musicbrainz::{MbRecording, MbRelease, MbTrack, credit_string};

/// Durations closer than this count as "the same recording".
pub const DURATION_TOLERANCE_MS: u64 = 3000;
/// Candidates below this score are not shown.
pub const MIN_SCORE: u8 = 50;
/// From this score on a candidate is labelled "recommended".
pub const RECOMMENDED_SCORE: u8 = 85;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalTrack {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalAlbum {
    pub title: String,
    pub artist: String,
    pub year: Option<i32>,
    pub tracks: Vec<LocalTrack>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandTrack {
    pub disc: u32,
    pub position: u32,
    pub title: String,
    pub artist: String,
    pub length_ms: Option<u64>,
    /// MusicBrainz release-track id.
    pub track_id: String,
    pub recording_id: String,
}

/// A looked-up release with all its tracks, flattened over its media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandRelease {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub date: Option<String>,
    pub country: Option<String>,
    pub status: Option<String>,
    pub format: Option<String>,
    pub release_group_id: String,
    pub tracks: Vec<CandTrack>,
}

impl CandRelease {
    pub fn from_mb(r: &MbRelease) -> Self {
        let artist = credit_string(&r.artist_credit);
        let mut tracks = Vec::new();
        for (mi, medium) in r.media.iter().enumerate() {
            let disc = medium
                .position
                .unwrap_or(u32::try_from(mi + 1).unwrap_or(1));
            for (ti, t) in medium.tracks.iter().enumerate() {
                let position = t.position.unwrap_or(u32::try_from(ti + 1).unwrap_or(1));
                let credit = credit_string(&t.artist_credit);
                tracks.push(CandTrack {
                    disc,
                    position,
                    title: t.title.clone(),
                    artist: if credit.is_empty() {
                        artist.clone()
                    } else {
                        credit
                    },
                    length_ms: t
                        .length
                        .or_else(|| t.recording.as_ref().and_then(|rec| rec.length)),
                    track_id: t.id.clone(),
                    recording_id: t
                        .recording
                        .as_ref()
                        .map(|rec| rec.id.clone())
                        .unwrap_or_default(),
                });
            }
        }
        Self {
            id: r.id.clone(),
            title: r.title.clone(),
            artist,
            date: r.date.clone().filter(|d| !d.is_empty()),
            country: r.country.clone().filter(|d| !d.is_empty()),
            status: r.status.clone(),
            format: r.media.first().and_then(|m| m.format.clone()),
            release_group_id: r
                .release_group
                .as_ref()
                .map(|g| g.id.clone())
                .unwrap_or_default(),
            tracks,
        }
    }

    pub fn year(&self) -> Option<i32> {
        year_of(self.date.as_deref())
    }
}

/// Year from a MusicBrainz date (`YYYY`, `YYYY-MM`, `YYYY-MM-DD`).
pub fn year_of(date: Option<&str>) -> Option<i32> {
    date.and_then(|d| d.get(..4))
        .and_then(|y| y.parse::<i32>().ok())
        .filter(|y| *y > 0)
}

// ---------------------------------------------------------------------
// Text similarity
// ---------------------------------------------------------------------

/// Lowercase, punctuation replaced by spaces, whitespace collapsed.
pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        // `İ` lowercases to `i` + U+0307 COMBINING DOT ABOVE; drop the mark
        // so "İstanbul" and "ISTANBUL" compare as the same word.
        if c == '\u{0307}' {
            continue;
        }
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with(' ') {
            out.push(' ');
        }
    }
    out.trim().to_owned()
}

pub fn levenshtein(a: &[char], b: &[char]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// 0.0 (nothing alike) to 1.0 (equal after normalisation).
pub fn similarity(a: &str, b: &str) -> f64 {
    let (a, b): (Vec<char>, Vec<char>) = (
        normalize(a).chars().collect(),
        normalize(b).chars().collect(),
    );
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let max = a.len().max(b.len());
    1.0 - levenshtein(&a, &b) as f64 / max as f64
}

/// 1.0 within the tolerance, falling linearly to 0.0 at five times the
/// tolerance; `None` when either length is unknown.
fn duration_closeness(local: Option<u64>, remote: Option<u64>) -> Option<f64> {
    let (l, r) = (local?, remote?);
    let diff = l.abs_diff(r);
    if diff <= DURATION_TOLERANCE_MS {
        return Some(1.0);
    }
    let span = (DURATION_TOLERANCE_MS * 4) as f64;
    Some((1.0 - (diff - DURATION_TOLERANCE_MS) as f64 / span).max(0.0))
}

pub fn duration_within(local: Option<u64>, remote: Option<u64>) -> bool {
    local
        .zip(remote)
        .is_none_or(|(l, r)| l.abs_diff(r) <= DURATION_TOLERANCE_MS)
}

// ---------------------------------------------------------------------
// Album scoring
// ---------------------------------------------------------------------

/// Which candidate track (index into `CandRelease::tracks`) belongs to each
/// local track.
pub type Assignment = Vec<Option<usize>>;

fn assign_by_number(local: &[LocalTrack], cand: &[CandTrack]) -> Option<Assignment> {
    if local.len() != cand.len() || local.iter().any(|t| t.track_no.is_none()) {
        return None;
    }
    let mut used = vec![false; cand.len()];
    let mut out = Vec::with_capacity(local.len());
    for t in local {
        let key = (t.disc_no.unwrap_or(1), t.track_no?);
        let idx = cand
            .iter()
            .position(|c| (c.disc, c.position) == key)
            .filter(|i| !used[*i])?;
        used[idx] = true;
        out.push(Some(idx));
    }
    Some(out)
}

fn pair_score(l: &LocalTrack, c: &CandTrack) -> f64 {
    let title = similarity(&l.title, &c.title);
    match duration_closeness(l.duration_ms, c.length_ms) {
        Some(d) => 0.7 * title + 0.3 * d,
        None => title,
    }
}

fn assign_greedy(local: &[LocalTrack], cand: &[CandTrack]) -> Assignment {
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for (i, l) in local.iter().enumerate() {
        for (j, c) in cand.iter().enumerate() {
            let s = pair_score(l, c);
            if s >= 0.35 {
                pairs.push((s, i, j));
            }
        }
    }
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut out = vec![None; local.len()];
    let mut used = vec![false; cand.len()];
    for (_, i, j) in pairs {
        if out[i].is_none() && !used[j] {
            out[i] = Some(j);
            used[j] = true;
        }
    }
    out
}

/// Assigns local tracks to candidate tracks: by (disc, number) when every
/// local track is numbered and the counts match, else greedily by title and
/// duration.
pub fn assign_tracks(local: &[LocalTrack], cand: &[CandTrack]) -> Assignment {
    assign_by_number(local, cand).unwrap_or_else(|| assign_greedy(local, cand))
}

/// Scores how well `cand` describes `local` (0-100) and returns the track
/// assignment used.
///
/// Components: equal track count 20, assigned tracks with a duration within
/// 3 s 40, mean title similarity 25, album title and artist similarity 15.
/// Unassigned local tracks count as zero in the two per-track components.
/// Durations are the strongest evidence, so when at least half of the local
/// tracks have a comparable duration the sum is scaled by 0.6 to 1.0
/// depending on the share of matching ones (all wrong: 0.6).
pub fn score_album(local: &LocalAlbum, cand: &CandRelease) -> (u8, Assignment) {
    let assignment = assign_tracks(&local.tracks, &cand.tracks);
    let n = local.tracks.len();
    if n == 0 {
        return (0, assignment);
    }
    let count = if local.tracks.len() == cand.tracks.len() {
        20.0
    } else {
        0.0
    };

    let (mut title_sum, mut known, mut matched) = (0.0, 0usize, 0usize);
    let mut assigned = 0usize;
    for (l, a) in local.tracks.iter().zip(&assignment) {
        let Some(j) = a else { continue };
        let c = &cand.tracks[*j];
        assigned += 1;
        title_sum += similarity(&l.title, &c.title);
        if let (Some(ld), Some(cd)) = (l.duration_ms, c.length_ms) {
            known += 1;
            if ld.abs_diff(cd) <= DURATION_TOLERANCE_MS {
                matched += 1;
            }
        }
    }
    let coverage = assigned as f64 / n as f64;
    let duration_share = if known == 0 {
        0.5 // nothing to compare: neither confirms nor contradicts
    } else {
        matched as f64 / known as f64
    };
    let duration = 40.0 * duration_share * coverage;
    let titles = 25.0 * title_sum / n as f64;
    let meta = 15.0
        * (similarity(&local.title, &cand.title) + similarity(&local.artist, &cand.artist))
        / 2.0;

    let mut total = count + duration + titles + meta;
    if known * 2 >= n {
        total *= 0.6 + 0.4 * (matched as f64 / known as f64);
    }
    (total.round().clamp(0.0, 100.0) as u8, assignment)
}

pub fn label_for(score: u8) -> &'static str {
    if score >= RECOMMENDED_SCORE {
        "recommended"
    } else {
        "check"
    }
}

// ---------------------------------------------------------------------
// Single tracks
// ---------------------------------------------------------------------

/// How wanted a release is for tagging a single track: lower is better.
fn release_rank(r: &MbRelease) -> (u8, u8, String) {
    let status = u8::from(
        !r.status
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case("official")),
    );
    let (primary, secondary) = r
        .release_group
        .as_ref()
        .map(|g| (g.primary_type.as_deref(), g.secondary_types.is_empty()))
        .unwrap_or((None, true));
    let kind = match primary {
        Some(p) if p.eq_ignore_ascii_case("album") && secondary => 0,
        Some(p) if p.eq_ignore_ascii_case("single") => 1,
        _ => 2,
    };
    // A missing date sorts after every real one.
    let date = r
        .date
        .clone()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "9999".into());
    (status, kind, date)
}

/// Chooses the release to write for a recording: Official first, then a
/// plain studio Album (no secondary types such as Compilation or Live), then
/// a Single, then anything; the earliest date among equals.
pub fn pick_release(releases: &[MbRelease]) -> Option<&MbRelease> {
    releases.iter().min_by_key(|r| release_rank(r))
}

/// Where a recording sits on a release: (disc, track number, release track id).
pub fn recording_position(release: &MbRelease) -> (Option<u32>, Option<u32>, String) {
    let Some(medium) = release
        .media
        .iter()
        .find(|m| !m.track.is_empty())
        .or(release.media.first())
    else {
        return (None, None, String::new());
    };
    let track: Option<&MbTrack> = medium.track.first();
    let number = track
        .and_then(|t| t.number.parse::<u32>().ok())
        .or_else(|| medium.track_offset.map(|o| o + 1));
    (
        medium.position,
        number,
        track.map(|t| t.id.clone()).unwrap_or_default(),
    )
}

/// Score (0-100) of a recording search hit for a local track: MusicBrainz'
/// own score, title similarity and duration closeness.
pub fn score_recording(local: &LocalTrack, rec: &MbRecording) -> u8 {
    let mb = f64::from(rec.score.unwrap_or(0).min(100));
    let title = similarity(&local.title, &rec.title);
    let duration = duration_closeness(local.duration_ms, rec.length).unwrap_or(0.5);
    (0.6 * mb + 20.0 * title + 20.0 * duration)
        .round()
        .clamp(0.0, 100.0) as u8
}
