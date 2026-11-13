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
