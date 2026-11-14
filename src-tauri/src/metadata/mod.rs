//! Automatic metadata matching: "Find info" for an album or a single track.
//!
//! The flow is search (MusicBrainz) -> rank -> preview -> apply -> undo:
//!
//! * [`musicbrainz`] – HTTP client (rate limited, tolerant JSON parsing).
//! * [`scoring`] – pure ranking functions, unit-tested without a network.
//! * [`MetadataService`] – builds candidates with a per-field diff, writes
//!   tags through [`crate::library::tags`] after the user confirmed, keeps
//!   the old values in `tag_history` for undo, and adds a cover when the
//!   album has none. Cover images are cache files shown through
//!   `convertFileSrc`; image bytes never travel over IPC.
//!
//! Nothing is written to a file before `apply` is called with a proposal.

pub mod musicbrainz;
pub mod scoring;

use crate::library::scan::cover_file_name;
use crate::library::tags::{self, EditableTags, MbIds, TagChanges};
use crate::library::{AlbumRow, Library};
use crate::net::{Feature, NetPolicy};
use musicbrainz::{MbClient, MbConfig, MbRecording, MbRelease, credit_string};
use scoring::{CandRelease, LocalAlbum, LocalTrack, MIN_SCORE};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[cfg(test)]
mod tests;

/// Releases looked up in full per album (one request each).
const MAX_LOOKUPS: usize = 4;
/// Requests per album, search included.
const MAX_ALBUM_REQUESTS: usize = 5;
const MAX_TRACK_CANDIDATES: usize = 6;
/// Candidates that get a preview cover.
const MAX_PREVIEWS: usize = 4;

#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error("MusicBrainz could not be reached: {0}")]
    Http(#[from] reqwest::Error),
    #[error("the server answered with status {0}")]
    Status(u16),
    #[error("unexpected response: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Library(String),
    #[error("{0}")]
    Invalid(String),
    #[error("task error: {0}")]
    Task(String),
    #[error("disabled in Settings")]
    Disabled,
}

impl From<crate::library::LibraryError> for MetadataError {
    fn from(e: crate::library::LibraryError) -> Self {
        Self::Library(e.to_string())
    }
}

impl From<tags::TagError> for MetadataError {
    fn from(e: tags::TagError) -> Self {
        Self::Invalid(e.to_string())
    }
}

type Result<T> = std::result::Result<T, MetadataError>;

async fn blocking<T, F>(f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| MetadataError::Task(e.to_string()))
}

// ---------------------------------------------------------------------
// Data sent to the UI and back
// ---------------------------------------------------------------------

/// One changed tag field, for the preview table.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FieldDiff {
    /// camelCase field name: title, artist, album, albumArtist, trackNo,
    /// discNo, year.
    pub field: &'static str,
    pub old: Option<String>,
    pub new: Option<String>,
}

/// What would change for one local track.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrackDiff {
    pub track_id: i64,
    pub path: String,
    /// Current title, to recognise the row.
    pub title: String,
    /// False when no track of the candidate corresponds to this file; it is
    /// left untouched.
    pub matched: bool,
    /// Only the fields that differ from the current tags.
    pub fields: Vec<FieldDiff>,
}

/// New values for one track. A missing field means "keep as is".
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackProposal {
    pub track_id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<i32>,
    pub mb_release_id: Option<String>,
    pub mb_release_group_id: Option<String>,
    pub mb_recording_id: Option<String>,
    pub mb_track_id: Option<String>,
}

impl TrackProposal {
    fn is_empty(&self) -> bool {
        *self
            == TrackProposal {
                track_id: self.track_id,
                ..TrackProposal::default()
            }
    }

    fn to_changes(&self) -> TagChanges {
        TagChanges {
            title: self.title.clone().map(Some),
            artist: self.artist.clone().map(Some),
            album: self.album.clone().map(Some),
            album_artist: self.album_artist.clone().map(Some),
            track_no: self.track_no.map(Some),
            disc_no: self.disc_no.map(Some),
            year: self.year.map(Some),
            mb_release_id: self.mb_release_id.clone().map(Some),
            mb_release_group_id: self.mb_release_group_id.clone().map(Some),
            mb_recording_id: self.mb_recording_id.clone().map(Some),
            mb_track_id: self.mb_track_id.clone().map(Some),
            ..TagChanges::default()
        }
    }
}

/// Everything `metadata_apply` needs; the UI sends back the proposal of the
/// candidate the user chose.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct Proposal {
    pub release_id: String,
    pub release_group_id: String,
    pub tracks: Vec<TrackProposal>,
    /// Add the release group's front cover when the album has none.
    pub cover: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AlbumCandidate {
    pub release_id: String,
    pub release_group_id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<i32>,
    pub country: Option<String>,
    pub format: Option<String>,
    pub status: Option<String>,
    pub track_count: usize,
    pub score: u8,
    /// "recommended" (85 and up) or "check".
    pub label: &'static str,
    /// Cached preview image (absolute path), if the archive has one.
    pub cover_preview: Option<String>,
    /// Album-level changes (album title, album artist, year).
    pub album_fields: Vec<FieldDiff>,
    pub tracks: Vec<TrackDiff>,
    pub proposal: Proposal,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrackCandidate {
    pub recording_id: String,
    pub release_id: String,
    pub release_group_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub year: Option<i32>,
    pub country: Option<String>,
    pub format: Option<String>,
    pub status: Option<String>,
    pub score: u8,
    pub label: &'static str,
    pub cover_preview: Option<String>,
    pub track: TrackDiff,
    pub proposal: Proposal,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOutcome {
    /// Id to pass to `metadata_undo`; 0 when nothing was written.
    pub batch_id: i64,
    pub updated: usize,
    /// (path, error) per file that could not be written.
    pub failed: Vec<(String, String)>,
    /// Cover file added to the album, if any.
    pub cover: Option<String>,
    /// Files written; used to refresh the player bar.
    #[serde(skip)]
    pub updated_paths: Vec<String>,
}

/// Tag values saved before a write (stored as JSON in `tag_history`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct OldTags {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    album_artist: Option<String>,
    track_no: Option<u32>,
    disc_no: Option<u32>,
    year: Option<i32>,
    #[serde(flatten)]
    mb: MbIds,
}

impl OldTags {
    fn read(path: &Path) -> std::result::Result<Self, tags::TagError> {
        let t = tags::read_tags(path)?;
        Ok(Self {
            title: t.title,
            artist: t.artist,
            album: t.album,
            album_artist: t.album_artist,
            track_no: t.track_no,
            disc_no: t.disc_no,
            year: t.year,
            mb: tags::read_mb_ids(path)?,
        })
    }

    /// Changes that put every field back, clearing the ones that were empty.
    fn restore_changes(&self) -> TagChanges {
        TagChanges {
            title: Some(self.title.clone()),
            artist: Some(self.artist.clone()),
            album: Some(self.album.clone()),
            album_artist: Some(self.album_artist.clone()),
            track_no: Some(self.track_no),
            disc_no: Some(self.disc_no),
            year: Some(self.year),
            mb_release_id: Some(self.mb.release_id.clone()),
            mb_release_group_id: Some(self.mb.release_group_id.clone()),
            mb_recording_id: Some(self.mb.recording_id.clone()),
            mb_track_id: Some(self.mb.track_id.clone()),
            ..TagChanges::default()
        }
    }
}

// ---------------------------------------------------------------------
// Diffs
// ---------------------------------------------------------------------

fn text_diff(
    out: &mut Vec<FieldDiff>,
    field: &'static str,
    old: Option<&str>,
    new: Option<&str>,
) -> Option<String> {
    let new = new.map(str::trim).filter(|s| !s.is_empty())?;
    if old.map(str::trim) == Some(new) {
        return None;
    }
    out.push(FieldDiff {
        field,
        old: old.map(str::to_owned),
        new: Some(new.to_owned()),
    });
    Some(new.to_owned())
}

fn num_diff(
    out: &mut Vec<FieldDiff>,
    field: &'static str,
    old: Option<i64>,
    new: Option<i64>,
) -> Option<i64> {
    let new = new?;
    if old == Some(new) {
        return None;
    }
    out.push(FieldDiff {
        field,
        old: old.map(|n| n.to_string()),
        new: Some(new.to_string()),
    });
    Some(new)
}

/// New values for one local track, as a proposal plus the visible diff.
#[derive(Debug, Default)]
struct NewValues<'a> {
    title: Option<&'a str>,
    artist: Option<&'a str>,
    album: Option<&'a str>,
    album_artist: Option<&'a str>,
    track_no: Option<u32>,
    disc_no: Option<u32>,
    year: Option<i32>,
    release_id: &'a str,
    release_group_id: &'a str,
    recording_id: &'a str,
    track_id: &'a str,
}

fn diff_track(
    local: &LocalTrack,
    old: &EditableTags,
    old_ids: &MbIds,
    new: &NewValues<'_>,
) -> (TrackProposal, TrackDiff) {
    let mut fields = Vec::new();
    let mut p = TrackProposal {
        track_id: local.id,
        ..TrackProposal::default()
    };
    p.title = text_diff(&mut fields, "title", old.title.as_deref(), new.title);
    p.artist = text_diff(&mut fields, "artist", old.artist.as_deref(), new.artist);
    p.album = text_diff(&mut fields, "album", old.album.as_deref(), new.album);
    p.album_artist = text_diff(
        &mut fields,
        "albumArtist",
        old.album_artist.as_deref(),
        new.album_artist,
    );
    p.track_no = num_diff(
        &mut fields,
        "trackNo",
        old.track_no.map(i64::from),
        new.track_no.map(i64::from),
    )
    .and_then(|n| u32::try_from(n).ok());
    p.disc_no = num_diff(
        &mut fields,
        "discNo",
        old.disc_no.map(i64::from),
        new.disc_no.map(i64::from),
    )
    .and_then(|n| u32::try_from(n).ok());
    p.year = num_diff(
        &mut fields,
        "year",
        old.year.map(i64::from),
        new.year.map(i64::from),
    )
    .and_then(|n| i32::try_from(n).ok());
    // Ids are written silently: they are not something the user reads.
    let id = |old: &Option<String>, new: &str| {
        (!new.is_empty() && old.as_deref() != Some(new)).then(|| new.to_owned())
    };
    p.mb_release_id = id(&old_ids.release_id, new.release_id);
    p.mb_release_group_id = id(&old_ids.release_group_id, new.release_group_id);
    p.mb_recording_id = id(&old_ids.recording_id, new.recording_id);
    p.mb_track_id = id(&old_ids.track_id, new.track_id);
    let diff = TrackDiff {
        track_id: local.id,
        path: local.path.clone(),
        title: local.title.clone(),
        matched: true,
        fields,
    };
    (p, diff)
}

/// A local track with the exact tags read from its file.
struct LocalTagged {
    track: LocalTrack,
    tags: EditableTags,
    ids: MbIds,
}

fn load_tagged(rows: &[crate::library::TrackRow]) -> Result<Vec<LocalTagged>> {
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        // Unreadable files are left out; they cannot be written either.
        let Ok(tags) = tags::read_tags(Path::new(&r.path)) else {
            continue;
        };
        let ids = tags::read_mb_ids(Path::new(&r.path)).unwrap_or_default();
        out.push(LocalTagged {
            track: LocalTrack {
                id: r.id,
                path: r.path.clone(),
                title: r.title.clone(),
                artist: r.artist.clone(),
                track_no: r.track_no,
                disc_no: r.disc_no,
                duration_ms: r.duration_ms,
            },
            tags,
            ids,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------

pub struct MetadataService {
    mb: MbClient,
    /// Serialises apply and undo: a batch id is `MAX(batch_id) + 1`, so two
    /// writes at once would otherwise share one and undo together.
    writes: Arc<Mutex<()>>,
    net: Arc<NetPolicy>,
}

impl MetadataService {
    pub fn new() -> Result<Self> {
        Self::with_config(MbConfig::default())
    }

    pub fn with_config(cfg: MbConfig) -> Result<Self> {
        Ok(Self {
            mb: MbClient::new(cfg)?,
            writes: Arc::new(Mutex::new(())),
            net: Arc::new(NetPolicy::default()),
        })
    }

    /// Ties the service to the app's internet switches: with "Find info"
    /// off (or offline mode on) nothing is requested.
    pub fn with_net_policy(mut self, policy: Arc<NetPolicy>) -> Self {
        self.mb = self.mb.with_policy(Arc::clone(&policy));
        self.net = policy;
        self
    }

    fn require_net(&self) -> Result<()> {
        self.net
            .require(Feature::Metadata)
            .map_err(|_| MetadataError::Disabled)
    }

    /// Downloads (once) the 250 px front cover of a release group into the
    /// cache and returns its path. Missing covers and download errors give
    /// `None`: a preview is never worth failing a search for.
    async fn preview(&self, lib: &Library, release_group_id: &str) -> Option<String> {
        if release_group_id.is_empty() {
            return None;
        }
        let dir = lib.covers_dir().join("musicbrainz");
        for ext in ["jpg", "png", "webp"] {
            let p = dir.join(format!("{release_group_id}-250.{ext}"));
            if p.is_file() {
                return Some(scan_display(&p));
            }
        }
        let bytes = match self.mb.front_cover(release_group_id, 250).await {
            Ok(Some(b)) => b,
            Ok(None) => return None,
            Err(e) => {
                log::debug!("cover preview for {release_group_id} failed: {e}");
                return None;
            }
        };
        let ext = image_ext(&bytes)?;
        let dest = dir.join(format!("{release_group_id}-250.{ext}"));
        let written = blocking({
            let dest = dest.clone();
            move || {
                std::fs::create_dir_all(dest.parent().unwrap_or(Path::new(".")))?;
                crate::library::scan::write_atomic(&dest, &bytes)
            }
        })
        .await
        .ok()?;
        written.ok()?;
        Some(scan_display(&dest))
    }

    // ----- album ------------------------------------------------------

    pub async fn match_album(
        &self,
        lib: &Arc<Library>,
        album_id: i64,
    ) -> Result<Vec<AlbumCandidate>> {
        self.require_net()?;
        let (album, local) = {
            let lib = Arc::clone(lib);
            blocking(move || -> Result<(AlbumRow, Vec<LocalTagged>)> {
                let detail = lib.album(album_id)?;
                let local = load_tagged(&detail.tracks)?;
                Ok((detail.album, local))
            })
            .await??
        };
        if local.is_empty() {
            return Err(MetadataError::Invalid(
                "the album has no readable tracks".into(),
            ));
        }
        let local_album = LocalAlbum {
            title: album.title.clone(),
            artist: album.album_artist.clone(),
            year: album.year,
            tracks: local.iter().map(|l| l.track.clone()).collect(),
        };

        let (mut found, mut requests) = self
            .mb
            .search_releases(&album.title, &album.album_artist, Some(local.len()))
            .await?;
        // MusicBrainz' own score first, then releases with our track count.
        found.sort_by_key(|r| {
            (
                std::cmp::Reverse(r.score.unwrap_or(0)),
                usize::from(r.track_count.map(|n| n as usize) != Some(local.len())),
            )
        });
        let mut seen = std::collections::HashSet::new();
        found.retain(|r| !r.id.is_empty() && seen.insert(r.id.clone()));
        let lookups = MAX_LOOKUPS.min(MAX_ALBUM_REQUESTS.saturating_sub(requests));

        let mut scored: Vec<(u8, scoring::Assignment, CandRelease)> = Vec::new();
        for r in found.iter().take(lookups) {
            requests += 1;
            let full = match self.mb.lookup_release(&r.id).await {
                Ok(full) => full,
                // One failed lookup must not hide the others.
                Err(e) => {
                    log::warn!("release lookup {} failed: {e}", r.id);
                    continue;
                }
            };
            let cand = CandRelease::from_mb(&full);
            let (score, assignment) = scoring::score_album(&local_album, &cand);
            if score >= MIN_SCORE {
                scored.push((score, assignment, cand));
            }
        }
        debug_assert!(requests <= MAX_ALBUM_REQUESTS + 1);
        scored.sort_by_key(|(s, _, _)| std::cmp::Reverse(*s));

        let mut out = Vec::with_capacity(scored.len());
        for (i, (score, assignment, cand)) in scored.into_iter().enumerate() {
            let cover_preview = if i < MAX_PREVIEWS {
                self.preview(lib, &cand.release_group_id).await
            } else {
                None
            };
            out.push(build_album_candidate(
                &local_album,
                &local,
                &cand,
                score,
                &assignment,
                cover_preview,
            ));
        }
        Ok(out)
    }

    // ----- single track -----------------------------------------------

    pub async fn match_track(
        &self,
        lib: &Arc<Library>,
        track_id: i64,
    ) -> Result<Vec<TrackCandidate>> {
        self.require_net()?;
        let local = {
            let lib = Arc::clone(lib);
            blocking(move || -> Result<LocalTagged> {
                let row = lib
                    .track_by_id(track_id)?
                    .ok_or_else(|| MetadataError::Invalid(format!("track {track_id} not found")))?;
                load_tagged(std::slice::from_ref(&row))?
                    .into_iter()
                    .next()
                    .ok_or_else(|| MetadataError::Invalid("the file cannot be read".into()))
            })
            .await??
        };
        let title = local.tags.title.clone().unwrap_or_default();
        let artist = local.tags.artist.clone().unwrap_or_default();
        if title.is_empty() || artist.is_empty() {
            return Err(MetadataError::Invalid(
                "the track needs a title and an artist to be searched".into(),
            ));
        }
        let recordings = self.mb.search_recordings(&title, &artist).await?;

        let mut scored: Vec<(u8, &MbRecording, &MbRelease)> = recordings
            .iter()
            .filter(|r| scoring::duration_within(local.track.duration_ms, r.length))
            .filter_map(|r| {
                let release = scoring::pick_release(&r.releases)?;
                Some((scoring::score_recording(&local.track, r), r, release))
            })
            .filter(|(s, _, _)| *s >= MIN_SCORE)
            .collect();
        scored.sort_by_key(|(s, _, _)| std::cmp::Reverse(*s));
        scored.truncate(MAX_TRACK_CANDIDATES);

        let mut out = Vec::with_capacity(scored.len());
        for (i, (score, rec, release)) in scored.into_iter().enumerate() {
            let rg = release
                .release_group
                .as_ref()
                .map(|g| g.id.clone())
                .unwrap_or_default();
            let cover_preview = if i < MAX_PREVIEWS {
                self.preview(lib, &rg).await
            } else {
                None
            };
            out.push(build_track_candidate(
                &local,
                rec,
                release,
                &rg,
                score,
                cover_preview,
            ));
        }
        Ok(out)
    }

    // ----- apply / undo -----------------------------------------------

    /// Writes the proposal. Each track's old values are saved first so that
    /// [`Self::undo`] can restore them. A failing file is reported and left
    /// as it was; the others are still written.
    pub async fn apply(
        &self,
        lib: &Arc<Library>,
        proposal: Proposal,
        current: Option<String>,
    ) -> Result<ApplyOutcome> {
        let (mut outcome, track_ids) = {
            let (lib, proposal) = (Arc::clone(lib), proposal.clone());
            let writes = Arc::clone(&self.writes);
            blocking(move || {
                let _guard = writes.lock().unwrap_or_else(|e| e.into_inner());
                apply_tags(&lib, &proposal, current.as_deref())
            })
            .await??
        };
        if proposal.cover
            && self.net.allowed(Feature::Metadata)
            && !proposal.release_group_id.is_empty()
            && !track_ids.is_empty()
        {
            outcome.cover = self.add_missing_cover(lib, &proposal, &track_ids).await;
        }
        Ok(outcome)
    }

    /// Adds the release group's front cover to the albums of the written
    /// tracks that have none. Errors are logged, never fatal: the tags are
    /// already written.
    async fn add_missing_cover(
        &self,
        lib: &Arc<Library>,
        proposal: &Proposal,
        track_ids: &[i64],
    ) -> Option<String> {
        let missing: Vec<AlbumRow> = {
            let (lib, ids) = (Arc::clone(lib), track_ids.to_vec());
            blocking(move || {
                let mut albums: Vec<AlbumRow> = Vec::new();
                for id in ids {
                    if let Ok(Some(a)) = lib.track_album(id)
                        && !albums.iter().any(|x| x.id == a.id)
                        && !a.cover.as_deref().is_some_and(|c| Path::new(c).is_file())
                    {
                        albums.push(a);
                    }
                }
                albums
            })
            .await
            .ok()?
        };
        if missing.is_empty() {
            return None;
        }
        let bytes = match self.mb.front_cover(&proposal.release_group_id, 500).await {
            Ok(Some(b)) => b,
            Ok(None) => return None,
            Err(e) => {
                log::warn!("cover download failed: {e}");
                return None;
            }
        };
        let ext = image_ext(&bytes)?;
        let lib = Arc::clone(lib);
        blocking(move || {
            let mut first = None;
            for a in missing {
                let dest = lib.covers_dir().join(cover_file_name(
                    &a.album_artist,
                    &a.title,
                    a.year.map(i64::from),
                    ext,
                ));
                let done = crate::library::scan::write_atomic(&dest, &bytes)
                    .map_err(|e| log::warn!("cannot write cover {}: {e}", dest.display()))
                    .is_ok()
                    && lib
                        .set_album_cover(a.id, &scan_display(&dest))
                        .map_err(|e| log::warn!("cannot set cover: {e}"))
                        .is_ok();
                if done && first.is_none() {
                    first = Some(scan_display(&dest));
                }
            }
            first
        })
        .await
        .ok()
        .flatten()
    }

    /// Puts back the tags saved for `batch_id` and forgets them. Rows whose
    /// file could not be restored stay in the history so the undo can be
    /// retried. A cover that was added stays.
    pub async fn undo(
        &self,
        lib: &Arc<Library>,
        batch_id: i64,
        current: Option<String>,
    ) -> Result<ApplyOutcome> {
        let lib = Arc::clone(lib);
        let writes = Arc::clone(&self.writes);
        blocking(move || {
            let _guard = writes.lock().unwrap_or_else(|e| e.into_inner());
            undo_batch(&lib, batch_id, current.as_deref())
        })
        .await?
    }
}

fn scan_display(p: &Path) -> String {
    crate::library::scan::display_path(p)
}

fn image_ext(bytes: &[u8]) -> Option<&'static str> {
    match image::guess_format(bytes).ok()? {
        image::ImageFormat::Jpeg => Some("jpg"),
        image::ImageFormat::Png => Some("png"),
        image::ImageFormat::WebP => Some("webp"),
        _ => None,
    }
}

fn apply_tags(
    lib: &Library,
    proposal: &Proposal,
    current: Option<&str>,
) -> Result<(ApplyOutcome, Vec<i64>)> {
    let batch_id = lib.next_tag_batch_id()?;
    let mut out = ApplyOutcome::default();
    let mut written_ids = Vec::new();
    for tp in proposal.tracks.iter().filter(|t| !t.is_empty()) {
        let Some((_, path, _)) = lib.track_files(&[tp.track_id])?.into_iter().next() else {
            out.failed.push((
                format!("track {}", tp.track_id),
                "not in the library".into(),
            ));
            continue;
        };
        if tags::is_playing_path(&path, current) {
            out.failed.push((path, tags::PLAYING_ERROR.into()));
            continue;
        }
        let old = match OldTags::read(Path::new(&path)) {
            Ok(o) => o,
            Err(e) => {
                out.failed.push((path, e.to_string()));
                continue;
            }
        };
        let json = serde_json::to_string(&old)?;
        let history_id = lib.save_tag_history(batch_id, Some(tp.track_id), &path, &json)?;
        let res = tags::write_for_tracks(lib, &[tp.track_id], &tp.to_changes(), current)?;
        if let Some(f) = res.failed.into_iter().next() {
            lib.delete_tag_history_row(history_id)?;
            out.failed.push((f.path, f.error));
        } else {
            written_ids.push(tp.track_id);
            out.updated_paths.extend(res.updated);
        }
    }
    out.updated = written_ids.len();
    out.batch_id = if written_ids.is_empty() { 0 } else { batch_id };
    Ok((out, written_ids))
}

fn undo_batch(lib: &Library, batch_id: i64, current: Option<&str>) -> Result<ApplyOutcome> {
    let rows = lib.tag_history_batch(batch_id)?;
    if rows.is_empty() {
        return Err(MetadataError::Invalid(
            "nothing to undo for this change".into(),
        ));
    }
    let mut out = ApplyOutcome {
        batch_id,
        ..ApplyOutcome::default()
    };
    for row in rows {
        // The track id can have changed if the file was re-added: go by path.
        let id = lib.track_id_by_path(&row.path)?.or(row.track_id);
        let old: OldTags = match serde_json::from_str(&row.old_json) {
            Ok(o) => o,
            Err(e) => {
                out.failed.push((row.path, e.to_string()));
                continue;
            }
        };
        let Some(id) = id else {
            out.failed.push((row.path, "not in the library".into()));
            continue;
        };
        let res = tags::write_for_tracks(lib, &[id], &old.restore_changes(), current)?;
        match res.failed.into_iter().next() {
            Some(f) => out.failed.push((f.path, f.error)),
            None => {
                lib.delete_tag_history_row(row.id)?;
                out.updated_paths.extend(res.updated);
                out.updated += 1;
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------
// Candidate construction
// ---------------------------------------------------------------------

fn build_album_candidate(
    local_album: &LocalAlbum,
    local: &[LocalTagged],
    cand: &CandRelease,
    score: u8,
    assignment: &scoring::Assignment,
    cover_preview: Option<String>,
) -> AlbumCandidate {
    let year = cand.year();
    let mut tracks = Vec::with_capacity(local.len());
    let mut proposals = Vec::new();
    for (l, a) in local.iter().zip(assignment) {
        let Some(ct) = a.map(|j| &cand.tracks[j]) else {
            tracks.push(TrackDiff {
                track_id: l.track.id,
                path: l.track.path.clone(),
                title: l.track.title.clone(),
                matched: false,
                fields: Vec::new(),
            });
            continue;
        };
        let new = NewValues {
            title: Some(&ct.title),
            artist: Some(&ct.artist),
            album: Some(&cand.title),
            album_artist: Some(&cand.artist),
            track_no: Some(ct.position),
            disc_no: Some(ct.disc),
            year,
            release_id: &cand.id,
            release_group_id: &cand.release_group_id,
            recording_id: &ct.recording_id,
            track_id: &ct.track_id,
        };
        let (p, d) = diff_track(&l.track, &l.tags, &l.ids, &new);
        proposals.push(p);
        tracks.push(d);
    }
    let mut album_fields = Vec::new();
    text_diff(
        &mut album_fields,
        "album",
        Some(&local_album.title),
        Some(&cand.title),
    );
    text_diff(
        &mut album_fields,
        "albumArtist",
        Some(&local_album.artist),
        Some(&cand.artist),
    );
    num_diff(
        &mut album_fields,
        "year",
        local_album.year.map(i64::from),
        year.map(i64::from),
    );
    AlbumCandidate {
        release_id: cand.id.clone(),
        release_group_id: cand.release_group_id.clone(),
        title: cand.title.clone(),
        artist: cand.artist.clone(),
        year,
        country: cand.country.clone(),
        format: cand.format.clone(),
        status: cand.status.clone(),
        track_count: cand.tracks.len(),
        score,
        label: scoring::label_for(score),
        cover_preview,
        album_fields,
        tracks,
        proposal: Proposal {
            release_id: cand.id.clone(),
            release_group_id: cand.release_group_id.clone(),
            tracks: proposals,
            cover: true,
        },
    }
}

fn build_track_candidate(
    local: &LocalTagged,
    rec: &MbRecording,
    release: &MbRelease,
    release_group_id: &str,
    score: u8,
    cover_preview: Option<String>,
) -> TrackCandidate {
    let rec_artist = credit_string(&rec.artist_credit);
    let release_artist = credit_string(&release.artist_credit);
    let album_artist = if release_artist.is_empty() {
        rec_artist.clone()
    } else {
        release_artist
    };
    let (disc, number, track_id) = scoring::recording_position(release);
    let year = scoring::year_of(release.date.as_deref());
    let new = NewValues {
        title: Some(&rec.title),
        artist: Some(&rec_artist),
        album: Some(&release.title),
        album_artist: Some(&album_artist),
        track_no: number,
        disc_no: disc,
        year,
        release_id: &release.id,
        release_group_id,
        recording_id: &rec.id,
        track_id: &track_id,
    };
    let (p, diff) = diff_track(&local.track, &local.tags, &local.ids, &new);
    TrackCandidate {
        recording_id: rec.id.clone(),
        release_id: release.id.clone(),
        release_group_id: release_group_id.to_owned(),
        title: rec.title.clone(),
        artist: rec_artist,
        album: release.title.clone(),
        year,
        country: release.country.clone().filter(|c| !c.is_empty()),
        format: release.media.first().and_then(|m| m.format.clone()),
        status: release.status.clone(),
        score,
        label: scoring::label_for(score),
        cover_preview,
        track: diff,
        proposal: Proposal {
            release_id: release.id.clone(),
            release_group_id: release_group_id.to_owned(),
            tracks: vec![p],
            cover: true,
        },
    }
}

/// Folder holding the preview images, for tests.
#[cfg(test)]
pub(crate) fn preview_dir(lib: &Library) -> std::path::PathBuf {
    lib.covers_dir().join("musicbrainz")
}
