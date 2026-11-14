//! Audio output. Everything outside this module talks to the [`AudioEngine`]
//! trait only; libmpv is never called from anywhere else (decision K6).
//!
//! The engine is deliberately dumb about queues: it holds at most two
//! entries, the *current* track and the *next* track. The queue lives in
//! Rust (`crate::queue`) and the player (`crate::player`) keeps the engine's
//! "next" slot in sync with it (decision K3). This is what makes gapless
//! playback work without a second, competing playlist inside mpv.

#[cfg(target_os = "android")]
pub mod android;
mod android_events;
#[cfg(not(target_os = "android"))]
pub mod mpv;
pub mod replaygain;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// ReplayGain normalisation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplayGain {
    Off,
    Track,
    #[default]
    Album,
}

impl ReplayGain {
    /// Value of mpv's `replaygain` property.
    pub fn mpv_value(self) -> &'static str {
        match self {
            Self::Off => "no",
            Self::Track => "track",
            Self::Album => "album",
        }
    }
}

/// Why a track stopped playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// Reached the end of the file.
    Eof,
    /// Stopped by a command (`stop`, `load` replacing the current track).
    Stopped,
    /// The file could not be played.
    Error,
}

/// Events emitted by an engine, in the order they happen.
///
/// Engines send these through the `std::sync::mpsc::Sender<EngineEvent>`
/// they receive at construction time.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineEvent {
    /// `path` started playing. `auto_advanced` is true when the engine moved
    /// into its "next" slot on its own (the previous track ended), false when
    /// playback started because of `load`.
    TrackStarted { path: PathBuf, auto_advanced: bool },
    /// The current track ended.
    TrackEnded { path: PathBuf, reason: EndReason },
    /// Duration of the current track became known (seconds).
    Duration { seconds: f64 },
    /// Playback position (seconds). Throttled by the engine to at most
    /// ~4 events per second (architecture rule 3).
    Position { seconds: f64 },
    /// Pause state changed.
    Paused(bool),
    /// Nothing is playing any more (end of the last track, or `stop`).
    Idle,
    /// A non-fatal error worth showing or logging.
    Error(String),
    /// The OS asked for a queue move (Android media notification or headset
    /// button). ExoPlayer only holds two items, so the player handles it.
    Remote(RemoteCommand),
}

/// Queue moves requested from outside the app, see [`EngineEvent::Remote`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteCommand {
    Next,
    Previous,
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("audio engine failed to start: {0}")]
    Init(String),
    #[error("audio engine command failed: {0}")]
    Command(String),
}

pub type EngineResult<T> = Result<T, EngineError>;

/// Playback backend. Implementations must be cheap to call from any thread.
pub trait AudioEngine: Send + Sync {
    /// Replace whatever is playing with `path` and start playing it.
    /// Clears the "next" slot.
    fn load(&self, path: &Path) -> EngineResult<()>;
    /// Set (or clear with `None`) the track that plays right after the
    /// current one. Replaces any previous "next" track.
    fn set_next(&self, path: Option<&Path>) -> EngineResult<()>;
    fn play(&self) -> EngineResult<()>;
    fn pause(&self) -> EngineResult<()>;
    /// Stop playback and clear both slots.
    fn stop(&self) -> EngineResult<()>;
    /// Seek to an absolute position in the current track (seconds).
    fn seek(&self, seconds: f64) -> EngineResult<()>;
    /// Volume in percent, 0.0..=100.0.
    fn set_volume(&self, percent: f64) -> EngineResult<()>;
    /// Choose the ReplayGain mode. Takes effect for the current and later tracks.
    fn set_replaygain(&self, mode: ReplayGain) -> EngineResult<()>;
}

/// Wraps an engine so that online streams (`http(s)://` paths) are only
/// loaded while the online features are allowed. Local files always pass.
/// Without this, a stream resolved before the user went offline would keep
/// being fetched by the engine.
pub struct NetGuardedEngine {
    inner: Box<dyn AudioEngine>,
    net: std::sync::Arc<crate::net::NetPolicy>,
}

impl NetGuardedEngine {
    pub fn new(inner: Box<dyn AudioEngine>, net: std::sync::Arc<crate::net::NetPolicy>) -> Self {
        Self { inner, net }
    }

    fn blocked(&self, path: &Path) -> bool {
        crate::session::is_stream_url(path) && !self.net.allowed(crate::net::Feature::Online)
    }
}

impl AudioEngine for NetGuardedEngine {
    fn load(&self, path: &Path) -> EngineResult<()> {
        if self.blocked(path) {
            return Err(EngineError::Command(crate::net::DISABLED_MESSAGE.into()));
        }
        self.inner.load(path)
    }

    fn set_next(&self, path: Option<&Path>) -> EngineResult<()> {
        // Leave the next slot empty; the player then fails the load of that
        // entry when the current track ends, instead of streaming it.
        self.inner.set_next(path.filter(|p| !self.blocked(p)))
    }

    fn play(&self) -> EngineResult<()> {
        self.inner.play()
    }

    fn pause(&self) -> EngineResult<()> {
        self.inner.pause()
    }

    fn stop(&self) -> EngineResult<()> {
        self.inner.stop()
    }

    fn seek(&self, seconds: f64) -> EngineResult<()> {
        self.inner.seek(seconds)
    }

    fn set_volume(&self, percent: f64) -> EngineResult<()> {
        self.inner.set_volume(percent)
    }

    fn set_replaygain(&self, mode: ReplayGain) -> EngineResult<()> {
        self.inner.set_replaygain(mode)
    }
}
