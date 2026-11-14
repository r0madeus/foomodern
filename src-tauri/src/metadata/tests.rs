//! Metadata matching tests: fixtures for parsing, pure scoring tests, and an
//! end-to-end run against a local mock server. No internet, no sound card.

use super::musicbrainz::{
    MbClient, MbConfig, RecordingSearch, ReleaseSearch, credit_string, lucene_escape,
    recording_query, release_query,
};
use super::scoring::{
    CandRelease, LocalAlbum, LocalTrack, levenshtein, pick_release, score_album, similarity,
};
use super::{MetadataError, MetadataService, preview_dir};
use crate::library::tags::{read_mb_ids, read_tags};
use crate::library::{Hooks, Library};
use lofty::config::WriteOptions;
use lofty::prelude::*;
use lofty::tag::{Tag, TagType};
use std::io::{Cursor, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

const SEARCH: &str = include_str!("fixtures/release_search.json");
const STANDARD: &str = include_str!("fixtures/release_lookup_standard.json");
const DELUXE: &str = include_str!("fixtures/release_lookup_deluxe.json");
const RECORDINGS: &str = include_str!("fixtures/recording_search.json");

const RG: &str = "bbbbbbbb-0000-4000-8000-000000000001";
const REL: &str = "aaaaaaaa-0000-4000-8000-000000000001";

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fut)
}

fn standard() -> CandRelease {
    CandRelease::from_mb(&serde_json::from_str(STANDARD).unwrap())
}

fn deluxe() -> CandRelease {
    CandRelease::from_mb(&serde_json::from_str(DELUXE).unwrap())
}

/// A local album that exactly mirrors the candidate's tracks.
fn local_from(c: &CandRelease) -> LocalAlbum {
    LocalAlbum {
        title: c.title.clone(),
        artist: c.artist.clone(),
        year: c.year(),
        tracks: c
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| LocalTrack {
                id: i as i64 + 1,
                path: format!("/m/{i}.mp3"),
                title: t.title.clone(),
                artist: t.artist.clone(),
                track_no: Some(t.position),
                disc_no: Some(t.disc),
                duration_ms: t.length_ms,
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------

#[test]
fn parses_release_search() {
    let s: ReleaseSearch = serde_json::from_str(SEARCH).unwrap();
    assert_eq!(s.releases.len(), 3);
    let r = &s.releases[0];
    assert_eq!(r.id, REL);
    assert_eq!(r.score, Some(100));
    assert_eq!(r.date.as_deref(), Some("2019-03-08"));
    assert_eq!(r.track_count, Some(4));
    assert_eq!(r.media[0].format.as_deref(), Some("CD"));
    let g = r.release_group.as_ref().unwrap();
    assert_eq!(g.primary_type.as_deref(), Some("Album"));
    assert!(g.secondary_types.is_empty());
    assert_eq!(credit_string(&r.artist_credit), "The Lanterns");
    assert_eq!(
        s.releases[2]
            .release_group
            .as_ref()
            .unwrap()
            .secondary_types,
        ["Live"]
    );
    assert_eq!(s.releases[1].date.as_deref(), Some("2020"));
    assert_eq!(s.releases[2].date, None);
}

#[test]
fn parses_release_lookup_into_flat_tracks() {
    let c = standard();
    assert_eq!(c.id, REL);
    assert_eq!(c.release_group_id, RG);
    assert_eq!(c.year(), Some(2019));
    assert_eq!(c.format.as_deref(), Some("CD"));
    assert_eq!(c.tracks.len(), 4);
    assert_eq!(c.tracks[1].title, "Paper Moons");
    assert_eq!(c.tracks[1].position, 2);
    assert_eq!(c.tracks[1].length_ms, Some(215_000));
    assert_eq!(c.tracks[1].track_id, "std-t2");
    assert_eq!(c.tracks[1].recording_id, "std-r2");
    assert_eq!(c.tracks[3].artist, "The Lanterns");
}

#[test]
fn parses_recording_search() {
    let s: RecordingSearch = serde_json::from_str(RECORDINGS).unwrap();
    assert_eq!(s.recordings.len(), 2);
    let r = &s.recordings[0];
    assert_eq!(r.length, Some(200_000));
    assert_eq!(r.releases.len(), 5);
    let m = &r.releases[0].media[0];
    assert_eq!(m.track_offset, Some(6));
    assert_eq!(m.track[0].number, "7");
}

#[test]
fn parsing_is_tolerant() {
    let s: ReleaseSearch =
        serde_json::from_str(r#"{"releases":[{"id":"x","media":[{}]}],"extra":1}"#).unwrap();
    assert_eq!(s.releases[0].id, "x");
    let none: RecordingSearch = serde_json::from_str("{}").unwrap();
    assert!(none.recordings.is_empty());
}

#[test]
fn artist_credit_joins_names_and_phrases() {
    let r: super::musicbrainz::MbRelease = serde_json::from_str(
        r#"{"artist-credit":[{"name":"A","joinphrase":" feat. ","artist":{"name":"A"}},{"name":"B","joinphrase":""}]}"#,
    )
    .unwrap();
    assert_eq!(credit_string(&r.artist_credit), "A feat. B");
}

// ---------------------------------------------------------------------
// Lucene and text similarity
// ---------------------------------------------------------------------

#[test]
fn lucene_escaping() {
    assert_eq!(lucene_escape("plain words"), "plain words");
    assert_eq!(lucene_escape("AC/DC"), "AC\\/DC");
    assert_eq!(
        lucene_escape(r#"a+b-c!d(e)f{g}h[i]j^k"l~m*n?o:p\q/r"#),
        r#"a\+b\-c\!d\(e\)f\{g\}h\[i\]j\^k\"l\~m\*n\?o\:p\\q\/r"#
    );
    assert_eq!(lucene_escape("a && b || c"), "a \\&\\& b \\|\\| c");
    assert_eq!(
        release_query("Who's \"Next\"", "The Who", Some(9)),
        r#"release:"Who's \"Next\"" AND artist:"The Who" AND tracks:9"#
    );
    assert_eq!(
        release_query("A", "B", None),
        r#"release:"A" AND artist:"B""#
    );
    assert_eq!(
        recording_query("T:1", "X"),
        r#"recording:"T\:1" AND artist:"X""#
    );
}

#[test]
fn turkish_dotted_capital_i_compares_like_plain_i() {
    use super::scoring::{normalize, similarity};
    assert_eq!(normalize("İSTANBUL"), "istanbul");
    assert_eq!(similarity("ISTANBUL", "İstanbul"), 1.0);
}

#[test]
fn similarity_basics() {
    let l = |a: &str, b: &str| {
        levenshtein(
            &a.chars().collect::<Vec<_>>(),
            &b.chars().collect::<Vec<_>>(),
        )
    };
    assert_eq!(l("kitten", "sitting"), 3);
    assert_eq!(l("", "abc"), 3);
    assert_eq!(similarity("Hello, World!", "hello world"), 1.0);
    assert_eq!(similarity("", "x"), 0.0);
    assert!(similarity("Glass Harbor", "Glass Harbour") > 0.9);
    assert!(similarity("Abc", "Xyz") < 0.1);
    assert_eq!(similarity("İstanbul", "İstanbul"), 1.0);
}

// ---------------------------------------------------------------------
// Album scoring
// ---------------------------------------------------------------------

#[test]
fn exact_match_is_recommended() {
    let cand = standard();
    let (score, assignment) = score_album(&local_from(&cand), &cand);
    assert!(score >= 85, "score {score}");
    assert_eq!(assignment, vec![Some(0), Some(1), Some(2), Some(3)]);
}

#[test]
fn deluxe_scores_lower_than_standard() {
    let local = local_from(&standard());
    let (std_score, _) = score_album(&local, &standard());
    let (dlx_score, assignment) = score_album(&local, &deluxe());
    assert!(dlx_score < std_score, "{dlx_score} !< {std_score}");
    // The four shared tracks are still found, by title and duration.
    assert_eq!(assignment, vec![Some(0), Some(1), Some(2), Some(3)]);
    // The other way round: local tracks without a counterpart cost points.
    let (rev, rev_assignment) = score_album(&local_from(&deluxe()), &standard());
    assert!(rev < std_score);
    assert_eq!(rev_assignment.iter().filter(|a| a.is_none()).count(), 2);
}

#[test]
fn wrong_durations_drop_below_the_threshold() {
    let cand = standard();
    let mut local = local_from(&cand);
    for t in &mut local.tracks {
        t.duration_ms = t.duration_ms.map(|d| d + 40_000);
    }
    let (score, _) = score_album(&local, &cand);
    assert!(score < 50, "score {score}");
}

#[test]
fn unnumbered_shuffled_tracks_still_assign_by_title_and_duration() {
    let cand = standard();
    let mut local = local_from(&cand);
    for t in &mut local.tracks {
        t.track_no = None;
        t.disc_no = None;
    }
    local.tracks.reverse();
    let (score, assignment) = score_album(&local, &cand);
    assert_eq!(assignment, vec![Some(3), Some(2), Some(1), Some(0)]);
    assert!(score >= 85, "score {score}");
}

#[test]
fn missing_durations_are_neutral() {
    let cand = standard();
    let mut local = local_from(&cand);
    for t in &mut local.tracks {
        t.duration_ms = None;
    }
    let (score, _) = score_album(&local, &cand);
    assert!((50..85).contains(&score), "score {score}");
}

#[test]
fn wrong_track_titles_score_low() {
    let cand = standard();
    let mut local = local_from(&cand);
    for (i, t) in local.tracks.iter_mut().enumerate() {
        t.title = format!("zzzz qqqq {i}");
        t.duration_ms = Some(60_000 * (i as u64 + 1));
    }
    local.title = "Something Else".into();
    let (score, _) = score_album(&local, &cand);
    assert!(score < 50, "score {score}");
}

// ---------------------------------------------------------------------
// Release choice
// ---------------------------------------------------------------------

#[test]
fn pick_release_prefers_studio_album_then_earliest() {
    let s: RecordingSearch = serde_json::from_str(RECORDINGS).unwrap();
    let releases = &s.recordings[0].releases;
    // Compilation, live, single and two plain albums: the earlier album wins.
    let picked = pick_release(releases).unwrap();
    assert_eq!(picked.id, "dddddddd-0000-4000-8000-000000000004");

    // Without an album, a single beats compilation and live.
    let no_album: Vec<_> = releases[..3].to_vec();
    assert_eq!(
        pick_release(&no_album).unwrap().id,
        "dddddddd-0000-4000-8000-000000000003"
    );

    // Official beats an older bootleg album.
    let mut mixed = vec![releases[3].clone(), releases[0].clone()];
    mixed[0].status = Some("Bootleg".into());
    assert_eq!(pick_release(&mixed).unwrap().id, releases[0].id);

    // Undated releases come after dated ones.
    let mut undated = vec![releases[4].clone(), releases[3].clone()];
    undated[0].date = None;
    assert_eq!(pick_release(&undated).unwrap().id, releases[3].id);

    assert!(pick_release(&[]).is_none());
}

// ---------------------------------------------------------------------
// Mock server
// ---------------------------------------------------------------------

struct Reply {
    code: u16,
    content_type: &'static str,
    body: Vec<u8>,
    location: Option<String>,
}

impl Reply {
    fn json(body: &str) -> Self {
        Self {
            code: 200,
            content_type: "application/json",
            body: body.as_bytes().to_vec(),
            location: None,
        }
    }
    fn status(code: u16) -> Self {
        Self {
            code,
            content_type: "text/plain",
            body: Vec::new(),
            location: None,
        }
    }
    fn png(body: Vec<u8>) -> Self {
        Self {
            code: 200,
            content_type: "image/png",
            body,
            location: None,
        }
    }
    fn redirect(to: &str) -> Self {
        Self {
            code: 307,
            content_type: "text/plain",
            body: Vec::new(),
            location: Some(to.into()),
        }
    }
}

type Handler = dyn Fn(&str) -> Reply + Send + Sync;

struct Mock {
    base: String,
    requests: Arc<Mutex<Vec<(String, Instant)>>>,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Mock {
    fn start(handler: Arc<Handler>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (reqs, st) = (Arc::clone(&requests), Arc::clone(&stop));
        let handle = thread::spawn(move || {
            while !st.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
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
                        let text = String::from_utf8_lossy(&req).into_owned();
                        let target = text
                            .lines()
                            .next()
                            .and_then(|l| l.split_whitespace().nth(1))
                            .unwrap_or("/")
                            .to_owned();
                        reqs.lock().unwrap().push((target.clone(), Instant::now()));
                        let r = handler(&target);
                        let mut head = format!(
                            "HTTP/1.1 {} X\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                            r.code,
                            r.content_type,
                            r.body.len()
                        );
                        if let Some(l) = r.location {
                            head.push_str(&format!("Location: {l}\r\n"));
                        }
                        head.push_str("\r\n");
                        let _ = stream.write_all(head.as_bytes());
                        let _ = stream.write_all(&r.body);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            base,
            requests,
            stop,
            handle: Some(handle),
        }
    }

    fn targets(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.0.clone())
            .collect()
    }

    fn config(&self) -> MbConfig {
        MbConfig {
            mb_base_url: format!("{}/ws/2", self.base),
            caa_base_url: format!("{}/caa", self.base),
            min_interval: Duration::ZERO,
            retry_delay: Duration::from_millis(20),
        }
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn png_bytes() -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    image::RgbImage::new(4, 4)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

#[test]
fn rate_limit_spaces_requests() {
    let mock = Mock::start(Arc::new(|_| Reply::json(r#"{"releases":[{"id":"x"}]}"#)));
    let mut cfg = mock.config();
    cfg.min_interval = Duration::from_millis(200);
    let client = MbClient::new(cfg).unwrap();
    block_on(async {
        client.search_releases("A", "B", None).await.unwrap();
        client.search_releases("A", "B", None).await.unwrap();
        client.search_releases("A", "B", None).await.unwrap();
    });
    let times: Vec<Instant> = mock.requests.lock().unwrap().iter().map(|r| r.1).collect();
    assert_eq!(times.len(), 3);
    for w in times.windows(2) {
        assert!(
            w[1] - w[0] >= Duration::from_millis(190),
            "{:?}",
            w[1] - w[0]
        );
    }
}

#[test]
fn retries_once_on_503() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&calls);
    let mock = Mock::start(Arc::new(move |_| {
        if c.fetch_add(1, Ordering::SeqCst) == 0 {
            Reply::status(503)
        } else {
            Reply::json(SEARCH)
        }
    }));
    let client = MbClient::new(mock.config()).unwrap();
    let (found, used) = block_on(client.search_releases("A", "B", Some(4))).unwrap();
    assert_eq!(found.len(), 3);
    assert_eq!(used, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    // A second 503 in a row is an error.
    let always = Mock::start(Arc::new(|_| Reply::status(503)));
    let client = MbClient::new(always.config()).unwrap();
    assert!(block_on(client.search_releases("A", "B", None)).is_err());
    assert_eq!(always.targets().len(), 2);
}

#[test]
fn query_is_sent_with_json_format_and_retried_without_track_count() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&calls);
    let mock = Mock::start(Arc::new(move |_| {
        if c.fetch_add(1, Ordering::SeqCst) == 0 {
            Reply::json(r#"{"releases":[]}"#)
        } else {
            Reply::json(SEARCH)
        }
    }));
    let client = MbClient::new(mock.config()).unwrap();
    let (found, used) =
        block_on(client.search_releases("Night Signals", "The Lanterns", Some(4))).unwrap();
    assert_eq!((found.len(), used), (3, 2));
    let t = mock.targets();
    assert!(t[0].starts_with("/ws/2/release/?"), "{}", t[0]);
    assert!(t[0].contains("fmt=json") && t[0].contains("limit=8"));
    assert!(t[0].contains("tracks%3A4"), "{}", t[0]);
    assert!(!t[1].contains("tracks"), "{}", t[1]);
}

// ---------------------------------------------------------------------
// End to end
// ---------------------------------------------------------------------

fn write_tagged_wav(path: &Path, title: &str, artist: &str, track: u32) {
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
    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_title(title.into());
    tag.set_artist(artist.into());
    tag.set_album("Night Signals".into());
    tag.insert_text(ItemKey::AlbumArtist, "Lanterns".into());
    tag.set_track(track);
    tag.save_to_path(path, WriteOptions::default()).unwrap();
}

/// Release lookup for the three local test files (1 s long).
fn e2e_lookup() -> String {
    let artist = serde_json::json!([{"name":"The Lanterns","joinphrase":"","artist":{"id":"a1","name":"The Lanterns"}}]);
    let titles = ["Night Signals", "Paper Moons", "Glass Harbour"];
    let tracks: Vec<_> = titles
        .iter()
        .enumerate()
        .map(|(i, t)| {
            serde_json::json!({"id": format!("trk{}", i + 1), "number": (i + 1).to_string(),
                "position": i + 1, "title": t, "length": 1000,
                "recording": {"id": format!("rec{}", i + 1), "title": t, "length": 1000}})
        })
        .collect();
    serde_json::json!({
        "id": REL, "title": "Night Signals", "date": "2019-03-08", "country": "GB",
        "status": "Official",
        "release-group": {"id": RG, "primary-type": "Album", "secondary-types": []},
        "artist-credit": artist,
        "media": [{"position": 1, "format": "CD", "tracks": tracks}]
    })
    .to_string()
}

fn caa_reply(path: &str) -> Reply {
    // front-250 goes through a redirect like the real archive.
    if path.starts_with("/caa/release-group/") && path.ends_with("/front-250") {
        return Reply::redirect("/img/small.png");
    }
    if path.starts_with("/caa/release-group/") && path.ends_with("/front-500") {
        return Reply::redirect("/img/big.png");
    }
    Reply::png(png_bytes())
}

fn handler() -> Arc<Handler> {
    let lookup = e2e_lookup();
    Arc::new(move |path| {
        if path.starts_with("/ws/2/release/?") {
            Reply::json(SEARCH)
        } else if path.starts_with(&format!("/ws/2/release/{REL}")) {
            Reply::json(&lookup)
        } else if path.starts_with("/ws/2/release/") {
            Reply::status(404) // the other candidates cannot be looked up
        } else if path.starts_with("/ws/2/recording/?") {
            Reply::json(&RECORDINGS.replace("200000", "1000"))
        } else if path.starts_with("/caa/") || path.starts_with("/img/") {
            caa_reply(path)
        } else {
            Reply::status(404)
        }
    })
}

struct Env {
    _tmp: TempDir,
    music: PathBuf,
    lib: Arc<Library>,
    files: Vec<PathBuf>,
}

fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let music = tmp.path().join("music");
    let lib = Arc::new(
        Library::open(
            &tmp.path().join("data/library.sqlite3"),
            &tmp.path().join("cache/covers"),
            Hooks::none(),
        )
        .unwrap(),
    );
    std::fs::create_dir_all(&music).unwrap();
    let folder_id = lib.add_folder(music.to_str().unwrap()).unwrap();
    let files: Vec<PathBuf> = [
        ("night signals", "Lanterns"),
        ("paper moons", "Lanterns"),
        ("glass harbor", "Lanterns"),
    ]
    .iter()
    .enumerate()
    .map(|(i, (t, a))| {
        let p = music.join(format!("{:02}.wav", i + 1));
        write_tagged_wav(&p, t, a, i as u32 + 1);
        p
    })
    .collect();
    crate::library::scan::scan_folder(&lib, folder_id, &music).unwrap();
    Env {
        _tmp: tmp,
        music,
        lib,
        files,
    }
}

impl Env {
    fn album_id(&self) -> i64 {
        let albums = self.lib.albums_page(0, 10).unwrap();
        assert_eq!(albums.len(), 1);
        albums[0].id
    }
}

#[test]
fn match_apply_and_undo_album() {
    let env = env();
    let mock = Mock::start(handler());
    let svc = MetadataService::with_config(mock.config()).unwrap();
    let album_id = env.album_id();
    assert!(env.lib.album(album_id).unwrap().album.cover.is_none());

    let candidates = block_on(svc.match_album(&env.lib, album_id)).unwrap();
    assert_eq!(candidates.len(), 1, "only the lookup that worked is scored");
    let c = &candidates[0];
    assert_eq!(c.release_id, REL);
    assert_eq!(c.label, "recommended", "score {}", c.score);
    assert_eq!(c.track_count, 3);
    assert_eq!(c.year, Some(2019));
    assert_eq!(c.tracks.len(), 3);
    assert!(c.tracks.iter().all(|t| t.matched));
    let preview = c.cover_preview.clone().expect("preview downloaded");
    assert!(Path::new(&preview).is_file());
    assert!(Path::new(&preview).starts_with(preview_dir(&env.lib)));
    let names: Vec<&str> = c.album_fields.iter().map(|f| f.field).collect();
    assert_eq!(names, ["albumArtist", "year"]);
    // A pure casing fix is a visible change.
    assert_eq!(c.tracks[1].fields[0].old.as_deref(), Some("paper moons"));
    assert_eq!(c.tracks[1].fields[0].new.as_deref(), Some("Paper Moons"));
    assert_eq!(c.tracks[0].fields[0].field, "title");
    // Requests: search, lookups, previews; MusicBrainz ones <= 5.
    let mb_requests = mock
        .targets()
        .iter()
        .filter(|t| t.starts_with("/ws/2/"))
        .count();
    assert!(mb_requests <= 5, "{mb_requests}");
    // Nothing was written by matching.
    assert_eq!(
        read_tags(&env.files[0]).unwrap().title.as_deref(),
        Some("night signals")
    );

    let outcome = block_on(svc.apply(&env.lib, c.proposal.clone(), None)).unwrap();
    assert_eq!(outcome.updated, 3);
    assert!(outcome.failed.is_empty());
    assert!(outcome.batch_id > 0);
    assert_eq!(
        env.lib.tag_history_batch(outcome.batch_id).unwrap().len(),
        3
    );

    let t = read_tags(&env.files[0]).unwrap();
    assert_eq!(t.title.as_deref(), Some("Night Signals"));
    assert_eq!(t.artist.as_deref(), Some("The Lanterns"));
    assert_eq!(t.album_artist.as_deref(), Some("The Lanterns"));
    assert_eq!(t.year, Some(2019));
    assert_eq!(t.track_no, Some(1));
    assert_eq!(
        read_tags(&env.files[2]).unwrap().title.as_deref(),
        Some("Glass Harbour")
    );
    let ids = read_mb_ids(&env.files[1]).unwrap();
    assert_eq!(ids.release_id.as_deref(), Some(REL));
    assert_eq!(ids.release_group_id.as_deref(), Some(RG));
    assert_eq!(ids.recording_id.as_deref(), Some("rec2"));
    assert_eq!(ids.track_id.as_deref(), Some("trk2"));

    // The library follows: one album with the new artist and year, a cover.
    let albums = env.lib.albums_page(0, 10).unwrap();
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].album_artist, "The Lanterns");
    assert_eq!(albums[0].year, Some(2019));
    let cover = albums[0].cover.clone().expect("cover added");
    assert!(Path::new(&cover).is_file());
    assert_eq!(outcome.cover.as_deref(), Some(cover.as_str()));
    let detail = env.lib.album(albums[0].id).unwrap();
    assert_eq!(detail.tracks[2].title, "Glass Harbour");
    assert_eq!(detail.tracks[0].artist, "The Lanterns");

    // Undo restores every tag and clears the ids that were not there.
    let undone = block_on(svc.undo(&env.lib, outcome.batch_id, None)).unwrap();
    assert_eq!(undone.updated, 3);
    assert!(undone.failed.is_empty());
    assert!(
        env.lib
            .tag_history_batch(outcome.batch_id)
            .unwrap()
            .is_empty()
    );
    let t = read_tags(&env.files[0]).unwrap();
    assert_eq!(t.title.as_deref(), Some("night signals"));
    assert_eq!(t.artist.as_deref(), Some("Lanterns"));
    assert_eq!(t.album_artist.as_deref(), Some("Lanterns"));
    assert_eq!(t.year, None);
    assert_eq!(read_mb_ids(&env.files[1]).unwrap(), Default::default());
    assert_eq!(
        env.lib.albums_page(0, 10).unwrap()[0].album_artist,
        "Lanterns"
    );
    // A second undo has nothing left.
    assert!(block_on(svc.undo(&env.lib, outcome.batch_id, None)).is_err());
    let _ = &env.music;
}

#[test]
fn offline_mode_and_the_metadata_switch_make_no_request() {
    use crate::net::{Feature, NetPolicy};
    let env = env();
    let mock = Mock::start(handler());
    let policy = Arc::new(NetPolicy::default());
    let svc = MetadataService::with_config(mock.config())
        .unwrap()
        .with_net_policy(Arc::clone(&policy));
    let album_id = env.album_id();
    let track_id = env.lib.album(album_id).unwrap().tracks[0].id;

    // Offline mode, then only the metadata switch.
    for offline in [true, false] {
        policy.set_offline(offline);
        policy.set_feature(Feature::Metadata, offline);
        let e = block_on(svc.match_album(&env.lib, album_id)).unwrap_err();
        assert!(matches!(e, MetadataError::Disabled), "{e:?}");
        assert_eq!(e.to_string(), "disabled in Settings");
        let e = block_on(svc.match_track(&env.lib, track_id)).unwrap_err();
        assert!(matches!(e, MetadataError::Disabled), "{e:?}");
        // Other switches do not matter for matching.
        policy.set_offline(false);
        policy.set_feature(Feature::Metadata, false);
        policy.set_feature(Feature::Lyrics, true);
        assert!(block_on(svc.match_album(&env.lib, album_id)).is_err());
    }
    assert!(mock.targets().is_empty(), "{:?}", mock.targets());

    // The low-level client refuses too (cover archive included).
    let client = MbClient::new(mock.config())
        .unwrap()
        .with_policy(Arc::clone(&policy));
    assert!(block_on(client.search_releases("A", "B", None)).is_err());
    assert!(block_on(client.front_cover(RG, 250)).is_err());
    assert!(mock.targets().is_empty());

    // Everything else off, metadata on: matching works.
    for f in [
        Feature::Lyrics,
        Feature::Online,
        Feature::ArtistInfo,
        Feature::Updates,
    ] {
        policy.set_feature(f, false);
    }
    policy.set_feature(Feature::Metadata, true);
    assert!(block_on(svc.match_album(&env.lib, album_id)).is_ok());
    assert!(!mock.targets().is_empty());

    // Applying a proposal is local: it still writes tags offline, and adds
    // no cover request.
    let candidates = block_on(svc.match_album(&env.lib, album_id)).unwrap();
    policy.set_offline(true);
    let before = mock.targets().len();
    let outcome = block_on(svc.apply(&env.lib, candidates[0].proposal.clone(), None)).unwrap();
    assert_eq!(outcome.updated, 3);
    assert_eq!(mock.targets().len(), before);
}

#[test]
fn apply_skips_the_playing_file_and_reports_it() {
    let env = env();
    let mock = Mock::start(handler());
    let svc = MetadataService::with_config(mock.config()).unwrap();
    let candidates = block_on(svc.match_album(&env.lib, env.album_id())).unwrap();
    let playing = env.files[0].to_string_lossy().into_owned();
    let outcome = block_on(svc.apply(
        &env.lib,
        candidates[0].proposal.clone(),
        Some(playing.clone()),
    ))
    .unwrap();
    assert_eq!(outcome.updated, 2);
    assert_eq!(outcome.failed.len(), 1);
    assert_eq!(outcome.failed[0].0, playing);
    assert_eq!(
        env.lib.tag_history_batch(outcome.batch_id).unwrap().len(),
        2
    );
    assert_eq!(
        read_tags(&env.files[0]).unwrap().title.as_deref(),
        Some("night signals")
    );
}

#[test]
fn match_apply_and_undo_single_track() {
    let env = env();
    let mock = Mock::start(handler());
    let svc = MetadataService::with_config(mock.config()).unwrap();
    let track_id = env.lib.album(env.album_id()).unwrap().tracks[0].id;

    let candidates = block_on(svc.match_track(&env.lib, track_id)).unwrap();
    // The second recording is 172 s... after the fixture rewrite 172000 stays
    // and is rejected by the duration filter (local file is 1 s long).
    assert_eq!(candidates.len(), 1);
    let c = &candidates[0];
    assert_eq!(c.recording_id, "cccccccc-0000-4000-8000-000000000001");
    assert_eq!(c.release_id, "dddddddd-0000-4000-8000-000000000004");
    assert_eq!(c.release_group_id, RG);
    assert_eq!(c.album, "Night Signals");
    assert_eq!(c.year, Some(2019));
    assert!(c.score >= 85);
    let fields: Vec<&str> = c.track.fields.iter().map(|f| f.field).collect();
    assert_eq!(fields, ["title", "artist", "albumArtist", "discNo", "year"]);
    assert_eq!(c.proposal.tracks.len(), 1);

    let outcome = block_on(svc.apply(&env.lib, c.proposal.clone(), None)).unwrap();
    assert_eq!(outcome.updated, 1);
    let t = read_tags(&env.files[0]).unwrap();
    assert_eq!(t.title.as_deref(), Some("Night Signals"));
    assert_eq!(t.artist.as_deref(), Some("The Lanterns"));
    assert_eq!(t.album.as_deref(), Some("Night Signals"));
    assert_eq!(t.track_no, Some(1));
    assert_eq!(t.year, Some(2019));
    let ids = read_mb_ids(&env.files[0]).unwrap();
    assert_eq!(
        ids.recording_id.as_deref(),
        Some("cccccccc-0000-4000-8000-000000000001")
    );
    assert_eq!(
        ids.release_id.as_deref(),
        Some("dddddddd-0000-4000-8000-000000000004")
    );

    block_on(svc.undo(&env.lib, outcome.batch_id, None)).unwrap();
    let t = read_tags(&env.files[0]).unwrap();
    assert_eq!(t.title.as_deref(), Some("night signals"));
    assert_eq!(t.year, None);
}

#[test]
fn unreachable_server_is_an_error() {
    let env = env();
    let cfg = MbConfig {
        mb_base_url: "http://127.0.0.1:9/ws/2".into(),
        caa_base_url: "http://127.0.0.1:9/caa".into(),
        min_interval: Duration::ZERO,
        ..MbConfig::default()
    };
    let svc = MetadataService::with_config(cfg).unwrap();
    let err = block_on(svc.match_album(&env.lib, env.album_id())).unwrap_err();
    assert!(err.to_string().contains("could not be reached"), "{err}");
}

/// Talks to the real MusicBrainz. Run by hand:
/// `cargo test live_musicbrainz -- --ignored --nocapture`
#[test]
#[ignore = "needs internet access to musicbrainz.org"]
fn live_musicbrainz_search_and_lookup() {
    let client = MbClient::new(MbConfig::default()).unwrap();
    block_on(async {
        let (found, _) = client
            .search_releases("OK Computer", "Radiohead", Some(12))
            .await
            .unwrap();
        assert!(!found.is_empty());
        let full = client.lookup_release(&found[0].id).await.unwrap();
        let cand = CandRelease::from_mb(&full);
        assert!(cand.tracks.len() >= 10, "{} tracks", cand.tracks.len());
        let recs = client
            .search_recordings("Paranoid Android", "Radiohead")
            .await
            .unwrap();
        assert!(!recs.is_empty());
        let cover = client
            .front_cover(&cand.release_group_id, 250)
            .await
            .unwrap();
        println!("cover bytes: {:?}", cover.map(|c| c.len()));
    });
}
