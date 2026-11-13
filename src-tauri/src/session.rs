//! Persistent playback settings and session queue/position state.

use crate::audio::ReplayGain;
use crate::library::scan::write_atomic;
use crate::player::PlayerSnapshot;
use crate::queue::RepeatMode;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const POSITION_SAVE_INTERVAL: Duration = Duration::from_secs(5);
const VOLUME_SAVE_INTERVAL: Duration = Duration::from_secs(1);

fn default_volume() -> f64 {
    100.0
}

fn default_close_to_tray() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PinnedKind {
    Album,
    Playlist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedRef {
    pub kind: PinnedKind,
    pub id: i64,
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
    pub replaygain: ReplayGain,
    /// Closing the window hides it to the tray instead of quitting. Owned by
    /// [`SessionStore`], which fills it in on every save.
    #[serde(default = "default_close_to_tray")]
    pub close_to_tray: bool,
    /// Folder where downloaded tracks/albums are saved. `None` = first library
    /// folder or the OS Music folder.
    #[serde(default)]
    pub download_dir: Option<PathBuf>,
    /// Optional custom path to the `yt-dlp` executable (`None` = search PATH).
    #[serde(default)]
    pub ytdlp_path: Option<PathBuf>,
    /// Albums and playlists pinned by the user to the desktop sidebar.
    #[serde(default)]
    pub pinned: Vec<PinnedRef>,
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
            replaygain: ReplayGain::default(),
            close_to_tray: default_close_to_tray(),
            download_dir: None,
            ytdlp_path: None,
            pinned: Vec::new(),
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
        self.position_seconds = if self.current_index.is_some() && self.position_seconds.is_finite()
        {
            self.position_seconds.max(0.0)
        } else {
            0.0
        };
        let mut seen = Vec::with_capacity(self.pinned.len());
        for item in self.pinned {
            if !seen.contains(&item) {
                seen.push(item);
            }
        }
        self.pinned = seen;
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

fn clean_opt_path(path: Option<PathBuf>) -> Option<PathBuf> {
    path.filter(|p| !p.as_os_str().is_empty())
}

/// Serializes disk writes and throttles high-frequency position/volume saves.
pub struct SessionStore {
    path: PathBuf,
    guard: Mutex<GuardState>,
    close_to_tray: AtomicBool,
    download_dir: Mutex<Option<PathBuf>>,
    ytdlp_path: Mutex<Option<PathBuf>>,
    pinned: Mutex<Vec<PinnedRef>>,
}

impl SessionStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            guard: Mutex::new(GuardState::default()),
            close_to_tray: AtomicBool::new(default_close_to_tray()),
            download_dir: Mutex::new(None),
            ytdlp_path: Mutex::new(None),
            pinned: Mutex::new(Vec::new()),
        }
    }

    /// Loads the saved state and adopts its window and download settings.
    pub fn load(&self) -> SessionState {
        let state = load(&self.path);
        self.close_to_tray
            .store(state.close_to_tray, Ordering::SeqCst);
        *self.download_dir.lock().unwrap_or_else(|e| e.into_inner()) =
            clean_opt_path(state.download_dir.clone());
        *self.ytdlp_path.lock().unwrap_or_else(|e| e.into_inner()) =
            clean_opt_path(state.ytdlp_path.clone());
        *self.pinned.lock().unwrap_or_else(|e| e.into_inner()) = state.pinned.clone();
        state
    }

    pub fn close_to_tray(&self) -> bool {
        self.close_to_tray.load(Ordering::SeqCst)
    }

    pub fn set_close_to_tray(&self, enabled: bool) {
        self.close_to_tray.store(enabled, Ordering::SeqCst);
    }

    pub fn download_dir(&self) -> Option<PathBuf> {
        self.download_dir
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_download_dir(&self, dir: Option<PathBuf>) {
        *self.download_dir.lock().unwrap_or_else(|e| e.into_inner()) = clean_opt_path(dir);
    }

    pub fn ytdlp_path(&self) -> Option<PathBuf> {
        self.ytdlp_path
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_ytdlp_path(&self, path: Option<PathBuf>) {
        *self.ytdlp_path.lock().unwrap_or_else(|e| e.into_inner()) = clean_opt_path(path);
    }

    pub fn pinned(&self) -> Vec<PinnedRef> {
        self.pinned.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn toggle_pin(&self, item: PinnedRef) -> bool {
        let mut pinned = self.pinned.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(pos) = pinned.iter().position(|p| *p == item) {
            pinned.remove(pos);
            false
        } else {
            pinned.push(item);
            true
        }
    }

    fn write_locked(&self, g: &mut GuardState, mut state: SessionState, now: Instant) {
        state.close_to_tray = self.close_to_tray();
        state.download_dir = self.download_dir();
        state.ytdlp_path = self.ytdlp_path();
        state.pinned = self.pinned();
        if let Err(e) = save(&self.path, &state) {
            log::warn!("failed to save session state: {e}");
        } else {
            g.last_written_at = Some(now);
        }
    }

    pub fn save_now(&self, state: &SessionState) {
        let mut g = self.guard.lock().unwrap_or_else(|e| e.into_inner());
        self.write_locked(&mut g, state.clone(), Instant::now());
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
        self.write_locked(&mut g, build(), now);
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
        self.write_locked(&mut g, build(), now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn missing_or_corrupt_file_falls_back_to_default() {
        let dir = TempDir::new().unwrap();
        assert_eq!(
            load(&dir.path().join("missing.json")),
            SessionState::default()
        );

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
            replaygain: ReplayGain::Track,
            close_to_tray: false,
            download_dir: Some(PathBuf::from("/music/downloads")),
            ytdlp_path: None,
            pinned: vec![
                PinnedRef {
                    kind: PinnedKind::Album,
                    id: 7,
                },
                PinnedRef {
                    kind: PinnedKind::Playlist,
                    id: 3,
                },
            ],
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
            replaygain: ReplayGain::Off,
            ..SessionState::default()
        }
        .normalized();
        assert_eq!(clamped.volume, 100.0);
        assert_eq!(clamped.current_index, None);
        assert_eq!(clamped.position_seconds, 0.0);
    }

    #[test]
    fn old_session_files_without_replaygain_still_load() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("session.json");
        std::fs::write(&file, r#"{"volume":55.0,"shuffle":true}"#).unwrap();
        let s = load(&file);
        assert_eq!(s.volume, 55.0);
        assert!(s.shuffle);
        assert_eq!(s.replaygain, ReplayGain::Album);

        let json = serde_json::to_value(SessionState {
            replaygain: ReplayGain::Off,
            ..SessionState::default()
        })
        .unwrap();
        assert_eq!(json["replaygain"], "off");
    }

    #[test]
    fn close_to_tray_defaults_on_and_is_persisted_by_the_store() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("session.json");

        // Old files without the field, and missing files, default to on.
        std::fs::write(&file, r#"{"volume":55.0}"#).unwrap();
        assert!(load(&file).close_to_tray);
        assert!(SessionState::default().close_to_tray);
        assert!(SessionStore::new(file.clone()).close_to_tray());
        assert!(SessionStore::new(file.clone()).load().close_to_tray);

        // The store writes its own value, whatever the state it is given says.
        let store = SessionStore::new(file.clone());
        store.set_close_to_tray(false);
        store.save_now(&SessionState::default());
        assert!(!load(&file).close_to_tray);
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(json["closeToTray"], false);

        // ...and a new store picks it up on load.
        let again = SessionStore::new(file.clone());
        assert!(again.close_to_tray());
        assert!(!again.load().close_to_tray);
        assert!(!again.close_to_tray());

        // Later saves driven by player events keep the value.
        again.maybe_save_position(SessionState::default);
        assert!(!load(&file).close_to_tray);
    }

    #[test]
    fn pinned_items_dedup_toggle_and_persist() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("session.json");
        let album = PinnedRef {
            kind: PinnedKind::Album,
            id: 42,
        };
        let playlist = PinnedRef {
            kind: PinnedKind::Playlist,
            id: 9,
        };

        let store = SessionStore::new(file.clone());
        assert!(store.toggle_pin(album));
        assert!(store.toggle_pin(playlist));
        assert_eq!(store.pinned(), vec![album, playlist]);
        store.save_now(&SessionState::default());

        let reloaded = SessionStore::new(file.clone());
        let state = reloaded.load();
        assert_eq!(state.pinned, vec![album, playlist]);
        assert!(!reloaded.toggle_pin(album));
        assert_eq!(reloaded.pinned(), vec![playlist]);
    }
}
