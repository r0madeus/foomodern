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

use crate::audio::{AudioEngine, EndReason, EngineError, EngineEvent, RemoteCommand, ReplayGain};
use crate::queue::{EntryId, Queue, RepeatMode, Track};
use crate::session::{SavedTrack, SessionState};
use serde::Serialize;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MIN_LISTEN_MS: u64 = 3_000;

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

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
    pub replaygain: ReplayGain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListenRecord {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub duration_ms: Option<u64>,
    pub started_at: i64,
    pub ended_at: i64,
    pub listened_ms: u64,
    pub completed: bool,
    pub skipped: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    State(PlayerSnapshot),
    Position {
        seconds: f64,
        duration: Option<f64>,
    },
    /// A finished or interrupted playback session long enough to record.
    Listen(ListenRecord),
    /// Something the user should know about (e.g. a file failed to play).
    Error(String),
}

pub type Observer = Box<dyn Fn(PlayerEvent) + Send + Sync>;

struct ListenSession {
    path: String,
    title: String,
    artist: String,
    duration_ms: Option<u64>,
    started_at: i64,
    accumulated: Duration,
    playing_since: Option<Instant>,
}

impl ListenSession {
    fn set_playing(&mut self, playing: bool) {
        if playing {
            if self.playing_since.is_none() {
                self.playing_since = Some(Instant::now());
            }
        } else if let Some(since) = self.playing_since.take() {
            self.accumulated += since.elapsed();
        }
    }

    fn total_ms(&mut self) -> u64 {
        if let Some(since) = self.playing_since.take() {
            self.accumulated += since.elapsed();
        }
        u64::try_from(self.accumulated.as_millis()).unwrap_or(u64::MAX)
    }
}

struct Inner {
    queue: Queue,
    engine: Box<dyn AudioEngine>,
    status: Status,
    position: f64,
    duration: Option<f64>,
    volume: f64,
    replaygain: ReplayGain,
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
    /// Active listening session for the current track, if any.
    listen: Option<ListenSession>,
    /// Completed listening sessions queued for delivery outside the lock.
    pending_listens: Vec<ListenRecord>,
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
            replaygain: self.replaygain,
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
        self.close_listen(false, true);
        self.queue.set_current(Some(index));
        self.engine_next = None; // `load` clears the next slot
        self.position = 0.0;
        self.duration = None;
        self.resume_position = None;
        self.pending_load_seek = None;
        self.status = Status::Playing;
        self.pending_load = true;
        self.open_listen();
        self.engine.load(&path)?;
        self.engine.play()?; // `load` keeps a paused engine paused
        self.sync_next()
    }

    fn stop_engine(&mut self) -> Result<(), PlayerError> {
        self.close_listen(false, true);
        self.pending_load = false;
        self.resume_position = None;
        self.pending_load_seek = None;
        self.status = Status::Stopped;
        self.position = 0.0;
        self.engine_next = None;
        self.engine.stop()?;
        Ok(())
    }

    fn open_listen(&mut self) {
        let Some(entry) = self.queue.current() else {
            return;
        };
        let info = track_info(entry);
        let playing_since = (self.status == Status::Playing).then(Instant::now);
        self.listen = Some(ListenSession {
            path: info.path,
            title: info.title,
            artist: info.artist,
            duration_ms: self
                .duration
                .filter(|d| d.is_finite() && *d > 0.0)
                .map(|d| (d * 1000.0).round() as u64),
            started_at: unix_now(),
            accumulated: Duration::ZERO,
            playing_since,
        });
    }

    fn set_listen_playing(&mut self, playing: bool) {
        if let Some(listen) = &mut self.listen {
            listen.set_playing(playing);
        }
    }

    fn close_listen(&mut self, ended_naturally: bool, skipped_if_early: bool) {
        let Some(mut session) = self.listen.take() else {
            return;
        };
        let listened_ms = session.total_ms();
        if listened_ms < MIN_LISTEN_MS {
            return;
        }
        let completed = ended_naturally
            || session
                .duration_ms
                .is_some_and(|d| d > 0 && listened_ms.saturating_mul(10) >= d.saturating_mul(8));
        let skipped = skipped_if_early && !completed;
        let ended_at = unix_now().max(session.started_at);
        self.pending_listens.push(ListenRecord {
            path: session.path,
            title: session.title,
            artist: session.artist,
            duration_ms: session.duration_ms,
            started_at: session.started_at,
            ended_at,
            listened_ms,
            completed,
            skipped,
        });
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
                replaygain: ReplayGain::default(),
                engine_next: None,
                pending_load: false,
                resume_position: None,
                pending_load_seek: None,
                event_seq: 0,
                listen: None,
                pending_listens: Vec::new(),
            }),
            observer,
            delivered_seq: Mutex::new(0),
        }
    }

    /// Hand `event` (built under the lock with sequence `seq`) to the
    /// observer, in order. Stale state/position events are dropped; errors
    /// and completed listen records are always delivered.
    fn deliver(&self, seq: u64, event: PlayerEvent) {
        let mut last = self.delivered_seq.lock().unwrap_or_else(|e| e.into_inner());
        if seq <= *last && !matches!(event, PlayerEvent::Error(_) | PlayerEvent::Listen(_)) {
            return;
        }
        *last = (*last).max(seq);
        (self.observer)(event);
    }

    fn deliver_listens(&self, listens: Vec<ListenRecord>) {
        for rec in listens {
            (self.observer)(PlayerEvent::Listen(rec));
        }
    }

    /// Close any in-progress listening session cleanly (used on app exit).
    pub fn flush_listen(&self) {
        let listens = {
            let mut inner = self.lock();
            inner.close_listen(false, false);
            std::mem::take(&mut inner.pending_listens)
        };
        self.deliver_listens(listens);
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
        let (result, snap, seq, listens) = {
            let mut inner = self.lock();
            let r = f(&mut inner);
            let seq = inner.next_seq();
            let listens = std::mem::take(&mut inner.pending_listens);
            (r, inner.snapshot(), seq, listens)
        };
        self.deliver_listens(listens);
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
        if let Err(e) = s.engine.set_replaygain(state.replaygain) {
            log::warn!("failed to restore replaygain mode: {e}");
        }
        s.replaygain = state.replaygain;
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
            replaygain: s.replaygain,
            ..SessionState::default()
        }
        .normalized()
    }

    /// The path of the loaded (current) queue entry, if any.
    pub fn current_path(&self) -> Option<String> {
        self.lock()
            .queue
            .current()
            .map(|e| e.path.to_string_lossy().into_owned())
    }

    /// Refreshes the title/artist shown for queue entries playing `path`
    /// (after its tags were edited) and publishes a new state. Returns how
    /// many entries changed.
    pub fn update_track_meta(&self, path: &str, title: &str, artist: &str) -> usize {
        let key = crate::library::scan::display_path(Path::new(path));
        let mut changed = 0;
        let _ = self.run(|s| {
            changed = s.queue.update_meta(
                |p| crate::library::scan::display_path(p) == key,
                title,
                artist,
            );
            Ok(())
        });
        changed
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

    /// Insert `track` right after the queue entry `anchor`. Returns the new
    /// entry's id, or `None` when `anchor` is gone (the queue was replaced or
    /// the entry removed), so background fillers know to stop.
    pub fn insert_after(
        &self,
        anchor: EntryId,
        track: impl Into<Track>,
    ) -> Result<Option<EntryId>, PlayerError> {
        let mut inserted = None;
        self.run(|s| {
            inserted = s.queue.insert_after(anchor, track);
            if inserted.is_some() {
                s.sync_next()?;
            }
            Ok(())
        })?;
        Ok(inserted)
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
                s.set_listen_playing(true);
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
                s.set_listen_playing(false);
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

    pub fn set_replaygain(&self, mode: ReplayGain) -> Result<PlayerSnapshot, PlayerError> {
        self.run(|s| {
            s.engine.set_replaygain(mode)?;
            s.replaygain = mode;
            Ok(())
        })
    }

    /// Apply an event coming from the audio engine.
    pub fn handle_engine_event(&self, event: EngineEvent) {
        if let EngineEvent::Remote(command) = event {
            let result = match command {
                RemoteCommand::Next => self.next(),
                RemoteCommand::Previous => self.previous(),
            };
            if let Err(e) = result {
                log::warn!("remote {command:?} failed: {e}");
            }
            return;
        }
        enum Out {
            None,
            State,
            Position(f64, Option<f64>),
            Error(String),
        }
        let (out, snap, seq, listens) = {
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
                    s.set_listen_playing(true);
                    Out::State
                }
                EngineEvent::Idle => {
                    s.close_listen(true, false);
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
                    s.set_listen_playing(new == Status::Playing);
                    if new != s.status {
                        s.status = new;
                        Out::State
                    } else {
                        Out::None
                    }
                }
                EngineEvent::Duration { seconds } => {
                    s.duration = Some(seconds);
                    if seconds.is_finite() && seconds > 0.0 {
                        let ms = (seconds * 1000.0).round() as u64;
                        if let Some(l) = &mut s.listen {
                            l.duration_ms = Some(ms);
                        }
                    }
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
                    let same_track = s
                        .listen
                        .as_ref()
                        .is_some_and(|l| l.path == path.to_string_lossy());
                    if same_track {
                        match reason {
                            EndReason::Eof => s.close_listen(true, false),
                            EndReason::Error => s.close_listen(false, false),
                            EndReason::Stopped => {}
                        }
                    }
                    if reason == EndReason::Error {
                        log::warn!("track failed to play: {}", path.display());
                    }
                    Out::None
                }
                EngineEvent::Error(msg) => {
                    log::warn!("audio engine: {msg}");
                    Out::Error(msg)
                }
                EngineEvent::Remote(_) => Out::None, // handled above
            };
            let seq = s.next_seq();
            let listens = std::mem::take(&mut s.pending_listens);
            (out, s.snapshot(), seq, listens)
        };
        self.deliver_listens(listens);
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
        s.close_listen(true, false);
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
                s.open_listen();
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
        ReplayGain(ReplayGain),
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
        fn set_replaygain(&self, m: ReplayGain) -> Result<(), EngineError> {
            self.rec(Call::ReplayGain(m))
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
    fn update_track_meta_changes_matching_entries_and_publishes_state() {
        let (p, _, events) = setup();
        let tracks = vec![
            Track {
                path: pb("/music/a.mp3"),
                title: Some("Old".into()),
                artist: Some("Old Artist".into()),
            },
            Track::from(pb("/music/b.mp3")),
            Track {
                path: pb("/music/a.mp3"),
                title: Some("Old".into()),
                artist: None,
            },
        ];
        p.replace_queue(tracks, 1).unwrap();
        let before = p.snapshot().queue_version;
        events.lock().unwrap().clear();

        assert_eq!(p.update_track_meta("/music/a.mp3", "New", "New Artist"), 2);
        let page = p.queue_page(0, 3);
        assert_eq!(
            page.iter()
                .map(|t| (t.title.as_str(), t.artist.as_str()))
                .collect::<Vec<_>>(),
            [("New", "New Artist"), ("b", ""), ("New", "New Artist")]
        );
        assert!(p.snapshot().queue_version > before);
        assert!(matches!(
            events.lock().unwrap().last(),
            Some(PlayerEvent::State(_))
        ));
        assert_eq!(p.current_path().as_deref(), Some("/music/b.mp3"));

        // Nothing matches: nothing changes.
        assert_eq!(p.update_track_meta("/music/zzz.mp3", "X", "Y"), 0);
        assert_eq!(p.update_track_meta("/music/a.mp3", "New", "New Artist"), 0);
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
    fn remote_commands_move_through_the_rust_queue() {
        let (p, calls, _) = setup();
        p.replace_queue(pbs(&["a", "b", "c"]), 0).unwrap();
        calls.take();
        p.handle_engine_event(EngineEvent::Remote(RemoteCommand::Next));
        assert_eq!(p.snapshot().current_index, Some(1));
        assert_eq!(
            calls.take(),
            [
                Call::Load(pb("b")),
                Call::Play,
                Call::SetNext(Some(pb("c")))
            ]
        );
        p.handle_engine_event(EngineEvent::Remote(RemoteCommand::Previous));
        assert_eq!(p.snapshot().current_index, Some(0));
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
    fn replaygain_defaults_to_album_and_is_set_on_engine() {
        let (p, calls, _) = setup();
        assert_eq!(p.snapshot().replaygain, ReplayGain::Album);
        let snap = p.set_replaygain(ReplayGain::Off).unwrap();
        assert_eq!(snap.replaygain, ReplayGain::Off);
        assert_eq!(p.export_session().replaygain, ReplayGain::Off);
        assert_eq!(calls.take(), [Call::ReplayGain(ReplayGain::Off)]);
        let v = serde_json::to_value(p.snapshot()).unwrap();
        assert_eq!(v["replaygain"], "off");
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
            replaygain: ReplayGain::Track,
            ..SessionState::default()
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
        assert_eq!(snap.replaygain, ReplayGain::Track);
        assert_eq!(
            calls.take(),
            [Call::Volume(37.0), Call::ReplayGain(ReplayGain::Track)]
        );

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
            replaygain: ReplayGain::Album,
            ..SessionState::default()
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

    #[test]
    fn listen_tracking_records_skips_completions_and_ignores_short_taps() {
        let (p, _, events) = setup();
        p.replace_queue(pbs(&["a.mp3", "b.mp3", "c.mp3"]), 0)
            .unwrap();

        // Short tap (< 3s) skipped immediately: must not emit Listen.
        p.next().unwrap();
        assert!(
            !events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, PlayerEvent::Listen(_)))
        );

        // Simulate 10s on a 120s track then skip with next() -> skipped = true.
        p.handle_engine_event(EngineEvent::Duration { seconds: 120.0 });
        p.lock().listen.as_mut().unwrap().accumulated += Duration::from_secs(10);
        p.next().unwrap();
        let listens: Vec<ListenRecord> = events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                PlayerEvent::Listen(r) => Some(r.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(listens.len(), 1);
        assert_eq!(listens[0].path, "b.mp3");
        assert!(listens[0].skipped);
        assert!(!listens[0].completed);
        assert!(listens[0].listened_ms >= 10_000);

        // Simulate 90s on c.mp3 finishing naturally via EOF -> completed = true, skipped = false.
        p.handle_engine_event(EngineEvent::Duration { seconds: 90.0 });
        p.lock().listen.as_mut().unwrap().accumulated += Duration::from_secs(90);
        p.handle_engine_event(EngineEvent::TrackEnded {
            path: pb("c.mp3"),
            reason: EndReason::Eof,
        });
        let listens: Vec<ListenRecord> = events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                PlayerEvent::Listen(r) => Some(r.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(listens.len(), 2);
        assert_eq!(listens[1].path, "c.mp3");
        assert!(listens[1].completed);
        assert!(!listens[1].skipped);
    }

    #[test]
    fn insert_after_current_fills_the_next_slot_and_stops_when_queue_replaced() {
        let (p, calls, _) = setup();
        let snap = p.replace_queue(pbs(&["a"]), 0).unwrap();
        calls.take();
        let a = snap.current.unwrap().id;
        let b = p.insert_after(a, pb("b")).unwrap().unwrap();
        assert_eq!(calls.take(), vec![Call::SetNext(Some(pb("b")))]);
        p.insert_after(b, pb("c")).unwrap().unwrap();
        assert_eq!(calls.take(), vec![], "next slot is still b");
        assert_eq!(p.snapshot().queue_length, 3);

        p.replace_queue(pbs(&["z"]), 0).unwrap();
        assert_eq!(p.insert_after(b, pb("late")).unwrap(), None);
        assert_eq!(p.snapshot().queue_length, 1);
    }

    #[test]
    fn exported_session_leaves_out_online_streams() {
        let (p, _, _) = setup();
        p.replace_queue(
            pbs(&["a.flac", "https://cdn.example/stream?x=1", "b.flac"]),
            2,
        )
        .unwrap();
        let saved = p.export_session();
        let paths: Vec<_> = saved.queue.iter().map(|t| t.path.clone()).collect();
        assert_eq!(paths, pbs(&["a.flac", "b.flac"]));
        assert_eq!(saved.current_index, Some(1), "still points at b.flac");

        p.play_index(1).unwrap();
        let saved = p.export_session();
        assert_eq!(saved.current_index, None);
        assert_eq!(saved.position_seconds, 0.0);
    }
}
