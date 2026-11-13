//! Events sent by the Kotlin audio plugin (see `android.rs`). Kept free of
//! Android-only APIs so the parser is unit tested on every platform.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use super::{EndReason, EngineEvent, RemoteCommand};
use serde_json::Value;
use std::path::PathBuf;

/// Turns one event object from Kotlin into an [`EngineEvent`].
pub fn parse_event(v: &Value) -> Option<EngineEvent> {
    let path = || v.get("path")?.as_str().map(PathBuf::from);
    let seconds = || v.get("seconds")?.as_f64();
    Some(match v.get("type")?.as_str()? {
        "started" => EngineEvent::TrackStarted {
            path: path()?,
            auto_advanced: v.get("auto")?.as_bool()?,
        },
        "ended" => EngineEvent::TrackEnded {
            path: path()?,
            reason: match v.get("reason")?.as_str()? {
                "eof" => EndReason::Eof,
                "error" => EndReason::Error,
                _ => EndReason::Stopped,
            },
        },
        "duration" => EngineEvent::Duration {
            seconds: seconds()?,
        },
        "position" => EngineEvent::Position {
            seconds: seconds()?,
        },
        "paused" => EngineEvent::Paused(v.get("paused")?.as_bool()?),
        "idle" => EngineEvent::Idle,
        "error" => EngineEvent::Error(v.get("message")?.as_str()?.to_owned()),
        "remote" => EngineEvent::Remote(match v.get("action")?.as_str()? {
            "next" => RemoteCommand::Next,
            "previous" => RemoteCommand::Previous,
            _ => return None,
        }),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_every_event_type() {
        let cases = [
            (
                json!({"type": "started", "path": "/m/a.flac", "auto": true}),
                EngineEvent::TrackStarted {
                    path: "/m/a.flac".into(),
                    auto_advanced: true,
                },
            ),
            (
                json!({"type": "ended", "path": "/m/a.flac", "reason": "error"}),
                EngineEvent::TrackEnded {
                    path: "/m/a.flac".into(),
                    reason: EndReason::Error,
                },
            ),
            (
                json!({"type": "ended", "path": "/m/a.flac", "reason": "stopped"}),
                EngineEvent::TrackEnded {
                    path: "/m/a.flac".into(),
                    reason: EndReason::Stopped,
                },
            ),
            (
                json!({"type": "duration", "seconds": 201.5}),
                EngineEvent::Duration { seconds: 201.5 },
            ),
            (
                json!({"type": "position", "seconds": 3}),
                EngineEvent::Position { seconds: 3.0 },
            ),
            (
                json!({"type": "paused", "paused": false}),
                EngineEvent::Paused(false),
            ),
            (json!({"type": "idle"}), EngineEvent::Idle),
            (
                json!({"type": "error", "message": "boom"}),
                EngineEvent::Error("boom".into()),
            ),
            (
                json!({"type": "remote", "action": "previous"}),
                EngineEvent::Remote(RemoteCommand::Previous),
            ),
        ];
        for (input, want) in cases {
            assert_eq!(parse_event(&input), Some(want), "{input}");
        }
    }

    #[test]
    fn rejects_malformed_events() {
        for input in [
            json!({}),
            json!({"type": "nope"}),
            json!({"type": "started", "path": "/a"}),
            json!({"type": "position"}),
            json!({"type": "remote", "action": "shuffle"}),
        ] {
            assert_eq!(parse_event(&input), None, "{input}");
        }
    }
}
