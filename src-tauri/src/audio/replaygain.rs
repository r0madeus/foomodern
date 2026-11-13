//! ReplayGain for engines without built-in support (Android's ExoPlayer):
//! read the gain tags and turn them into a volume factor. mpv does this
//! itself on desktop.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use super::ReplayGain;
use lofty::file::TaggedFileExt;
use lofty::tag::ItemKey;
use std::path::Path;

/// Gain (dB) and peak (linear, 1.0 = full scale) of one ReplayGain level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Level {
    pub gain_db: f64,
    pub peak: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Tags {
    pub track: Option<Level>,
    pub album: Option<Level>,
}

/// "-6.54 dB", "+1.2", "0.98" → number.
fn number(text: &str) -> Option<f64> {
    let t = text.trim();
    let t = t
        .strip_suffix("dB")
        .or_else(|| t.strip_suffix("db"))
        .unwrap_or(t);
    t.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

fn level(gain: Option<&str>, peak: Option<&str>) -> Option<Level> {
    Some(Level {
        gain_db: number(gain?)?,
        peak: peak.and_then(number).filter(|p| *p > 0.0),
    })
}

/// Reads the ReplayGain tags of a file; missing or unreadable tags give
/// an empty result.
pub fn read(path: &Path) -> Tags {
    let Ok(tagged) = lofty::read_from_path(path) else {
        return Tags::default();
    };
    let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) else {
        return Tags::default();
    };
    Tags {
        track: level(
            tag.get_string(ItemKey::ReplayGainTrackGain),
            tag.get_string(ItemKey::ReplayGainTrackPeak),
        ),
        album: level(
            tag.get_string(ItemKey::ReplayGainAlbumGain),
            tag.get_string(ItemKey::ReplayGainAlbumPeak),
        ),
    }
}

/// Volume factor (0..=1) for `mode`, like mpv's defaults: album mode falls
/// back to the track gain, no tags means no change, and the gain is
/// lowered so the peak does not clip. A volume can only attenuate, so
/// positive gains are capped at 1.0 (mpv could amplify quiet tracks).
pub fn factor(tags: Tags, mode: ReplayGain) -> f64 {
    let chosen = match mode {
        ReplayGain::Off => None,
        ReplayGain::Track => tags.track,
        ReplayGain::Album => tags.album.or(tags.track),
    };
    let Some(Level { gain_db, peak }) = chosen else {
        return 1.0;
    };
    let mut f = 10f64.powf(gain_db / 20.0);
    if let Some(p) = peak {
        f = f.min(1.0 / p);
    }
    f.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::config::WriteOptions;
    use lofty::tag::{Accessor, Tag, TagExt, TagType};

    fn lvl(gain_db: f64, peak: Option<f64>) -> Option<Level> {
        Some(Level { gain_db, peak })
    }

    #[test]
    fn parses_gain_text() {
        assert_eq!(number("-6.54 dB"), Some(-6.54));
        assert_eq!(number("+1.20 dB"), Some(1.2));
        assert_eq!(number(" 0.987 "), Some(0.987));
        assert_eq!(number("loud"), None);
    }

    #[test]
    fn modes_and_fallbacks() {
        let tags = Tags {
            track: lvl(-6.0, None),
            album: lvl(-12.0, None),
        };
        assert_eq!(factor(tags, ReplayGain::Off), 1.0);
        assert!((factor(tags, ReplayGain::Track) - 0.501).abs() < 0.001);
        assert!((factor(tags, ReplayGain::Album) - 0.251).abs() < 0.001);
        let only_track = Tags {
            track: lvl(-6.0, None),
            album: None,
        };
        assert_eq!(
            factor(only_track, ReplayGain::Album),
            factor(only_track, ReplayGain::Track)
        );
        assert_eq!(factor(Tags::default(), ReplayGain::Album), 1.0);
    }

    #[test]
    fn peak_limits_and_positive_gain_is_capped() {
        let clipping = Tags {
            track: lvl(-1.0, Some(1.5)),
            album: None,
        };
        assert!((factor(clipping, ReplayGain::Track) - 1.0 / 1.5).abs() < 1e-9);
        let quiet = Tags {
            track: lvl(4.0, Some(0.5)),
            album: None,
        };
        assert_eq!(factor(quiet, ReplayGain::Track), 1.0);
    }

    #[test]
    fn reads_tags_from_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..800 {
            w.write_sample((i % 50) as i16).unwrap();
        }
        w.finalize().unwrap();
        let mut tag = Tag::new(TagType::Id3v2);
        tag.set_title("t".into());
        tag.insert_text(ItemKey::ReplayGainTrackGain, "-7.50 dB".into());
        tag.insert_text(ItemKey::ReplayGainTrackPeak, "0.9".into());
        tag.save_to_path(&path, WriteOptions::default()).unwrap();

        let tags = read(&path);
        assert_eq!(tags.track, lvl(-7.5, Some(0.9)));
        assert_eq!(tags.album, None);
        assert_eq!(read(&dir.path().join("missing.flac")), Tags::default());
    }
}
