//! M3U playlists, to move playlists between devices (PC <-> phone).
//!
//! Export writes extended M3U (`#EXTINF:<seconds>,<artist> - <title>`)
//! with absolute paths. Import cannot rely on paths alone, because the
//! same song lives at `D:\Music\...` on the PC and `/storage/.../Music/...`
//! on the phone, so each entry is matched in this order:
//! 1. the exact path (separators and case normalised);
//! 2. the last 3, 2 or 1 path components (artist/album/file), if unique;
//! 3. artist and title from `#EXTINF`, closest duration on ties.

use super::TrackRow;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Entry {
    pub path: String,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub duration_secs: Option<i64>,
}

/// A library track as the matcher sees it.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: i64,
    pub path: String,
    pub artist: String,
    pub title: String,
    pub duration_ms: Option<u64>,
}

pub fn parse(text: &str) -> Vec<Entry> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut out = Vec::new();
    let mut info: Option<Entry> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            let (secs, label) = rest.split_once(',').unwrap_or((rest, ""));
            let secs = secs
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|s| *s > 0.0)
                .map(|s| s.round() as i64);
            let (artist, title) = match label.split_once(" - ") {
                Some((a, t)) => (non_empty(a), non_empty(t)),
                None => (None, non_empty(label)),
            };
            info = Some(Entry {
                path: String::new(),
                artist,
                title,
                duration_secs: secs,
            });
        } else if line.starts_with('#') {
            continue;
        } else {
            let mut e = info.take().unwrap_or_default();
            e.path = line.to_string();
            out.push(e);
        }
    }
    out
}

fn non_empty(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

pub fn render(rows: &[TrackRow]) -> String {
    let mut out = String::from("#EXTM3U\n");
    for t in rows {
        let secs = t
            .duration_ms
            .map(|ms| (ms as f64 / 1000.0).round() as i64)
            .unwrap_or(-1);
        let label = if t.artist.is_empty() {
            t.title.clone()
        } else {
            format!("{} - {}", t.artist, t.title)
        };
        // Line breaks would break the format.
        let label = label.replace(['\r', '\n'], " ");
        out.push_str(&format!("#EXTINF:{secs},{label}\n{}\n", t.path));
    }
    out
}

fn norm_path(p: &str) -> String {
    p.replace('\\', "/").to_lowercase()
}

fn components(p: &str) -> Vec<&str> {
    p.split('/').filter(|c| !c.is_empty()).collect()
}

fn suffix(parts: &[&str], n: usize) -> Option<String> {
    (parts.len() >= n).then(|| parts[parts.len() - n..].join("/"))
}

fn tag_key(artist: &str, title: &str) -> String {
    format!(
        "{}\u{0}{}",
        artist.trim().to_lowercase(),
        title.trim().to_lowercase()
    )
}

/// Library track id for each entry (`None` when nothing matched).
pub fn resolve(entries: &[Entry], library: &[Candidate]) -> Vec<Option<i64>> {
    let mut exact: HashMap<String, i64> = HashMap::new();
    let mut by_suffix: [HashMap<String, Vec<i64>>; 3] = Default::default();
    let mut by_tags: HashMap<String, Vec<(i64, Option<u64>)>> = HashMap::new();
    for c in library {
        let p = norm_path(&c.path);
        let parts = components(&p);
        for n in 1..=3 {
            if let Some(s) = suffix(&parts, n) {
                by_suffix[n - 1].entry(s).or_default().push(c.id);
            }
        }
        exact.insert(p, c.id);
        if !c.title.is_empty() {
            by_tags
                .entry(tag_key(&c.artist, &c.title))
                .or_default()
                .push((c.id, c.duration_ms));
        }
    }

    entries
        .iter()
        .map(|e| {
            let p = norm_path(&e.path);
            if let Some(id) = exact.get(&p) {
                return Some(*id);
            }
            let parts = components(&p);
            for n in (1..=3).rev() {
                if let Some(ids) = suffix(&parts, n).and_then(|s| by_suffix[n - 1].get(&s))
                    && ids.len() == 1
                {
                    return Some(ids[0]);
                }
            }
            let title = e.title.as_deref()?;
            let found = by_tags.get(&tag_key(e.artist.as_deref().unwrap_or(""), title))?;
            let closest = found
                .iter()
                .min_by_key(|(_, ms)| match (e.duration_secs, ms) {
                    (Some(want), Some(ms)) => (want * 1000 - *ms as i64).unsigned_abs(),
                    _ => u64::MAX,
                });
            closest.map(|(id, _)| *id)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: i64, path: &str, artist: &str, title: &str, secs: u64) -> Candidate {
        Candidate {
            id,
            path: path.into(),
            artist: artist.into(),
            title: title.into(),
            duration_ms: Some(secs * 1000),
        }
    }

    #[test]
    fn parses_extended_m3u() {
        let text = "\u{feff}#EXTM3U\r\n#EXTINF:245,Buray - Deli Divane\r\nD:\\Music\\Buray\\01.flac\r\n\r\n# a comment\r\n/plain/path.mp3\r\n#EXTINF:-1,Only Title\r\nx.ogg\r\n";
        let e = parse(text);
        assert_eq!(e.len(), 3);
        assert_eq!(e[0].artist.as_deref(), Some("Buray"));
        assert_eq!(e[0].title.as_deref(), Some("Deli Divane"));
        assert_eq!(e[0].duration_secs, Some(245));
        assert_eq!(e[0].path, "D:\\Music\\Buray\\01.flac");
        assert_eq!(
            e[1],
            Entry {
                path: "/plain/path.mp3".into(),
                ..Default::default()
            }
        );
        assert_eq!(e[2].artist, None);
        assert_eq!(e[2].title.as_deref(), Some("Only Title"));
        assert_eq!(e[2].duration_secs, None);
    }

    #[test]
    fn round_trips_through_render() {
        let rows = vec![TrackRow {
            id: 1,
            title: "Deli Divane".into(),
            artist: "Buray".into(),
            album: "Sahiden".into(),
            album_id: None,
            track_no: None,
            disc_no: None,
            year: None,
            duration_ms: Some(234_400),
            path: "/m/Buray/01.flac".into(),
            cover: None,
        }];
        let e = parse(&render(&rows));
        assert_eq!(
            e,
            vec![Entry {
                path: "/m/Buray/01.flac".into(),
                artist: Some("Buray".into()),
                title: Some("Deli Divane".into()),
                duration_secs: Some(234),
            }]
        );
    }

    #[test]
    fn matches_across_devices() {
        let phone = vec![
            cand(
                1,
                "/storage/emulated/0/Music/Buray/Sahiden/01 Deli Divane.flac",
                "Buray",
                "Deli Divane",
                234,
            ),
            cand(
                2,
                "/storage/emulated/0/Music/Other/Live/01 Deli Divane.flac",
                "Other",
                "Deli Divane",
                300,
            ),
            cand(
                3,
                "/storage/emulated/0/Music/Misc/song.mp3",
                "Aria Vale",
                "Golden Hour",
                180,
            ),
            cand(
                4,
                "/storage/emulated/0/Music/Dup/a.mp3",
                "Twin",
                "Same",
                100,
            ),
            cand(
                5,
                "/storage/emulated/0/Music/Dup2/b.mp3",
                "Twin",
                "Same",
                200,
            ),
        ];
        let pc = parse(
            "#EXTM3U\n\
             #EXTINF:234,Buray - Deli Divane\nD:\\Music\\Buray\\Sahiden\\01 Deli Divane.flac\n\
             #EXTINF:180,Aria Vale - Golden Hour\nD:\\Elsewhere\\renamed.mp3\n\
             #EXTINF:199,Twin - Same\nC:\\x\\gone.mp3\n\
             #EXTINF:10,Nobody - Missing\nC:\\x\\missing.mp3\n\
             /storage/emulated/0/Music/Misc/song.mp3\n",
        );
        assert_eq!(
            resolve(&pc, &phone),
            vec![Some(1), Some(3), Some(5), None, Some(3)]
        );
    }

    #[test]
    fn ambiguous_file_names_fall_back_to_tags() {
        let lib = vec![
            cand(1, "/a/CD1/01.flac", "A", "One", 100),
            cand(2, "/b/CD1/01.flac", "B", "Two", 100),
        ];
        let e = parse("#EXTINF:100,B - Two\nZ:\\x\\CD1\\01.flac\n");
        assert_eq!(resolve(&e, &lib), vec![Some(2)]);
    }
}
