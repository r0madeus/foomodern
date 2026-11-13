//! Persistent playback settings and session queue/position state.

use crate::library::scan::write_atomic;
use crate::player::PlayerSnapshot;
use crate::queue::RepeatMode;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const POSITION_SAVE_INTERVAL: Duration = Duration::from_secs(5);
const VOLUME_SAVE_INTERVAL: Duration = Duration::from_secs(1);

fn default_volume() -> f64 {
    100.0
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedTrack {
    pub path: PathBuf,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionState {
    #[serde(default = "default_volume")]
    pub volume: f64,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub queue: Vec<SavedTrack>,
    pub current_index: Option<usize>,
    pub position_seconds: f64,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            volume: default_volume(),
            shuffle: false,
            repeat: RepeatMode::Off,
            queue: Vec::new(),
            current_index: None,
            position_seconds: 0.0,
        }
    }
}

impl SessionState {
    /// Clamp out-of-range values so corrupted or hand-edited files stay safe.
    pub fn normalized(mut self) -> Self {
        self.volume = if self.volume.is_finite() {
            self.volume.clamp(0.0, 100.0)
        } else {
            default_volume()
        };
        self.current_index = self.current_index.filter(|&i| i < self.queue.len());
        self.position_seconds =
            if self.current_index.is_some() && self.position_seconds.is_finite() {
                self.position_seconds.max(0.0)
            } else {
                0.0
            };
        self
    }
}

pub fn load(path: &Path) -> SessionState {
    let Ok(bytes) = std::fs::read(path) else {
        return SessionState::default();
    };
    serde_json::from_slice::<SessionState>(&bytes)
        .map(SessionState::normalized)
        .unwrap_or_default()
}

pub fn save(path: &Path, state: &SessionState) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec(&state.clone().normalized())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    write_atomic(path, &json)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StructuralKey {
    queue_version: u64,
    current_index: Option<usize>,
    shuffle: bool,
    repeat: RepeatMode,
}

impl From<&PlayerSnapshot> for StructuralKey {
    fn from(s: &PlayerSnapshot) -> Self {
        Self {
            queue_version: s.queue_version,
            current_index: s.current_index,
            shuffle: s.shuffle,
            repeat: s.repeat,
        }
    }
}

#[derive(Default)]
struct GuardState {
    last_written_at: Option<Instant>,
    last_structure: Option<StructuralKey>,
}

/// Serializes disk writes and throttles high-frequency position/volume saves.
pub struct SessionStore {
    path: PathBuf,
    guard: Mutex<GuardState>,
}

impl SessionStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            guard: Mutex::new(GuardState::default()),
        }
    }

    pub fn load(&self) -> SessionState {
        load(&self.path)
    }

    fn write_locked(&self, g: &mut GuardState, state: &SessionState, now: Instant) {
        if let Err(e) = save(&self.path, state) {
            log::warn!("failed to save session state: {e}");
        } else {
            g.last_written_at = Some(now);
        }
    }

    pub fn save_now(&self, state: &SessionState) {
        let mut g = self.guard.lock().unwrap_or_else(|e| e.into_inner());
        self.write_locked(&mut g, state, Instant::now());
    }

    pub fn on_player_state(&self, snap: &PlayerSnapshot, build: impl FnOnce() -> SessionState) {
        let key = StructuralKey::from(snap);
        let now = Instant::now();
        let mut g = self.guard.lock().unwrap_or_else(|e| e.into_inner());
        let structure_changed = g.last_structure != Some(key);
        let due = g
            .last_written_at
            .is_none_or(|t| now.duration_since(t) >= VOLUME_SAVE_INTERVAL);
        if !structure_changed && !due {
            return;
        }
        self.write_locked(&mut g, &build(), now);
        g.last_structure = Some(key);
    }

    pub fn maybe_save_position(&self, build: impl FnOnce() -> SessionState) {
        let now = Instant::now();
        let mut g = self.guard.lock().unwrap_or_else(|e| e.into_inner());
        let due = g
            .last_written_at
            .is_none_or(|t| now.duration_since(t) >= POSITION_SAVE_INTERVAL);
        if !due {
            return;
        }
        self.write_locked(&mut g, &build(), now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn missing_or_corrupt_file_falls_back_to_default() {
        let dir = TempDir::new().unwrap();
        assert_eq!(load(&dir.path().join("missing.json")), SessionState::default());

        let bad = dir.path().join("bad.json");
        std::fs::write(&bad, "{not valid json").unwrap();
        assert_eq!(load(&bad), SessionState::default());
    }

    #[test]
    fn roundtrip_and_normalization() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("nested/session.json");
        let state = SessionState {
            volume: 42.5,
            shuffle: true,
            repeat: RepeatMode::All,
            queue: vec![SavedTrack {
                path: PathBuf::from("/music/track.flac"),
                title: Some("Track".into()),
                artist: Some("Artist".into()),
            }],
            current_index: Some(0),
            position_seconds: 18.25,
        };
        save(&file, &state).unwrap();
        assert_eq!(load(&file), state);

        let clamped = SessionState {
            volume: 150.0,
            shuffle: false,
            repeat: RepeatMode::One,
            queue: vec![],
            current_index: Some(3),
            position_seconds: 99.0,
        }
        .normalized();
        assert_eq!(clamped.volume, 100.0);
        assert_eq!(clamped.current_index, None);
        assert_eq!(clamped.position_seconds, 0.0);
    }
}
