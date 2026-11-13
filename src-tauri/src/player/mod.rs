//! The player owns the single queue and drives the audio engine.
//!
//! Key invariant (decision K3): after every operation that can change what
//! plays next, `queue.next()` is compared with the engine's next slot
//! (`engine_next`) and the engine is updated when they differ. The engine
//! never sees more than current + next.
//!
//! Locking: all state lives behind one mutex. Events for the observer are
//! built under the lock and delivered after it is released, so an observer
//! may call back into the player.
//!
//! Known benign race: engine events are asynchronous, so an event produced
//! just before a user command (e.g. a late `Idle`) can be applied after it.

use crate::audio::{AudioEngine, EndReason, EngineError, EngineEvent};
use crate::queue::{EntryId, Queue, RepeatMode, Track};
use crate::session::{SavedTrack, SessionState};
use serde::Serialize;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error(transparent)]
    Engine(#[from] EngineError),
    #[error("index {0} is out of range")]
    InvalidIndex(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackInfo {
    pub id: EntryId,
    pub path: String,
    /// Tag title, or the file name without extension.
    pub title: String,
    /// Tag artist, empty when unknown.
    pub artist: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub status: Status,
    pub current_index: Option<usize>,
    pub current: Option<TrackInfo>,
    pub queue_length: usize,
    pub queue_version: u64,
    pub position_seconds: f64,
    pub duration_seconds: Option<f64>,
    pub volume: f64,
    pub shuffle: bool,
    pub repeat: RepeatMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    State(PlayerSnapshot),
    Position {
        seconds: f64,
        duration: Option<f64>,
    },
    /// Something the user should know about (e.g. a file failed to play).
    Error(String),
}

pub type Observer = Box<dyn Fn(PlayerEvent) + Send + Sync>;

struct Inner {
    queue: Queue,
    engine: Box<dyn AudioEngine>,
    status: Status,
    position: f64,
    duration: Option<f64>,
    volume: f64,
    /// The entry currently sitting in the engine's "next" slot.
    engine_next: Option<EntryId>,
    /// True between `engine.load` and the matching non-auto `TrackStarted`.
    /// An auto-advance reported in that window happened before our load and
    /// is stale: the load replaces it anyway.
    pending_load: bool,
    /// Saved position from the previous session waiting for the first `play`.
    resume_position: Option<f64>,
    /// Seek deferred until the engine finishes opening the loaded file.
    pending_load_seek: Option<f64>,
    /// Sequence number of the last observer event built under this lock.
    event_seq: u64,
}

pub struct Player {
    inner: Mutex<Inner>,
    observer: Observer,
    /// Sequence number of the last event handed to the observer.
    ///
    /// Events are built under `inner` but delivered after it is released,
    /// so two threads (a user command and the engine-event thread) can
    /// deliver out of order. Delivery is serialised here and an event older
    /// than one already delivered is dropped: the newer snapshot supersedes
    /// it. Without this a stale "playing" snapshot could overwrite a newer
    /// "paused" one in the UI and the OS media controls.
    delivered_seq: Mutex<u64>,
}

fn track_info(e: &crate::queue::QueueEntry) -> TrackInfo {
    TrackInfo {
        id: e.id,
        path: e.path.to_string_lossy().into_owned(),
        title: e
            .title
            .clone()
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| {
                e.path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default()
            }),
        artist: e.artist.clone().unwrap_or_default(),
    }
}

impl Inner {
    /// Next observer event sequence number (always increasing).
    fn next_seq(&mut self) -> u64 {
        self.event_seq += 1;
        self.event_seq
    }

    fn snapshot(&self) -> PlayerSnapshot {
        PlayerSnapshot {
            status: self.status,
            current_index: self.queue.current_index(),
            current: self.queue.current().map(track_info),
            queue_length: self.queue.len(),
            queue_version: self.queue.version(),
            position_seconds: self.position,
            duration_seconds: self.duration,
            volume: self.volume,
            shuffle: self.queue.shuffle(),
            repeat: self.queue.repeat(),
        }
    }

    /// Load and play the entry at `index` (must be in range).
    fn start_index(&mut self, index: usize) -> Result<(), PlayerError> {
        let path = self
            .queue
            .get(index)
            .ok_or(PlayerError::InvalidIndex(index))?
            .path
            .clone();
        self.queue.set_current(Some(index));
        self.engine_next = None; // `load` clears the next slot
        self.position = 0.0;
        self.duration = None;
        self.resume_position = None;
        self.pending_load_seek = None;
        self.status = Status::Playing;
        self.pending_load = true;
        self.engine.load(&path)?;
        self.engine.play()?; // `load` keeps a paused engine paused
        self.sync_next()
    }

    fn stop_engine(&mut self) -> Result<(), PlayerError> {
        self.pending_load = false;
        self.resume_position = None;
        self.pending_load_seek = None;
        self.status = Status::Stopped;
        self.position = 0.0;
        self.engine_next = None;
        self.engine.stop()?;
        Ok(())
    }

    /// Decision K3: keep the engine's next slot equal to `queue.next()`.
    fn sync_next(&mut self) -> Result<(), PlayerError> {
        if self.status == Status::Stopped {
            return Ok(());
        }
        let want = self.queue.next().map(|e| (e.id, e.path.clone()));
        let want_id = want.as_ref().map(|w| w.0);
        if want_id != self.engine_next {
            self.engine.set_next(want.as_ref().map(|w| w.1.as_path()))?;
            self.engine_next = want_id;
        }
        Ok(())
    }
}

impl Player {
    pub fn new(engine: Box<dyn AudioEngine>, observer: Observer) -> Player {
        Player {
            inner: Mutex::new(Inner {
                queue: Queue::new(),
                engine,
                status: Status::Stopped,
                position: 0.0,
                duration: None,
                volume: 100.0,
                engine_next: None,
                pending_load: false,
                resume_position: None,
                pending_load_seek: None,
                event_seq: 0,
            }),
            observer,
            delivered_seq: Mutex::new(0),
        }
    }

    /// Hand `event` (built under the lock with sequence `seq`) to the
    /// observer, in order. Stale state/position events are dropped; errors
    /// are always delivered.
    fn deliver(&self, seq: u64, event: PlayerEvent) {
        let mut last = self.delivered_seq.lock().unwrap_or_else(|e| e.into_inner());
        if seq <= *last && !matches!(event, PlayerEvent::Error(_)) {
            return;
        }
        *last = (*last).max(seq);
        (self.observer)(event);
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Run `f` under the lock, then emit a `State` event (also on error,
    /// since `f` may have changed state before failing) outside the lock.
    fn run<F>(&self, f: F) -> Result<PlayerSnapshot, PlayerError>
    where
        F: FnOnce(&mut Inner) -> Result<(), PlayerError>,
    {
        let (result, snap, seq) = {
            let mut inner = self.lock();
            let r = f(&mut inner);
            let seq = inner.next_seq();
            (r, inner.snapshot(), seq)
        };
        self.deliver(seq, PlayerEvent::State(snap.clone()));
        result.map(|()| snap)
    }

    pub fn snapshot(&self) -> PlayerSnapshot {
        self.lock().snapshot()
    }

    pub fn restore_session(&self, state: SessionState) {
        let state = state.normalized();
        let mut s = self.lock();
        if let Err(e) = s.engine.set_volume(state.volume) {
            log::warn!("failed to restore volume: {e}");
        }
        s.volume = state.volume;
        let tracks: Vec<Track> = state
            .queue
            .into_iter()
            .map(|t| Track {
                path: t.path,
                title: t.title,
                artist: t.artist,
            })
            .collect();
        s.queue
            .restore(tracks, state.current_index, state.shuffle, state.repeat);
        let pos = if s.queue.current_index().is_some() && state.position_seconds > 0.0 {
            Some(state.position_seconds)
        } else {
            None
        };
        s.position = pos.unwrap_or(0.0);
        s.resume_position = pos;
        s.pending_load_seek = None;
    }

    pub fn export_session(&self) -> SessionState {
        let s = self.lock();
        let queue = s
            .queue
            .entries()
            .iter()
            .map(|e| SavedTrack {
                path: e.path.clone(),
                title: e.title.clone(),
                artist: e.artist.clone(),
            })
            .collect();
        let current_index = s.queue.current_index();
        let position_seconds = if current_index.is_some() {
            s.position.max(0.0)
        } else {
            0.0
        };
        SessionState {
            volume: s.volume,
            shuffle: s.queue.shuffle(),
            repeat: s.queue.repeat(),
            queue,
            current_index,
            position_seconds,
        }
    }

    pub fn queue_page(&self, offset: usize, limit: usize) -> Vec<TrackInfo> {
        self.lock()
            .queue
            .page(offset, limit)
            .iter()
            .map(track_info)
            .collect()
    }

    /// Replace the queue and start playing `start_index`. An empty list
    /// clears the queue and stops playback.
    pub fn replace_queue(
        &self,
        paths: Vec<impl Into<Track>>,
        start_index: usize,
    ) -> Result<PlayerSnapshot, PlayerError> {
        if !paths.is_empty() && start_index >= paths.len() {
            return Err(PlayerError::InvalidIndex(start_index));
        }
        self.run(|s| {
            let empty = paths.is_empty();
            s.queue.replace(paths, (!empty).then_some(start_index));
            if empty {
                return s.stop_engine();
            }
            s.start_index(start_index)
        })
    }

    pub fn append(&self, paths: Vec<impl Into<Track>>) -> Result<PlayerSnapshot, PlayerError> {
        self.run(|s| {
            s.queue.append(paths);
            s.sync_next()
        })
    }

    pub fn remove(&self, index: usize) -> Result<PlayerSnapshot, PlayerError> {
        self.run(|s| {
            let was_current = s.queue.current_index() == Some(index);
            if s.queue.remove(index).is_none() {
                return Ok(());
            }
            if was_current && s.status != Status::Stopped {
                // The old next entry now sits at the same index.
                if index < s.queue.len() {
                    return s.start_index(index);
                }
                return s.stop_engine();
            }
            s.sync_next()
        })
    }

    pub fn move_entry(&self, from: usize, to: usize) -> Result<PlayerSnapshot, PlayerError> {
        self.run(|s| {
            if s.queue.move_entry(from, to) {
                s.sync_next()?;
            }
            Ok(())
        })
    }

    pub fn play_index(&self, index: usize) -> Result<PlayerSnapshot, PlayerError> {
        self.run(|s| {
            if index >= s.queue.len() {
                return Err(PlayerError::InvalidIndex(index));
            }
            s.start_index(index)
        })
    }

    pub fn play(&self) -> Result<PlayerSnapshot, PlayerError> {
        self.run(|s| match s.status {
            Status::Playing => Ok(()),
            Status::Paused => {
                s.engine.play()?;
                s.status = Status::Playing;
                Ok(())
            }
            Status::Stopped => {
                if let Some(idx) = s.queue.current_index().or_else(|| s.queue.first_index()) {
                    let resume = s.resume_position.take();
                    s.start_index(idx)?;
                    if let Some(pos) = resume {
                        s.position = pos;
                        s.pending_load_seek = Some(pos);
                    }
                    Ok(())
                } else {
                    Ok(())
                }
            }
        })
    }

    pub fn pause(&self) -> Result<PlayerSnapshot, PlayerError> {
        self.run(|s| {
            if s.status == Status::Playing {
                s.engine.pause()?;
                s.status = Status::Paused;
            }
            Ok(())
        })
    }

    pub fn toggle_pause(&self) -> Result<PlayerSnapshot, PlayerError> {
        if self.lock().status == Status::Playing {
            self.pause()
        } else {
            self.play()
        }
    }

    /// Go to the next queue entry according to shuffle and repeat rules.
    pub fn next(&self) -> Result<PlayerSnapshot, PlayerError> {
        self.run(|s| {
            if let Some(target) = s.queue.next_manual_index() {
                s.start_index(target)
            } else {
                Ok(())
            }
        })
    }

    pub fn set_shuffle(&self, enabled: bool) -> Result<PlayerSnapshot, PlayerError> {
        self.run(|s| {
            s.queue.set_shuffle(enabled);
            s.sync_next()
        })
    }

    pub fn set_repeat(&self, mode: RepeatMode) -> Result<PlayerSnapshot, PlayerError> {
        self.run(|s| {
            s.queue.set_repeat(mode);
            s.sync_next()
        })
    }

    /// Past 3 s of the current track (or at the first entry): restart it.
    /// Otherwise go to the previous entry.
    pub fn previous(&self) -> Result<PlayerSnapshot, PlayerError> {
        self.run(|s| {
            if s.queue.current_index().is_none() {
                return Ok(());
            }
            match s.queue.previous_index() {
                Some(prev) if s.position <= 3.0 => s.start_index(prev),
                _ => {
                    s.engine.seek(0.0)?;
                    s.position = 0.0;
                    Ok(())
                }
            }
        })
    }

    pub fn seek(&self, seconds: f64) -> Result<PlayerSnapshot, PlayerError> {
        self.run(|s| {
            let seconds = if seconds.is_finite() {
                seconds.max(0.0)
            } else {
                0.0
            };
            if s.status != Status::Stopped {
                s.resume_position = None;
                s.pending_load_seek = None;
                s.engine.seek(seconds)?;
                s.position = seconds;
            }
            Ok(())
        })
    }

    pub fn set_volume(&self, percent: f64) -> Result<PlayerSnapshot, PlayerError> {
        self.run(|s| {
            let v = if percent.is_finite() {
                percent.clamp(0.0, 100.0)
            } else {
                100.0
            };
            s.engine.set_volume(v)?;
            s.volume = v;
            Ok(())
        })
    }

    /// Apply an event coming from the audio engine.
    pub fn handle_engine_event(&self, event: EngineEvent) {
        enum Out {
            None,
            State,
            Position(f64, Option<f64>),
            Error(String),
        }
        let (out, snap, seq) = {
            let mut s = self.lock();
            let out = match event {
                EngineEvent::TrackStarted {
                    auto_advanced: true,
                    ..
                } if s.pending_load => {
                    log::debug!("ignoring auto-advance that raced a load");
                    Out::None
                }
                EngineEvent::TrackStarted {
                    path,
                    auto_advanced: true,
                } => {
                    if let Err(e) = Self::on_auto_advance(&mut s, &path) {
                        log::warn!("auto-advance handling failed: {e}");
                    }
                    Out::State
                }
                EngineEvent::TrackStarted {
                    auto_advanced: false,
                    ..
                } => {
                    s.pending_load = false;
                    s.status = Status::Playing;
                    Out::State
                }
                EngineEvent::Idle => {
                    s.status = Status::Stopped;
                    s.position = 0.0;
                    s.engine_next = None;
                    Out::State
                }
                EngineEvent::Paused(b) => {
                    let new = match (s.status, b) {
                        (Status::Stopped, _) => Status::Stopped,
                        (_, true) => Status::Paused,
                        (_, false) => Status::Playing,
                    };
                    if new != s.status {
                        s.status = new;
                        Out::State
                    } else {
                        Out::None
                    }
                }
                EngineEvent::Duration { seconds } => {
                    s.duration = Some(seconds);
                    if let Some(pos) = s.pending_load_seek.take() {
                        let target = pos.min(seconds.max(0.0));
                        if let Err(e) = s.engine.seek(target) {
                            log::warn!("failed to seek to saved position: {e}");
                        } else {
                            s.position = target;
                        }
                    }
                    Out::State
                }
                EngineEvent::Position { seconds } => {
                    s.position = seconds;
                    Out::Position(seconds, s.duration)
                }
                EngineEvent::TrackEnded { path, reason } => {
                    if reason == EndReason::Error {
                        log::warn!("track failed to play: {}", path.display());
                    }
                    Out::None
                }
                EngineEvent::Error(msg) => {
                    log::warn!("audio engine: {msg}");
                    Out::Error(msg)
                }
            };
            let seq = s.next_seq();
            (out, s.snapshot(), seq)
        };
        match out {
            Out::None => {}
            Out::State => self.deliver(seq, PlayerEvent::State(snap)),
            Out::Position(seconds, duration) => {
                self.deliver(seq, PlayerEvent::Position { seconds, duration })
            }
            Out::Error(msg) => self.deliver(seq, PlayerEvent::Error(msg)),
        }
    }

    /// The engine moved into the slot we filled. If that entry is gone from
    /// the queue meanwhile, the engine is playing something that is no longer
    /// queued: we load the entry after the last known current one, or stop
    /// when there is none.
    fn on_auto_advance(s: &mut Inner, path: &Path) -> Result<(), PlayerError> {
        s.position = 0.0;
        s.duration = None;
        // The track change may have been reported after the user paused at
        // the boundary: mpv stays paused, so the player must too.
        if s.status != Status::Paused {
            s.status = Status::Playing;
        }
        let idx = s
            .engine_next
            .and_then(|id| s.queue.index_of(id))
            .filter(|&i| s.queue.get(i).is_some_and(|e| e.path == path));
        s.engine_next = None;
        match idx {
            Some(i) => {
                s.queue.set_current(Some(i));
                s.sync_next()
            }
            None => match s.queue.next_index() {
                Some(target) => s.start_index(target),
                None => s.stop_engine(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[derive(Debug, Clone, PartialEq)]
    enum Call {
        Load(PathBuf),
        SetNext(Option<PathBuf>),
        Play,
        Pause,
        Stop,
        Seek(f64),
        Volume(f64),
    }

    #[derive(Clone, Default)]
    struct Calls(Arc<Mutex<Vec<Call>>>);

    impl Calls {
        fn take(&self) -> Vec<Call> {
            std::mem::take(&mut *self.0.lock().unwrap())
        }
    }

    struct FakeEngine(Calls);

    impl FakeEngine {
        fn rec(&self, c: Call) -> Result<(), EngineError> {
            self.0.0.lock().unwrap().push(c);
            Ok(())
        }
    }

    impl AudioEngine for FakeEngine {
        fn load(&self, p: &Path) -> Result<(), EngineError> {
            self.rec(Call::Load(p.into()))
        }
        fn set_next(&self, p: Option<&Path>) -> Result<(), EngineError> {
            self.rec(Call::SetNext(p.map(Into::into)))
        }
        fn play(&self) -> Result<(), EngineError> {
            self.rec(Call::Play)
        }
        fn pause(&self) -> Result<(), EngineError> {
            self.rec(Call::Pause)
        }
        fn stop(&self) -> Result<(), EngineError> {
            self.rec(Call::Stop)
        }
        fn seek(&self, s: f64) -> Result<(), EngineError> {
            self.rec(Call::Seek(s))
        }
        fn set_volume(&self, v: f64) -> Result<(), EngineError> {
            self.rec(Call::Volume(v))
        }
    }

    fn setup() -> (Player, Calls, Arc<Mutex<Vec<PlayerEvent>>>) {
        let calls = Calls::default();
        let events = Arc::new(Mutex::new(Vec::new()));
        let ev = events.clone();
        let p = Player::new(
            Box::new(FakeEngine(calls.clone())),
            Box::new(move |e| ev.lock().unwrap().push(e)),
        );
        (p, calls, events)
    }

    fn pb(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn pbs(v: &[&str]) -> Vec<PathBuf> {
        v.iter().map(|s| pb(s)).collect()
    }

    fn started(p: &str, auto: bool) -> EngineEvent {
        EngineEvent::TrackStarted {
            path: pb(p),
            auto_advanced: auto,
        }
    }

    #[test]
    fn replace_queue_loads_and_sets_next() {
        let (p, calls, _) = setup();
        let snap = p
            .replace_queue(pbs(&["a.mp3", "b.mp3", "c.mp3"]), 0)
            .unwrap();
        assert_eq!(
            calls.take(),
            [
                Call::Load(pb("a.mp3")),
                Call::Play,
                Call::SetNext(Some(pb("b.mp3")))
            ]
        );
        assert_eq!(snap.status, Status::Playing);
        assert_eq!(snap.current_index, Some(0));
        assert_eq!(snap.current.unwrap().title, "a");
        assert_eq!(snap.queue_length, 3);
    }

    #[test]
    fn replace_queue_invalid_start_is_error() {
        let (p, calls, _) = setup();
        assert!(matches!(
            p.replace_queue(pbs(&["a"]), 3),
            Err(PlayerError::InvalidIndex(3))
        ));
        assert!(calls.take().is_empty());
    }

    #[test]
    fn replace_queue_last_entry_has_no_next() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b"]), 1).unwrap();
        assert_eq!(calls.take(), [Call::Load(pb("b")), Call::Play]);
    }

    #[test]
    fn append_when_at_end_sets_next() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a"]), 0).unwrap();
        calls.take();
        p.append(pbs(&["b", "c"])).unwrap();
        assert_eq!(calls.take(), [Call::SetNext(Some(pb("b")))]);
        // Further appends do not touch the engine.
        p.append(pbs(&["d"])).unwrap();
        assert!(calls.take().is_empty());
    }

    #[test]
    fn remove_next_entry_resyncs() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b", "c"]), 0).unwrap();
        calls.take();
        p.remove(1).unwrap();
        assert_eq!(calls.take(), [Call::SetNext(Some(pb("c")))]);
        p.remove(1).unwrap();
        assert_eq!(calls.take(), [Call::SetNext(None)]);
    }

    #[test]
    fn remove_before_current_keeps_next() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b", "c"]), 1).unwrap();
        calls.take();
        let snap = p.remove(0).unwrap();
        assert!(calls.take().is_empty());
        assert_eq!(snap.current_index, Some(0));
    }

    #[test]
    fn remove_current_plays_following_entry_or_stops() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b", "c"]), 0).unwrap();
        calls.take();
        let snap = p.remove(0).unwrap();
        assert_eq!(
            calls.take(),
            [
                Call::Load(pb("b")),
                Call::Play,
                Call::SetNext(Some(pb("c")))
            ]
        );
        assert_eq!(snap.current_index, Some(0));
        p.remove(1).unwrap();
        calls.take();
        let snap = p.remove(0).unwrap();
        assert_eq!(calls.take(), [Call::Stop]);
        assert_eq!(snap.status, Status::Stopped);
        assert_eq!(snap.current_index, None);
    }

    #[test]
    fn move_entry_resyncs_next() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b", "c"]), 0).unwrap();
        calls.take();
        p.move_entry(2, 1).unwrap();
        assert_eq!(calls.take(), [Call::SetNext(Some(pb("c")))]);
        // Moving the current entry past its neighbour changes next too.
        p.move_entry(0, 1).unwrap();
        assert_eq!(calls.take(), [Call::SetNext(Some(pb("b")))]);
        // Move not involving current or next: no engine call.
        p.move_entry(2, 2).unwrap();
        assert!(calls.take().is_empty());
    }

    #[test]
    fn auto_advance_updates_current_and_sets_following_next() {
        let (p, calls, events) = setup();
        p.replace_queue(pbs(&["a", "b", "c"]), 0).unwrap();
        p.handle_engine_event(started("a", false));
        calls.take();
        p.handle_engine_event(started("b", true));
        assert_eq!(calls.take(), [Call::SetNext(Some(pb("c")))]);
        let snap = p.snapshot();
        assert_eq!(snap.current_index, Some(1));
        assert_eq!(snap.status, Status::Playing);
        assert!(matches!(
            events.lock().unwrap().last(),
            Some(PlayerEvent::State(s)) if s.current_index == Some(1)
        ));
        p.handle_engine_event(started("c", true));
        assert!(calls.take().is_empty());
        assert_eq!(p.snapshot().current_index, Some(2));
    }

    #[test]
    fn auto_advance_into_removed_entry_loads_following_one() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b", "c"]), 0).unwrap();
        p.handle_engine_event(started("a", false));
        // The engine moves to b right as b is removed (event arrives later).
        p.remove(1).unwrap();
        calls.take();
        // Engine still reports b: simulate by forcing engine_next to b's id.
        p.handle_engine_event(started("b", true));
        // engine_next was already c, path mismatch -> treated as stale.
        assert_eq!(calls.take(), [Call::Load(pb("c")), Call::Play]);
        assert_eq!(p.snapshot().current_index, Some(1));
    }

    #[test]
    fn tags_from_the_library_are_shown_instead_of_the_file_name() {
        let (p, _, _) = setup();
        let tagged = Track {
            path: pb("/music/01 - Track 1.mp3"),
            title: Some("Morning Light".into()),
            artist: Some("Nadia Okafor".into()),
        };
        let snap = p
            .replace_queue(vec![tagged, Track::from(pb("/music/02 - Plain.mp3"))], 0)
            .unwrap();
        let cur = snap.current.unwrap();
        assert_eq!(
            (cur.title.as_str(), cur.artist.as_str()),
            ("Morning Light", "Nadia Okafor")
        );
        let page = p.queue_page(1, 1);
        assert_eq!(
            (page[0].title.as_str(), page[0].artist.as_str()),
            ("02 - Plain", "")
        );
    }

    #[test]
    fn concurrent_updates_never_leave_a_stale_snapshot_last() {
        // A user command and the engine-event thread build snapshots under
        // the lock but deliver them after releasing it. The observer must
        // still end up with the newest state (regression: the OS media
        // controls kept showing "playing" after a pause).
        for _ in 0..200 {
            let (p, _, events) = setup();
            p.replace_queue(pbs(&["a", "b"]), 0).unwrap();
            p.handle_engine_event(started("a", false));
            std::thread::scope(|sc| {
                sc.spawn(|| {
                    for _ in 0..25 {
                        p.toggle_pause().unwrap();
                    }
                });
                sc.spawn(|| {
                    for i in 0..25 {
                        p.handle_engine_event(EngineEvent::Duration {
                            seconds: 100.0 + f64::from(i),
                        });
                    }
                });
            });
            let last = events
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find_map(|e| match e {
                    PlayerEvent::State(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap();
            assert_eq!(last, p.snapshot());
        }
    }

    #[test]
    fn pausing_right_at_a_track_change_stays_paused() {
        let (p, _, _) = setup();
        p.replace_queue(pbs(&["a", "b", "c"]), 0).unwrap();
        p.handle_engine_event(started("a", false));
        p.pause().unwrap();
        // mpv had already moved to b when the pause arrived; the event
        // reporting it is handled afterwards.
        p.handle_engine_event(started("b", true));
        let snap = p.snapshot();
        assert_eq!(snap.status, Status::Paused);
        assert_eq!(snap.current_index, Some(1));
    }

    #[test]
    fn engine_errors_reach_the_observer() {
        let (p, _, events) = setup();
        p.handle_engine_event(EngineEvent::Error("Could not play a.mp3".into()));
        assert_eq!(
            events.lock().unwrap().last(),
            Some(&PlayerEvent::Error("Could not play a.mp3".into()))
        );
    }

    #[test]
    fn auto_advance_racing_a_user_load_is_ignored() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b", "c", "d"]), 0).unwrap();
        p.handle_engine_event(started("a", false));
        // The engine advances into b while the user picks d; the stale
        // auto-advance event arrives after the load.
        p.play_index(3).unwrap();
        calls.take();
        p.handle_engine_event(started("b", true));
        assert!(calls.take().is_empty());
        assert_eq!(p.snapshot().current_index, Some(3));
        // Our own load then reports in and later auto-advances work again.
        p.handle_engine_event(started("d", false));
        assert_eq!(p.snapshot().current_index, Some(3));
    }

    #[test]
    fn auto_advance_into_removed_last_entry_stops() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b"]), 0).unwrap();
        p.handle_engine_event(started("a", false));
        p.remove(1).unwrap();
        calls.take();
        p.handle_engine_event(started("b", true));
        assert_eq!(calls.take(), [Call::Stop]);
        assert_eq!(p.snapshot().status, Status::Stopped);
    }

    #[test]
    fn previous_restarts_after_three_seconds() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b"]), 1).unwrap();
        calls.take();
        p.handle_engine_event(EngineEvent::Position { seconds: 5.0 });
        let snap = p.previous().unwrap();
        assert_eq!(calls.take(), [Call::Seek(0.0)]);
        assert_eq!(snap.current_index, Some(1));
        assert_eq!(snap.position_seconds, 0.0);
    }

    #[test]
    fn previous_goes_back_before_three_seconds() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b"]), 1).unwrap();
        calls.take();
        p.handle_engine_event(EngineEvent::Position { seconds: 1.0 });
        let snap = p.previous().unwrap();
        assert_eq!(
            calls.take(),
            [
                Call::Load(pb("a")),
                Call::Play,
                Call::SetNext(Some(pb("b")))
            ]
        );
        assert_eq!(snap.current_index, Some(0));
    }

    #[test]
    fn previous_at_first_entry_seeks_to_start() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b"]), 0).unwrap();
        calls.take();
        p.previous().unwrap();
        assert_eq!(calls.take(), [Call::Seek(0.0)]);
    }

    #[test]
    fn next_moves_forward_and_is_noop_at_end() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b"]), 0).unwrap();
        calls.take();
        let snap = p.next().unwrap();
        assert_eq!(snap.current_index, Some(1));
        assert_eq!(calls.take(), [Call::Load(pb("b")), Call::Play]);
        let snap = p.next().unwrap();
        assert_eq!(snap.current_index, Some(1));
        assert!(calls.take().is_empty());
    }

    #[test]
    fn idle_stops_but_keeps_current() {
        let (p, calls, events) = setup();
        p.replace_queue(pbs(&["a"]), 0).unwrap();
        calls.take();
        p.handle_engine_event(EngineEvent::Idle);
        let snap = p.snapshot();
        assert_eq!(snap.status, Status::Stopped);
        assert_eq!(snap.current_index, Some(0));
        assert_eq!(snap.position_seconds, 0.0);
        assert!(matches!(
            events.lock().unwrap().last(),
            Some(PlayerEvent::State(_))
        ));
        // Appending while stopped does not touch the engine.
        p.append(pbs(&["b"])).unwrap();
        assert!(calls.take().is_empty());
    }

    #[test]
    fn pause_play_toggle_and_events() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a"]), 0).unwrap();
        calls.take();
        assert_eq!(p.toggle_pause().unwrap().status, Status::Paused);
        assert_eq!(p.toggle_pause().unwrap().status, Status::Playing);
        assert_eq!(calls.take(), [Call::Pause, Call::Play]);
        p.handle_engine_event(EngineEvent::Paused(true));
        assert_eq!(p.snapshot().status, Status::Paused);
    }

    #[test]
    fn position_and_duration_events() {
        let (p, _, events) = setup();
        p.replace_queue(pbs(&["a"]), 0).unwrap();
        p.handle_engine_event(EngineEvent::Duration { seconds: 10.0 });
        p.handle_engine_event(EngineEvent::Position { seconds: 2.5 });
        assert_eq!(
            events.lock().unwrap().last(),
            Some(&PlayerEvent::Position {
                seconds: 2.5,
                duration: Some(10.0)
            })
        );
        assert_eq!(p.snapshot().duration_seconds, Some(10.0));
    }

    #[test]
    fn volume_is_clamped() {
        let (p, calls, _) = setup();
        assert_eq!(p.set_volume(150.0).unwrap().volume, 100.0);
        assert_eq!(p.set_volume(-5.0).unwrap().volume, 0.0);
        assert_eq!(calls.take(), [Call::Volume(100.0), Call::Volume(0.0)]);
    }

    #[test]
    fn play_index_out_of_range_errors() {
        let (p, _, _) = setup();
        assert!(matches!(p.play_index(0), Err(PlayerError::InvalidIndex(0))));
    }

    #[test]
    fn snapshot_serializes_camel_case() {
        let (p, _, _) = setup();
        p.replace_queue(pbs(&["dir/a.mp3"]), 0).unwrap();
        let v = serde_json::to_value(p.snapshot()).unwrap();
        assert_eq!(v["status"], "playing");
        assert_eq!(v["currentIndex"], 0);
        assert!(v["current"]["id"].is_u64());
        assert_eq!(v["current"]["title"], "a");
        assert_eq!(v["queueLength"], 1);
        assert_eq!(v["shuffle"], false);
        assert_eq!(v["repeat"], "off");
    }

    #[test]
    fn set_repeat_updates_engine_next_slot_for_all_and_one() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b"]), 1).unwrap();
        p.handle_engine_event(started("b", false));
        calls.take();

        // At the end of the queue, Repeat::All queues "a" in the engine's next slot.
        let snap = p.set_repeat(RepeatMode::All).unwrap();
        assert_eq!(snap.repeat, RepeatMode::All);
        assert_eq!(calls.take(), [Call::SetNext(Some(pb("a")))]);

        // Repeat::One switches the engine's next slot to the current track "b".
        let snap = p.set_repeat(RepeatMode::One).unwrap();
        assert_eq!(snap.repeat, RepeatMode::One);
        assert_eq!(calls.take(), [Call::SetNext(Some(pb("b")))]);

        // Auto-advancing on Repeat::One keeps current at "b" and re-queues "b".
        p.handle_engine_event(started("b", true));
        assert_eq!(p.snapshot().current_index, Some(1));
        assert_eq!(calls.take(), [Call::SetNext(Some(pb("b")))]);

        // Repeat::Off clears the engine's next slot at the end of the queue.
        let snap = p.set_repeat(RepeatMode::Off).unwrap();
        assert_eq!(snap.repeat, RepeatMode::Off);
        assert_eq!(calls.take(), [Call::SetNext(None)]);
    }

    #[test]
    fn repeat_all_wraps_on_next_and_previous() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b"]), 1).unwrap();
        p.set_repeat(RepeatMode::All).unwrap();
        calls.take();

        let snap = p.next().unwrap();
        assert_eq!(snap.current_index, Some(0));
        assert_eq!(
            calls.take(),
            [
                Call::Load(pb("a")),
                Call::Play,
                Call::SetNext(Some(pb("b")))
            ]
        );

        let snap = p.previous().unwrap();
        assert_eq!(snap.current_index, Some(1));
        assert_eq!(
            calls.take(),
            [
                Call::Load(pb("b")),
                Call::Play,
                Call::SetNext(Some(pb("a")))
            ]
        );
    }

    #[test]
    fn set_shuffle_syncs_next_slot_with_shuffled_order() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b", "c", "d", "e"]), 0).unwrap();
        calls.take();

        let snap = p.set_shuffle(true).unwrap();
        assert!(snap.shuffle);
        // Either next changed to something else and issued SetNext, or stayed "b".
        let recorded = calls.take();
        assert!(recorded.len() <= 1);

        // Turning shuffle off on an entry keeps it current and restores sequential next.
        let snap = p.set_shuffle(false).unwrap();
        assert!(!snap.shuffle);
        assert_eq!(snap.current_index, Some(0));
    }

    #[test]
    fn restore_and_export_session_preserves_state_and_seeks_on_play() {
        let (p, calls, _) = setup();
        let saved = SessionState {
            volume: 37.0,
            shuffle: true,
            repeat: RepeatMode::All,
            queue: vec![
                SavedTrack {
                    path: pb("a"),
                    title: Some("Title A".into()),
                    artist: Some("Artist A".into()),
                },
                SavedTrack {
                    path: pb("b"),
                    title: None,
                    artist: None,
                },
            ],
            current_index: Some(1),
            position_seconds: 42.5,
        };
        p.restore_session(saved.clone());

        let snap = p.snapshot();
        assert_eq!(snap.status, Status::Stopped);
        assert_eq!(snap.volume, 37.0);
        assert!(snap.shuffle);
        assert_eq!(snap.repeat, RepeatMode::All);
        assert_eq!(snap.current_index, Some(1));
        assert_eq!(snap.position_seconds, 42.5);
        assert_eq!(snap.queue_length, 2);
        assert_eq!(p.export_session(), saved);
        assert_eq!(calls.take(), [Call::Volume(37.0)]);

        // Playing from stopped loads the saved track and seeks once Duration arrives.
        let snap = p.play().unwrap();
        assert_eq!(snap.status, Status::Playing);
        assert_eq!(snap.position_seconds, 42.5);
        assert_eq!(
            calls.take(),
            [
                Call::Load(pb("b")),
                Call::Play,
                Call::SetNext(Some(pb("a")))
            ]
        );

        p.handle_engine_event(EngineEvent::TrackStarted {
            path: pb("b"),
            auto_advanced: false,
        });
        p.handle_engine_event(EngineEvent::Duration { seconds: 120.0 });
        assert_eq!(calls.take(), [Call::Seek(42.5)]);
        assert_eq!(p.snapshot().position_seconds, 42.5);
    }

    #[test]
    fn switching_track_before_play_clears_saved_resume_position() {
        let (p, calls, _) = setup();
        p.restore_session(SessionState {
            volume: 80.0,
            shuffle: false,
            repeat: RepeatMode::Off,
            queue: vec![
                SavedTrack {
                    path: pb("a"),
                    title: None,
                    artist: None,
                },
                SavedTrack {
                    path: pb("b"),
                    title: None,
                    artist: None,
                },
            ],
            current_index: Some(0),
            position_seconds: 30.0,
        });
        calls.take();

        let snap = p.play_index(1).unwrap();
        assert_eq!(snap.position_seconds, 0.0);
        p.handle_engine_event(EngineEvent::Duration { seconds: 90.0 });
        assert!(
            !calls.take().iter().any(|c| matches!(c, Call::Seek(_))),
            "explicit track selection must start from 0.0"
        );
    }
}
