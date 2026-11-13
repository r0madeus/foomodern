//! libmpv backend. This is the only file that talks to libmpv.
//!
//! mpv's playlist is used as a two-slot buffer only: `[current, next]`
//! (plus, briefly, the finished previous entry until the engine removes it
//! on the following `StartFile`). The real queue lives in `crate::queue`.
//!
//! Entries are tracked by mpv's stable playlist entry ids (`playlist/N/id`),
//! not by `playlist-pos`: right after `loadfile`, and around track changes,
//! `playlist-pos` is transitional and can not be trusted. The engine
//! remembers the id of the entry it appended as "next"; `set_next` only ever
//! removes that entry, and a `StartFile` for that id is an auto-advance.
//!
//! Threading: one background thread owns `wait_event` and turns mpv events
//! into [`EngineEvent`]s. Commands may be called from any thread; all
//! playlist edits (commands and the event thread) hold the `slots` mutex.
//!
//! `Idle` is only emitted after something has started playing, so the
//! initial idle state at startup produces no event.

use super::{AudioEngine, EndReason, EngineError, EngineEvent, EngineResult};
use libmpv2::events::{Event, PropertyData};
use libmpv2::{Format, Mpv};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const POSITION_INTERVAL: Duration = Duration::from_millis(250);
const OBS_PAUSE: u64 = 1;
const OBS_DURATION: u64 = 2;
const OBS_TIME_POS: u64 = 3;
const OBS_IDLE: u64 = 4;

// mpv_end_file_reason values.
const END_EOF: u32 = 0;
const END_ERROR: u32 = 4;
const END_REDIRECT: u32 = 5;

#[derive(Debug, Clone, Default)]
pub struct MpvOptions {
    /// Audio output driver (`ao`), e.g. `"null"` in tests. `None` = mpv default.
    pub audio_output: Option<String>,
    /// `ao-null-untimed`: the null output plays as fast as possible.
    pub untimed: bool,
}

/// An mpv playlist entry we created: its stable entry id and our path.
#[derive(Debug, Clone)]
struct Slot {
    id: i64,
    path: PathBuf,
}

/// The two slots of the engine.
#[derive(Debug, Default)]
struct Slots {
    /// Entry created by the last `load` (or promoted from `next`).
    current: Option<Slot>,
    /// Entry appended by `set_next`, if any.
    next: Option<Slot>,
}

pub struct MpvEngine {
    mpv: Arc<Mpv>,
    slots: Arc<Mutex<Slots>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

fn cmd_err(name: &str, e: impl std::fmt::Debug) -> EngineError {
    EngineError::Command(format!("{name}: {e:?}"))
}

/// Human-readable text for the libmpv errors a user can run into.
/// Codes are mpv's stable `mpv_error` values from `client.h`.
fn describe(e: &libmpv2::Error) -> String {
    match e {
        libmpv2::Error::Raw(code) => match *code {
            -13 => "the file could not be loaded".into(),
            -14 => "the audio output could not be opened".into(),
            -16 => "there is nothing to play in this file".into(),
            -17 => "the file format is not recognised".into(),
            -18 => "the file uses an unsupported format".into(),
            other => format!("mpv error {other}"),
        },
        other => format!("{other:?}"),
    }
}

fn lock_slots(slots: &Mutex<Slots>) -> MutexGuard<'_, Slots> {
    slots.lock().unwrap_or_else(|e| e.into_inner())
}

fn get_i64(mpv: &Mpv, name: &str) -> EngineResult<i64> {
    mpv.get_property::<i64>(name).map_err(|e| cmd_err(name, e))
}

/// Id of the last playlist entry (the one `loadfile` just added).
fn last_entry_id(mpv: &Mpv) -> EngineResult<Option<i64>> {
    let count = get_i64(mpv, "playlist-count")?;
    if count == 0 {
        return Ok(None);
    }
    get_i64(mpv, &format!("playlist/{}/id", count - 1)).map(Some)
}

/// Index of the playlist entry with mpv entry id `id`.
fn index_of_entry(mpv: &Mpv, id: i64) -> EngineResult<Option<i64>> {
    let count = get_i64(mpv, "playlist-count")?;
    for i in 0..count {
        if get_i64(mpv, &format!("playlist/{i}/id"))? == id {
            return Ok(Some(i));
        }
    }
    Ok(None)
}

fn path_str(path: &Path) -> EngineResult<&str> {
    path.to_str()
        .ok_or_else(|| EngineError::Command(format!("path is not valid UTF-8: {}", path.display())))
}

impl MpvEngine {
    pub fn new(options: MpvOptions, events: Sender<EngineEvent>) -> EngineResult<MpvEngine> {
        let mpv = Mpv::with_initializer(|i| {
            i.set_property("vid", "no")?;
            i.set_property("audio-display", "no")?;
            i.set_property("idle", "yes")?;
            i.set_property("keep-open", "no")?;
            i.set_property("prefetch-playlist", "yes")?;
            i.set_property("gapless-audio", "weak")?;
            i.set_property("replaygain", "album")?;
            i.set_property("config", "no")?;
            i.set_property("load-scripts", "no")?;
            i.set_property("input-default-bindings", "no")?;
            i.set_property("terminal", "no")?;
            i.set_property("ytdl", "no")?;
            if let Some(ao) = &options.audio_output {
                i.set_property("ao", ao.as_str())?;
            }
            if options.untimed {
                i.set_property("ao-null-untimed", "yes")?;
            }
            Ok(())
        })
        .map_err(|e| EngineError::Init(e.to_string()))?;

        for (name, format, id) in [
            ("pause", Format::Flag, OBS_PAUSE),
            ("duration", Format::Double, OBS_DURATION),
            ("time-pos", Format::Double, OBS_TIME_POS),
            ("idle-active", Format::Flag, OBS_IDLE),
        ] {
            mpv.observe_property(name, format, id)
                .map_err(|e| EngineError::Init(format!("observe {name}: {e}")))?;
        }

        let mpv = Arc::new(mpv);
        let slots = Arc::new(Mutex::new(Slots::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let worker = EventLoop {
            mpv: mpv.clone(),
            slots: slots.clone(),
            stop: stop.clone(),
            tx: events,
            current_path: None,
            started_any: false,
            last_position: None,
        };
        let thread = std::thread::Builder::new()
            .name("mpv-events".into())
            .spawn(move || worker.run())
            .map_err(|e| EngineError::Init(e.to_string()))?;

        Ok(MpvEngine {
            mpv,
            slots,
            stop,
            thread: Some(thread),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Slots> {
        lock_slots(&self.slots)
    }

    fn command(&self, name: &str, args: &[&str]) -> EngineResult<()> {
        self.mpv.command(name, args).map_err(|e| cmd_err(name, e))
    }
}

impl Drop for MpvEngine {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl AudioEngine for MpvEngine {
    fn load(&self, path: &Path) -> EngineResult<()> {
        let mut slots = self.lock();
        self.command("loadfile", &[path_str(path)?, "replace"])?;
        slots.next = None;
        slots.current = last_entry_id(&self.mpv)?.map(|id| Slot {
            id,
            path: path.to_path_buf(),
        });
        Ok(())
    }

    fn set_next(&self, path: Option<&Path>) -> EngineResult<()> {
        let mut slots = self.lock();
        if let Some(Slot { id, .. }) = slots.next.take() {
            // The old next entry may already be playing (mpv advanced, but
            // the event thread has not handled StartFile yet): keep it then.
            let playing = get_i64(&self.mpv, "playlist-playing-pos").unwrap_or(-1);
            if let Some(index) = index_of_entry(&self.mpv, id)?
                && index != playing
            {
                self.command("playlist-remove", &[&index.to_string()])?;
            }
        }
        let Some(path) = path else { return Ok(()) };
        if slots.current.is_none() {
            // Nothing loaded, so there is nothing to play after it.
            return Ok(());
        }
        self.command("loadfile", &[path_str(path)?, "append"])?;
        slots.next = last_entry_id(&self.mpv)?.map(|id| Slot {
            id,
            path: path.to_path_buf(),
        });
        Ok(())
    }

    fn play(&self) -> EngineResult<()> {
        self.mpv
            .set_property("pause", false)
            .map_err(|e| cmd_err("pause", e))
    }

    fn pause(&self) -> EngineResult<()> {
        self.mpv
            .set_property("pause", true)
            .map_err(|e| cmd_err("pause", e))
    }

    fn stop(&self) -> EngineResult<()> {
        let mut slots = self.lock();
        *slots = Slots::default();
        self.command("stop", &[])
    }

    fn seek(&self, seconds: f64) -> EngineResult<()> {
        self.command("seek", &[&seconds.max(0.0).to_string(), "absolute"])
    }

    fn set_volume(&self, percent: f64) -> EngineResult<()> {
        self.mpv
            .set_property("volume", percent.clamp(0.0, 100.0))
            .map_err(|e| cmd_err("volume", e))
    }
}

struct EventLoop {
    mpv: Arc<Mpv>,
    slots: Arc<Mutex<Slots>>,
    stop: Arc<AtomicBool>,
    tx: Sender<EngineEvent>,
    current_path: Option<PathBuf>,
    started_any: bool,
    last_position: Option<Instant>,
}

impl EventLoop {
    fn send(&self, ev: EngineEvent) {
        // The receiver going away just means nobody listens any more.
        let _ = self.tx.send(ev);
    }

    fn run(mut self) {
        while !self.stop.load(Ordering::SeqCst) {
            match self.mpv.clone().wait_event(0.2) {
                None => {}
                Some(Err(e)) => {
                    // Also covers EndFile with an error code: libmpv2 reports
                    // those as Err without the reason, so the player learns
                    // about it through `Error` (and the following Idle or
                    // TrackStarted).
                    let what = match &self.current_path {
                        Some(p) => format!(
                            "Could not play {}: {}",
                            p.file_name().unwrap_or_default().to_string_lossy(),
                            describe(&e)
                        ),
                        None => format!("Playback failed: {}", describe(&e)),
                    };
                    self.send(EngineEvent::Error(what));
                }
                Some(Ok(ev)) => self.handle(ev),
            }
        }
    }

    fn handle(&mut self, ev: Event<'_>) {
        match ev {
            Event::StartFile => self.on_start_file(),
            Event::EndFile(reason) => {
                if reason == END_REDIRECT {
                    return;
                }
                let reason = match reason {
                    END_EOF => EndReason::Eof,
                    END_ERROR => EndReason::Error,
                    _ => EndReason::Stopped,
                };
                if let Some(path) = self.current_path.clone() {
                    self.send(EngineEvent::TrackEnded { path, reason });
                }
            }
            Event::FileLoaded => {
                // Observed `duration` only reports changes: two tracks of the
                // same length in a row would produce no event, so read it.
                if let Ok(seconds) = self.mpv.get_property::<f64>("duration") {
                    self.send(EngineEvent::Duration { seconds });
                }
            }
            Event::Seek | Event::PlaybackRestart => {
                if let Ok(seconds) = self.mpv.get_property::<f64>("time-pos") {
                    self.last_position = Some(Instant::now());
                    self.send(EngineEvent::Position { seconds });
                }
            }
            Event::PropertyChange {
                name,
                change,
                reply_userdata,
            } => self.on_property(reply_userdata, name, change),
            _ => {}
        }
    }

    fn on_start_file(&mut self) {
        self.started_any = true;
        self.last_position = None;
        let started = {
            let mut slots = lock_slots(&self.slots);
            self.started_entry(&mut slots)
        };
        match started {
            Ok((path, auto_advanced)) => {
                self.current_path = Some(path.clone());
                self.send(EngineEvent::TrackStarted {
                    path,
                    auto_advanced,
                });
            }
            Err(e) => self.send(EngineEvent::Error(format!("start-file: {e}"))),
        }
    }

    /// Identify the entry that just started and drop the finished entries
    /// before it, so the playlist is back to `[current, next?]`.
    ///
    /// Events are handled asynchronously, so by now the entry may already
    /// have finished (`playlist-playing-pos` is -1). Then it can only be one
    /// of our slots: the next entry if we have one, else the loaded entry.
    fn started_entry(&self, slots: &mut Slots) -> EngineResult<(PathBuf, bool)> {
        let pos = get_i64(&self.mpv, "playlist-playing-pos")?;
        let started = if pos >= 0 {
            let id = get_i64(&self.mpv, &format!("playlist/{pos}/id"))?;
            [&slots.next, &slots.current]
                .into_iter()
                .flatten()
                .find(|s| s.id == id)
                .cloned()
                .unwrap_or_else(|| {
                    let path = self
                        .mpv
                        .get_property::<String>(&format!("playlist/{pos}/filename"))
                        .unwrap_or_default();
                    log::warn!("mpv started an entry the engine did not queue: {path}");
                    Slot {
                        id,
                        path: PathBuf::from(path),
                    }
                })
        } else {
            slots
                .next
                .clone()
                .or_else(|| slots.current.clone())
                .ok_or_else(|| EngineError::Command("no playing entry".into()))?
        };
        let auto_advanced = slots.next.as_ref().is_some_and(|s| s.id == started.id);
        if auto_advanced {
            slots.next = None;
        }
        slots.current = Some(started.clone());
        if let Some(index) = index_of_entry(&self.mpv, started.id)? {
            for _ in 0..index {
                self.mpv
                    .command("playlist-remove", &["0"])
                    .map_err(|e| cmd_err("playlist-remove", e))?;
            }
        }
        Ok((started.path, auto_advanced))
    }

    fn on_property(&mut self, id: u64, name: &str, change: PropertyData<'_>) {
        match (id, change) {
            (OBS_PAUSE, PropertyData::Flag(b)) => self.send(EngineEvent::Paused(b)),
            (OBS_DURATION, PropertyData::Double(seconds)) => {
                self.send(EngineEvent::Duration { seconds })
            }
            (OBS_TIME_POS, PropertyData::Double(seconds)) => {
                let due = self
                    .last_position
                    .is_none_or(|t| t.elapsed() >= POSITION_INTERVAL);
                if due {
                    self.last_position = Some(Instant::now());
                    self.send(EngineEvent::Position { seconds });
                }
            }
            (OBS_IDLE, PropertyData::Flag(true)) => {
                if self.started_any {
                    self.current_path = None;
                    self.send(EngineEvent::Idle);
                }
            }
            _ => {
                let _ = name;
            }
        }
    }
}
