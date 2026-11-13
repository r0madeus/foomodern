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
//! Kotlin sends events back through a channel as JSON objects with a `type`
//! field; [`parse_event`] turns them into [`EngineEvent`]s.

use super::android_events::parse_event;
use super::{AudioEngine, EngineError, EngineEvent, EngineResult, ReplayGain};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};
use tauri::Wry;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::plugin::PluginHandle;

struct Call {
    command: &'static str,
    payload: Value,
}

pub struct AndroidEngine {
    calls: Mutex<Sender<Call>>,
}

impl AndroidEngine {
    pub fn new(handle: PluginHandle<Wry>, events: Sender<EngineEvent>) -> EngineResult<Self> {
        let (calls, queue) = mpsc::channel::<Call>();
        let worker_events = events.clone();
        std::thread::Builder::new()
            .name("android-audio".into())
            .spawn(move || {
                while let Ok(call) = queue.recv() {
                    if let Err(e) = handle.run_mobile_plugin::<Value>(call.command, call.payload) {
                        let _ = worker_events.send(EngineEvent::Error(format!(
                            "android audio `{}` failed: {e}",
                            call.command
                        )));
                    }
                }
            })
            .map_err(|e| EngineError::Init(e.to_string()))?;

        let channel = Channel::<Value>::new(move |body| {
            let parsed = match body {
                InvokeResponseBody::Json(json) => serde_json::from_str::<Value>(&json).ok(),
                InvokeResponseBody::Raw(bytes) => serde_json::from_slice::<Value>(&bytes).ok(),
            };
            match parsed.as_ref().and_then(parse_event) {
                Some(event) => {
                    let _ = events.send(event);
                }
                None => log::warn!("unexpected android audio event: {parsed:?}"),
            }
            Ok(())
        });

        let engine = Self {
            calls: Mutex::new(calls),
        };
        engine.call("attach", json!({ "events": channel }))?;
        Ok(engine)
    }

    fn call(&self, command: &'static str, payload: Value) -> EngineResult<()> {
        self.calls
            .lock()
            .map_err(|_| EngineError::Command("android audio queue poisoned".into()))?
            .send(Call { command, payload })
            .map_err(|_| EngineError::Command("android audio worker stopped".into()))
    }
}

fn path_value(path: &Path) -> EngineResult<Value> {
    path.to_str()
        .map(|s| Value::String(s.to_owned()))
        .ok_or_else(|| EngineError::Command(format!("path is not UTF-8: {}", path.display())))
}

impl AudioEngine for AndroidEngine {
    fn load(&self, path: &Path) -> EngineResult<()> {
        self.call("loadTrack", json!({ "path": path_value(path)? }))
    }

    fn set_next(&self, path: Option<&Path>) -> EngineResult<()> {
        let path = path.map(path_value).transpose()?;
        self.call("setNextTrack", json!({ "path": path }))
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
        // ExoPlayer has no ReplayGain support; not implemented on Android yet.
        if mode != ReplayGain::Off {
            log::info!("ReplayGain is not supported on Android yet ({mode:?} ignored)");
        }
        Ok(())
    }
}
