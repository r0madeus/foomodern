//! Turning lyrics into text and saving them permanently: as a `.lrc` sidecar
//! next to the audio file or inside its lyrics tag.

use super::{LyricsResult, lrc};
use crate::library::tags::{self, TagError};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Where lyrics found online are kept besides the app's cache.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SaveMode {
    /// A `<audio stem>.lrc` file next to the song.
    #[default]
    Sidecar,
    /// The song's lyrics tag.
    Embed,
    /// Only the app's cache.
    Off,
}

/// `[mm:ss.xx]` stamp of a line.
fn stamp(seconds: f64) -> String {
    let cs = (seconds.max(0.0) * 100.0).round() as u64;
    format!("[{:02}:{:02}.{:02}]", cs / 6000, cs / 100 % 60, cs % 100)
}

/// The text to store: LRC with timestamps for synced lyrics, text lines for
/// plain ones. `None` for instrumental/not found.
pub fn to_lrc_text(result: &LyricsResult) -> Option<String> {
    match result {
        LyricsResult::Synced { lines, .. } if !lines.is_empty() => Some(
            lines
                .iter()
                .map(|l| format!("{}{}", stamp(l.time_seconds), l.text))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        LyricsResult::Plain { lines, .. } if !lines.is_empty() => Some(lines.join("\n")),
        _ => None,
    }
}

/// The lyrics without timestamps (LRCLIB `plainLyrics`).
pub fn to_plain_text(result: &LyricsResult) -> Option<String> {
    match result {
        LyricsResult::Synced { lines, .. } if !lines.is_empty() => {
            Some(lrc::trim_empty_edges(lines.iter().map(|l| l.text.clone()).collect()).join("\n"))
        }
        LyricsResult::Plain { lines, .. } if !lines.is_empty() => Some(lines.join("\n")),
        _ => None,
    }
}

fn sidecar_exists(audio: &Path) -> bool {
    audio.with_extension("lrc").exists() || audio.with_extension("LRC").exists()
}

/// Writes `<stem>.lrc` next to `audio` (temp file + rename). Returns `false`
/// without touching anything when a `.lrc` already exists.
pub fn write_sidecar(audio: &Path, text: &str) -> std::io::Result<bool> {
    let Some(name) = audio.file_name() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid track path",
        ));
    };
    if sidecar_exists(audio) {
        return Ok(false);
    }
    let tmp = audio.with_file_name(format!(".{}.foomodern-lrc.tmp", name.to_string_lossy()));
    let result = std::fs::write(&tmp, text).and_then(|()| {
        // Re-check right before the replace: never clobber a file that
        // appeared while the temp file was written.
        if sidecar_exists(audio) {
            Ok(false)
        } else {
            std::fs::rename(&tmp, audio.with_extension("lrc")).map(|()| true)
        }
    });
    if !matches!(result, Ok(true)) {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Writes the lyrics into the file's tag unless it already has some.
pub fn write_embedded(audio: &Path, text: &str) -> Result<bool, TagError> {
    tags::write_lyrics(audio, text)
}

/// Saves `text` with `mode`. `Ok(true)` = written, `Ok(false)` = skipped
/// (already present or mode is off).
pub fn save(mode: SaveMode, audio: &Path, text: &str) -> Result<bool, String> {
    match mode {
        SaveMode::Off => Ok(false),
        SaveMode::Sidecar => write_sidecar(audio, text).map_err(|e| e.to_string()),
        SaveMode::Embed => write_embedded(audio, text).map_err(|e| e.to_string()),
    }
}
