//! Integration tests against real libmpv with the `null` audio output.

use foomodern_lib::audio::mpv::{MpvEngine, MpvOptions};
use foomodern_lib::audio::{AudioEngine, EndReason, EngineError, EngineEvent, ReplayGain};
use foomodern_lib::player::{Player, PlayerEvent, Status};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(10);

fn write_wav(dir: &Path, name: &str, freq: f32, seconds: f32) -> PathBuf {
    let path = dir.join(name);
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    let n = (44_100.0 * seconds) as usize;
    for i in 0..n {
        let t = i as f32 / 44_100.0;
        let s = (t * freq * std::f32::consts::TAU).sin();
        w.write_sample((s * i16::MAX as f32 * 0.3) as i16).unwrap();
    }
    w.finalize().unwrap();
    path
}

fn engine() -> (MpvEngine, Receiver<EngineEvent>) {
    engine_with(true)
}

/// `untimed: false` plays in real time (still no sound card needed).
fn engine_with(untimed: bool) -> (MpvEngine, Receiver<EngineEvent>) {
    let (tx, rx) = mpsc::channel();
    let engine = MpvEngine::new(
        MpvOptions {
            audio_output: Some("null".into()),
            untimed,
        },
        tx,
    )
    .expect("mpv engine");
    (engine, rx)
}

/// Keeps every received event so tests can assert on order.
struct Log {
    rx: Receiver<EngineEvent>,
    seen: Vec<EngineEvent>,
}

impl Log {
    fn new(rx: Receiver<EngineEvent>) -> Log {
        Log {
            rx,
            seen: Vec::new(),
        }
    }

    /// Wait until an event matching `pred` arrives; returns it.
    fn wait(&mut self, what: &str, pred: impl Fn(&EngineEvent) -> bool) -> EngineEvent {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(ev) => {
                    self.seen.push(ev.clone());
                    if pred(&ev) {
                        return ev;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    panic!("timed out waiting for {what}; saw {:#?}", self.seen)
                }
                Err(RecvTimeoutError::Disconnected) => panic!("channel closed waiting for {what}"),
            }
        }
    }

    fn started(&mut self, path: &Path, auto: bool) {
        self.wait("TrackStarted", |e| {
            matches!(e, EngineEvent::TrackStarted { path: p, auto_advanced }
                if p == path && *auto_advanced == auto)
        });
    }

    fn idle(&mut self) {
        self.wait("Idle", |e| matches!(e, EngineEvent::Idle));
    }

    /// Paths of all TrackStarted events seen so far, in order.
    fn started_paths(&self) -> Vec<PathBuf> {
        self.seen
            .iter()
            .filter_map(|e| match e {
                EngineEvent::TrackStarted { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect()
    }
}

#[test]
fn replaygain_mode_can_be_changed_at_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let a = write_wav(dir.path(), "rg.wav", 440.0, 2.0);
    let (eng, rx) = engine();
    for mode in [ReplayGain::Off, ReplayGain::Track, ReplayGain::Album] {
        eng.set_replaygain(mode).unwrap();
    }
    eng.load(&a).unwrap();
    // Changing the mode while a file is playing must not error either.
    for mode in [ReplayGain::Track, ReplayGain::Off, ReplayGain::Album] {
        eng.set_replaygain(mode).unwrap();
    }
    drop(rx);
}

#[test]
fn single_file_plays_to_idle() {
    let dir = tempfile::tempdir().unwrap();
    let a = write_wav(dir.path(), "a.wav", 440.0, 0.3);
    let (engine, rx) = engine();
    let mut log = Log::new(rx);
    // Start paused: with untimed output a 0.3 s file can finish before mpv
    // reports its duration (property changes are coalesced).
    engine.pause().unwrap();
    engine.load(&a).unwrap();
    log.started(&a, false);
    let ev = log.wait("Duration", |e| matches!(e, EngineEvent::Duration { .. }));
    let EngineEvent::Duration { seconds } = ev else {
        unreachable!()
    };
    assert!((seconds - 0.3).abs() < 0.05, "duration {seconds}");
    engine.play().unwrap();
    log.wait(
        "TrackEnded Eof",
        |e| matches!(e, EngineEvent::TrackEnded { path, reason: EndReason::Eof } if *path == a),
    );
    log.idle();
}

#[test]
fn gapless_pair_advances_automatically() {
    let dir = tempfile::tempdir().unwrap();
    let a = write_wav(dir.path(), "a.wav", 440.0, 0.3);
    let b = write_wav(dir.path(), "b.wav", 660.0, 0.3);
    let (engine, rx) = engine();
    let mut log = Log::new(rx);
    // Start paused so set_next can not race the end of A.
    engine.pause().unwrap();
    engine.load(&a).unwrap();
    log.started(&a, false);
    engine.set_next(Some(&b)).unwrap();
    engine.play().unwrap();
    log.started(&b, true);
    log.idle();
    assert_eq!(log.started_paths(), [a.clone(), b.clone()]);
    assert!(log.seen.iter().any(|e| matches!(e,
        EngineEvent::TrackEnded { path, reason: EndReason::Eof } if *path == a)));
}

#[test]
fn duration_is_reported_for_same_length_tracks() {
    // Regression: observing `duration` alone reports nothing when the next
    // track has the same length, which left the UI without a duration.
    let dir = tempfile::tempdir().unwrap();
    // Real time and long enough for mpv to prefetch b before a ends.
    let a = write_wav(dir.path(), "a.wav", 440.0, 3.0);
    let b = write_wav(dir.path(), "b.wav", 660.0, 3.0);
    let (engine, rx) = engine_with(false);
    let mut log = Log::new(rx);
    engine.pause().unwrap();
    engine.load(&a).unwrap();
    log.started(&a, false);
    engine.set_next(Some(&b)).unwrap();
    log.wait("Duration of a", |e| {
        matches!(e, EngineEvent::Duration { .. })
    });
    engine.seek(2.0).unwrap();
    engine.play().unwrap();
    log.started(&b, true);
    let ev = log.wait("Duration of b", |e| {
        matches!(e, EngineEvent::Duration { .. })
    });
    let EngineEvent::Duration { seconds } = ev else {
        unreachable!()
    };
    assert!((seconds - 3.0).abs() < 0.05, "duration {seconds}");
    engine.stop().unwrap();
}

#[test]
fn replaced_next_wins() {
    let dir = tempfile::tempdir().unwrap();
    let a = write_wav(dir.path(), "a.wav", 440.0, 0.3);
    let b = write_wav(dir.path(), "b.wav", 660.0, 0.3);
    let c = write_wav(dir.path(), "c.wav", 880.0, 0.3);
    let (engine, rx) = engine();
    let mut log = Log::new(rx);
    engine.pause().unwrap();
    engine.load(&a).unwrap();
    log.started(&a, false);
    engine.set_next(Some(&b)).unwrap();
    engine.set_next(Some(&c)).unwrap();
    engine.play().unwrap();
    log.started(&c, true);
    log.idle();
    assert_eq!(log.started_paths(), [a, c]);
}

#[test]
fn cleared_next_goes_idle_after_current() {
    let dir = tempfile::tempdir().unwrap();
    let a = write_wav(dir.path(), "a.wav", 440.0, 0.3);
    let b = write_wav(dir.path(), "b.wav", 660.0, 0.3);
    let (engine, rx) = engine();
    let mut log = Log::new(rx);
    engine.pause().unwrap();
    engine.load(&a).unwrap();
    log.started(&a, false);
    engine.set_next(Some(&b)).unwrap();
    engine.set_next(None).unwrap();
    engine.play().unwrap();
    log.idle();
    assert_eq!(log.started_paths(), [a]);
}

/// Wraps the real engine; `play` is ignored while the gate is closed so the
/// test controls when playback really begins.
struct Gated {
    inner: Arc<MpvEngine>,
    open: Arc<AtomicBool>,
}

impl AudioEngine for Gated {
    fn load(&self, p: &Path) -> Result<(), EngineError> {
        self.inner.load(p)
    }
    fn set_next(&self, p: Option<&Path>) -> Result<(), EngineError> {
        self.inner.set_next(p)
    }
    fn play(&self) -> Result<(), EngineError> {
        if self.open.load(Ordering::SeqCst) {
            self.inner.play()
        } else {
            Ok(())
        }
    }
    fn pause(&self) -> Result<(), EngineError> {
        self.inner.pause()
    }
    fn stop(&self) -> Result<(), EngineError> {
        self.inner.stop()
    }
    fn seek(&self, s: f64) -> Result<(), EngineError> {
        self.inner.seek(s)
    }
    fn set_volume(&self, v: f64) -> Result<(), EngineError> {
        self.inner.set_volume(v)
    }
    fn set_replaygain(&self, m: ReplayGain) -> Result<(), EngineError> {
        self.inner.set_replaygain(m)
    }
    fn set_gapless(&self, enabled: bool) -> Result<(), EngineError> {
        self.inner.set_gapless(enabled)
    }
    fn set_wasapi_exclusive(&self, enabled: bool) -> Result<(), EngineError> {
        self.inner.set_wasapi_exclusive(enabled)
    }
}

fn run_queue_playback(gapless: bool) {
    let dir = tempfile::tempdir().unwrap();
    let files: Vec<PathBuf> = [440.0, 660.0, 880.0]
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let secs = if gapless { 3.0 } else { 1.0 };
            write_wav(dir.path(), &format!("t{i}.wav"), *f, secs)
        })
        .collect();

    let (engine, rx) = engine();
    let engine = Arc::new(engine);
    let open = Arc::new(AtomicBool::new(false));
    let (ptx, prx) = mpsc::channel::<PlayerEvent>();
    let ptx = std::sync::Mutex::new(ptx);
    let player = Arc::new(Player::new(
        Box::new(Gated {
            inner: engine.clone(),
            open: open.clone(),
        }),
        Box::new(move |e| {
            let _ = ptx.lock().unwrap().send(e);
        }),
    ));
    if !gapless {
        player.set_gapless(false).unwrap();
    }

    // Pump engine events into the player. After every track start the engine
    // is paused while the player re-arms the next slot, so the (fast,
    // untimed) null output cannot finish the track first.
    let done = Arc::new(AtomicBool::new(false));
    let pump = {
        let done = done.clone();
        let player = player.clone();
        let engine = engine.clone();
        std::thread::spawn(move || {
            let mut started = Vec::new();
            while !done.load(Ordering::SeqCst) {
                let Ok(ev) = rx.recv_timeout(Duration::from_millis(50)) else {
                    continue;
                };
                if let EngineEvent::TrackStarted { path, .. } = &ev {
                    started.push(path.clone());
                    engine.pause().unwrap();
                    player.handle_engine_event(ev);
                    engine.play().unwrap();
                } else {
                    player.handle_engine_event(ev);
                }
            }
            started
        })
    };

    engine.pause().unwrap();
    let snap = player.replace_queue(files.clone(), 0).unwrap();
    assert_eq!(snap.current_index, Some(0));
    open.store(true, Ordering::SeqCst);
    engine.play().unwrap();

    let deadline = Instant::now() + TIMEOUT;
    let mut last = None;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match prx.recv_timeout(left) {
            Ok(PlayerEvent::State(s)) => {
                let finished = s.status == Status::Stopped && s.current_index == Some(2);
                last = Some(s);
                if finished {
                    break;
                }
            }
            Ok(_) => {}
            Err(_) => panic!("timed out (gapless={gapless}); last snapshot {last:?}"),
        }
    }
    let last = last.unwrap();
    assert_eq!(last.current_index, Some(2));
    assert_eq!(last.status, Status::Stopped);

    done.store(true, Ordering::SeqCst);
    let started = pump.join().unwrap();
    drop(player);
    drop(engine);
    assert_eq!(started, files);
}

#[test]
fn player_plays_whole_queue_in_order() {
    run_queue_playback(true);
}

#[test]
fn player_plays_whole_queue_with_gapless_off() {
    run_queue_playback(false);
}
