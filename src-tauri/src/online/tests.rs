use super::*;
use hound::{SampleFormat, WavSpec, WavWriter};
use lofty::file::TaggedFileExt;
use lofty::picture::PictureType;
use lofty::prelude::{Accessor, ItemKey};

#[test]
fn parses_itunes_album_search_and_upgrades_artwork() {
    let json = r#"{
        "resultCount": 2,
        "results": [
            {
                "wrapperType": "collection",
                "collectionId": 42,
                "collectionName": "Discovery",
                "artistName": "Daft Punk",
                "trackCount": 14,
                "releaseDate": "2001-03-12T08:00:00Z",
                "artworkUrl100": "https://example.com/100x100bb.jpg"
            },
            {
                "wrapperType": "collection",
                "collectionId": 43,
                "collectionName": "  "
            }
        ]
    }"#;
    let albums = parse_itunes_albums(json).unwrap();
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].id, "itunes:42");
    assert_eq!(albums[0].title, "Discovery");
    assert_eq!(albums[0].artist, "Daft Punk");
    assert_eq!(albums[0].year, Some(2001));
    assert_eq!(albums[0].track_count, 14);
    assert_eq!(
        albums[0].cover_url.as_deref(),
        Some("https://example.com/600x600bb.jpg")
    );
}

#[test]
fn parses_itunes_album_lookup_and_sorts_discs_and_tracks() {
    let fallback = OnlineAlbum {
        id: "itunes:99".into(),
        title: "Fallback".into(),
        artist: "Band".into(),
        year: None,
        track_count: 0,
        cover: None,
        cover_url: None,
        source_url: None,
    };
    let json = r#"{
        "results": [
            {
                "wrapperType": "collection",
                "collectionId": 99,
                "collectionName": "Real Album",
                "artistName": "Album Artist",
                "releaseDate": "2015-06-01T00:00:00Z",
                "trackCount": 2,
                "artworkUrl100": "https://cdn/100x100bb.jpg"
            },
            {
                "wrapperType": "track",
                "trackId": 200,
                "trackName": "Disc Two",
                "artistName": "Guest",
                "discNumber": 2,
                "trackNumber": 1,
                "trackTimeMillis": 185000
            },
            {
                "wrapperType": "track",
                "trackId": 100,
                "trackName": "Opening",
                "artistName": "Album Artist",
                "discNumber": 1,
                "trackNumber": 1,
                "trackTimeMillis": 210500
            }
        ]
    }"#;
    let detail = parse_itunes_album_detail(json, &fallback).unwrap();
    assert_eq!(detail.album.title, "Real Album");
    assert_eq!(detail.album.artist, "Album Artist");
    assert_eq!(detail.album.year, Some(2015));
    assert_eq!(detail.tracks.len(), 2);
    assert_eq!(detail.tracks[0].title, "Opening");
    assert_eq!(detail.tracks[0].duration_seconds, Some(210.5));
    assert_eq!(detail.tracks[1].title, "Disc Two");
    assert_eq!(detail.tracks[1].artist, "Guest");
    assert_eq!(detail.tracks[1].album_artist, "Album Artist");
}

#[test]
fn parses_ytdlp_flat_search_and_playlist_json() {
    let search_json = r#"{
        "_type": "playlist",
        "title": "ytsearch12:duman",
        "entries": [
            {
                "id": "abc123",
                "title": "Seni Kendime Sakladim",
                "uploader": "Duman - Topic",
                "duration": 244.0,
                "thumbnails": [{"url": "https://img/hq.jpg"}]
            }
        ]
    }"#;
    let res = parse_ytdlp_flat_json(search_json, None).unwrap();
    assert!(res.albums.is_empty());
    assert_eq!(res.tracks.len(), 1);
    assert_eq!(res.tracks[0].artist, "Duman");
    assert_eq!(
        res.tracks[0].source_url.as_deref(),
        Some("https://www.youtube.com/watch?v=abc123")
    );

    let playlist_url = "https://youtube.com/playlist?list=PL1";
    let playlist_json = r#"{
        "_type": "playlist",
        "id": "PL1",
        "title": "My Album",
        "channel": "Artist Name - Topic",
        "entries": [
            {"id": "t1", "title": "First", "duration": 100},
            {"id": "t2", "title": "Second", "duration": 200}
        ]
    }"#;
    let pl = parse_ytdlp_flat_json(playlist_json, Some(playlist_url)).unwrap();
    assert_eq!(pl.albums.len(), 1);
    assert_eq!(pl.albums[0].id, format!("ytpl:{playlist_url}"));
    assert_eq!(pl.albums[0].artist, "Artist Name");
    assert_eq!(pl.tracks.len(), 2);
    assert_eq!(pl.tracks[0].album, "My Album");
    assert_eq!(pl.tracks[0].track_no, Some(1));
    assert_eq!(pl.tracks[1].track_no, Some(2));
}

#[test]
fn sanitizes_paths_and_formats_album_and_single_destinations() {
    assert_eq!(safe_segment("A/B:C?*  D."), "A_B_C__ D");
    assert_eq!(safe_segment("...   "), "Unknown");

    let album_track = OnlineTrack {
        id: "1".into(),
        title: "Track: One".into(),
        artist: "Song Artist".into(),
        album: "Record/Name".into(),
        album_artist: "Album:Artist".into(),
        track_no: Some(3),
        disc_no: Some(2),
        year: Some(2021),
        duration_seconds: None,
        source_url: None,
        cover: None,
        cover_url: None,
    };
    let (dirs, stem) = target_track_stem(&album_track);
    assert_eq!(dirs, ["Album_Artist", "Record_Name (2021)"]);
    assert_eq!(stem, "2-03 - Track_ One");

    let single = OnlineTrack {
        album: String::new(),
        album_artist: String::new(),
        artist: "Solo Artist".into(),
        track_no: None,
        disc_no: None,
        ..album_track
    };
    let (dirs, stem) = target_track_stem(&single);
    assert_eq!(dirs, ["Solo Artist"]);
    assert_eq!(stem, "Track_ One");
}

#[test]
fn yt_search_query_prefers_source_url_else_artist_title() {
    let mut track = OnlineTrack {
        id: "1".into(),
        title: "Aman Aman".into(),
        artist: "Duman".into(),
        album: String::new(),
        album_artist: String::new(),
        track_no: None,
        disc_no: None,
        year: None,
        duration_seconds: None,
        source_url: Some("https://www.youtube.com/watch?v=xyz".into()),
        cover: None,
        cover_url: None,
    };
    assert_eq!(
        yt_search_query(&track),
        "https://www.youtube.com/watch?v=xyz"
    );
    track.source_url = None;
    assert_eq!(yt_search_query(&track), "ytsearch1:Duman - Aman Aman");
}

#[test]
fn writes_tags_and_cover_to_downloaded_audio_file() {
    let tmp = tempfile::tempdir().unwrap();
    let wav = tmp.path().join("song.wav");
    let spec = WavSpec {
        channels: 1,
        sample_rate: 8000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(&wav, spec).unwrap();
    for _ in 0..800 {
        writer.write_sample(0i16).unwrap();
    }
    writer.finalize().unwrap();

    let png = [&[0x89u8, b'P', b'N', b'G'][..], b"fake cover"].concat();
    let track = OnlineTrack {
        id: "1".into(),
        title: "Downloaded Song".into(),
        artist: "Singer".into(),
        album: "New Record".into(),
        album_artist: "Band".into(),
        track_no: Some(4),
        disc_no: Some(1),
        year: Some(2024),
        duration_seconds: Some(10.0),
        source_url: None,
        cover: None,
        cover_url: None,
    };
    write_downloaded_tags(&wav, &track, Some(&png)).unwrap();

    let tagged = lofty::read_from_path(&wav).unwrap();
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag()).unwrap();
    assert_eq!(tag.title().as_deref(), Some("Downloaded Song"));
    assert_eq!(tag.artist().as_deref(), Some("Singer"));
    assert_eq!(tag.album().as_deref(), Some("New Record"));
    assert_eq!(tag.get_string(ItemKey::AlbumArtist), Some("Band"));
    assert_eq!(tag.track(), Some(4));
    assert_eq!(tag.disk(), Some(1));
    assert_eq!(tag.date().map(|d| d.year), Some(2024));
    assert_eq!(tag.pictures().len(), 1);
    assert_eq!(tag.pictures()[0].pic_type(), PictureType::CoverFront);
}

#[test]
fn parses_deezer_artist_and_filters_wikipedia_bio() {
    let dz = r#"{
        "data": [
            {"name": "Someone Else", "picture_xl": "https://img/other.jpg"},
            {"name": "Duman", "picture_xl": "https://img/duman.jpg"}
        ]
    }"#;
    assert_eq!(
        parse_deezer_artist_image(dz, "duman").as_deref(),
        Some("https://img/duman.jpg")
    );

    let non_music = r#"{
        "type": "standard",
        "description": "Chemical element",
        "extract": "Mercury is a chemical element with symbol Hg."
    }"#;
    assert_eq!(parse_wikipedia_bio(non_music), None);

    let band = r#"{
        "type": "standard",
        "description": "Turkish rock band",
        "extract": "Duman is a Turkish rock band formed in 1999 in Istanbul."
    }"#;
    assert!(parse_wikipedia_bio(band).is_some());
}

#[cfg(unix)]
mod ytdlp_process {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A stand-in `yt-dlp` that runs `body` as a shell script.
    fn fake_ytdlp(dir: &Path, body: &str) -> String {
        let path = dir.join("yt-dlp");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn passes_utf8_flags_and_reads_large_output() {
        let tmp = tempfile::tempdir().unwrap();
        // Echo the arguments and the UTF-8 env, then 1 MB more than a pipe holds.
        let bin = fake_ytdlp(
            tmp.path(),
            r#"echo "$@ $PYTHONUTF8"; head -c 1048576 /dev/zero | tr '\0' 'x'"#,
        );
        let out = run_ytdlp_bytes(&bin, &["-g", "q"], Duration::from_secs(10), &|| false).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.starts_with("--encoding utf-8 -g q 1\n"),
            "{}",
            &text[..40]
        );
        assert_eq!(text.len(), "--encoding utf-8 -g q 1\n".len() + 1_048_576);
    }

    #[test]
    fn kills_a_hung_process_after_the_timeout() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = fake_ytdlp(tmp.path(), "sleep 30");
        let started = std::time::Instant::now();
        let err = run_ytdlp_bytes(&bin, &[], Duration::from_millis(300), &|| false).unwrap_err();
        assert!(err.to_string().contains("timed out"), "{err}");
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn stops_when_cancelled() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = fake_ytdlp(tmp.path(), "sleep 30");
        let started = std::time::Instant::now();
        let cancel_after = started + Duration::from_millis(200);
        let err = run_ytdlp_bytes(&bin, &[], Duration::from_secs(60), &|| {
            std::time::Instant::now() >= cancel_after
        })
        .unwrap_err();
        assert!(matches!(err, OnlineError::Cancelled), "{err}");
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn reports_stderr_of_a_failed_run() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = fake_ytdlp(tmp.path(), "echo 'ERROR: no video' >&2; exit 1");
        let err = run_ytdlp_bytes(&bin, &[], Duration::from_secs(10), &|| false).unwrap_err();
        assert_eq!(err.to_string(), "ERROR: no video");
    }

    #[test]
    fn finds_the_download_when_the_printed_path_is_unusable() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("Şebnem Ferah").join("Kelebek (1996)");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("01 - Çakıl Taşları.m4a.part"), b"partial").unwrap();
        std::fs::write(dir.join("01 - Çakıl Taşları.webp"), b"thumb").unwrap();
        std::fs::write(dir.join("01 - Çakıl Taşları.m4a"), b"audio").unwrap();
        // The printed path came back mangled; the download still succeeds.
        let bin = fake_ytdlp(tmp.path(), "echo '/garbled/01 - ??ak?l Ta?lar?.m4a'");
        let track = OnlineTrack {
            id: "itunes:1".into(),
            title: "Çakıl Taşları".into(),
            artist: "Şebnem Ferah".into(),
            album: "Kelebek".into(),
            album_artist: "Şebnem Ferah".into(),
            track_no: Some(1),
            disc_no: Some(1),
            year: Some(1996),
            duration_seconds: None,
            source_url: None,
            cover: None,
            cover_url: None,
        };
        let path = download_one_track(&track, tmp.path(), &bin, true, None, &|| false).unwrap();
        assert_eq!(path, dir.join("01 - Çakıl Taşları.m4a"));
    }
}

fn album_track(title: &str, artist: &str, seconds: f64) -> OnlineTrack {
    OnlineTrack {
        id: "itunes:1".into(),
        title: title.into(),
        artist: artist.into(),
        album: "Album".into(),
        album_artist: artist.into(),
        track_no: Some(1),
        disc_no: Some(1),
        year: Some(1996),
        duration_seconds: Some(seconds),
        source_url: None,
        cover: None,
        cover_url: None,
    }
}

fn hit(url: &str, title: &str, channel: &str, seconds: f64) -> YtCandidate {
    YtCandidate {
        url: url.into(),
        title: title.into(),
        channel: channel.into(),
        duration_seconds: Some(seconds),
    }
}

#[test]
fn chooses_the_album_recording_over_live_and_wrong_length_hits() {
    let track = album_track("Bu Akşam", "Duman", 245.0);
    let hits = [
        hit("u/live", "Duman - Bu Akşam (Live)", "Duman", 251.0),
        hit(
            "u/clip",
            "Duman - Bu Akşam (Official Video)",
            "DumanVEVO",
            290.0,
        ),
        hit("u/topic", "Bu Akşam", "Duman - Topic", 246.0),
        hit("u/cover", "Bu Akşam cover", "Someone", 244.0),
    ];
    assert_eq!(choose_candidate(&track, &hits).as_deref(), Some("u/topic"));

    // Without a Topic upload the closest plain upload wins, not the live one.
    let hits = [
        hits[0].clone(),
        hits[1].clone(),
        hit("u/plain", "Duman - Bu Akşam", "x", 247.0),
    ];
    assert_eq!(choose_candidate(&track, &hits).as_deref(), Some("u/plain"));
}

#[test]
fn a_live_track_may_pick_a_live_upload_and_bad_hits_fall_back() {
    let track = album_track("Hayatı Tespih Yapmışım (Live)", "Barış Manço", 300.0);
    let hits = [hit("u/live", "Hayatı Tespih Yapmışım (Live)", "x", 301.0)];
    assert_eq!(choose_candidate(&track, &hits).as_deref(), Some("u/live"));

    // Every hit is far too short or long: no pick, the caller falls back.
    let track = album_track("Song", "Band", 200.0);
    let hits = [
        hit("u/a", "Song", "x", 60.0),
        hit("u/b", "Song", "x", 600.0),
    ];
    assert_eq!(choose_candidate(&track, &hits), None);
}

#[test]
fn whole_words_only_for_variant_markers() {
    let track = album_track("Oliver", "Band", 200.0);
    let hits = [hit("u/a", "Band - Oliver", "x", 200.0)];
    assert_eq!(choose_candidate(&track, &hits).as_deref(), Some("u/a"));
}

#[test]
fn parses_search_hits_with_channel_and_duration() {
    let json = r#"{"_type":"playlist","entries":[
        {"id":"abc","title":"Bu Akşam","channel":"Duman - Topic","duration":246.0},
        {"id":"def","title":"Other","uploader":"Up","url":"https://www.youtube.com/watch?v=def"}
    ]}"#;
    let hits = parse_yt_candidates(json).unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].url, "https://www.youtube.com/watch?v=abc");
    assert_eq!(hits[0].channel, "Duman - Topic");
    assert_eq!(hits[1].channel, "Up");
    assert_eq!(hits[1].duration_seconds, None);
}
