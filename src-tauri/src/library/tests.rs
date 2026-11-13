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
    assert_eq!(v, 2);
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
fn migrates_a_v1_database_to_v2() {
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
    assert_eq!(v, 2);
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
