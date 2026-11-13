use super::cache::{LyricsCache, NOT_FOUND_TTL_SECS, cache_key};
use super::lrc::{ParsedLyrics, parse};
use super::lrclib::LrclibRecord;
use super::{
    LyricLine, LyricsError, LyricsResult, LyricsService, LyricsSource, attach_local_lrc,
    find_local_lrc,
};
use crate::library::{Hooks, Library};
use lofty::config::WriteOptions;
use lofty::prelude::*;
use lofty::tag::{Tag, TagType};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
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
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let mut buf = [0u8; 4096];
                        let mut req = Vec::new();
                        while let Ok(n) = stream.read(&mut buf) {
                            if n == 0 {
                                break;
                            }
                            req.extend_from_slice(&buf[..n]);
                            if req.windows(4).any(|w| w == b"\r\n\r\n") {
                                break;
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
            .contains("user-agent: foomodern/")
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
