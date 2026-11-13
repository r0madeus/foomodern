//! "A new version is available" check against the project's GitHub
//! releases. Only reads the latest release; downloading and installing is
//! left to the user (the in-app updater needs a signing key first).

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
pub async fn check(current: &str) -> Result<Option<UpdateInfo>, reqwest::Error> {
    let release: Release = crate::net::client_builder()
        .user_agent(concat!("FooModern/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()?
        .get(LATEST_URL)
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

    /// Hits the real GitHub API: `cargo test -- --ignored live_latest_release`.
    #[test]
    #[ignore]
    fn live_latest_release() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let found = rt
            .block_on(check("0.0.1"))
            .unwrap()
            .expect("a release newer than 0.0.1");
        assert!(found.url.starts_with(RELEASES_PREFIX), "{found:?}");
        assert_eq!(rt.block_on(check("999.0.0")).unwrap(), None);
    }
}
