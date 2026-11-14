//! "A new version is available" check against the project's GitHub
//! releases. Only reads the latest release; downloading and installing is
//! left to the user (the in-app updater needs a signing key first).

use crate::net::{Feature, NetPolicy};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const LATEST_URL: &str = "https://api.github.com/repos/r0madeus/foomodern/releases/latest";
/// Only release pages of this project may be opened from the app.
pub const RELEASES_PREFIX: &str = "https://github.com/r0madeus/foomodern/releases/";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// Version of the latest release, without the leading `v`.
    pub version: String,
    /// Release page.
    pub url: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

/// `x.y.z` (a leading `v`, missing parts and a `-suffix` are allowed) as
/// numbers; `None` if it is not a version.
fn parse(version: &str) -> Option<[u64; 3]> {
    let core = version.trim().trim_start_matches('v');
    let core = core.split(['-', '+']).next()?;
    let mut out = [0u64; 3];
    let mut parts = core.split('.');
    for slot in out.iter_mut() {
        match parts.next() {
            Some(p) => *slot = p.parse().ok()?,
            None => break,
        }
    }
    if parts.next().is_some() {
        return None;
    }
    Some(out)
}

/// Whether `latest` is a newer version than `current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse(latest), parse(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// The latest release if it is newer than `current`.
pub async fn check(current: &str, policy: &NetPolicy) -> Result<Option<UpdateInfo>, String> {
    check_at(LATEST_URL, current, policy).await
}

/// [`check`] against another address (tests).
async fn check_at(
    url: &str,
    current: &str,
    policy: &NetPolicy,
) -> Result<Option<UpdateInfo>, String> {
    policy.require(Feature::Updates).map_err(str::to_owned)?;
    fetch_release(url, current).await.map_err(|e| e.to_string())
}

async fn fetch_release(url: &str, current: &str) -> Result<Option<UpdateInfo>, reqwest::Error> {
    let release: Release = crate::net::client_builder()
        .user_agent(concat!("FooModern/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if release.draft
        || release.prerelease
        || !release.html_url.starts_with(RELEASES_PREFIX)
        || !is_newer(&release.tag_name, current)
    {
        return Ok(None);
    }
    Ok(Some(UpdateInfo {
        version: release.tag_name.trim_start_matches('v').to_string(),
        url: release.html_url,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("v0.4", "0.3.0"));
        assert!(is_newer("v0.3.1", "0.3.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("v0.10.0", "0.9.0"));
        assert!(!is_newer("v0.3", "0.3.0"));
        assert!(!is_newer("v0.2.9", "0.3.0"));
        assert!(!is_newer("v0.4.0-beta", "0.4.0"));
    }

    #[test]
    fn rejects_non_versions() {
        assert!(!is_newer("nightly", "0.3.0"));
        assert!(!is_newer("v1.2.3.4", "0.3.0"));
        assert!(!is_newer("v1.0", "dev"));
    }

    /// A one-shot HTTP server that counts the requests it receives.
    fn serve(body: &'static str) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/latest", listener.local_addr().unwrap());
        let count = std::sync::Arc::new(AtomicUsize::new(0));
        let seen = std::sync::Arc::clone(&count);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                seen.fetch_add(1, Ordering::SeqCst);
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        (url, count)
    }

    #[test]
    fn update_check_is_refused_without_a_request_when_off() {
        use std::sync::atomic::Ordering;
        let (url, count) = serve(
            r#"{"tag_name":"v99.0.0","html_url":"https://github.com/r0madeus/foomodern/releases/tag/v99.0.0"}"#,
        );
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let policy = NetPolicy::default();

        // Offline mode, and the updates switch alone: error, no request.
        policy.set_offline(true);
        let err = rt.block_on(check_at(&url, "0.1.0", &policy)).unwrap_err();
        assert_eq!(err, "disabled in Settings");
        policy.set_offline(false);
        policy.set_feature(Feature::Updates, false);
        let err = rt.block_on(check_at(&url, "0.1.0", &policy)).unwrap_err();
        assert_eq!(err, "disabled in Settings");
        assert_eq!(count.load(Ordering::SeqCst), 0);

        // Other switches off does not stop updates.
        policy.set_feature(Feature::Updates, true);
        for f in [Feature::Lyrics, Feature::Online, Feature::Metadata] {
            policy.set_feature(f, false);
        }
        let found = rt
            .block_on(check_at(&url, "0.1.0", &policy))
            .unwrap()
            .expect("newer release");
        assert_eq!(found.version, "99.0.0");
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    /// Hits the real GitHub API: `cargo test -- --ignored live_latest_release`.
    #[test]
    #[ignore]
    fn live_latest_release() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let found = rt
            .block_on(check("0.0.1", &NetPolicy::default()))
            .unwrap()
            .expect("a release newer than 0.0.1");
        assert!(found.url.starts_with(RELEASES_PREFIX), "{found:?}");
        assert_eq!(
            rt.block_on(check("999.0.0", &NetPolicy::default()))
                .unwrap(),
            None
        );
    }
}
