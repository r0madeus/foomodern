//! Android backend: Media3/ExoPlayer through the Kotlin `AudioPlugin`
//! (`plugins/android-audio`). Decision K6 anticipated this: libmpv is not
//! ported to Android, the `AudioEngine` trait hides the difference.
//!
//! Threading: Tauri runs mobile plugin calls on the Android main thread and
//! `run_mobile_plugin` blocks until Kotlin answers. Engine methods are called
//! from the main thread too (sync commands, app setup), so calling Kotlin
//! directly would deadlock. Every engine method therefore only queues a
//! command; one worker thread sends them to Kotlin in order. Failures are
//! reported as [`EngineEvent::Error`].
//!
//! ReplayGain: ExoPlayer has none, so the worker reads the gain tags of
//! each loaded track (`replaygain`) and sends a volume factor with it.
//!
//! Kotlin sends events back through a channel as JSON objects with a `type`
//! field; [`parse_event`] turns them into [`EngineEvent`]s.

use super::android_events::parse_event;
use super::{AudioEngine, EngineError, EngineEvent, EngineResult, ReplayGain, replaygain};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use tauri::Wry;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::plugin::PluginHandle;

struct Call {
    command: &'static str,
    payload: Value,
    /// Adds a `gain` (volume factor) for this file to the payload.
    gain_for: Option<PathBuf>,
}

pub struct AndroidEngine {
    calls: Mutex<Sender<Call>>,
    mode: Arc<Mutex<ReplayGain>>,
    /// Current and next track, to resend their gains when the mode changes.
    /// Updated from the event channel when ExoPlayer moves to the next one.
    slots: Arc<Mutex<(Option<PathBuf>, Option<PathBuf>)>>,
}

impl AndroidEngine {
    pub fn new(handle: PluginHandle<Wry>, events: Sender<EngineEvent>) -> EngineResult<Self> {
        let (calls, queue) = mpsc::channel::<Call>();
        let worker_events = events.clone();
        let mode = Arc::new(Mutex::new(ReplayGain::default()));
        let worker_mode = Arc::clone(&mode);
        std::thread::Builder::new()
            .name("android-audio".into())
            .spawn(move || {
                while let Ok(mut call) = queue.recv() {
                    if let Some(path) = &call.gain_for {
                        let mode = worker_mode.lock().map(|m| *m).unwrap_or_default();
                        let gain = replaygain::factor(replaygain::read(path), mode);
                        call.payload["gain"] = json!(gain);
                    }
                    if let Err(e) = handle.run_mobile_plugin::<Value>(call.command, call.payload) {
                        let _ = worker_events.send(EngineEvent::Error(format!(
                            "android audio `{}` failed: {e}",
                            call.command
                        )));
                    }
                }
            })
            .map_err(|e| EngineError::Init(e.to_string()))?;

        let slots = Arc::new(Mutex::new((None, None)));
        let channel_slots = Arc::clone(&slots);
        let channel = Channel::<Value>::new(move |body| {
            let parsed = match body {
                InvokeResponseBody::Json(json) => serde_json::from_str::<Value>(&json).ok(),
                InvokeResponseBody::Raw(bytes) => serde_json::from_slice::<Value>(&bytes).ok(),
            };
            match parsed.as_ref().and_then(parse_event) {
                Some(event) => {
                    if let EngineEvent::TrackStarted { path, .. } = &event
                        && let Ok(mut s) = channel_slots.lock()
                    {
                        s.0 = Some(path.clone());
                    }
                    let _ = events.send(event);
                }
                None => log::warn!("unexpected android audio event: {parsed:?}"),
            }
            Ok(())
        });

        let engine = Self {
            calls: Mutex::new(calls),
            mode,
            slots,
        };
        engine.call("attach", json!({ "events": channel }))?;
        Ok(engine)
    }

    fn call(&self, command: &'static str, payload: Value) -> EngineResult<()> {
        self.send(Call {
            command,
            payload,
            gain_for: None,
        })
    }

    fn send(&self, call: Call) -> EngineResult<()> {
        self.calls
            .lock()
            .map_err(|_| EngineError::Command("android audio queue poisoned".into()))?
            .send(call)
            .map_err(|_| EngineError::Command("android audio worker stopped".into()))
    }

    fn slots(&self) -> std::sync::MutexGuard<'_, (Option<PathBuf>, Option<PathBuf>)> {
        self.slots.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn path_value(path: &Path) -> EngineResult<Value> {
    path.to_str()
        .map(|s| Value::String(s.to_owned()))
        .ok_or_else(|| EngineError::Command(format!("path is not UTF-8: {}", path.display())))
}

impl AudioEngine for AndroidEngine {
    fn load(&self, path: &Path) -> EngineResult<()> {
        let value = path_value(path)?;
        *self.slots() = (Some(path.to_path_buf()), None);
        self.send(Call {
            command: "loadTrack",
            payload: json!({ "path": value }),
            gain_for: Some(path.to_path_buf()),
        })
    }

    fn set_next(&self, path: Option<&Path>) -> EngineResult<()> {
        let value = path.map(path_value).transpose()?;
        self.slots().1 = path.map(Path::to_path_buf);
        self.send(Call {
            command: "setNextTrack",
            payload: json!({ "path": value }),
            gain_for: path.map(Path::to_path_buf),
        })
    }

    fn play(&self) -> EngineResult<()> {
        self.call("play", json!({}))
    }

    fn pause(&self) -> EngineResult<()> {
        self.call("pause", json!({}))
    }

    fn stop(&self) -> EngineResult<()> {
        self.call("stop", json!({}))
    }

    fn seek(&self, seconds: f64) -> EngineResult<()> {
        self.call("seek", json!({ "seconds": seconds }))
    }

    fn set_volume(&self, percent: f64) -> EngineResult<()> {
        self.call("setVolume", json!({ "percent": percent }))
    }

    fn set_replaygain(&self, mode: ReplayGain) -> EngineResult<()> {
        *self.mode.lock().unwrap_or_else(|e| e.into_inner()) = mode;
        let (current, next) = self.slots().clone();
        for path in [current, next].into_iter().flatten() {
            self.send(Call {
                command: "setGain",
                payload: json!({ "path": path_value(&path)? }),
                gain_for: Some(path),
            })?;
        }
        Ok(())
    }
}
