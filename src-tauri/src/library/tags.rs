//! Reading and writing the editable tag fields of audio files with `lofty`.
//!
//! Writes go to the file's primary tag (created when missing). Everything
//! else in the file (other tag items, pictures, other tag formats) is kept.
//! To keep a failed write from damaging the original, the new tags are
//! written to a temporary copy next to the file which then replaces it.

use super::scan::display_path;
use lofty::config::WriteOptions;
use lofty::error::{FileEncodingError, FileParseError};
use lofty::file::{AudioFile, TaggedFile, TaggedFileExt};
use lofty::prelude::{Accessor, ItemKey};
use lofty::tag::items::Timestamp;
use lofty::tag::{Tag, TagType};
use serde::{Deserialize, Deserializer, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum TagError {
    #[error("cannot read tags: {0}")]
    Read(#[from] FileParseError),
    #[error("cannot write tags: {0}")]
    Write(#[from] FileEncodingError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    InUse(String),
}

/// The tag fields the editor shows. `None` means "no value in the file".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditableTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
}

/// JSON `null` becomes `Some(None)` (clear); a missing key stays `None`
/// (keep); a value becomes `Some(Some(v))` (set).
fn keep_clear_set<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// What to do with each field. Per field: `None` keeps it as it is,
/// `Some(None)` removes it, `Some(Some(v))` sets it. Fields the user did not
/// touch must stay `None` so editing several files at once never overwrites
/// their differing values.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TagChanges {
    #[serde(deserialize_with = "keep_clear_set")]
    pub title: Option<Option<String>>,
    #[serde(deserialize_with = "keep_clear_set")]
    pub artist: Option<Option<String>>,
    #[serde(deserialize_with = "keep_clear_set")]
    pub album: Option<Option<String>>,
    #[serde(deserialize_with = "keep_clear_set")]
    pub album_artist: Option<Option<String>>,
    #[serde(deserialize_with = "keep_clear_set")]
    pub track_no: Option<Option<u32>>,
    #[serde(deserialize_with = "keep_clear_set")]
    pub disc_no: Option<Option<u32>>,
    #[serde(deserialize_with = "keep_clear_set")]
    pub year: Option<Option<i32>>,
    #[serde(deserialize_with = "keep_clear_set")]
    pub genre: Option<Option<String>>,
    /// MusicBrainz identifiers (release, release group, recording, release
    /// track). The editor never sets these; the metadata matcher does.
    #[serde(deserialize_with = "keep_clear_set")]
    pub mb_release_id: Option<Option<String>>,
    #[serde(deserialize_with = "keep_clear_set")]
    pub mb_release_group_id: Option<Option<String>>,
    #[serde(deserialize_with = "keep_clear_set")]
    pub mb_recording_id: Option<Option<String>>,
    #[serde(deserialize_with = "keep_clear_set")]
    pub mb_track_id: Option<Option<String>>,
}

/// The MusicBrainz identifiers stored in a file's tags.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MbIds {
    pub release_id: Option<String>,
    pub release_group_id: Option<String>,
    pub recording_id: Option<String>,
    pub track_id: Option<String>,
}

impl TagChanges {
    pub fn is_empty(&self) -> bool {
        *self == TagChanges::default()
    }

    /// Checks the values and normalizes text (trimmed, blank = clear).
    pub fn validated(&self) -> Result<TagChanges, TagError> {
        fn text(v: &Option<Option<String>>) -> Option<Option<String>> {
            v.as_ref().map(|inner| {
                inner
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
            })
        }
        if let Some(Some(y)) = self.year
            && !(1..=9999).contains(&y)
        {
            return Err(TagError::Invalid(format!("year {y} is out of range")));
        }
        Ok(TagChanges {
            title: text(&self.title),
            artist: text(&self.artist),
            album: text(&self.album),
            album_artist: text(&self.album_artist),
            track_no: self.track_no,
            disc_no: self.disc_no,
            year: self.year,
            genre: text(&self.genre),
            mb_release_id: text(&self.mb_release_id),
            mb_release_group_id: text(&self.mb_release_group_id),
            mb_recording_id: text(&self.mb_recording_id),
            mb_track_id: text(&self.mb_track_id),
        })
    }
}

fn non_empty(s: Option<impl AsRef<str>>) -> Option<String> {
    s.map(|s| s.as_ref().trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn from_tag(tag: &Tag) -> EditableTags {
    EditableTags {
        title: non_empty(tag.title()),
        artist: non_empty(tag.artist()),
        album: non_empty(tag.album()),
        album_artist: non_empty(tag.get_string(ItemKey::AlbumArtist)),
        track_no: tag.track(),
        disc_no: tag.disk(),
        year: tag.date().map(|d| i32::from(d.year)).filter(|y| *y > 0),
        genre: non_empty(tag.genre()),
    }
}

fn mb_ids_from_tag(tag: &Tag) -> MbIds {
    MbIds {
        release_id: non_empty(tag.get_string(ItemKey::MusicBrainzReleaseId)),
        release_group_id: non_empty(tag.get_string(ItemKey::MusicBrainzReleaseGroupId)),
        recording_id: non_empty(tag.get_string(ItemKey::MusicBrainzRecordingId)),
        track_id: non_empty(tag.get_string(ItemKey::MusicBrainzTrackId)),
    }
}

/// MusicBrainz ids of a file (empty when the file has none).
pub fn read_mb_ids(path: &Path) -> Result<MbIds, TagError> {
    let tagged = lofty::read_from_path(path)?;
    Ok(tagged
        .primary_tag()
        .or_else(|| tagged.first_tag())
        .map(mb_ids_from_tag)
        .unwrap_or_default())
}

/// Same tag the library scanner reads: the primary tag, else the first one.
pub fn read_tags(path: &Path) -> Result<EditableTags, TagError> {
    let tagged = lofty::read_from_path(path)?;
    Ok(tagged
        .primary_tag()
        .or_else(|| tagged.first_tag())
        .map(from_tag)
        .unwrap_or_default())
}

fn apply(tag: &mut Tag, c: &TagChanges) {
    match &c.title {
        None => {}
        Some(None) => tag.remove_title(),
        Some(Some(v)) => tag.set_title(v.clone()),
    }
    match &c.artist {
        None => {}
        Some(None) => tag.remove_artist(),
        Some(Some(v)) => tag.set_artist(v.clone()),
    }
    match &c.album {
        None => {}
        Some(None) => tag.remove_album(),
        Some(Some(v)) => tag.set_album(v.clone()),
    }
    match &c.album_artist {
        None => {}
        Some(None) => tag.remove_key(ItemKey::AlbumArtist),
        Some(Some(v)) => {
            tag.insert_text(ItemKey::AlbumArtist, v.clone());
        }
    }
    match c.track_no {
        None => {}
        Some(None) => tag.remove_track(),
        Some(Some(n)) => tag.set_track(n),
    }
    match c.disc_no {
        None => {}
        Some(None) => tag.remove_disk(),
        Some(Some(n)) => tag.set_disk(n),
    }
    match c.year {
        None => {}
        Some(None) => tag.remove_date(),
        Some(Some(y)) => tag.set_date(Timestamp {
            // Range checked by `TagChanges::validated`.
            year: u16::try_from(y).unwrap_or(0),
            ..Timestamp::default()
        }),
    }
    match &c.genre {
        None => {}
        Some(None) => tag.remove_genre(),
        Some(Some(v)) => tag.set_genre(v.clone()),
    }
    for (change, key) in [
        (&c.mb_release_id, ItemKey::MusicBrainzReleaseId),
        (&c.mb_release_group_id, ItemKey::MusicBrainzReleaseGroupId),
        (&c.mb_recording_id, ItemKey::MusicBrainzRecordingId),
        (&c.mb_track_id, ItemKey::MusicBrainzTrackId),
    ] {
        match change {
            None => {}
            Some(None) => tag.remove_key(key),
            Some(Some(v)) => {
                tag.insert_text(key, v.clone());
            }
        }
    }
}

/// Makes sure the file has its format's primary tag, creating it (seeded from
/// another readable tag, e.g. ID3v1) when it is missing.
fn ensure_primary(tagged: &mut TaggedFile) {
    let primary: TagType = tagged.primary_tag_type();
    if tagged.tag(primary).is_some() {
        return;
    }
    let mut tag = match tagged.first_tag() {
        Some(other) => other.clone(),
        None => Tag::new(primary),
    };
    tag.re_map(primary);
    tagged.insert_tag(tag);
}

fn temp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!(".{name}.foomodern-tags.tmp"))
}

/// Writes the (modified) tags of `tagged` to a temporary copy of `path` and
/// replaces the file with it. On any error the original is left as it was.
fn save_replacing(path: &Path, tagged: &TaggedFile) -> Result<(), TagError> {
    let tmp = temp_path(path);
    let result = (|| {
        std::fs::copy(path, &tmp)?;
        tagged.save_to_path(&tmp, WriteOptions::default())?;
        std::fs::rename(&tmp, path)?;
        Ok::<(), TagError>(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Applies `changes` to the file at `path`. Unchanged fields are not
/// touched. On any error the original file is left as it was.
pub fn write_tags(path: &Path, changes: &TagChanges) -> Result<(), TagError> {
    let changes = changes.validated()?;
    let mut tagged = lofty::read_from_path(path)?;
    ensure_primary(&mut tagged);
    let primary = tagged.primary_tag_type();
    let Some(tag) = tagged.tag_mut(primary) else {
        return Err(TagError::Invalid(
            "this file format does not support tags".into(),
        ));
    };
    apply(tag, &changes);
    save_replacing(path, &tagged)
}

/// The text of the lyrics item of a tag, whichever key the format uses.
/// lofty maps `ItemKey::Lyrics` to the Vorbis `LYRICS` comment, the MP4
/// `(c)lyr` atom and the APE `Lyrics` item, but not to ID3v2: there the
/// text lives in the `USLT` frame, which is `ItemKey::UnsyncLyrics`.
fn lyrics_from_tag(tag: &Tag) -> Option<String> {
    non_empty(tag.get_string(ItemKey::Lyrics))
        .or_else(|| non_empty(tag.get_string(ItemKey::UnsyncLyrics)))
}

/// The lyrics stored in the file's tags (`None` when missing or blank).
pub fn read_lyrics(path: &Path) -> Option<String> {
    let tagged = lofty::read_from_path(path).ok()?;
    tagged
        .primary_tag()
        .and_then(lyrics_from_tag)
        .or_else(|| tagged.tags().iter().find_map(lyrics_from_tag))
}

/// Stores `text` in the lyrics tag of `path` (temp file + replace, like
/// [`write_tags`]). Returns `false` without touching the file when it
/// already has non-empty lyrics.
pub fn write_lyrics(path: &Path, text: &str) -> Result<bool, TagError> {
    if text.trim().is_empty() {
        return Err(TagError::Invalid("no lyrics to write".into()));
    }
    let mut tagged = lofty::read_from_path(path)?;
    if tagged.tags().iter().any(|t| lyrics_from_tag(t).is_some()) {
        return Ok(false);
    }
    ensure_primary(&mut tagged);
    let primary = tagged.primary_tag_type();
    let Some(tag) = tagged.tag_mut(primary) else {
        return Err(TagError::Invalid(
            "this file format does not support tags".into(),
        ));
    };
    let key = if primary == TagType::Id3v2 {
        ItemKey::UnsyncLyrics
    } else {
        ItemKey::Lyrics
    };
    if !tag.insert_text(key, text.to_owned()) {
        return Err(TagError::Invalid(
            "this tag format cannot hold lyrics".into(),
        ));
    }
    save_replacing(path, &tagged)?;
    Ok(true)
}

/// Message shown when the user tries to edit the file the player has open.
pub const PLAYING_ERROR: &str =
    "This file is loaded in the player. Play another track to edit it (pausing is not enough).";

fn same_file(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => {
            let (a, b) = (display_path(Path::new(a)), display_path(Path::new(b)));
            if cfg!(windows) {
                a.eq_ignore_ascii_case(&b)
            } else {
                a == b
            }
        }
    }
}

/// True when `path` is the file the player currently has loaded. mpv keeps
/// it open, so rewriting it can fail (Windows) or disturb playback.
pub fn is_playing_path(path: &str, current: Option<&str>) -> bool {
    current.is_some_and(|c| same_file(path, c))
}

/// Result of merging several files' tags for the editor.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonTags {
    /// The shared value per field; `None` when empty everywhere or mixed.
    pub values: EditableTags,
    /// camelCase names of the fields whose values differ between files.
    pub mixed: Vec<&'static str>,
}

pub fn common_tags(all: &[EditableTags]) -> CommonTags {
    let mut out = CommonTags::default();
    let Some(first) = all.first() else {
        return out;
    };
    macro_rules! field {
        ($f:ident, $name:literal) => {
            if all.iter().all(|t| t.$f == first.$f) {
                out.values.$f = first.$f.clone();
            } else {
                out.mixed.push($name);
            }
        };
    }
    field!(title, "title");
    field!(artist, "artist");
    field!(album, "album");
    field!(album_artist, "albumArtist");
    field!(track_no, "trackNo");
    field!(disc_no, "discNo");
    field!(year, "year");
    field!(genre, "genre");
    out
}

/// A file that could not be edited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagFailure {
    pub path: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WriteOutcome {
    /// Paths written and re-read into the library.
    pub updated: Vec<String>,
    pub failed: Vec<TagFailure>,
}

/// One library track with the tags read from its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackTagsRead {
    pub id: i64,
    pub path: String,
    pub tags: EditableTags,
}

/// Tags of the given library tracks. Unreadable files are reported in the
/// second list instead of failing the whole call.
pub fn read_for_tracks(
    lib: &super::Library,
    ids: &[i64],
) -> super::Result<(Vec<TrackTagsRead>, Vec<TagFailure>)> {
    let mut tracks = Vec::new();
    let mut failed = Vec::new();
    for (id, path, _) in lib.track_files(ids)? {
        match read_tags(Path::new(&path)) {
            Ok(tags) => tracks.push(TrackTagsRead { id, path, tags }),
            Err(e) => failed.push(TagFailure {
                path,
                error: e.to_string(),
            }),
        }
    }
    Ok((tracks, failed))
}

/// Writes `changes` to the files of the given library tracks, skipping (and
/// reporting) the file in `current`, then re-reads every file that was
/// written into the library. A file whose write fails is left as it was and
/// its database row is not touched.
pub fn write_for_tracks(
    lib: &super::Library,
    ids: &[i64],
    changes: &TagChanges,
    current: Option<&str>,
) -> super::Result<WriteOutcome> {
    let changes = changes
        .validated()
        .map_err(|e| super::LibraryError::Other(e.to_string()))?;
    let mut out = WriteOutcome::default();
    if changes.is_empty() {
        return Ok(out);
    }
    let mut written: Vec<(String, i64)> = Vec::new();
    for (_, path, folder_id) in lib.track_files(ids)? {
        let result = if is_playing_path(&path, current) {
            Err(TagError::InUse(PLAYING_ERROR.into()))
        } else {
            write_tags(Path::new(&path), &changes)
        };
        match result {
            Ok(()) => written.push((path, folder_id)),
            Err(e) => out.failed.push(TagFailure {
                path,
                error: e.to_string(),
            }),
        }
    }
    if !written.is_empty() {
        lib.rescan_files(&written)?;
        out.updated = written.into_iter().map(|(p, _)| p).collect();
    }
    Ok(out)
}
