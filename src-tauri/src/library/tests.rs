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
    assert_eq!(v, 8);
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
fn same_title_and_artist_in_different_years_are_separate_albums() {
    let e = env();
    let png_a = [&[0x89u8, b'P', b'N', b'G'][..], b"edition 1995"].concat();
    let png_b = [&[0x89u8, b'P', b'N', b'G'][..], b"edition 2020"].concat();
    let track = |title, year, picture| Meta {
        title,
        artist: "Band",
        album: "Edition",
        album_artist: "Band",
        year,
        picture,
        ..Default::default()
    };
    tagged_wav(
        &e.music.join("y1995/1.wav"),
        &track("Old", 1995, Some(&png_a)),
    );
    tagged_wav(
        &e.music.join("y2020/1.wav"),
        &track("New 1", 2020, Some(&png_b)),
    );
    tagged_wav(&e.music.join("y2020/2.wav"), &track("New 2", 2020, None));
    tagged_wav(&e.music.join("noyear/1.wav"), &track("Undated 1", 0, None));
    tagged_wav(&e.music.join("noyear/2.wav"), &track("Undated 2", 0, None));
    e.scan();

    let albums = e.lib.albums_page(0, 10).unwrap();
    let summary: Vec<_> = albums.iter().map(|a| (a.year, a.track_count)).collect();
    assert_eq!(summary, [(None, 2), (Some(1995), 1), (Some(2020), 2)]);

    let cover_1995 = PathBuf::from(albums[1].cover.clone().expect("1995 cover"));
    let cover_2020 = PathBuf::from(albums[2].cover.clone().expect("2020 cover"));
    assert_ne!(cover_1995, cover_2020);
    assert_eq!(
        cover_1995.file_name().unwrap().to_string_lossy(),
        cover_file_name("Band", "Edition", Some(1995), "png")
    );
    assert_eq!(
        cover_2020.file_name().unwrap().to_string_lossy(),
        cover_file_name("Band", "Edition", Some(2020), "png")
    );
    assert_eq!(std::fs::read(&cover_1995).unwrap(), png_a);
    assert_eq!(std::fs::read(&cover_2020).unwrap(), png_b);

    // Removing one year's tracks leaves other editions and their covers intact.
    std::fs::remove_file(e.music.join("y1995/1.wav")).unwrap();
    e.scan();
    let remaining = e.lib.albums_page(0, 10).unwrap();
    assert_eq!(remaining.len(), 2);
    assert!(!cover_1995.exists());
    assert!(cover_2020.exists());
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
        cover_file_name("A", "Covered", None, "png")
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

#[test]
fn search_filters_tracks_albums_and_escapes_wildcards() {
    let e = env();
    tagged_wav(
        &e.music.join("1.wav"),
        &Meta {
            title: "Time",
            artist: "Pink Floyd",
            album: "Dark Side",
            track: 1,
            ..Default::default()
        },
    );
    tagged_wav(
        &e.music.join("2.wav"),
        &Meta {
            title: "Money",
            artist: "Pink Floyd",
            album: "Dark Side",
            track: 2,
            ..Default::default()
        },
    );
    tagged_wav(
        &e.music.join("3.wav"),
        &Meta {
            title: "100% Pure",
            artist: "Miles Davis",
            album: "Blue_Note",
            track: 1,
            ..Default::default()
        },
    );
    e.scan();

    // Multi-token AND query spans artist + title.
    let res = e
        .lib
        .search_tracks_page(0, 10, SortOrder::Title, "  pink  money ")
        .unwrap();
    assert_eq!(res.total, 1);
    assert_eq!(res.items[0].title, "Money");

    // Empty query returns everything with accurate total and pagination.
    let page = e
        .lib
        .search_tracks_page(1, 1, SortOrder::Title, "   ")
        .unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.items.len(), 1);

    // Escaped wildcards only match literal `%` and `_`.
    let pct = e
        .lib
        .search_tracks_page(0, 10, SortOrder::Title, "100%")
        .unwrap();
    assert_eq!(pct.total, 1);
    assert_eq!(pct.items[0].title, "100% Pure");
    let under = e
        .lib
        .search_tracks_page(0, 10, SortOrder::Title, "blue_note")
        .unwrap();
    assert_eq!(under.total, 1);
    assert_eq!(under.items[0].artist, "Miles Davis");

    // Albums search matches both album metadata and contained track titles.
    let alb = e.lib.search_albums_page(0, 10, "money").unwrap();
    assert_eq!(alb.total, 1);
    assert_eq!(alb.items[0].title, "Dark Side");

    // Filtered play list returns only matching tracks in sort order.
    let queued = e.lib.matching_tracks(SortOrder::Title, "pink").unwrap();
    let titles: Vec<_> = queued.iter().filter_map(|t| t.title.as_deref()).collect();
    assert_eq!(titles, ["Money", "Time"]);
}

// ---------------------------------------------------------------------
// Playlists
// ---------------------------------------------------------------------

/// Scans `names` as tracks of one artist; returns their ids in file-name order.
fn scanned_tracks(e: &Env, names: &[&str]) -> Vec<i64> {
    for (i, name) in names.iter().enumerate() {
        tagged_wav(
            &e.music.join(format!("{name}.wav")),
            &Meta {
                title: name,
                artist: "A",
                album: "Alb",
                track: i as u32 + 1,
                ..Default::default()
            },
        );
    }
    e.scan();
    names
        .iter()
        .map(|n| {
            e.tracks(SortOrder::Title)
                .into_iter()
                .find(|t| t.title == *n)
                .unwrap()
                .id
        })
        .collect()
}

fn playlist_titles(e: &Env, id: i64) -> Vec<String> {
    e.lib
        .playlist_tracks(id, 0, 1000)
        .unwrap()
        .into_iter()
        .map(|t| t.title)
        .collect()
}

#[test]
fn migrates_a_v1_database_to_latest() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("library.sqlite3");
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(super::db::SCHEMA_V1).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute("INSERT INTO folders(id, path) VALUES (1, '/m')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO tracks(folder_id, path, mtime, size, title, artist, added_at)
             VALUES (1, '/m/a.flac', 1, 1, 'Old', 'Artist', 1)",
            [],
        )
        .unwrap();
    }
    let lib = Library::open(&db_path, &tmp.path().join("covers"), Hooks::none()).unwrap();
    let v: i64 = lib
        .conn()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, 8);
    let tracks = lib.tracks_page(0, 10, SortOrder::Title).unwrap();
    assert_eq!(tracks.len(), 1, "existing data survives the migration");
    let pl = lib.create_playlist("Mix", &[tracks[0].id]).unwrap();
    assert_eq!(pl.track_count, 1);
    drop(lib);

    // Reopening an up-to-date database is a no-op.
    let lib = Library::open(&db_path, &tmp.path().join("covers"), Hooks::none()).unwrap();
    assert_eq!(lib.playlists().unwrap().len(), 1);
}

#[test]
fn migrates_a_v2_database_and_keeps_a_folder_album_whole() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("library.sqlite3");
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(super::db::SCHEMA_V1).unwrap();
        conn.execute_batch(super::db::SCHEMA_V2).unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
        conn.execute_batch(
            "INSERT INTO folders(id, path) VALUES (1, '/m');
             INSERT INTO albums(id, title, album_artist, year, cover)
             VALUES (1, 'Gold', 'Artist', 1995, NULL);
             INSERT INTO tracks(id, folder_id, path, mtime, size, title, artist, album_id,
                                year, added_at)
             VALUES (10, 1, '/m/a.flac', 1, 1, 'A', 'Artist', 1, 1995, 1),
                    (11, 1, '/m/b.flac', 1, 1, 'B', 'Artist', 1, 2020, 1);
             INSERT INTO playlists(id, name, created_at) VALUES (1, 'Mix', 1);
             INSERT INTO playlist_tracks(playlist_id, position, track_id)
             VALUES (1, 0, 10), (1, 1, 11);",
        )
        .unwrap();
    }
    let lib = Library::open(&db_path, &tmp.path().join("covers"), Hooks::none()).unwrap();
    {
        let conn = lib.conn();
        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 8);
        let fk_issues = conn
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query_map([], |_| Ok(()))
            .unwrap()
            .count();
        assert_eq!(fk_issues, 0);
    }
    let albums = lib.albums_page(0, 10).unwrap();
    let years: Vec<_> = albums.iter().map(|a| (a.year, a.track_count)).collect();
    // Both tracks sit in `/m`: one album, the later year on a tie.
    assert_eq!(years, [(Some(2020), 2)]);
    assert_eq!(lib.playlists().unwrap()[0].track_count, 2);
}

#[test]
fn playlist_create_rename_delete() {
    let e = env();
    let ids = scanned_tracks(&e, &["a", "b"]);
    assert!(e.lib.create_playlist("   ", &[]).is_err());

    let pl = e.lib.create_playlist("  Road trip ", &ids).unwrap();
    assert_eq!(pl.name, "Road trip");
    assert_eq!(pl.track_count, 2);
    let empty = e.lib.create_playlist("alpha", &[]).unwrap();
    assert_eq!(empty.track_count, 0);

    let list = e.lib.playlists().unwrap();
    assert_eq!(
        list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        ["alpha", "Road trip"],
        "sorted by name, case-insensitively"
    );

    e.lib.rename_playlist(pl.id, "Roadtrip").unwrap();
    assert!(e.lib.rename_playlist(pl.id, "").is_err());
    assert!(e.lib.rename_playlist(9999, "x").is_err());
    assert_eq!(e.lib.playlists().unwrap()[1].name, "Roadtrip");

    e.lib.delete_playlist(pl.id).unwrap();
    assert!(e.lib.delete_playlist(pl.id).is_err());
    assert!(e.lib.playlist_tracks(pl.id, 0, 10).is_err());
    let orphans: i64 = e
        .lib
        .conn()
        .query_row("SELECT COUNT(*) FROM playlist_tracks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(orphans, 0, "entries cascade with the playlist");
    // Deleting a playlist never touches the library.
    assert_eq!(e.tracks(SortOrder::Title).len(), 2);
}

#[test]
fn playlist_add_keeps_order_allows_duplicates_and_skips_unknown_ids() {
    let e = env();
    let ids = scanned_tracks(&e, &["a", "b", "c"]);
    let pl = e.lib.create_playlist("P", &[ids[2], ids[0]]).unwrap();
    assert_eq!(
        e.lib
            .playlist_add_tracks(pl.id, &[ids[1], 9999, ids[2]])
            .unwrap(),
        2
    );
    assert_eq!(playlist_titles(&e, pl.id), ["c", "a", "b", "c"]);
    assert!(e.lib.playlist_add_tracks(4242, &ids).is_err());

    // Paging.
    let page = e.lib.playlist_tracks(pl.id, 1, 2).unwrap();
    assert_eq!(
        page.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(e.lib.playlists().unwrap()[0].track_count, 4);

    // Queue view: same order, with tags.
    let queue = e.lib.playlist_queue_tracks(pl.id).unwrap();
    assert_eq!(queue.len(), 4);
    assert_eq!(queue[0].title.as_deref(), Some("c"));
    assert_eq!(queue[0].artist.as_deref(), Some("A"));
    assert!(queue[0].path.ends_with("c.wav"));
}

#[test]
fn playlist_add_album_appends_in_album_order() {
    let e = env();
    scanned_tracks(&e, &["a", "b", "c"]);
    let album = e.lib.albums_page(0, 10).unwrap()[0].id;
    let pl = e.lib.create_playlist("P", &[]).unwrap();
    assert_eq!(e.lib.playlist_add_album(pl.id, album).unwrap(), 3);
    assert_eq!(e.lib.playlist_add_album(pl.id, album).unwrap(), 3);
    assert_eq!(playlist_titles(&e, pl.id), ["a", "b", "c", "a", "b", "c"]);
}

#[test]
fn playlist_remove_and_move_keep_positions_compact() {
    let e = env();
    let ids = scanned_tracks(&e, &["a", "b", "c", "d", "e"]);
    let pl = e.lib.create_playlist("P", &ids).unwrap();

    e.lib.playlist_remove(pl.id, &[1, 3, 3, 99]).unwrap();
    assert_eq!(playlist_titles(&e, pl.id), ["a", "c", "e"]);

    e.lib.playlist_move(pl.id, 0, 2).unwrap();
    assert_eq!(playlist_titles(&e, pl.id), ["c", "e", "a"]);
    e.lib.playlist_move(pl.id, 2, 0).unwrap();
    assert_eq!(playlist_titles(&e, pl.id), ["a", "c", "e"]);
    e.lib.playlist_move(pl.id, 1, 1).unwrap();
    assert_eq!(playlist_titles(&e, pl.id), ["a", "c", "e"]);
    assert!(e.lib.playlist_move(pl.id, 0, 3).is_err());
    assert!(e.lib.playlist_move(pl.id, 3, 0).is_err());

    let positions: Vec<i64> = {
        let conn = e.lib.conn();
        let mut st = conn
            .prepare(
                "SELECT position FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position",
            )
            .unwrap();
        st.query_map([pl.id], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(positions, [0, 1, 2]);

    // Appending after removal continues the order.
    e.lib.playlist_add_tracks(pl.id, &[ids[1]]).unwrap();
    assert_eq!(playlist_titles(&e, pl.id), ["a", "c", "e", "b"]);
}

#[test]
fn playlist_survives_rescan_and_loses_deleted_tracks() {
    let e = env();
    let ids = scanned_tracks(&e, &["a", "b", "c", "d"]);
    let pl = e
        .lib
        .create_playlist("P", &[ids[3], ids[1], ids[0], ids[2]])
        .unwrap();

    // Rescan with one file modified (different size): ids must not change.
    tagged_wav(
        &e.music.join("b.wav"),
        &Meta {
            title: "b",
            artist: "A",
            album: "Alb",
            track: 2,
            picture: Some(b"extra bytes to change the size of this file"),
            ..Default::default()
        },
    );
    let stats = e.scan();
    assert_eq!(stats.updated, 1);
    let after: Vec<i64> = ["a", "b", "c", "d"]
        .iter()
        .map(|n| {
            e.tracks(SortOrder::Title)
                .into_iter()
                .find(|t| t.title == *n)
                .unwrap()
                .id
        })
        .collect();
    assert_eq!(after, ids, "track ids are stable across a rescan");
    assert_eq!(playlist_titles(&e, pl.id), ["d", "b", "a", "c"]);

    // A file that disappears drops out of the playlist, leaving a gap.
    std::fs::remove_file(e.music.join("b.wav")).unwrap();
    assert_eq!(e.scan().removed, 1);
    assert_eq!(playlist_titles(&e, pl.id), ["d", "a", "c"]);
    assert_eq!(e.lib.playlists().unwrap()[0].track_count, 3);

    // Index-based edits still work across the gap.
    e.lib.playlist_move(pl.id, 0, 2).unwrap();
    assert_eq!(playlist_titles(&e, pl.id), ["a", "c", "d"]);
    e.lib.playlist_remove(pl.id, &[1]).unwrap();
    assert_eq!(playlist_titles(&e, pl.id), ["a", "d"]);
}

#[test]
fn removing_a_library_folder_cascades_into_playlists() {
    let e = env();
    let ids = scanned_tracks(&e, &["a", "b"]);
    let pl = e.lib.create_playlist("P", &ids).unwrap();
    e.lib.remove_folder(e.folder_id).unwrap();
    assert_eq!(e.lib.playlists().unwrap()[0].track_count, 0);
    assert!(e.lib.playlist_tracks(pl.id, 0, 10).unwrap().is_empty());
}

#[test]
fn playlist_from_paths_maps_library_files_and_counts_skipped() {
    let e = env();
    let ids = scanned_tracks(&e, &["a", "b"]);
    let paths = vec![
        e.music.join("b.wav").to_string_lossy().into_owned(),
        "/not/in/library.mp3".to_string(),
        e.music.join("a.wav").to_string_lossy().into_owned(),
        e.music.join("b.wav").to_string_lossy().into_owned(),
    ];
    let saved = e.lib.playlist_from_paths("From queue", &paths).unwrap();
    assert_eq!(saved.skipped, 1);
    assert_eq!(saved.playlist.track_count, 3);
    assert_eq!(playlist_titles(&e, saved.playlist.id), ["b", "a", "b"]);
    assert_eq!(ids.len(), 2);

    assert!(e.lib.playlist_from_paths(" ", &paths).is_err());
    let none = e.lib.playlist_from_paths("Empty", &[]).unwrap();
    assert_eq!((none.skipped, none.playlist.track_count), (0, 0));
}

// ---------------------------------------------------------------------
// Tag editing
// ---------------------------------------------------------------------

mod tag_editing {
    use super::*;
    use crate::library::tags::{
        EditableTags, PLAYING_ERROR, TagChanges, common_tags, is_playing_path, read_tags,
        write_for_tracks, write_tags,
    };

    fn set(s: &str) -> Option<Option<String>> {
        Some(Some(s.to_string()))
    }

    fn everything() -> TagChanges {
        TagChanges {
            title: set("New Title"),
            artist: set("New Artist"),
            album: set("New Album"),
            album_artist: set("New Album Artist"),
            track_no: Some(Some(7)),
            disc_no: Some(Some(2)),
            year: Some(Some(1999)),
            genre: set("Jazz"),
        }
    }

    fn expected_everything() -> EditableTags {
        EditableTags {
            title: Some("New Title".into()),
            artist: Some("New Artist".into()),
            album: Some("New Album".into()),
            album_artist: Some("New Album Artist".into()),
            track_no: Some(7),
            disc_no: Some(2),
            year: Some(1999),
            genre: Some("Jazz".into()),
        }
    }

    /// A minimal MPEG-1 Layer III file (silent frames, no tag).
    fn write_mp3(path: &Path) {
        // 0xFFFB9004 = MPEG1 L3, 128 kbps, 44.1 kHz, no padding; 417-byte frames.
        let mut frame = vec![0u8; 417];
        frame[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x04]);
        let data: Vec<u8> = (0..8).flat_map(|_| frame.clone()).collect();
        std::fs::write(path, data).unwrap();
    }

    /// A FLAC file with only a STREAMINFO block and no tag.
    fn write_flac(path: &Path) {
        let mut data = b"fLaC".to_vec();
        data.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]); // last block, STREAMINFO, 34 bytes
        data.extend_from_slice(&[0x10, 0x00, 0x10, 0x00]); // min/max block size
        data.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // min/max frame size
        // 44100 Hz, 2 channels, 16 bits, 44100 samples
        data.extend_from_slice(&[0x0A, 0xC4, 0x42, 0xF0, 0x00, 0x00, 0xAC, 0x44]);
        data.extend_from_slice(&[0u8; 16]); // md5
        std::fs::write(path, data).unwrap();
    }

    #[test]
    fn wav_roundtrip_changes_every_field() {
        let e = env();
        let p = e.music.join("a.wav");
        tagged_wav(
            &p,
            &Meta {
                title: "Old",
                artist: "Old Artist",
                album: "Old Album",
                track: 1,
                year: 1980,
                ..Meta::default()
            },
        );
        write_tags(&p, &everything()).unwrap();
        assert_eq!(read_tags(&p).unwrap(), expected_everything());
        let leftovers: Vec<_> = std::fs::read_dir(&e.music).unwrap().flatten().collect();
        assert_eq!(leftovers.len(), 1, "no temporary file may remain");
    }

    #[test]
    fn mp3_and_flac_without_a_tag_get_a_primary_tag() {
        let e = env();
        for (name, make) in [
            ("a.mp3", write_mp3 as fn(&Path)),
            ("a.flac", write_flac as fn(&Path)),
        ] {
            let p = e.music.join(name);
            make(&p);
            assert_eq!(read_tags(&p).unwrap(), EditableTags::default(), "{name}");
            write_tags(&p, &everything()).unwrap();
            assert_eq!(read_tags(&p).unwrap(), expected_everything(), "{name}");
        }
    }

    #[test]
    fn only_edited_fields_change_and_clear_removes() {
        let e = env();
        let p = e.music.join("a.wav");
        tagged_wav(
            &p,
            &Meta {
                title: "Keep Me",
                artist: "Artist",
                album: "Album",
                album_artist: "Album Artist",
                track: 3,
                disc: 1,
                year: 2001,
                ..Meta::default()
            },
        );
        let before = read_tags(&p).unwrap();
        let change = TagChanges {
            genre: set("Rock"),
            artist: Some(None),
            ..TagChanges::default()
        };
        write_tags(&p, &change).unwrap();
        let after = read_tags(&p).unwrap();
        assert_eq!(
            after,
            EditableTags {
                artist: None,
                genre: Some("Rock".into()),
                ..before
            }
        );
    }

    #[test]
    fn other_items_and_pictures_survive_an_edit() {
        let e = env();
        let p = e.music.join("a.wav");
        tagged_wav(
            &p,
            &Meta {
                title: "T",
                album: "A",
                picture: Some(b"\x89PNG-cover"),
                ..Meta::default()
            },
        );
        // Add an item the editor does not know about.
        let mut tagged = lofty::read_from_path(&p).unwrap();
        tagged
            .primary_tag_mut()
            .unwrap()
            .insert_text(ItemKey::Composer, "Some Composer".into());
        tagged.save_to_path(&p, WriteOptions::default()).unwrap();

        write_tags(
            &p,
            &TagChanges {
                title: set("T2"),
                ..TagChanges::default()
            },
        )
        .unwrap();

        let tagged = lofty::read_from_path(&p).unwrap();
        let tag = tagged.primary_tag().unwrap();
        assert_eq!(tag.title().as_deref(), Some("T2"));
        assert_eq!(tag.get_string(ItemKey::Composer), Some("Some Composer"));
        assert_eq!(tag.pictures().len(), 1);
        assert_eq!(tag.pictures()[0].data(), b"\x89PNG-cover");
    }

    #[test]
    fn invalid_year_and_unreadable_files_are_errors_and_leave_the_file() {
        let e = env();
        let p = e.music.join("a.wav");
        tagged_wav(
            &p,
            &Meta {
                title: "T",
                ..Meta::default()
            },
        );
        let bytes = std::fs::read(&p).unwrap();
        let bad_year = TagChanges {
            year: Some(Some(0)),
            ..TagChanges::default()
        };
        assert!(write_tags(&p, &bad_year).is_err());
        assert_eq!(std::fs::read(&p).unwrap(), bytes);

        let junk = e.music.join("junk.mp3");
        std::fs::write(&junk, b"definitely not audio").unwrap();
        assert!(write_tags(&junk, &everything()).is_err());
        assert_eq!(std::fs::read(&junk).unwrap(), b"definitely not audio");
        assert_eq!(std::fs::read_dir(&e.music).unwrap().count(), 2);
    }

    #[test]
    fn changes_json_distinguishes_keep_clear_and_set() {
        let c: TagChanges =
            serde_json::from_str(r#"{"title":"X","artist":null,"trackNo":5,"year":null}"#).unwrap();
        assert_eq!(c.title, set("X"));
        assert_eq!(c.artist, Some(None));
        assert_eq!(c.track_no, Some(Some(5)));
        assert_eq!(c.year, Some(None));
        assert_eq!(c.album, None);
        assert_eq!(c.genre, None);
        assert!(TagChanges::default().is_empty());
        assert!(!c.is_empty());
        // Blank text clears; text is trimmed.
        let v = TagChanges {
            title: set("  Hi  "),
            album: set("   "),
            ..TagChanges::default()
        }
        .validated()
        .unwrap();
        assert_eq!(v.title, set("Hi"));
        assert_eq!(v.album, Some(None));
    }

    #[test]
    fn common_tags_reports_mixed_fields() {
        let a = EditableTags {
            title: Some("A".into()),
            album: Some("Same".into()),
            year: Some(2000),
            ..EditableTags::default()
        };
        let b = EditableTags {
            title: Some("B".into()),
            album: Some("Same".into()),
            year: Some(2000),
            ..EditableTags::default()
        };
        let c = common_tags(&[a, b]);
        assert_eq!(c.values.album.as_deref(), Some("Same"));
        assert_eq!(c.values.year, Some(2000));
        assert_eq!(c.values.title, None);
        assert_eq!(c.mixed, ["title"]);
        assert_eq!(common_tags(&[]), Default::default());
    }

    #[test]
    fn playing_path_check() {
        assert!(is_playing_path("/m/a.flac", Some("/m/a.flac")));
        assert!(!is_playing_path("/m/a.flac", Some("/m/b.flac")));
        assert!(!is_playing_path("/m/a.flac", None));
        // Different spellings of the same existing file.
        let e = env();
        let p = e.music.join("a.wav");
        write_wav(&p);
        let dotted = e.music.join("sub/../a.wav");
        std::fs::create_dir_all(e.music.join("sub")).unwrap();
        assert!(is_playing_path(
            p.to_str().unwrap(),
            Some(dotted.to_str().unwrap())
        ));
    }

    #[test]
    fn multi_file_edit_updates_the_database_and_regroups_albums() {
        let e = env();
        for (name, title, artist, track) in
            [("a", "One", "Artist A", 1), ("b", "Two", "Artist B", 2)]
        {
            tagged_wav(
                &e.music.join(format!("{name}.wav")),
                &Meta {
                    title,
                    artist,
                    album: "Old Album",
                    album_artist: "Band",
                    track,
                    year: 1990,
                    ..Meta::default()
                },
            );
        }
        e.scan();
        let ids: Vec<i64> = e.tracks(SortOrder::Title).iter().map(|t| t.id).collect();
        let before = e.changes.load(Ordering::SeqCst);

        let out = write_for_tracks(
            &e.lib,
            &ids,
            &TagChanges {
                album: set("Renamed Album"),
                year: Some(Some(2010)),
                ..TagChanges::default()
            },
            None,
        )
        .unwrap();
        assert_eq!(out.updated.len(), 2);
        assert!(out.failed.is_empty());
        assert!(
            e.changes.load(Ordering::SeqCst) > before,
            "library version must bump"
        );

        let rows = e.tracks(SortOrder::Title);
        // Titles and artists differ per file and must not be overwritten.
        assert_eq!(
            rows.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
            ["One", "Two"]
        );
        assert_eq!(
            rows.iter().map(|t| t.artist.as_str()).collect::<Vec<_>>(),
            ["Artist A", "Artist B"]
        );
        assert!(
            rows.iter()
                .all(|t| t.album == "Renamed Album" && t.year == Some(2010))
        );
        let albums = e.lib.albums_page(0, 10).unwrap();
        assert_eq!(albums.len(), 1, "the old, now empty album is removed");
        assert_eq!(albums[0].title, "Renamed Album");
        assert_eq!(albums[0].year, Some(2010));
        assert_eq!(albums[0].track_count, 2);
    }

    #[test]
    fn editing_a_track_year_keeps_the_album_whole_and_updates_its_year() {
        let e = env();
        for (name, title, track) in [("a", "One", 1), ("b", "Two", 2), ("c", "Three", 3)] {
            tagged_wav(
                &e.music.join(format!("{name}.wav")),
                &Meta {
                    title,
                    artist: "Band",
                    album: "Chronicle",
                    album_artist: "Band",
                    track,
                    year: 1990,
                    ..Meta::default()
                },
            );
        }
        e.scan();
        let albums = |e: &Env| -> Vec<_> {
            e.lib
                .albums_page(0, 10)
                .unwrap()
                .into_iter()
                .map(|a| (a.year, a.track_count))
                .collect()
        };
        let ids: Vec<i64> = e.tracks(SortOrder::Title).iter().map(|t| t.id).collect();
        let set_year = |ids: &[i64], year| {
            write_for_tracks(
                &e.lib,
                ids,
                &TagChanges {
                    year: Some(Some(year)),
                    ..TagChanges::default()
                },
                None,
            )
            .unwrap();
        };

        // One mistyped year does not split the album; the common year wins.
        set_year(&ids[..1], 2010);
        assert_eq!(albums(&e), [(Some(1990), 3)]);

        // Retagging the whole album moves its year.
        set_year(&ids, 2010);
        assert_eq!(albums(&e), [(Some(2010), 3)]);
    }

    #[test]
    fn playing_file_is_refused_and_left_untouched() {
        let e = env();
        for name in ["a", "b"] {
            tagged_wav(
                &e.music.join(format!("{name}.wav")),
                &Meta {
                    title: name,
                    ..Meta::default()
                },
            );
        }
        e.scan();
        let rows = e.tracks(SortOrder::Title);
        let ids: Vec<i64> = rows.iter().map(|t| t.id).collect();
        let playing = rows[0].path.clone();
        let bytes = std::fs::read(&playing).unwrap();

        let out = write_for_tracks(
            &e.lib,
            &ids,
            &TagChanges {
                genre: set("Pop"),
                ..TagChanges::default()
            },
            Some(&playing),
        )
        .unwrap();
        assert_eq!(out.updated, [rows[1].path.clone()]);
        assert_eq!(out.failed.len(), 1);
        assert_eq!(out.failed[0].path, playing);
        assert_eq!(out.failed[0].error, PLAYING_ERROR);
        assert_eq!(std::fs::read(&playing).unwrap(), bytes);
    }

    #[test]
    fn failed_write_does_not_touch_the_database() {
        let e = env();
        tagged_wav(
            &e.music.join("good.wav"),
            &Meta {
                title: "Good",
                ..Meta::default()
            },
        );
        // Scanned as a track (by extension) but not parseable audio.
        std::fs::write(e.music.join("bad.mp3"), b"not audio at all").unwrap();
        e.scan();
        let ids: Vec<i64> = e.tracks(SortOrder::Title).iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), 2);

        let out = write_for_tracks(
            &e.lib,
            &ids,
            &TagChanges {
                title: set("Edited"),
                ..TagChanges::default()
            },
            None,
        )
        .unwrap();
        assert_eq!(out.updated.len(), 1);
        assert_eq!(out.failed.len(), 1);
        assert!(out.failed[0].path.ends_with("bad.mp3"));
        let mut titles: Vec<String> = e
            .tracks(SortOrder::Title)
            .into_iter()
            .map(|t| t.title)
            .collect();
        titles.sort();
        assert_eq!(titles, ["Edited", "bad"]);
    }
}

#[test]
fn m3u_export_and_import_into_another_library() {
    let pc = env();
    let ids = scanned_tracks(&pc, &["One", "Two", "Three"]);
    let list = pc.lib.create_playlist("Trip", &[ids[2], ids[0]]).unwrap();
    let text = pc.lib.export_m3u(list.id).unwrap();
    assert!(text.starts_with("#EXTM3U\n#EXTINF:"), "{text}");

    // The "phone": same files under a different root, plus one song the
    // playlist does not know.
    let phone = env();
    scanned_tracks(&phone, &["One", "Two", "Three", "Four"]);
    let imported = phone
        .lib
        .import_m3u("Trip", &(text + "#EXTINF:5,Z - Gone\n/x/gone.wav\n"), None)
        .unwrap();
    assert_eq!(imported.matched, 2);
    assert_eq!(imported.missing, 1);
    assert_eq!(imported.playlist.track_count, 2);
    assert_eq!(
        playlist_titles(&phone, imported.playlist.id),
        ["Three", "One"]
    );
}

#[test]
fn m3u_relative_paths_use_the_playlist_folder() {
    let e = env();
    scanned_tracks(&e, &["Solo"]);
    let imported = e
        .lib
        .import_m3u("Rel", "Solo.wav\n", Some(&e.music))
        .unwrap();
    assert_eq!(imported.matched, 1);
}

#[test]
fn favorites_toggle_paging_and_cascade() {
    let e = env();
    scanned_tracks(&e, &["Alpha", "Beta", "Gamma"]);
    let tracks = e.lib.tracks_page(0, 10, SortOrder::Title).unwrap();
    let a = &tracks[0];
    let b = &tracks[1];

    assert!(e.lib.toggle_favorite(a.id).unwrap());
    assert!(e.lib.toggle_favorite_by_path(&b.path).unwrap());
    assert_eq!(e.lib.favorites().unwrap().len(), 2);

    let page = e.lib.favorites_page(0, 10).unwrap();
    assert_eq!(page.total, 2);
    let q = e.lib.favorites_queue_tracks().unwrap();
    assert_eq!(q.len(), 2);

    assert!(!e.lib.toggle_favorite(a.id).unwrap());
    assert_eq!(e.lib.favorites().unwrap().len(), 1);

    e.lib.remove_folder(e.lib.folders().unwrap()[0].id).unwrap();
    assert_eq!(e.lib.favorites().unwrap().len(), 0);
}

#[test]
fn playlist_cover_file_preset_and_cleanup() {
    let e = env();
    let pl = e.lib.create_playlist("Mix", &[]).unwrap();
    assert_eq!(pl.cover, None);
    assert!(pl.album_covers.is_empty());

    let src = e._tmp.path().join("cover.png");
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([20, 140, 200, 255]));
    img.save(&src).unwrap();

    e.lib.set_playlist_cover_file(pl.id, &src).unwrap();
    let custom_path = e.lib.playlists().unwrap()[0].cover.clone().unwrap();
    assert!(std::path::Path::new(&custom_path).exists());

    e.lib
        .set_playlist_cover_preset(pl.id, "preset:teal:star")
        .unwrap();
    assert_eq!(
        e.lib.playlists().unwrap()[0].cover.as_deref(),
        Some("preset:teal:star")
    );
    assert!(
        !std::path::Path::new(&custom_path).exists(),
        "old custom cover file should be removed when replaced"
    );

    e.lib.clear_playlist_cover(pl.id).unwrap();
    assert_eq!(e.lib.playlists().unwrap()[0].cover, None);
}

#[test]
fn listening_history_aggregates_and_survives_track_removal() {
    let e = env();
    scanned_tracks(&e, &["Alpha", "Beta"]);
    let tracks = e.lib.tracks_page(0, 10, SortOrder::Title).unwrap();
    let alpha = &tracks[0];
    let beta = &tracks[1];

    let day1 = 1_700_000_000i64;
    let day2 = day1 + 86_400;
    let day3 = day2 + 86_400;

    e.lib
        .record_listen_event(super::NewListenEvent {
            path: alpha.path.clone(),
            title: alpha.title.clone(),
            artist: alpha.artist.clone(),
            duration_ms: alpha.duration_ms,
            started_at: day1,
            ended_at: day1 + 180,
            listened_ms: 180_000,
            completed: true,
            skipped: false,
        })
        .unwrap();
    e.lib
        .record_listen_event(super::NewListenEvent {
            path: alpha.path.clone(),
            title: alpha.title.clone(),
            artist: alpha.artist.clone(),
            duration_ms: alpha.duration_ms,
            started_at: day2,
            ended_at: day2 + 12,
            listened_ms: 12_000,
            completed: false,
            skipped: true,
        })
        .unwrap();
    e.lib
        .record_listen_event(super::NewListenEvent {
            path: "https://example.org/stream/live".into(),
            title: "Stream Track".into(),
            artist: "Online Artist".into(),
            duration_ms: Some(210_000),
            started_at: day3,
            ended_at: day3 + 200,
            listened_ms: 200_000,
            completed: true,
            skipped: false,
        })
        .unwrap();
    e.lib
        .record_listen_event(super::NewListenEvent {
            path: beta.path.clone(),
            title: beta.title.clone(),
            artist: beta.artist.clone(),
            duration_ms: beta.duration_ms,
            started_at: day3 + 500,
            ended_at: day3 + 560,
            listened_ms: 60_000,
            completed: true,
            skipped: false,
        })
        .unwrap();

    // Full range: 4 raw events, 3 valid plays (>= 15s or completed), 1 skip, 3 consecutive days.
    let all = e.lib.listening_stats(None, None).unwrap();
    assert_eq!(all.summary.raw_event_count, 4);
    assert_eq!(all.summary.play_count, 3);
    assert_eq!(all.summary.skip_count, 1);
    assert_eq!(all.summary.completed_count, 3);
    assert_eq!(all.summary.total_listened_ms, 452_000);
    assert_eq!(all.summary.longest_streak_days, 3);
    assert_eq!(all.summary.active_days, 3);
    assert_eq!(all.recent[0].source, "local");
    assert_eq!(all.recent[1].source, "online");

    // Date range filter `[day2, day3)` excludes day1 and day3.
    let mid = e.lib.listening_stats(Some(day2), Some(day3)).unwrap();
    assert_eq!(mid.summary.raw_event_count, 1);
    assert_eq!(mid.summary.play_count, 0);
    assert_eq!(mid.summary.skip_count, 1);
    assert_eq!(mid.summary.total_listened_ms, 12_000);

    // Removing the music folder deletes `tracks`, but listen_events keep snapshots.
    e.lib.remove_folder(e.lib.folders().unwrap()[0].id).unwrap();
    let after_rm = e.lib.listening_stats(None, None).unwrap();
    assert_eq!(after_rm.summary.raw_event_count, 4);
    assert_eq!(after_rm.summary.total_listened_ms, 452_000);

    assert_eq!(e.lib.clear_listen_history().unwrap(), 4);
    assert_eq!(
        e.lib
            .listening_stats(None, None)
            .unwrap()
            .summary
            .raw_event_count,
        0
    );
}

#[test]
fn artist_splitting_profiles_and_meta_cache() {
    assert_eq!(
        super::db::split_artists("Simon & Garfunkel"),
        vec!["Simon & Garfunkel"]
    );
    assert_eq!(super::db::split_artists("AC/DC"), vec!["AC/DC"]);
    assert_eq!(
        super::db::split_artists("Daft Punk (feat. Pharrell Williams), Nile Rodgers"),
        vec!["Daft Punk", "Pharrell Williams", "Nile Rodgers"]
    );

    let e = env();
    tagged_wav(
        &e.music.join("track1.wav"),
        &Meta {
            title: "Collab Song",
            artist: "Artist One feat. Artist Two",
            album: "Album One",
            track: 1,
            ..Default::default()
        },
    );
    e.scan();

    let page = e.lib.search_artists_page(0, 20, "").unwrap();
    let names: Vec<String> = page.items.into_iter().map(|a| a.name).collect();
    assert!(names.contains(&"Artist One".to_string()));
    assert!(names.contains(&"Artist Two".to_string()));

    let prof = e.lib.artist_profile("Artist Two").unwrap();
    assert_eq!(prof.track_count, 1);
    assert_eq!(prof.album_count, 1);
    assert_eq!(prof.tracks[0].title, "Collab Song");

    let meta = e
        .lib
        .save_artist_meta("Artist Two", Some("/cache/a2.jpg"), Some("Turkish singer."))
        .unwrap();
    assert_eq!(meta.image.as_deref(), Some("/cache/a2.jpg"));
    assert_eq!(meta.bio.as_deref(), Some("Turkish singer."));
    assert_eq!(
        e.lib.artist_profile("Artist Two").unwrap().image.as_deref(),
        Some("/cache/a2.jpg")
    );
}

#[test]
fn artist_splitting_matches_the_shared_cases() {
    let raw = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fixtures/artist-split-cases.json"
    ));
    let doc: serde_json::Value = serde_json::from_str(raw).unwrap();
    let cases = doc["cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let input = case[0].as_str().unwrap();
        let expected: Vec<String> = serde_json::from_value(case[1].clone()).unwrap();
        assert_eq!(
            super::db::split_artists(input),
            expected,
            "input: {input:?}"
        );
    }
}

#[test]
fn album_year_is_picked_by_majority_then_latest() {
    use super::db::pick_album_year;
    assert_eq!(pick_album_year([]), None);
    assert_eq!(pick_album_year([None, None]), None);
    assert_eq!(pick_album_year([None, Some(1994), None]), Some(1994));
    assert_eq!(
        pick_album_year([Some(1994), Some(1994), Some(1987)]),
        Some(1994)
    );
    assert_eq!(
        pick_album_year([Some(1975), Some(1983), Some(1991)]),
        Some(1991)
    );
}

#[test]
fn a_folder_album_stays_whole_with_mixed_or_missing_years() {
    let e = env();
    let track = |title, track, year| Meta {
        title,
        artist: "Queen",
        album: "Greatest Hits",
        album_artist: "Queen",
        track,
        year,
        ..Default::default()
    };
    tagged_wav(&e.music.join("gh/1.wav"), &track("One", 1, 1975));
    tagged_wav(&e.music.join("gh/2.wav"), &track("Two", 2, 1981));
    tagged_wav(&e.music.join("gh/3.wav"), &track("Three", 3, 1981));
    tagged_wav(&e.music.join("gh/4.wav"), &track("Four", 4, 0));
    e.scan();
    let albums = |e: &Env| -> Vec<_> {
        e.lib
            .albums_page(0, 10)
            .unwrap()
            .into_iter()
            .map(|a| (a.year, a.track_count))
            .collect()
    };
    assert_eq!(albums(&e), [(Some(1981), 4)]);

    // A file added later with yet another year joins the same album.
    tagged_wav(&e.music.join("gh/5.wav"), &track("Five", 5, 1991));
    e.scan();
    assert_eq!(albums(&e), [(Some(1981), 5)]);

    // Removing tracks re-derives the year of what is left.
    for n in [2, 3] {
        std::fs::remove_file(e.music.join(format!("gh/{n}.wav"))).unwrap();
    }
    e.scan();
    assert_eq!(albums(&e), [(Some(1991), 3)]);
}

#[test]
fn disc_folders_of_one_album_share_the_album() {
    let e = env();
    for (dir, title, disc) in [("cd1", "A", 1), ("cd2", "B", 2)] {
        tagged_wav(
            &e.music.join(format!("Album/{dir}/1.wav")),
            &Meta {
                title,
                artist: "Band",
                album: "Double",
                album_artist: "Band",
                track: 1,
                disc,
                year: 2001,
                ..Default::default()
            },
        );
    }
    e.scan();
    let albums = e.lib.albums_page(0, 10).unwrap();
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].track_count, 2);
}

#[test]
fn migrating_to_v8_merges_year_split_albums_and_resplits_artists() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("library.sqlite3");
    let covers = tmp.path().join("covers");
    std::fs::create_dir_all(&covers).unwrap();
    let stray_cover = covers.join("stray.jpg");
    std::fs::write(&stray_cover, b"jpg").unwrap();
    {
        let lib = Library::open(&db_path, &covers, Hooks::none()).unwrap();
        let conn = lib.conn();
        // The state a v7 database is in after the v3 split: one folder's
        // tracks spread over one album per track year, and track artists
        // mangled by the old splitter.
        conn.execute_batch(&format!(
            "INSERT INTO folders(id, path) VALUES (1, '/m');
             INSERT INTO albums(id, title, album_artist, year, cover)
             VALUES (1, 'Gold', 'İbrahim Tatlıses', 1994, NULL),
                    (2, 'Gold', 'İbrahim Tatlıses', NULL, '{}'),
                    (3, 'Gold', 'İbrahim Tatlıses', 1987, NULL);
             INSERT INTO tracks(id, folder_id, path, mtime, size, title, artist, album_id,
                                year, added_at)
             VALUES (10, 1, '/m/gold/a.flac', 1, 1, 'A', 'İbrahim Tatlıses feat. Sibel Can',
                     1, 1994, 1),
                    (11, 1, '/m/gold/b.flac', 1, 1, 'B', 'İbrahim Tatlıses', 1, 1994, 1),
                    (12, 1, '/m/gold/c.flac', 1, 1, 'C', 'İbrahim Tatlıses', 2, NULL, 1),
                    (13, 1, '/m/gold/d.flac', 1, 1, 'D', 'İbrahim Tatlıses', 3, 1987, 1);
             DELETE FROM track_artists;
             INSERT INTO track_artists(track_id, artist, position)
             VALUES (10, 'İbrahim Tatlıses', 0), (10, 'ibel Can', 1);
             PRAGMA user_version = 7;",
            stray_cover.display()
        ))
        .unwrap();
    }
    let lib = Library::open(&db_path, &covers, Hooks::none()).unwrap();
    let v: i64 = lib
        .conn()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, 8);
    let albums = lib.albums_page(0, 10).unwrap();
    assert_eq!(albums.len(), 1);
    assert_eq!((albums[0].year, albums[0].track_count), (Some(1994), 4));
    // The surviving album adopted the only cover, so the file is kept.
    assert_eq!(
        albums[0].cover.as_deref(),
        Some(stray_cover.to_string_lossy().as_ref())
    );
    assert!(stray_cover.exists());

    let names: Vec<String> = lib
        .search_artists_page(0, 20, "")
        .unwrap()
        .items
        .into_iter()
        .map(|a| a.name)
        .collect();
    assert!(names.contains(&"Sibel Can".to_string()), "{names:?}");
    assert!(!names.contains(&"ibel Can".to_string()), "{names:?}");
}
