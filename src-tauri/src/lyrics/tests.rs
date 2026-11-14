use super::cache::{LyricsCache, NOT_FOUND_TTL_SECS, cache_key};
use super::lrc::{ParsedLyrics, parse};
use super::lrclib::LrclibRecord;
use super::netease::{self, Candidate};
use super::publish::{self, Solve};
use super::save::{self, SaveMode};
use super::{
    LyricLine, LyricsError, LyricsOptions, LyricsResult, LyricsService, LyricsSource, ShareBlock,
    ShareOutcome, TrackQuery, attach_local_lrc, find_local_lrc, read_embedded,
};
use crate::library::tags;
use crate::library::{Hooks, Library};
use lofty::config::WriteOptions;
use lofty::prelude::*;
use lofty::tag::{ItemKey, Tag, TagType};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fut)
}

fn write_tagged_wav(path: &Path, title: &str, artist: &str, album: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 8000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..16000 {
        w.write_sample((i % 100) as i16).unwrap();
    }
    w.finalize().unwrap();

    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_title(title.into());
    tag.set_artist(artist.into());
    if !album.is_empty() {
        tag.set_album(album.into());
    }
    tag.save_to_path(path, WriteOptions::default()).unwrap();
}

struct MockServer {
    addr: String,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl MockServer {
    fn start(responses: Vec<(u16, String)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let queue = Arc::new(Mutex::new(VecDeque::from(responses)));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));

        let q = Arc::clone(&queue);
        let reqs = Arc::clone(&requests);
        let st = Arc::clone(&stop);

        let handle = thread::spawn(move || {
            while !st.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // On Windows an accepted socket inherits the listener's
                        // non-blocking mode: the first read could then fail with
                        // WouldBlock before the request arrived, and the reply
                        // was sent (and the socket closed) with the request
                        // unread, which the client reports as a broken exchange.
                        stream.set_nonblocking(false).unwrap();
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let mut buf = [0u8; 4096];
                        let mut req = Vec::new();
                        // Read the headers and then the body (Content-Length).
                        while let Ok(n) = stream.read(&mut buf) {
                            if n == 0 {
                                break;
                            }
                            req.extend_from_slice(&buf[..n]);
                            if let Some(end) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                                let head =
                                    String::from_utf8_lossy(&req[..end]).to_ascii_lowercase();
                                let want = head
                                    .lines()
                                    .find_map(|l| l.strip_prefix("content-length:"))
                                    .and_then(|v| v.trim().parse::<usize>().ok())
                                    .unwrap_or(0);
                                if req.len() >= end + 4 + want {
                                    break;
                                }
                            }
                        }
                        reqs.lock()
                            .unwrap()
                            .push(String::from_utf8_lossy(&req).into_owned());

                        let (code, body) = q
                            .lock()
                            .unwrap()
                            .pop_front()
                            .unwrap_or((404, String::new()));
                        let reason = match code {
                            200 => "OK",
                            201 => "Created",
                            400 => "Bad Request",
                            404 => "Not Found",
                            _ => "Internal Server Error",
                        };
                        let reply = format!(
                            "HTTP/1.1 {code} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(reply.as_bytes());
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            addr,
            requests,
            stop,
            handle: Some(handle),
        }
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn empty_library(dir: &Path) -> Arc<Library> {
    Arc::new(Library::open(&dir.join("lib.sqlite3"), &dir.join("covers"), Hooks::none()).unwrap())
}

#[test]
fn parse_standard_and_sorted_lrc() {
    let raw = "\
[ar:Artist Name]
[ti:Song Title]
[00:15.500]Second line
[00:05.25]First [Chorus] line
[00:20.0]
";
    let parsed = parse(raw).unwrap();
    assert_eq!(
        parsed,
        ParsedLyrics::Synced(vec![
            LyricLine {
                time_seconds: 5.25,
                text: "First [Chorus] line".into(),
            },
            LyricLine {
                time_seconds: 15.5,
                text: "Second line".into(),
            },
            LyricLine {
                time_seconds: 20.0,
                text: String::new(),
            },
        ])
    );
}

#[test]
fn parse_multi_timestamps_offset_and_enhanced_tags() {
    let raw = "\
\u{feff}[offset:+500]
[00:00.20]<00:00.20>Intro <00:00.40>beat
[00:12.00][01:04.50]Chorus line
";
    let parsed = parse(raw).unwrap();
    let ParsedLyrics::Synced(lines) = parsed else {
        panic!("expected synced");
    };
    assert_eq!(lines.len(), 3);
    assert!((lines[0].time_seconds - 0.0).abs() < 1e-6);
    assert_eq!(lines[0].text, "Intro beat");
    assert!((lines[1].time_seconds - 11.5).abs() < 1e-6);
    assert_eq!(lines[1].text, "Chorus line");
    assert!((lines[2].time_seconds - 64.0).abs() < 1e-6);
    assert_eq!(lines[2].text, "Chorus line");
}

#[test]
fn parse_plain_and_empty_inputs() {
    assert_eq!(parse("  \n[ti:Only Meta]\n  "), None);
    let plain = parse("\u{feff}\n[Verse 1]\nHello world\n\n").unwrap();
    assert_eq!(
        plain,
        ParsedLyrics::Plain(vec!["[Verse 1]".into(), "Hello world".into()])
    );
}

#[test]
fn cache_respects_not_found_ttl() {
    let dir = TempDir::new().unwrap();
    let cache = LyricsCache::new(&dir.path().join("lyrics"));
    let k1 = cache_key("Artist", "Song", "Album", Some(180));
    let k2 = cache_key("Artist", "Missing", "", Some(180));

    let hit = LyricsResult::Plain {
        source: LyricsSource::Lrclib,
        lines: vec!["Line".into()],
    };
    cache.put(&k1, &hit, 1000).unwrap();
    cache.put(&k2, &LyricsResult::NotFound, 1000).unwrap();

    assert_eq!(cache.get(&k1, 1000 + NOT_FOUND_TTL_SECS + 10), Some(hit));
    assert_eq!(
        cache.get(&k2, 1000 + NOT_FOUND_TTL_SECS - 1),
        Some(LyricsResult::NotFound)
    );
    assert_eq!(cache.get(&k2, 1000 + NOT_FOUND_TTL_SECS), None);
}

#[test]
fn local_lrc_takes_priority_over_network() {
    let dir = TempDir::new().unwrap();
    let wav = dir.path().join("track.wav");
    write_tagged_wav(&wav, "Song", "Artist", "Album");
    std::fs::write(wav.with_extension("lrc"), "[00:01.00]From local disk").unwrap();

    let lib = empty_library(dir.path());
    // Unreachable port: verifies local .lrc returns without making any HTTP call.
    let cache_dir = dir.path().join("cache");
    let svc = LyricsService::with_base_url(&cache_dir, "http://127.0.0.1:1").unwrap();
    let path = wav.to_string_lossy().into_owned();
    let res = block_on(svc.for_track(lib, path, None, false)).unwrap();

    assert_eq!(
        res,
        LyricsResult::Synced {
            source: LyricsSource::LocalLrc,
            lines: vec![LyricLine {
                time_seconds: 1.0,
                text: "From local disk".into(),
            }],
        }
    );
}

#[test]
fn lrclib_get_caches_and_force_refreshes() {
    let dir = TempDir::new().unwrap();
    let wav = dir.path().join("song.wav");
    write_tagged_wav(&wav, "Morning Light", "Nadia Okafor", "Daybreak");

    let body1 = serde_json::json!({
        "syncedLyrics": "[00:02.00]First version",
        "plainLyrics": "First version",
        "instrumental": false,
        "duration": 2.0
    })
    .to_string();
    let body2 = serde_json::json!({
        "syncedLyrics": "[00:02.00]Refreshed version",
        "plainLyrics": "Refreshed version",
        "instrumental": false,
        "duration": 2.0
    })
    .to_string();

    let server = MockServer::start(vec![(200, body1), (200, body2)]);
    let lib = empty_library(dir.path());
    let svc = LyricsService::with_base_url(&dir.path().join("cache"), &server.addr).unwrap();
    let path = wav.to_string_lossy().into_owned();

    let r1 = block_on(svc.for_track(Arc::clone(&lib), path.clone(), None, false)).unwrap();
    assert_eq!(
        r1,
        LyricsResult::Synced {
            source: LyricsSource::Lrclib,
            lines: vec![LyricLine {
                time_seconds: 2.0,
                text: "First version".into(),
            }],
        }
    );
    assert_eq!(server.requests().len(), 1);
    assert!(
        server.requests()[0]
            .to_ascii_lowercase()
            .contains("user-agent: neomodern/")
    );
    assert!(server.requests()[0].contains("/api/get?"));

    // Second call uses cache, no HTTP request.
    let r2 = block_on(svc.for_track(Arc::clone(&lib), path.clone(), None, false)).unwrap();
    assert_eq!(r2, r1);
    assert_eq!(server.requests().len(), 1);

    // Force refresh bypasses cache.
    let r3 = block_on(svc.for_track(lib, path, None, true)).unwrap();
    assert_eq!(
        r3,
        LyricsResult::Synced {
            source: LyricsSource::Lrclib,
            lines: vec![LyricLine {
                time_seconds: 2.0,
                text: "Refreshed version".into(),
            }],
        }
    );
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn lrclib_404_falls_back_to_search_with_duration_filter() {
    let dir = TempDir::new().unwrap();
    let wav = dir.path().join("search.wav");
    write_tagged_wav(&wav, "Track", "Artist", "Deluxe Edition");

    let search_body = serde_json::to_string(&vec![
        serde_json::json!({
            "syncedLyrics": "[00:01.00]Wrong duration remix",
            "plainLyrics": null,
            "instrumental": false,
            "duration": 180.0
        }),
        serde_json::json!({
            "syncedLyrics": null,
            "plainLyrics": "Plain match",
            "instrumental": false,
            "duration": 2.0
        }),
        serde_json::json!({
            "syncedLyrics": "[00:01.00]Synced match",
            "plainLyrics": "Synced match",
            "instrumental": false,
            "duration": 3.0
        }),
    ])
    .unwrap();

    let server = MockServer::start(vec![(404, String::new()), (200, search_body)]);
    let lib = empty_library(dir.path());
    let svc = LyricsService::with_base_url(&dir.path().join("cache"), &server.addr).unwrap();
    let path = wav.to_string_lossy().into_owned();
    let res = block_on(svc.for_track(lib, path, None, false)).unwrap();
    assert_eq!(
        res,
        LyricsResult::Synced {
            source: LyricsSource::Lrclib,
            lines: vec![LyricLine {
                time_seconds: 1.0,
                text: "Synced match".into(),
            }],
        }
    );
    assert_eq!(server.requests().len(), 2);
    assert!(server.requests()[1].contains("/api/search?"));
}

#[test]
fn lrclib_500_is_not_cached() {
    let dir = TempDir::new().unwrap();
    let wav = dir.path().join("err.wav");
    write_tagged_wav(&wav, "Track", "Artist", "");

    let ok_body = serde_json::json!({
        "syncedLyrics": null,
        "plainLyrics": null,
        "instrumental": true,
        "duration": 2.0
    })
    .to_string();

    let server = MockServer::start(vec![(500, String::new()), (200, ok_body)]);
    let lib = empty_library(dir.path());
    let svc = LyricsService::with_base_url(&dir.path().join("cache"), &server.addr).unwrap();
    let path = wav.to_string_lossy().into_owned();

    let err = block_on(svc.for_track(Arc::clone(&lib), path.clone(), None, false)).unwrap_err();
    assert!(matches!(err, LyricsError::HttpStatus(500)));

    let retry = block_on(svc.for_track(lib, path, None, false)).unwrap();
    assert_eq!(
        retry,
        LyricsResult::Instrumental {
            source: LyricsSource::Lrclib,
        }
    );
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn lrclib_record_deserialize_smoke() {
    let rec: LrclibRecord = serde_json::from_str(
        r#"{"syncedLyrics":null,"plainLyrics":"Hello\nWorld","instrumental":false,"duration":120}"#,
    )
    .unwrap();
    assert_eq!(
        rec.into_result(),
        LyricsResult::Plain {
            source: LyricsSource::Lrclib,
            lines: vec!["Hello".into(), "World".into()],
        }
    );
}

#[test]
fn attach_local_lrc_copies_valid_file_as_sidecar() {
    let dir = TempDir::new().unwrap();
    let audio = dir.path().join("music/song.flac");
    std::fs::create_dir_all(audio.parent().unwrap()).unwrap();
    let picked = dir.path().join("downloads/custom.lrc");
    std::fs::create_dir_all(picked.parent().unwrap()).unwrap();
    std::fs::write(&picked, "[00:02.00]Hello\n[00:05.50]World\n").unwrap();

    let res = attach_local_lrc(&audio, &picked).unwrap();
    let expected = LyricsResult::Synced {
        source: LyricsSource::LocalLrc,
        lines: vec![
            LyricLine {
                time_seconds: 2.0,
                text: "Hello".into(),
            },
            LyricLine {
                time_seconds: 5.5,
                text: "World".into(),
            },
        ],
    };
    assert_eq!(res, expected);
    assert_eq!(find_local_lrc(&audio), Some(expected));
}

#[test]
fn attach_local_lrc_rejects_empty_file_without_clobbering() {
    let dir = TempDir::new().unwrap();
    let audio = dir.path().join("song.flac");
    let sidecar = audio.with_extension("lrc");
    std::fs::write(&sidecar, "[00:01.00]Existing\n").unwrap();

    let invalid = dir.path().join("empty.lrc");
    std::fs::write(&invalid, "[ti:Metadata Only]\n\n").unwrap();

    let err = attach_local_lrc(&audio, &invalid).unwrap_err();
    assert!(matches!(err, LyricsError::InvalidFile));
    assert_eq!(
        std::fs::read_to_string(&sidecar).unwrap(),
        "[00:01.00]Existing\n"
    );
}

#[test]
fn attach_local_lrc_same_path_keeps_existing_sidecar() {
    let dir = TempDir::new().unwrap();
    let audio = dir.path().join("song.mp3");
    let sidecar = audio.with_extension("lrc");
    std::fs::write(&sidecar, "First line\nSecond line\n").unwrap();

    let res = attach_local_lrc(&audio, &sidecar).unwrap();
    assert_eq!(
        res,
        LyricsResult::Plain {
            source: LyricsSource::LocalLrc,
            lines: vec!["First line".into(), "Second line".into()],
        }
    );
    assert_eq!(
        std::fs::read_to_string(&sidecar).unwrap(),
        "First line\nSecond line\n"
    );
}

// ---------------------------------------------------------------------
// Embedded tag, saving, NetEase, sharing, bulk save and coverage
// ---------------------------------------------------------------------

fn synced_hit_body() -> String {
    serde_json::json!({
        "syncedLyrics": "[00:02.00]Online line\n[00:05.50]Second online line",
        "plainLyrics": "Online line\nSecond online line",
        "instrumental": false,
        "duration": 2.0
    })
    .to_string()
}

fn opts(save: SaveMode, netease: bool, current: Option<&Path>) -> LyricsOptions {
    LyricsOptions::new(
        save,
        netease,
        current.map(|p| p.to_string_lossy().into_owned()),
    )
}

fn lookup(
    svc: &LyricsService,
    lib: &Arc<Library>,
    wav: &Path,
    force: bool,
    o: &LyricsOptions,
) -> Result<LyricsResult, LyricsError> {
    block_on(svc.for_track_with(
        Arc::clone(lib),
        wav.to_string_lossy().into_owned(),
        None,
        force,
        o,
    ))
}

fn tag_lyrics(path: &Path) -> Option<String> {
    tags::read_lyrics(path)
}

#[test]
fn lofty_lyrics_key_mapping() {
    // ID3v2 has no frame for ItemKey::Lyrics, only for UnsyncLyrics (USLT).
    assert_eq!(ItemKey::Lyrics.map_key(TagType::Id3v2), None);
    assert_eq!(ItemKey::UnsyncLyrics.map_key(TagType::Id3v2), Some("USLT"));
    assert_eq!(
        ItemKey::Lyrics.map_key(TagType::VorbisComments),
        Some("LYRICS")
    );
    assert_eq!(ItemKey::Lyrics.map_key(TagType::Mp4Ilst), Some("\u{a9}lyr"));
    assert!(ItemKey::Lyrics.map_key(TagType::Ape).is_some());
}

#[test]
fn embedded_lyrics_are_read_between_sidecar_and_cache() {
    let dir = TempDir::new().unwrap();
    let wav = dir.path().join("emb.wav");
    write_tagged_wav(&wav, "Song", "Artist", "Album");
    assert_eq!(tag_lyrics(&wav), None);
    assert!(save::write_embedded(&wav, "[00:01.00]Inside the file\n[00:03.00]Tag").unwrap());

    let lib = empty_library(dir.path());
    // Unreachable LRCLIB: the embedded tag must answer without any HTTP call.
    let svc =
        LyricsService::with_base_url(&dir.path().join("cache"), "http://127.0.0.1:1").unwrap();
    let o = opts(SaveMode::Sidecar, false, None);
    let res = lookup(&svc, &lib, &wav, false, &o).unwrap();
    assert_eq!(
        res,
        LyricsResult::Synced {
            source: LyricsSource::Embedded,
            lines: vec![
                LyricLine {
                    time_seconds: 1.0,
                    text: "Inside the file".into()
                },
                LyricLine {
                    time_seconds: 3.0,
                    text: "Tag".into()
                },
            ],
        }
    );
    assert_eq!(
        read_embedded(&wav).map(|r| matches!(r, LyricsResult::Synced { .. })),
        Some(true)
    );

    // A sidecar wins over the tag.
    std::fs::write(wav.with_extension("lrc"), "[00:01.00]Sidecar").unwrap();
    let res = lookup(&svc, &lib, &wav, false, &o).unwrap();
    assert!(matches!(
        res,
        LyricsResult::Synced {
            source: LyricsSource::LocalLrc,
            ..
        }
    ));
}

#[test]
fn plain_embedded_text_is_plain() {
    let dir = TempDir::new().unwrap();
    let wav = dir.path().join("plain.wav");
    write_tagged_wav(&wav, "Song", "Artist", "Album");
    save::write_embedded(&wav, "Just words\nMore words").unwrap();
    assert_eq!(
        read_embedded(&wav),
        Some(LyricsResult::Plain {
            source: LyricsSource::Embedded,
            lines: vec!["Just words".into(), "More words".into()],
        })
    );
}

#[test]
fn existing_lyrics_tag_is_never_replaced() {
    let dir = TempDir::new().unwrap();
    let wav = dir.path().join("keep.wav");
    write_tagged_wav(&wav, "Song", "Artist", "Album");
    assert!(save::write_embedded(&wav, "Original lyrics").unwrap());
    assert!(!save::write_embedded(&wav, "Replacement").unwrap());
    assert_eq!(tag_lyrics(&wav).as_deref(), Some("Original lyrics"));
    // Other tag fields survive the write.
    let t = tags::read_tags(&wav).unwrap();
    assert_eq!(t.title.as_deref(), Some("Song"));
    assert_eq!(t.album.as_deref(), Some("Album"));
    assert!(save::write_embedded(&wav, "  ").is_err());
}

#[test]
fn sidecar_is_written_after_an_online_hit_and_never_overwrites() {
    let dir = TempDir::new().unwrap();
    let wav = dir.path().join("hit.wav");
    write_tagged_wav(&wav, "Morning Light", "Nadia Okafor", "Daybreak");
    let server = MockServer::start(vec![(200, synced_hit_body())]);
    let lib = empty_library(dir.path());
    let svc = LyricsService::with_base_url(&dir.path().join("cache"), &server.addr).unwrap();

    let res = lookup(
        &svc,
        &lib,
        &wav,
        false,
        &opts(SaveMode::Sidecar, false, None),
    )
    .unwrap();
    assert!(matches!(
        res,
        LyricsResult::Synced {
            source: LyricsSource::Lrclib,
            ..
        }
    ));
    let written = std::fs::read_to_string(wav.with_extension("lrc")).unwrap();
    assert_eq!(
        written,
        "[00:02.00]Online line\n[00:05.50]Second online line"
    );
    // No temp file is left behind.
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty());

    // The sidecar is found first from now on, without a request.
    let again = lookup(
        &svc,
        &lib,
        &wav,
        false,
        &opts(SaveMode::Sidecar, false, None),
    )
    .unwrap();
    assert!(matches!(
        again,
        LyricsResult::Synced {
            source: LyricsSource::LocalLrc,
            ..
        }
    ));
    assert_eq!(server.requests().len(), 1);

    // A file that exists but is not usable lyrics is still not overwritten.
    let other = dir.path().join("other.wav");
    write_tagged_wav(&other, "Other", "Artist", "Album");
    std::fs::write(other.with_extension("lrc"), "[ti:Only a title]\n").unwrap();
    let server2 = MockServer::start(vec![(200, synced_hit_body())]);
    let svc2 = LyricsService::with_base_url(&dir.path().join("cache2"), &server2.addr).unwrap();
    let res = lookup(
        &svc2,
        &lib,
        &other,
        false,
        &opts(SaveMode::Sidecar, false, None),
    )
    .unwrap();
    assert!(matches!(
        res,
        LyricsResult::Synced {
            source: LyricsSource::Lrclib,
            ..
        }
    ));
    assert_eq!(
        std::fs::read_to_string(other.with_extension("lrc")).unwrap(),
        "[ti:Only a title]\n"
    );
    assert!(
        !save::write_sidecar(&other, "x").unwrap(),
        "write_sidecar refuses to replace"
    );
}

#[test]
fn unwritable_sidecar_folder_is_not_an_error() {
    let dir = TempDir::new().unwrap();
    let wav = dir.path().join("gone/hit.wav");
    write_tagged_wav(&wav, "Morning Light", "Nadia Okafor", "Daybreak");
    let server = MockServer::start(vec![(200, synced_hit_body())]);
    let lib = empty_library(dir.path());
    let svc = LyricsService::with_base_url(&dir.path().join("cache"), &server.addr).unwrap();
    std::fs::remove_dir_all(dir.path().join("gone")).unwrap();
    // The audio file is gone, so neither the tag read nor the sidecar write can work.
    assert!(save::write_sidecar(&wav, "x").is_err());
    assert!(
        block_on(svc.for_track_with(
            lib,
            wav.to_string_lossy().into_owned(),
            Some(2.0),
            false,
            &opts(SaveMode::Sidecar, false, None),
        ))
        .is_ok()
    );
}

#[test]
fn save_mode_off_writes_nothing() {
    let dir = TempDir::new().unwrap();
    let wav = dir.path().join("off.wav");
    write_tagged_wav(&wav, "Morning Light", "Nadia Okafor", "Daybreak");
    let before = std::fs::read(&wav).unwrap();
    let server = MockServer::start(vec![(200, synced_hit_body())]);
    let lib = empty_library(dir.path());
    let svc = LyricsService::with_base_url(&dir.path().join("cache"), &server.addr).unwrap();
    let res = lookup(&svc, &lib, &wav, false, &opts(SaveMode::Off, false, None)).unwrap();
    assert!(matches!(res, LyricsResult::Synced { .. }));
    assert!(!wav.with_extension("lrc").exists());
    assert_eq!(std::fs::read(&wav).unwrap(), before);
    assert_eq!(svc.pending_embed_count(), 0);
    // The cache still works.
    let again = lookup(&svc, &lib, &wav, false, &opts(SaveMode::Off, false, None)).unwrap();
    assert_eq!(again, res);
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn embed_is_deferred_while_the_track_is_current() {
    let dir = TempDir::new().unwrap();
    let wav = dir.path().join("playing.wav");
    let next = dir.path().join("next.wav");
    write_tagged_wav(&wav, "Morning Light", "Nadia Okafor", "Daybreak");
    write_tagged_wav(&next, "Another", "Someone", "Else");
    let server = MockServer::start(vec![(200, synced_hit_body())]);
    let lib = empty_library(dir.path());
    let svc = LyricsService::with_base_url(&dir.path().join("cache"), &server.addr).unwrap();

    // The file is the playing one: the write is queued, the file untouched.
    let before = std::fs::read(&wav).unwrap();
    let res = lookup(
        &svc,
        &lib,
        &wav,
        false,
        &opts(SaveMode::Embed, false, Some(&wav)),
    )
    .unwrap();
    assert!(matches!(res, LyricsResult::Synced { .. }));
    assert_eq!(svc.pending_embed_count(), 1);
    assert_eq!(std::fs::read(&wav).unwrap(), before);
    assert_eq!(tag_lyrics(&wav), None);

    // Still current on a later lookup: still waiting.
    svc.flush_pending_embeds(Some(&wav.to_string_lossy()));
    assert_eq!(svc.pending_embed_count(), 1);
    assert_eq!(tag_lyrics(&wav), None);

    // Another track becomes current; the next lookup performs the write.
    let _ = lookup(
        &svc,
        &lib,
        &next,
        false,
        &opts(SaveMode::Embed, false, Some(&next)),
    );
    assert_eq!(svc.pending_embed_count(), 0);
    let written = tag_lyrics(&wav).unwrap();
    assert!(written.starts_with("[00:02.00]Online line"));
    assert_eq!(
        tags::read_tags(&wav).unwrap().title.as_deref(),
        Some("Morning Light")
    );

    // A track that is not current is embedded right away.
    let server2 = MockServer::start(vec![(200, synced_hit_body())]);
    let svc2 = LyricsService::with_base_url(&dir.path().join("cache2"), &server2.addr).unwrap();
    let other = dir.path().join("direct.wav");
    write_tagged_wav(&other, "Direct", "Artist", "Album");
    lookup(
        &svc2,
        &lib,
        &other,
        false,
        &opts(SaveMode::Embed, false, Some(&wav)),
    )
    .unwrap();
    assert_eq!(svc2.pending_embed_count(), 0);
    assert!(tag_lyrics(&other).is_some());
}

// ---- NetEase ----

const SEARCH_JSON: &str = r#"{"result":{"songs":[
 {"id":111,"name":"Morning Light (Live)","duration":183000,"artists":[{"name":"Other Band"}],"album":{"name":"Live"}},
 {"id":222,"name":"Morning Light","duration":2000,"artists":[{"name":"Nadia Okafor"}],"album":{"name":"Daybreak"}},
 {"id":"333","name":"Morning Light","duration":240000,"artists":[{"name":"Nadia Okafor"}],"album":{"name":"Remix"}}
],"songCount":3},"code":200}"#;

fn query(title: &str, artist: &str, secs: Option<u64>) -> TrackQuery {
    TrackQuery {
        title: title.into(),
        artist: artist.into(),
        album: String::new(),
        duration_secs: secs,
    }
}

#[test]
fn netease_search_fixture_parses_and_the_right_song_is_picked() {
    let list = netease::parse_search(SEARCH_JSON.as_bytes()).unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[1].id, 222);
    assert_eq!(list[1].duration_ms, Some(2000));
    assert_eq!(list[2].id, 333, "string ids are accepted");
    assert_eq!(list[0].artists, vec!["Other Band".to_string()]);

    // Title + artist + duration (within 3 s) choose song 222.
    let pick = netease::pick_song(&list, &query("Morning Light", "Nadia Okafor", Some(3)));
    assert_eq!(pick.map(|c| c.id), Some(222));
    // Without a duration the exact title/artist still match; the first best wins.
    assert!(netease::pick_song(&list, &query("Morning Light", "Nadia Okafor", None)).is_some());
    // Wrong artist, wrong duration, different title: nothing plausible.
    assert!(netease::pick_song(&list, &query("Morning Light", "Nobody", Some(2))).is_none());
    assert!(netease::pick_song(&list, &query("Morning Light", "Nadia Okafor", Some(60))).is_none());
    assert!(netease::pick_song(&list, &query("Evening Shadow", "Nadia Okafor", Some(2))).is_none());
    // Brackets in the title and several artists are tolerated.
    let list2 = vec![Candidate {
        id: 1,
        name: "Song Name (Remastered 2011)".into(),
        duration_ms: Some(200_000),
        artists: vec!["Alpha".into(), "Beta".into()],
    }];
    assert!(netease::pick_song(&list2, &query("Song Name", "Beta & Gamma", Some(201))).is_some());
    assert!(netease::pick_song(&list2, &query("Song Name", "Gamma", Some(201))).is_none());
    // Rejected / empty replies.
    assert!(netease::parse_search(br#"{"code":-460,"msg":"Cheating"}"#).is_err());
    assert!(netease::parse_search(b"<html>").is_err());
    assert!(
        netease::parse_search(br#"{"result":{"songCount":0},"code":200}"#)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn netease_lyrics_drop_credit_lines_and_flag_special_cases() {
    let raw = serde_json::json!({
        "lrc": {"lyric": "[00:00.00] 作词 : 张三\n[00:01.00] 作曲 : 李四\n[00:02.00] Producer: Someone\n[00:12.34]First real line\n[00:15.00]Hey: you there\n[00:18.00]Third"},
        "code": 200
    })
    .to_string();
    let Some(LyricsResult::Synced { source, lines }) =
        netease::parse_lyric(raw.as_bytes()).unwrap()
    else {
        panic!("expected synced lyrics");
    };
    assert_eq!(source, LyricsSource::Netease);
    let texts: Vec<_> = lines.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts, ["First real line", "Hey: you there", "Third"]);
    assert!((lines[0].time_seconds - 12.34).abs() < 1e-6);

    // Keyword credit lines inside the first lines go, even after real text.
    let raw = r#"{"lrc":{"lyric":"[00:00.00]Song - Artist\n[00:01.00]编曲 : X\n[00:10.00]Words"}}"#;
    let Some(LyricsResult::Synced { lines, .. }) = netease::parse_lyric(raw.as_bytes()).unwrap()
    else {
        panic!("expected synced lyrics");
    };
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].text, "Song - Artist");

    assert_eq!(netease::parse_lyric(br#"{"nolyric":true}"#).unwrap(), None);
    assert_eq!(
        netease::parse_lyric(br#"{"lrc":{"lyric":""}}"#).unwrap(),
        None
    );
    assert_eq!(netease::parse_lyric(br#"{"code":200}"#).unwrap(), None);
    assert_eq!(
        netease::parse_lyric(
            r#"{"pureMusic":true,"lrc":{"lyric":"[00:00.00]纯音乐，请欣赏"}}"#.as_bytes()
        )
        .unwrap(),
        Some(LyricsResult::Instrumental {
            source: LyricsSource::Netease
        })
    );
    // Only credits: nothing left.
    assert_eq!(
        netease::parse_lyric(
            r#"{"lrc":{"lyric":"[00:00.00]作词 : A\n[00:01.00]作曲 : B"}}"#.as_bytes()
        )
        .unwrap(),
        None
    );
}

fn netease_lyric_json() -> String {
    serde_json::json!({
        "lrc": {"lyric": "[00:00.00] 作词 : X\n[00:01.50]NetEase line one\n[00:04.00]NetEase line two"},
        "code": 200
    })
    .to_string()
}

fn netease_search_json() -> String {
    serde_json::json!({"result": {"songs": [
        {"id": 4242, "name": "Morning Light", "duration": 2000,
         "artists": [{"name": "Nadia Okafor"}], "album": {"name": "Daybreak"}}
    ]}, "code": 200})
    .to_string()
}

#[test]
fn netease_is_used_only_after_lrclib_not_found_cached_and_saved() {
    let dir = TempDir::new().unwrap();
    let wav = dir.path().join("ne.wav");
    write_tagged_wav(&wav, "Morning Light", "Nadia Okafor", "Daybreak");
    // LRCLIB: /api/get 404 and /api/search 404 = not found.
    let lrclib = MockServer::start(vec![(404, String::new()), (404, String::new())]);
    let ne = MockServer::start(vec![
        (200, netease_search_json()),
        (200, netease_lyric_json()),
    ]);
    let lib = empty_library(dir.path());
    let svc =
        LyricsService::with_base_urls(&dir.path().join("cache"), &lrclib.addr, &ne.addr).unwrap();

    let res = lookup(
        &svc,
        &lib,
        &wav,
        false,
        &opts(SaveMode::Sidecar, true, None),
    )
    .unwrap();
    let LyricsResult::Synced { source, lines } = &res else {
        panic!("expected synced, got {res:?}");
    };
    assert_eq!(*source, LyricsSource::Netease);
    assert_eq!(lines[0].text, "NetEase line one");

    let reqs = ne.requests();
    assert_eq!(reqs.len(), 2);
    assert!(reqs[0].starts_with("POST /api/search/get"));
    let lower = reqs[0].to_ascii_lowercase();
    assert!(lower.contains("referer: https://music.163.com"));
    assert!(lower.contains("user-agent: mozilla/5.0"));
    assert!(reqs[0].contains("s=Nadia%20Okafor%20Morning%20Light&type=1&limit=10"));
    assert!(reqs[1].starts_with("GET /api/song/lyric?id=4242"));
    assert!(reqs[1].contains("lv=1") && reqs[1].contains("tv=-1"));

    // Cached with source netease and saved as a sidecar.
    assert_eq!(
        std::fs::read_to_string(wav.with_extension("lrc")).unwrap(),
        "[00:01.50]NetEase line one\n[00:04.00]NetEase line two"
    );
    std::fs::remove_file(wav.with_extension("lrc")).unwrap();
    let again = lookup(&svc, &lib, &wav, false, &opts(SaveMode::Off, true, None)).unwrap();
    assert_eq!(again, res);
    assert_eq!(ne.requests().len(), 2);
    assert_eq!(lrclib.requests().len(), 2);
}

#[test]
fn netease_is_not_asked_when_lrclib_hits_errors_or_the_setting_is_off() {
    let dir = TempDir::new().unwrap();
    let lib = empty_library(dir.path());

    // LRCLIB hit.
    let wav = dir.path().join("a.wav");
    write_tagged_wav(&wav, "Morning Light", "Nadia Okafor", "Daybreak");
    let lrclib = MockServer::start(vec![(200, synced_hit_body())]);
    let ne = MockServer::start(vec![]);
    let svc =
        LyricsService::with_base_urls(&dir.path().join("c1"), &lrclib.addr, &ne.addr).unwrap();
    lookup(&svc, &lib, &wav, false, &opts(SaveMode::Off, true, None)).unwrap();
    assert!(ne.requests().is_empty());

    // LRCLIB network/server error: an error, and still no NetEase.
    let lrclib = MockServer::start(vec![(500, String::new())]);
    let ne = MockServer::start(vec![]);
    let svc =
        LyricsService::with_base_urls(&dir.path().join("c2"), &lrclib.addr, &ne.addr).unwrap();
    assert!(lookup(&svc, &lib, &wav, false, &opts(SaveMode::Off, true, None)).is_err());
    assert!(ne.requests().is_empty());

    // Setting off: LRCLIB not found stays not found.
    let lrclib = MockServer::start(vec![(404, String::new()), (404, String::new())]);
    let ne = MockServer::start(vec![
        (200, netease_search_json()),
        (200, netease_lyric_json()),
    ]);
    let svc =
        LyricsService::with_base_urls(&dir.path().join("c3"), &lrclib.addr, &ne.addr).unwrap();
    let res = lookup(&svc, &lib, &wav, false, &opts(SaveMode::Off, false, None)).unwrap();
    assert_eq!(res, LyricsResult::NotFound);
    assert!(ne.requests().is_empty());
}

#[test]
fn netease_search_falls_back_to_get_and_failures_are_not_cached() {
    let dir = TempDir::new().unwrap();
    let lib = empty_library(dir.path());
    let wav = dir.path().join("b.wav");
    write_tagged_wav(&wav, "Morning Light", "Nadia Okafor", "Daybreak");

    // POST fails (500) -> GET succeeds -> lyric.
    let lrclib = MockServer::start(vec![(404, String::new()), (404, String::new())]);
    let ne = MockServer::start(vec![
        (500, String::new()),
        (200, netease_search_json()),
        (200, netease_lyric_json()),
    ]);
    let svc =
        LyricsService::with_base_urls(&dir.path().join("c1"), &lrclib.addr, &ne.addr).unwrap();
    let res = lookup(&svc, &lib, &wav, false, &opts(SaveMode::Off, true, None)).unwrap();
    assert!(matches!(
        res,
        LyricsResult::Synced {
            source: LyricsSource::Netease,
            ..
        }
    ));
    let reqs = ne.requests();
    assert_eq!(reqs.len(), 3);
    assert!(reqs[0].starts_with("POST /api/search/get"));
    assert!(reqs[1].starts_with("GET /api/search/get?"));
    assert!(reqs[1].contains("type=1") && reqs[1].contains("limit=10"));

    // Both methods fail: not found for the user, nothing cached (retry works).
    let lrclib = MockServer::start(vec![
        (404, String::new()),
        (404, String::new()),
        (404, String::new()),
        (404, String::new()),
    ]);
    let ne = MockServer::start(vec![
        (500, String::new()),
        (500, String::new()),
        (200, netease_search_json()),
        (200, netease_lyric_json()),
    ]);
    let svc =
        LyricsService::with_base_urls(&dir.path().join("c2"), &lrclib.addr, &ne.addr).unwrap();
    let o = opts(SaveMode::Off, true, None);
    assert_eq!(
        lookup(&svc, &lib, &wav, false, &o).unwrap(),
        LyricsResult::NotFound
    );
    assert!(matches!(
        lookup(&svc, &lib, &wav, false, &o).unwrap(),
        LyricsResult::Synced {
            source: LyricsSource::Netease,
            ..
        }
    ));

    // A pure-music reply is an instrumental result.
    let lrclib = MockServer::start(vec![(404, String::new()), (404, String::new())]);
    let ne = MockServer::start(vec![
        (200, netease_search_json()),
        (200, r#"{"pureMusic":true,"code":200}"#.into()),
    ]);
    let svc =
        LyricsService::with_base_urls(&dir.path().join("c3"), &lrclib.addr, &ne.addr).unwrap();
    assert_eq!(
        lookup(&svc, &lib, &wav, false, &o).unwrap(),
        LyricsResult::Instrumental {
            source: LyricsSource::Netease
        }
    );
}

// ---- LRCLIB publish ----

fn easy_target() -> [u8; 32] {
    // About 1 in 256 hashes qualify.
    let mut t = [0xffu8; 32];
    t[0] = 0x00;
    t
}

/// The definition, written independently of `publish::is_valid`.
fn reference_valid(prefix: &str, nonce: u64, target: &[u8; 32]) -> bool {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(format!("{prefix}{nonce}").as_bytes());
    let digest: [u8; 32] = h.finalize().into();
    for (a, b) in digest.iter().zip(target.iter()) {
        if a != b {
            return a < b;
        }
    }
    true
}

#[test]
fn proof_of_work_solver_matches_the_definition() {
    let cancel = AtomicBool::new(false);
    let prefix = "NeoModernTestPrefix";
    let target = easy_target();
    let Solve::Found(nonce) = publish::solve(
        prefix,
        &target,
        &cancel,
        Instant::now() + Duration::from_secs(30),
    ) else {
        panic!("no nonce found for an easy target");
    };
    assert!(publish::is_valid(prefix, nonce, &target));
    assert!(reference_valid(prefix, nonce, &target));
    // The check is exactly the definition, also for nonces that fail.
    for n in 0..2000u64 {
        assert_eq!(
            publish::is_valid(prefix, n, &target),
            reference_valid(prefix, n, &target)
        );
    }
    // Hex decoding: full, short (left-padded) and bad input.
    assert_eq!(
        publish::decode_target(&"ff".repeat(32)).unwrap(),
        [0xff; 32]
    );
    let short = publish::decode_target("0f").unwrap();
    assert_eq!(short[31], 0x0f);
    assert!(short[..31].iter().all(|b| *b == 0));
    assert!(publish::decode_target("zz").is_none());
    assert!(publish::decode_target("").is_none());
    assert!(publish::decode_target(&"ff".repeat(33)).is_none());
}

#[test]
fn proof_of_work_can_be_cancelled_and_times_out() {
    let impossible = [0u8; 32];
    let cancel = AtomicBool::new(true);
    assert_eq!(
        publish::solve(
            "p",
            &impossible,
            &cancel,
            Instant::now() + Duration::from_secs(30)
        ),
        Solve::Cancelled
    );
    let cancel = AtomicBool::new(false);
    assert_eq!(
        publish::solve(
            "p",
            &impossible,
            &cancel,
            Instant::now() + Duration::from_millis(30)
        ),
        Solve::TimedOut
    );
}

fn shareable_track(dir: &Path, name: &str, album: &str) -> std::path::PathBuf {
    let wav = dir.join(name);
    write_tagged_wav(&wav, "Morning Light", "Nadia Okafor", album);
    std::fs::write(
        wav.with_extension("lrc"),
        "[00:01.00]My own line\n[00:02.50]Another one",
    )
    .unwrap();
    wav
}

fn challenge_json(target_hex: &str) -> String {
    serde_json::json!({"prefix": "prefix-abc", "target": target_hex}).to_string()
}

#[test]
fn publish_sends_token_header_and_fields_and_clears_the_not_found_cache() {
    let dir = TempDir::new().unwrap();
    let wav = shareable_track(dir.path(), "share.wav", "Daybreak");
    let easy_hex = "00".to_string() + &"ff".repeat(31);
    // get 404, search 404 (nothing on LRCLIB), challenge, publish 201.
    let server = MockServer::start(vec![
        (404, String::new()),
        (404, String::new()),
        (200, challenge_json(&easy_hex)),
        (201, String::new()),
    ]);
    let lib = empty_library(dir.path());
    let cache_dir = dir.path().join("cache");
    let svc = LyricsService::with_base_url(&cache_dir, &server.addr).unwrap();

    // A cached "not found" for the track exists and must be dropped.
    let key = super::cache::cache_key("Nadia Okafor", "Morning Light", "Daybreak", Some(2));
    let cache = LyricsCache::new(&cache_dir);
    cache
        .put(&key, &LyricsResult::NotFound, super::cache::now_unix_secs())
        .unwrap();
    assert!(cache.get(&key, super::cache::now_unix_secs()).is_some());

    let out = block_on(svc.share(lib, wav.to_string_lossy().into_owned(), None)).unwrap();
    assert_eq!(out, ShareOutcome::Published);
    assert_eq!(cache.get(&key, super::cache::now_unix_secs()), None);

    let reqs = server.requests();
    assert_eq!(reqs.len(), 4);
    assert!(reqs[2].starts_with("POST /api/request-challenge"));
    let publish_req = &reqs[3];
    assert!(publish_req.starts_with("POST /api/publish"));
    let token = publish_req
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case("x-publish-token")
                .then(|| v.trim().to_string())
        })
        .expect("token header");
    let (prefix, nonce) = token.split_once(':').unwrap();
    assert_eq!(prefix, "prefix-abc");
    let target = publish::decode_target(&easy_hex).unwrap();
    assert!(reference_valid(prefix, nonce.parse().unwrap(), &target));
    let body: serde_json::Value =
        serde_json::from_str(publish_req.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["trackName"], "Morning Light");
    assert_eq!(body["artistName"], "Nadia Okafor");
    assert_eq!(body["albumName"], "Daybreak");
    assert_eq!(body["duration"], 2);
    assert_eq!(body["plainLyrics"], "My own line\nAnother one");
    assert_eq!(
        body["syncedLyrics"],
        "[00:01.00]My own line\n[00:02.50]Another one"
    );
}

#[test]
fn publish_asks_for_a_new_challenge_when_one_expires_or_is_refused() {
    let dir = TempDir::new().unwrap();
    let wav = shareable_track(dir.path(), "expire.wav", "Daybreak");
    let easy_hex = "00".to_string() + &"ff".repeat(31);
    let impossible_hex = "00".repeat(32);
    let server = MockServer::start(vec![
        (404, String::new()),
        (404, String::new()),
        // First challenge cannot be solved within the (tiny) time limit.
        (200, challenge_json(&impossible_hex)),
        // Second one is solved but the server calls the token wrong...
        (200, challenge_json(&easy_hex)),
        (
            400,
            r#"{"code":400,"name":"IncorrectPublishTokenError","message":"x"}"#.into(),
        ),
        // ...so a third one is requested and accepted.
        (200, challenge_json(&easy_hex)),
        (201, String::new()),
    ]);
    let lib = empty_library(dir.path());
    let svc = LyricsService::with_base_url(&dir.path().join("cache"), &server.addr)
        .unwrap()
        .with_challenge_ttl(Duration::from_millis(100));
    let out = block_on(svc.share(lib, wav.to_string_lossy().into_owned(), None)).unwrap();
    assert_eq!(out, ShareOutcome::Published);
    let challenges = server
        .requests()
        .iter()
        .filter(|r| r.starts_with("POST /api/request-challenge"))
        .count();
    assert_eq!(challenges, 3);
}

#[test]
fn sharing_can_be_cancelled_and_is_refused_without_requirements() {
    let dir = TempDir::new().unwrap();
    let wav = shareable_track(dir.path(), "cancel.wav", "Daybreak");
    let impossible_hex = "00".repeat(32);
    let server = MockServer::start(vec![
        (404, String::new()),
        (404, String::new()),
        (200, challenge_json(&impossible_hex)),
    ]);
    let lib = empty_library(dir.path());
    let svc = LyricsService::with_base_url(&dir.path().join("cache"), &server.addr).unwrap();
    let path = wav.to_string_lossy().into_owned();
    let out = block_on(async {
        let canceller = svc.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(400)).await;
            canceller.cancel_share();
        });
        svc.share(Arc::clone(&lib), path.clone(), None).await
    });
    assert_eq!(out.unwrap(), ShareOutcome::Cancelled);
    assert!(
        !server
            .requests()
            .iter()
            .any(|r| r.starts_with("POST /api/publish"))
    );

    // No album: blocked before any request is made.
    let no_album = shareable_track(dir.path(), "noalbum.wav", "");
    let quiet = MockServer::start(vec![]);
    let svc = LyricsService::with_base_url(&dir.path().join("cache2"), &quiet.addr).unwrap();
    let status = block_on(svc.share_status(
        Arc::clone(&lib),
        no_album.to_string_lossy().into_owned(),
        None,
    ))
    .unwrap();
    assert!(!status.available);
    assert_eq!(status.reason, Some(ShareBlock::NeedsMetadata));
    assert!(
        block_on(svc.share(
            Arc::clone(&lib),
            no_album.to_string_lossy().into_owned(),
            None
        ))
        .is_err()
    );

    // No own lyrics at all.
    let bare = dir.path().join("bare.wav");
    write_tagged_wav(&bare, "Song", "Artist", "Album");
    let status =
        block_on(svc.share_status(Arc::clone(&lib), bare.to_string_lossy().into_owned(), None))
            .unwrap();
    assert_eq!(status.reason, Some(ShareBlock::NoLocalLyrics));
    assert!(quiet.requests().is_empty());

    // Lyrics already on LRCLIB: not offered.
    let on_lrclib = shareable_track(dir.path(), "there.wav", "Daybreak");
    let server = MockServer::start(vec![(200, synced_hit_body())]);
    let svc = LyricsService::with_base_url(&dir.path().join("cache3"), &server.addr).unwrap();
    let status = block_on(svc.share_status(
        Arc::clone(&lib),
        on_lrclib.to_string_lossy().into_owned(),
        None,
    ))
    .unwrap();
    assert_eq!(status.reason, Some(ShareBlock::OnLrclib));

    // Nothing there: offered.
    let server = MockServer::start(vec![(404, String::new()), (404, String::new())]);
    let svc = LyricsService::with_base_url(&dir.path().join("cache4"), &server.addr).unwrap();
    let status =
        block_on(svc.share_status(lib, on_lrclib.to_string_lossy().into_owned(), None)).unwrap();
    assert!(status.available);
}

// ---- bulk save and coverage ----

struct SmallLibrary {
    _dir: TempDir,
    lib: Arc<Library>,
    music: std::path::PathBuf,
    svc: LyricsService,
}

/// Five songs: a (sidecar), b (cached found), c (embedded tag), d (cached
/// not found), e (never looked up), plus b2 with different cached lyrics.
fn small_library() -> SmallLibrary {
    let dir = TempDir::new().unwrap();
    let music = dir.path().join("music");
    std::fs::create_dir_all(&music).unwrap();
    for (file, title) in [
        ("a.wav", "Alpha"),
        ("b.wav", "Bravo"),
        ("c.wav", "Charlie"),
        ("d.wav", "Delta"),
        ("e.wav", "Echo"),
        ("f.wav", "Foxtrot"),
    ] {
        write_tagged_wav(&music.join(file), title, "Band", "Record");
    }
    let lib = empty_library(dir.path());
    let id = lib.add_folder(music.to_str().unwrap()).unwrap();
    crate::library::scan::scan_folder(&lib, id, &music).unwrap();
    let svc =
        LyricsService::with_base_url(&dir.path().join("cache"), "http://127.0.0.1:1").unwrap();
    SmallLibrary {
        _dir: dir,
        lib,
        music,
        svc,
    }
}

impl SmallLibrary {
    fn cache_put(&self, title: &str, result: &LyricsResult) {
        let meta = self
            .lib
            .track_meta(
                self.music
                    .join(format!(
                        "{}.wav",
                        title.to_lowercase().chars().next().unwrap()
                    ))
                    .to_str()
                    .unwrap(),
            )
            .unwrap()
            .unwrap();
        let secs = Some((meta.duration_ms.unwrap() + 500) / 1000);
        let key = super::cache::cache_key("Band", title, "Record", secs);
        LyricsCache::new(&self._dir.path().join("cache"))
            .put(&key, result, super::cache::now_unix_secs())
            .unwrap();
    }
}

fn cached_lyrics(text: &str) -> LyricsResult {
    LyricsResult::Plain {
        source: LyricsSource::Lrclib,
        lines: vec![text.into()],
    }
}

#[test]
fn bulk_save_writes_cached_lyrics_with_the_chosen_mode() {
    let s = small_library();
    s.cache_put("Bravo", &cached_lyrics("bravo words"));
    s.cache_put("Echo", &cached_lyrics("echo words"));
    s.cache_put("Delta", &LyricsResult::NotFound);
    std::fs::write(s.music.join("a.lrc"), "[00:01.00]already").unwrap();
    s.cache_put("Alpha", &cached_lyrics("alpha cached"));

    // Off: nothing happens.
    let out = s.svc.save_cached_blocking(&s.lib, SaveMode::Off, None);
    assert_eq!((out.saved, out.skipped), (0, 0));

    // Sidecar: b and e are written; a already has a .lrc (skipped).
    let out = s.svc.save_cached_blocking(&s.lib, SaveMode::Sidecar, None);
    assert_eq!((out.saved, out.skipped), (2, 1));
    assert_eq!(
        std::fs::read_to_string(s.music.join("b.lrc")).unwrap(),
        "bravo words"
    );
    assert_eq!(
        std::fs::read_to_string(s.music.join("e.lrc")).unwrap(),
        "echo words"
    );
    assert_eq!(
        std::fs::read_to_string(s.music.join("a.lrc")).unwrap(),
        "[00:01.00]already"
    );
    assert!(!s.music.join("d.lrc").exists());
    assert!(!s.music.join("c.lrc").exists());

    // Running again changes nothing.
    let out = s.svc.save_cached_blocking(&s.lib, SaveMode::Sidecar, None);
    assert_eq!((out.saved, out.skipped), (0, 3));

    // Embed: the playing file is skipped, the others get the tag.
    for f in ["b", "e"] {
        std::fs::remove_file(s.music.join(format!("{f}.lrc"))).unwrap();
    }
    let playing = s.music.join("b.wav");
    let out = s
        .svc
        .save_cached_blocking(&s.lib, SaveMode::Embed, Some(&playing.to_string_lossy()));
    assert_eq!((out.saved, out.skipped), (2, 1));
    assert_eq!(tag_lyrics(&s.music.join("b.wav")), None);
    assert_eq!(
        tag_lyrics(&s.music.join("e.wav")).as_deref(),
        Some("echo words")
    );
    assert_eq!(
        tag_lyrics(&s.music.join("a.wav")).as_deref(),
        Some("alpha cached")
    );
    // An embedded tag is never replaced by a second run.
    let out = s.svc.save_cached_blocking(&s.lib, SaveMode::Embed, None);
    assert_eq!((out.saved, out.skipped), (1, 2));
    assert_eq!(
        tag_lyrics(&s.music.join("b.wav")).as_deref(),
        Some("bravo words")
    );
}

#[test]
fn coverage_counts_lyrics_checked_and_unchecked() {
    let s = small_library();
    // a: sidecar, b: cached found, c: embedded, d: cached not found,
    // e: instrumental (checked, no lyrics), f: never looked up.
    std::fs::write(s.music.join("a.lrc"), "[00:01.00]sidecar").unwrap();
    s.cache_put("Bravo", &cached_lyrics("bravo"));
    save::write_embedded(&s.music.join("c.wav"), "embedded words").unwrap();
    s.cache_put("Delta", &LyricsResult::NotFound);
    s.cache_put(
        "Echo",
        &LyricsResult::Instrumental {
            source: LyricsSource::Lrclib,
        },
    );
    let c = s.svc.coverage_blocking(&s.lib);
    assert_eq!((c.with_lyrics, c.without_lyrics, c.unchecked), (3, 2, 1));
    let json = serde_json::to_value(c).unwrap();
    assert_eq!(json["withLyrics"], 3);
    assert_eq!(json["withoutLyrics"], 2);
    assert_eq!(json["unchecked"], 1);
}

fn track_cache_key() -> String {
    super::cache::cache_key("Nadia Okafor", "Morning Light", "Daybreak", Some(2))
}

#[test]
fn offline_mode_and_the_lyrics_switch_make_no_request_and_cache_nothing() {
    use crate::net::{Feature, NetPolicy};
    let dir = TempDir::new().unwrap();
    let wav = dir.path().join("off.wav");
    write_tagged_wav(&wav, "Morning Light", "Nadia Okafor", "Daybreak");
    let lib = empty_library(dir.path());
    let lrclib = MockServer::start(vec![(200, synced_hit_body()), (200, synced_hit_body())]);
    let ne = MockServer::start(vec![]);
    let policy = Arc::new(NetPolicy::default());
    let cache_dir = dir.path().join("cache");
    let svc = LyricsService::with_base_urls(&cache_dir, &lrclib.addr, &ne.addr)
        .unwrap()
        .with_net_policy(Arc::clone(&policy));
    let cache = LyricsCache::new(&cache_dir);
    let now = super::cache::now_unix_secs();
    let o = opts(SaveMode::Off, true, None);

    // Offline mode: not found, no request to either service, and no
    // "not found" cache entry that would hide the song for a week.
    policy.set_offline(true);
    let res = lookup(&svc, &lib, &wav, false, &o).unwrap();
    assert_eq!(res, LyricsResult::NotFound);
    assert!(lrclib.requests().is_empty() && ne.requests().is_empty());
    assert_eq!(cache.get(&track_cache_key(), now), None);
    assert!(!wav.with_extension("lrc").exists());

    // The same with only the lyrics switch off.
    policy.set_offline(false);
    policy.set_feature(Feature::Lyrics, false);
    assert_eq!(
        lookup(&svc, &lib, &wav, true, &o).unwrap(),
        LyricsResult::NotFound
    );
    assert!(lrclib.requests().is_empty() && ne.requests().is_empty());
    assert_eq!(cache.get(&track_cache_key(), now), None);

    // The other switches do not stop lyrics.
    policy.set_feature(Feature::Lyrics, true);
    for f in [
        Feature::Online,
        Feature::ArtistInfo,
        Feature::Metadata,
        Feature::Updates,
    ] {
        policy.set_feature(f, false);
    }
    let found = lookup(&svc, &lib, &wav, false, &o).unwrap();
    assert!(matches!(found, LyricsResult::Synced { .. }));
    assert_eq!(lrclib.requests().len(), 1);

    // Cached lyrics and a sidecar still work offline.
    policy.set_offline(true);
    assert_eq!(lookup(&svc, &lib, &wav, false, &o).unwrap(), found);
    let side = dir.path().join("side.wav");
    write_tagged_wav(&side, "Other Song", "Nadia Okafor", "Daybreak");
    std::fs::write(side.with_extension("lrc"), "[00:01.00]Local line").unwrap();
    let local = lookup(&svc, &lib, &side, false, &o).unwrap();
    assert!(matches!(local, LyricsResult::Synced { .. }));
    assert_eq!(lrclib.requests().len(), 1);
    assert!(ne.requests().is_empty());
}

#[test]
fn the_netease_client_itself_refuses_when_lyrics_are_off() {
    use crate::net::{Feature, NetPolicy};
    let ne = MockServer::start(vec![(200, netease_search_json())]);
    let policy = Arc::new(NetPolicy::default());
    let client = netease::NeteaseClient::new(ne.addr.clone())
        .unwrap()
        .with_policy(Arc::clone(&policy));
    policy.set_feature(Feature::Lyrics, false);
    let err = block_on(client.search("anything")).unwrap_err();
    assert!(matches!(err, LyricsError::Disabled), "{err:?}");
    assert!(ne.requests().is_empty());
    policy.set_feature(Feature::Lyrics, true);
    assert!(block_on(client.search("anything")).is_ok());
    assert_eq!(ne.requests().len(), 1);
}

#[test]
fn sharing_is_refused_without_requests_when_lyrics_are_off() {
    use crate::net::{Feature, NetPolicy};
    let dir = TempDir::new().unwrap();
    let wav = shareable_track(dir.path(), "offshare.wav", "Daybreak");
    let path = wav.to_string_lossy().into_owned();
    let lib = empty_library(dir.path());
    let server = MockServer::start(vec![]);

    let offline = NetPolicy::default();
    offline.set_offline(true);
    let switched_off = NetPolicy::default();
    switched_off.set_feature(Feature::Lyrics, false);
    for policy in [offline, switched_off] {
        let svc = LyricsService::with_base_url(&dir.path().join("cache"), &server.addr)
            .unwrap()
            .with_net_policy(Arc::new(policy));
        let status = block_on(svc.share_status(Arc::clone(&lib), path.clone(), None)).unwrap();
        assert!(!status.available);
        let err = block_on(svc.share(Arc::clone(&lib), path.clone(), None)).unwrap_err();
        assert_eq!(err.to_string(), "disabled in Settings");
    }
    assert!(server.requests().is_empty());
}

// ---- live (network) checks: cargo test -- --ignored ----

#[test]
#[ignore = "talks to music.163.com"]
fn live_netease_search_and_lyric() {
    let client = netease::NeteaseClient::new(netease::DEFAULT_BASE_URL).unwrap();
    let q = query("Hello", "Adele", Some(295));
    let res = block_on(client.fetch(&q)).unwrap();
    println!("netease live result: {res:?}");
    assert!(res.is_some(), "expected lyrics for a very common song");
}

#[test]
#[ignore = "talks to lrclib.net (requests a challenge only, publishes nothing)"]
fn live_lrclib_challenge() {
    let client = super::lrclib::LrclibClient::new(super::lrclib::DEFAULT_BASE_URL).unwrap();
    let challenge = block_on(client.request_challenge()).unwrap();
    assert!(!challenge.prefix.is_empty());
    assert!(publish::decode_target(&challenge.target).is_some());
}
