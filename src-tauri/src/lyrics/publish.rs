//! Sharing lyrics on LRCLIB: solve the proof-of-work challenge, then publish.
//!
//! The challenge gives a `prefix` and a hex `target`; a nonce is valid when
//! `SHA-256(prefix + nonce)`, read as bytes, is <= the target. The challenge
//! expires after 5 minutes, so solving is bounded and restarted with a fresh
//! challenge when it takes too long.

use super::LyricsError;
use super::lrclib::{LrclibClient, PublishBody, PublishReply};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Shorter than LRCLIB's 5 minutes, to leave time for the publish request.
pub const CHALLENGE_TTL: Duration = Duration::from_secs(270);
const MAX_ATTEMPTS: u32 = 4;
const CHECK_EVERY: u64 = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Solve {
    Found(u64),
    Cancelled,
    TimedOut,
}

/// Decodes the hex target into 32 big-endian bytes (shorter values are
/// left-padded with zeros).
pub fn decode_target(hex: &str) -> Option<[u8; 32]> {
    let hex = hex.trim();
    if hex.is_empty() || hex.len() > 64 || !hex.is_ascii() {
        return None;
    }
    let padded = format!("{hex:0>64}");
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&padded[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// The definition: SHA-256 of `prefix` followed by the decimal nonce,
/// compared byte by byte with the target.
pub fn is_valid(prefix: &str, nonce: u64, target: &[u8; 32]) -> bool {
    let hash = Sha256::new_with_prefix(prefix.as_bytes())
        .chain_update(nonce.to_string().as_bytes())
        .finalize();
    hash.as_slice() <= target.as_slice()
}

/// Looks for a valid nonce on several threads until one is found, `cancel`
/// is set or `deadline` passes. Blocking: run it on a blocking thread.
pub fn solve(prefix: &str, target: &[u8; 32], cancel: &AtomicBool, deadline: Instant) -> Solve {
    // Half the cores, at most 4: music keeps playing while this runs.
    let workers = std::thread::available_parallelism()
        .map(|n| n.get() / 2)
        .unwrap_or(1)
        .clamp(1, 4) as u64;
    let stop = AtomicBool::new(false);
    let found: Mutex<Option<u64>> = Mutex::new(None);
    let base = Sha256::new_with_prefix(prefix.as_bytes());

    std::thread::scope(|scope| {
        for w in 0..workers {
            let (stop, found, base) = (&stop, &found, &base);
            scope.spawn(move || {
                let mut nonce = w;
                let mut since_check = 0u64;
                loop {
                    let hash = base.clone().chain_update(nonce.to_string().as_bytes());
                    if hash.finalize().as_slice() <= target.as_slice() {
                        found
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .get_or_insert(nonce);
                        stop.store(true, Ordering::Relaxed);
                        return;
                    }
                    nonce += workers;
                    since_check += 1;
                    if since_check >= CHECK_EVERY {
                        since_check = 0;
                        if stop.load(Ordering::Relaxed)
                            || cancel.load(Ordering::Relaxed)
                            || Instant::now() >= deadline
                        {
                            return;
                        }
                    }
                }
            });
        }
    });

    if let Some(n) = *found.lock().unwrap_or_else(|e| e.into_inner()) {
        Solve::Found(n)
    } else if cancel.load(Ordering::Relaxed) {
        Solve::Cancelled
    } else {
        Solve::TimedOut
    }
}

/// Requests a challenge, solves it and publishes. A challenge that expires
/// or is refused is replaced by a new one (a few times). `ttl` is how long
/// one challenge may be worked on.
pub async fn publish(
    client: &LrclibClient,
    body: &PublishBody,
    cancel: &Arc<AtomicBool>,
    ttl: Duration,
) -> Result<(), LyricsError> {
    for _ in 0..MAX_ATTEMPTS {
        if cancel.load(Ordering::Relaxed) {
            return Err(LyricsError::Cancelled);
        }
        let challenge = client.request_challenge().await?;
        let target = decode_target(&challenge.target)
            .ok_or_else(|| LyricsError::Task("LRCLIB sent an invalid challenge".into()))?;
        let deadline = Instant::now() + ttl;
        let (prefix, flag) = (challenge.prefix.clone(), Arc::clone(cancel));
        let outcome = tokio::task::spawn_blocking(move || solve(&prefix, &target, &flag, deadline))
            .await
            .map_err(|e| LyricsError::Task(e.to_string()))?;
        match outcome {
            Solve::Cancelled => return Err(LyricsError::Cancelled),
            Solve::TimedOut => continue,
            Solve::Found(nonce) => {
                let token = format!("{}:{nonce}", challenge.prefix);
                match client.publish(&token, body).await? {
                    PublishReply::Created => return Ok(()),
                    PublishReply::BadToken => continue,
                }
            }
        }
    }
    Err(LyricsError::Task(
        "LRCLIB kept rejecting the challenge, try again later".into(),
    ))
}
