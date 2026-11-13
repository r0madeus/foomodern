//! Library tests: generated WAV files tagged with lofty (ID3v2), no sound
//! card or Tauri needed.

use super::scan::{cover_file_name, fnv1a, scan_folder};
use super::{Hooks, Library, ScanStats, SortOrder};
use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::tag::{Tag, TagType};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tempfile::TempDir;

#[derive(Default, Clone)]
struct Meta<'a> {
    title: &'a str,
    artist: &'a str,
    album: &'a str,
    album_artist: &'a str,
    track: u32,
    disc: u32,
    year: u16,
    picture: Option<&'a [u8]>,
}

fn write_wav(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 8000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..8000 {
        w.write_sample((i % 100) as i16).unwrap();
    }
    w.finalize().unwrap();
}

fn tagged_wav(path: &Path, m: &Meta<'_>) {
    write_wav(path);
    let mut tag = Tag::new(TagType::Id3v2);
    if !m.title.is_empty() {
        tag.set_title(m.title.into());
    }
    if !m.artist.is_empty() {
        tag.set_artist(m.artist.into());
    }
    if !m.album.is_empty() {
        tag.set_album(m.album.into());
    }
    if !m.album_artist.is_empty() {
        tag.insert_text(ItemKey::AlbumArtist, m.album_artist.into());
    }
    if m.track > 0 {
        tag.set_track(m.track);
    }
    if m.disc > 0 {
        tag.set_disk(m.disc);
    }
    if m.year > 0 {
        tag.set_date(lofty::tag::items::Timestamp {
            year: m.year,
            ..Default::default()
        });
    }
    if let Some(data) = m.picture {
        tag.push_picture(
            Picture::unchecked(data.to_vec())
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Png)
                .build(),
        );
    }
    tag.save_to_path(path, WriteOptions::default()).unwrap();
}

struct Env {
    _tmp: TempDir,
    music: PathBuf,
    lib: Arc<Library>,
    folder_id: i64,
    changes: Arc<AtomicU64>,
}

fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    std::fs::create_dir_all(&music).unwrap();
    let changes = Arc::new(AtomicU64::new(0));
    let c = changes.clone();
    let lib = Arc::new(
        Library::open(
            &tmp.path().join("data/library.sqlite3"),
            &tmp.path().join("cache/covers"),
            Hooks {
                on_progress: Box::new(|_| {}),
                on_changed: Box::new(move |v| c.store(v, Ordering::SeqCst)),
            },
        )
        .unwrap(),
    );
    let folder_id = lib.add_folder(music.to_str().unwrap()).unwrap();
    Env {
        _tmp: tmp,
        music,
        lib,
        folder_id,
        changes,
    }
}

impl Env {
    fn scan(&self) -> ScanStats {
        let path = PathBuf::from(&self.lib.folders().unwrap()[0].path);
        scan_folder(&self.lib, self.folder_id, &path).unwrap()
    }
    fn tracks(&self, sort: SortOrder) -> Vec<super::TrackRow> {
        self.lib.tracks_page(0, 1000, sort).unwrap()
    }
}

#[test]
fn fnv1a_known_vectors() {
    assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(fnv1a(b"foobar"), 0x8594_4171_f739_67e8);
}

#[test]
fn schema_version_is_set() {
    let e = env();
    let v: i64 = e
        .lib
        .conn()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, 1);
}

#[test]
fn reads_tags_and_groups_albums() {
    let e = env();
    let m = |title, track, disc| Meta {
        title,
        artist: "Artist A",
        album: "Album One",
        album_artist: "Various",
        track,
        disc,
        year: 1999,
        ..Default::default()
    };
    tagged_wav(&e.music.join("a/1.wav"), &m("Second", 2, 1));
    tagged_wav(&e.music.join("a/2.wav"), &m("First", 1, 1));
    tagged_wav(&e.music.join("a/3.wav"), &m("Disc two", 1, 2));
    // Same album title, different album artist: a separate album.
    tagged_wav(
        &e.music.join("b/1.wav"),
        &Meta {
            title: "Other",
            artist: "Artist B",
            album: "Album One",
            ..Default::default()
        },
    );
    let stats = e.scan();
    assert_eq!(
        stats,
        ScanStats {
            scanned: 4,
            updated: 4,
            removed: 0
        }
    );

    let s = e.lib.stats().unwrap();
    assert_eq!((s.tracks, s.albums), (4, 2));

    let albums = e.lib.albums_page(0, 10).unwrap();
    assert_eq!(albums.len(), 2);
    // Sorted by album artist: "Artist B" (fallback to artist) < "Various".
    assert_eq!(albums[0].album_artist, "Artist B");
    assert_eq!(albums[1].album_artist, "Various");
    assert_eq!(albums[1].track_count, 3);
    assert_eq!(albums[1].year, Some(1999));

    let detail = e.lib.album(albums[1].id).unwrap();
    let order: Vec<_> = detail.tracks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(order, ["First", "Second", "Disc two"]);
    let t = &detail.tracks[0];
    assert_eq!(t.artist, "Artist A");
    assert_eq!(t.album, "Album One");
    assert_eq!(
        (t.track_no, t.disc_no, t.year),
        (Some(1), Some(1), Some(1999))
    );
    assert!(t.duration_ms.unwrap() >= 900 && t.duration_ms.unwrap() <= 1100);

    let queued = e.lib.album_tracks(albums[1].id).unwrap();
    assert_eq!(queued.len(), 3);
    assert!(queued[0].path.ends_with("2.wav"));
    // Tags travel with the queued track so the player can show them.
    assert_eq!(queued[0].title.as_deref(), Some(t.title.as_str()));
    assert_eq!(queued[0].artist.as_deref(), Some(t.artist.as_str()));
}

#[test]
fn untagged_file_falls_back_to_stem() {
    let e = env();
    write_wav(&e.music.join("My Song.WAV"));
    // Not a valid audio file at all: still listed.
    std::fs::write(e.music.join("broken.mp3"), b"not audio").unwrap();
    std::fs::write(e.music.join("notes.txt"), b"ignored").unwrap();
    assert_eq!(e.scan().scanned, 2);
    let tracks = e.tracks(SortOrder::Title);
    let titles: Vec<_> = tracks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["broken", "My Song"]);
    assert!(
        tracks
            .iter()
            .all(|t| t.artist.is_empty() && t.album.is_empty())
    );
    assert!(tracks.iter().all(|t| t.album_id.is_none()));
    assert_eq!(e.lib.stats().unwrap().albums, 0);
}

#[test]
fn incremental_rescan_skips_unchanged_and_removes_missing() {
    let e = env();
    for (i, name) in ["x", "y", "z"].iter().enumerate() {
        tagged_wav(
            &e.music.join(format!("{name}.wav")),
            &Meta {
                title: name,
                artist: "A",
                album: if i == 2 { "Lonely" } else { "Shared" },
                track: i as u32 + 1,
                ..Default::default()
            },
        );
    }
    assert_eq!(e.scan().updated, 3);
    let v1 = e.lib.version();

    let again = e.scan();
    assert_eq!(
        again,
        ScanStats {
            scanned: 3,
            updated: 0,
            removed: 0
        }
    );
    assert!(e.lib.version() > v1);
    assert_eq!(e.changes.load(Ordering::SeqCst), e.lib.version());

    // Change one file (different size) and delete the one in album "Lonely".
    tagged_wav(
        &e.music.join("x.wav"),
        &Meta {
            title: "x renamed",
            artist: "A",
            album: "Shared",
            track: 1,
            picture: Some(b"extra bytes to change the size of this file"),
            ..Default::default()
        },
    );
    std::fs::remove_file(e.music.join("z.wav")).unwrap();
    let third = e.scan();
    assert_eq!(
        third,
        ScanStats {
            scanned: 2,
            updated: 1,
            removed: 1
        }
    );
    let titles: Vec<_> = e
        .tracks(SortOrder::Title)
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(titles, ["x renamed", "y"]);
    let albums = e.lib.albums_page(0, 10).unwrap();
    assert_eq!(albums.len(), 1, "empty album must be deleted");
    assert_eq!(albums[0].title, "Shared");
}

#[test]
fn embedded_cover_is_written_once_and_removed_with_album() {
    let e = env();
    let png = [&[0x89u8, b'P', b'N', b'G'][..], b"fake png payload"].concat();
    tagged_wav(
        &e.music.join("1.wav"),
        &Meta {
            title: "One",
            artist: "A",
            album: "Covered",
            picture: Some(&png),
            ..Default::default()
        },
    );
    tagged_wav(
        &e.music.join("2.wav"),
        &Meta {
            title: "Two",
            artist: "A",
            album: "Covered",
            ..Default::default()
        },
    );
    e.scan();
    let album = &e.lib.albums_page(0, 10).unwrap()[0];
    let cover = PathBuf::from(album.cover.clone().expect("cover path"));
    assert_eq!(cover.parent().unwrap(), e.lib.covers_dir());
    assert_eq!(
        cover.file_name().unwrap().to_string_lossy(),
        cover_file_name("A", "Covered", "png")
    );
    assert_eq!(std::fs::read(&cover).unwrap(), png);
    assert_eq!(std::fs::read_dir(e.lib.covers_dir()).unwrap().count(), 1);

    // An existing cover file is not rewritten.
    std::fs::write(&cover, b"sentinel").unwrap();
    std::fs::remove_file(e.music.join("2.wav")).unwrap();
    tagged_wav(
        &e.music.join("3.wav"),
        &Meta {
            title: "Three",
            artist: "A",
            album: "Covered",
            picture: Some(&png),
            ..Default::default()
        },
    );
    e.scan();
    assert_eq!(std::fs::read(&cover).unwrap(), b"sentinel");

    // Removing every track removes the album and its cover file.
    std::fs::remove_file(e.music.join("1.wav")).unwrap();
    std::fs::remove_file(e.music.join("3.wav")).unwrap();
    e.scan();
    assert_eq!(e.lib.stats().unwrap().albums, 0);
    assert!(!cover.exists());
}

#[test]
fn folder_image_is_copied_as_cover() {
    let e = env();
    tagged_wav(
        &e.music.join("disc/1.wav"),
        &Meta {
            title: "One",
            artist: "A",
            album: "Folder Art",
            ..Default::default()
        },
    );
    std::fs::write(e.music.join("disc/Folder.PNG"), b"low priority").unwrap();
    std::fs::write(e.music.join("disc/COVER.jpg"), b"jpeg bytes").unwrap();
    e.scan();
    let album = &e.lib.albums_page(0, 10).unwrap()[0];
    let cover = PathBuf::from(album.cover.clone().unwrap());
    assert_eq!(cover.extension().unwrap(), "jpg");
    assert_eq!(std::fs::read(cover).unwrap(), b"jpeg bytes");
}

#[test]
fn track_sort_orders_and_paging() {
    let e = env();
    let files = [
        ("a.wav", "banana", "zed", "Alpha", 1),
        ("b.wav", "Apple", "Zed", "Alpha", 2),
        ("c.wav", "cherry", "abe", "Beta", 1),
    ];
    for (f, title, artist, album, track) in files {
        tagged_wav(
            &e.music.join(f),
            &Meta {
                title,
                artist,
                album,
                track,
                ..Default::default()
            },
        );
    }
    e.scan();
    let titles = |s| e.tracks(s).into_iter().map(|t| t.title).collect::<Vec<_>>();
    // Case-insensitive title order.
    assert_eq!(titles(SortOrder::Title), ["Apple", "banana", "cherry"]);
    // Artist, then album, disc, track.
    assert_eq!(titles(SortOrder::Artist), ["cherry", "banana", "Apple"]);
    assert_eq!(titles(SortOrder::Album), ["banana", "Apple", "cherry"]);
    assert_eq!(titles(SortOrder::Added).len(), 3);
    assert_eq!(SortOrder::parse("nonsense"), SortOrder::Artist);

    let page = e.lib.tracks_page(1, 1, SortOrder::Title).unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].title, "banana");
    assert!(
        e.lib
            .tracks_page(5, 10, SortOrder::Title)
            .unwrap()
            .is_empty()
    );
    assert_eq!(e.lib.all_tracks(SortOrder::Title).unwrap().len(), 3);
}

#[test]
fn remove_folder_cascades_and_missing_root_keeps_data() {
    let e = env();
    tagged_wav(
        &e.music.join("1.wav"),
        &Meta {
            title: "One",
            artist: "A",
            album: "Al",
            ..Default::default()
        },
    );
    e.scan();
    assert_eq!(e.lib.stats().unwrap().tracks, 1);

    // An unreachable root must not wipe the library.
    let gone = e.music.join("missing");
    assert!(scan_folder(&e.lib, e.folder_id, &gone).is_err());
    assert_eq!(e.lib.stats().unwrap().tracks, 1);

    e.lib.remove_folder(e.folder_id).unwrap();
    let s = e.lib.stats().unwrap();
    assert_eq!((s.tracks, s.albums), (0, 0));
    assert!(e.lib.folders().unwrap().is_empty());
}

#[test]
fn background_scan_runs_and_queues() {
    let e = env();
    tagged_wav(
        &e.music.join("1.wav"),
        &Meta {
            title: "One",
            artist: "A",
            album: "Al",
            ..Default::default()
        },
    );
    let folders = e.lib.folders().unwrap();
    e.lib.start_scan(folders.clone());
    e.lib.start_scan(folders); // queued or deduplicated, never concurrent
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while e.lib.is_scanning() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!e.lib.is_scanning());
    assert_eq!(e.lib.stats().unwrap().tracks, 1);
}

#[test]
fn add_folder_rejects_non_directories() {
    let e = env();
    assert!(e.lib.add_folder("/definitely/not/here").is_err());
    // Adding twice is idempotent.
    let again = e.lib.add_folder(e.music.to_str().unwrap()).unwrap();
    assert_eq!(again, e.folder_id);
    assert_eq!(e.lib.folders().unwrap().len(), 1);
}
