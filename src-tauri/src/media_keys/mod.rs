//! OS media controls (media keys, MPRIS on Linux, SMTC on Windows) via
//! `souvlaki`, kept isolated here so it can be swapped if needed.
//!
//! Locking rules: the OS handler runs on a souvlaki thread and only calls
//! `Player` methods (never touches [`MediaKeys`]'s mutex). The player observer
//! calls [`MediaKeys::publish`], which locks the controls; the player's own
//! lock is never held at that point.

use crate::player::{Player, PlayerSnapshot, Status};
use crate::queue::EntryId;
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use std::ffi::c_void;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

/// Step used for `Seek(direction)` events that carry no amount.
const DEFAULT_SEEK_STEP: f64 = 10.0;

/// What the player should do in response to an OS media event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    /// Relative seek in seconds (negative = backwards).
    SeekBy(f64),
    SeekTo(f64),
    /// Volume in percent (0..=100).
    SetVolume(f64),
}

/// Maps an OS event to a player action; `None` for events we ignore.
/// `Stop` maps to `Pause` because the player has no stop command.
pub fn action_for(event: &MediaControlEvent) -> Option<Action> {
    let signed = |dir: &SeekDirection, secs: f64| match dir {
        SeekDirection::Forward => secs,
        SeekDirection::Backward => -secs,
    };
    Some(match event {
        MediaControlEvent::Play => Action::Play,
        MediaControlEvent::Pause | MediaControlEvent::Stop => Action::Pause,
        MediaControlEvent::Toggle => Action::Toggle,
        MediaControlEvent::Next => Action::Next,
        MediaControlEvent::Previous => Action::Previous,
        MediaControlEvent::Seek(dir) => Action::SeekBy(signed(dir, DEFAULT_SEEK_STEP)),
        MediaControlEvent::SeekBy(dir, d) => Action::SeekBy(signed(dir, d.as_secs_f64())),
        MediaControlEvent::SetPosition(MediaPosition(d)) => Action::SeekTo(d.as_secs_f64()),
        MediaControlEvent::SetVolume(v) if v.is_finite() => {
            Action::SetVolume((v * 100.0).clamp(0.0, 100.0))
        }
        _ => return None,
    })
}

fn apply(player: &Player, action: Action) {
    let result = match action {
        Action::Play => player.play(),
        Action::Pause => player.pause(),
        Action::Toggle => player.toggle_pause(),
        Action::Next => player.next(),
        Action::Previous => player.previous(),
        Action::SeekBy(delta) => player.seek((player.snapshot().position_seconds + delta).max(0.0)),
        Action::SeekTo(secs) => player.seek(secs.max(0.0)),
        Action::SetVolume(percent) => player.set_volume(percent),
    };
    if let Err(e) = result {
        log::warn!("media control action {action:?} failed: {e}");
    }
}

/// The parts of the player state that are visible in the OS UI. Position is
/// deliberately not part of it: it changes constantly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Published {
    pub track: Option<EntryId>,
    pub status: Status,
    pub duration: Option<f64>,
}

impl Published {
    pub fn from_snapshot(snap: &PlayerSnapshot) -> Self {
        Published {
            track: snap.current.as_ref().map(|t| t.id),
            status: snap.status,
            duration: snap.duration_seconds,
        }
    }
}

/// What has to be re-sent to the OS for a state change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Changes {
    pub metadata: bool,
    pub playback: bool,
}

impl Changes {
    pub fn any(self) -> bool {
        self.metadata || self.playback
    }
}

/// Pure change detection: metadata when track or duration changed, playback
/// when the status or track changed. `None` previous means nothing sent yet.
pub fn changes(prev: Option<&Published>, new: &Published) -> Changes {
    match prev {
        None => Changes {
            metadata: true,
            playback: true,
        },
        Some(p) => Changes {
            metadata: p.track != new.track || p.duration != new.duration,
            playback: p.track != new.track || p.status != new.status,
        },
    }
}

pub fn playback_for(status: Status, position: f64) -> MediaPlayback {
    let progress = Some(MediaPosition(duration_from_secs(position)));
    match status {
        Status::Playing => MediaPlayback::Playing { progress },
        Status::Paused => MediaPlayback::Paused { progress },
        Status::Stopped => MediaPlayback::Stopped,
    }
}

fn duration_from_secs(secs: f64) -> Duration {
    if secs.is_finite() && secs > 0.0 {
        Duration::from_secs_f64(secs)
    } else {
        Duration::ZERO
    }
}

/// Builds the `cover_url` for a cover file path.
///
/// MPRIS wants a percent-encoded `file:///abs/path` URI. On Windows souvlaki
/// strips the `file://` prefix and passes the rest to `GetFileFromPathAsync`,
/// so it must be `file://` + the native path (`file://C:\dir\cover.jpg`).
pub fn cover_url(path: &str, windows: bool) -> String {
    if windows {
        return format!("file://{path}");
    }
    let mut out = String::from("file://");
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Album title and cover path of a track, supplied by the caller (library).
#[derive(Debug, Default, Clone)]
pub struct Extra {
    pub album: Option<String>,
    pub cover: Option<String>,
}

struct State {
    controls: MediaControls,
    last: Option<Published>,
}

pub struct MediaKeys {
    state: Mutex<State>,
}

impl MediaKeys {
    /// Creates the controls and attaches the event handler. Errors (e.g. no
    /// D-Bus session bus) are returned for the caller to log and ignore.
    pub fn new(hwnd: Option<*mut c_void>, player: Weak<Player>) -> Result<Self, souvlaki::Error> {
        let mut controls = MediaControls::new(PlatformConfig {
            display_name: "FooModern",
            dbus_name: "foomodern",
            hwnd,
        })?;
        controls.attach(move |event| {
            if let (Some(action), Some(player)) = (action_for(&event), player.upgrade()) {
                apply(&player, action);
            }
        })?;
        Ok(MediaKeys {
            state: Mutex::new(State {
                controls,
                last: None,
            }),
        })
    }

    /// Pushes a player state to the OS if something visible changed.
    /// `lookup` is only called when metadata must be re-sent.
    pub fn publish(&self, snap: &PlayerSnapshot, lookup: impl FnOnce(&str) -> Extra) {
        let new = Published::from_snapshot(snap);
        let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let ch = changes(st.last.as_ref(), &new);
        if !ch.any() {
            return;
        }
        if ch.metadata {
            let extra = snap
                .current
                .as_ref()
                .map(|t| lookup(&t.path))
                .unwrap_or_default();
            let url = extra
                .cover
                .as_deref()
                .map(|p| cover_url(p, cfg!(target_os = "windows")));
            let meta = match &snap.current {
                Some(t) => MediaMetadata {
                    title: Some(&t.title),
                    artist: (!t.artist.is_empty()).then_some(t.artist.as_str()),
                    album: extra.album.as_deref().filter(|a| !a.is_empty()),
                    cover_url: url.as_deref(),
                    duration: snap.duration_seconds.map(duration_from_secs),
                },
                None => MediaMetadata::default(),
            };
            if let Err(e) = st.controls.set_metadata(meta) {
                log::warn!("failed to set media metadata: {e:?}");
            }
        }
        if ch.playback || ch.metadata {
            let playback = playback_for(new.status, snap.position_seconds);
            if let Err(e) = st.controls.set_playback(playback) {
                log::warn!("failed to set media playback: {e:?}");
            }
        }
        st.last = Some(new);
    }
}

/// Convenience for `lib.rs`: wraps the shared player.
pub fn create(hwnd: Option<*mut c_void>, player: &Arc<Player>) -> Option<MediaKeys> {
    match MediaKeys::new(hwnd, Arc::downgrade(player)) {
        Ok(m) => Some(m),
        Err(e) => {
            log::warn!("media controls unavailable: {e:?}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(track: Option<EntryId>, status: Status, duration: Option<f64>) -> Published {
        Published {
            track,
            status,
            duration,
        }
    }

    #[test]
    fn first_publish_sends_everything() {
        let c = changes(None, &p(None, Status::Stopped, None));
        assert!(c.metadata && c.playback);
    }

    #[test]
    fn identical_state_is_ignored() {
        let a = p(None, Status::Playing, Some(10.0));
        assert!(!changes(Some(&a), &a).any());
    }

    #[test]
    fn status_change_updates_playback_only() {
        let a = p(None, Status::Playing, Some(10.0));
        let b = p(None, Status::Paused, Some(10.0));
        assert_eq!(
            changes(Some(&a), &b),
            Changes {
                metadata: false,
                playback: true
            }
        );
    }

    #[test]
    fn duration_change_updates_metadata() {
        let a = p(None, Status::Playing, None);
        let b = p(None, Status::Playing, Some(10.0));
        assert_eq!(
            changes(Some(&a), &b),
            Changes {
                metadata: true,
                playback: false
            }
        );
    }

    #[test]
    fn playback_mapping() {
        let prog = Some(MediaPosition(Duration::from_millis(1500)));
        assert_eq!(
            playback_for(Status::Playing, 1.5),
            MediaPlayback::Playing { progress: prog }
        );
        assert_eq!(
            playback_for(Status::Paused, 1.5),
            MediaPlayback::Paused { progress: prog }
        );
        assert_eq!(playback_for(Status::Stopped, 1.5), MediaPlayback::Stopped);
        assert_eq!(
            playback_for(Status::Playing, f64::NAN),
            MediaPlayback::Playing {
                progress: Some(MediaPosition(Duration::ZERO))
            }
        );
    }

    #[test]
    fn cover_url_linux() {
        assert_eq!(
            cover_url("/home/a b/covers/1.jpg", false),
            "file:///home/a%20b/covers/1.jpg"
        );
    }

    #[test]
    fn cover_url_windows() {
        assert_eq!(
            cover_url(r"C:\Users\a b\covers\1.jpg", true),
            r"file://C:\Users\a b\covers\1.jpg"
        );
    }

    #[test]
    fn event_mapping() {
        use MediaControlEvent as E;
        assert_eq!(action_for(&E::Play), Some(Action::Play));
        assert_eq!(action_for(&E::Pause), Some(Action::Pause));
        assert_eq!(action_for(&E::Stop), Some(Action::Pause));
        assert_eq!(action_for(&E::Toggle), Some(Action::Toggle));
        assert_eq!(action_for(&E::Next), Some(Action::Next));
        assert_eq!(action_for(&E::Previous), Some(Action::Previous));
        assert_eq!(
            action_for(&E::Seek(SeekDirection::Backward)),
            Some(Action::SeekBy(-10.0))
        );
        assert_eq!(
            action_for(&E::SeekBy(SeekDirection::Forward, Duration::from_secs(5))),
            Some(Action::SeekBy(5.0))
        );
        assert_eq!(
            action_for(&E::SetPosition(MediaPosition(Duration::from_secs(42)))),
            Some(Action::SeekTo(42.0))
        );
        assert_eq!(
            action_for(&E::SetVolume(0.5)),
            Some(Action::SetVolume(50.0))
        );
        assert_eq!(action_for(&E::SetVolume(f64::NAN)), None);
        assert_eq!(action_for(&E::Raise), None);
        assert_eq!(action_for(&E::Quit), None);
    }
}
