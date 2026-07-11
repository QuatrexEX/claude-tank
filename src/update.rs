//! GitHub release update check.
//!
//! Queries the public GitHub "latest release" API and compares its tag with the
//! compiled-in version. No auth needed (unauthenticated GitHub API allows 60
//! requests/hour per IP — far above one check per startup + every few hours).
//! Any failure (offline, rate limited, parse error) is swallowed and treated as
//! "no update", so a check never disrupts the tray.

const RELEASES_LATEST: &str =
    "https://api.github.com/repos/QuatrexEX/claude-tank/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/QuatrexEX/claude-tank/releases/latest";

pub struct UpdateInfo {
    /// Version without the leading "v" (e.g. "1.5.0").
    pub version: String,
    /// URL of the release page to open in the browser.
    pub url: String,
}

/// Return `Some` if GitHub's latest release is newer than the running build.
pub fn check() -> Option<UpdateInfo> {
    let agent = crate::api::build_agent();
    let mut resp = agent
        .get(RELEASES_LATEST)
        // GitHub rejects requests without a User-Agent.
        .header("User-Agent", "claude-tank")
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?;
    let body: serde_json::Value = resp.body_mut().read_json().ok()?;

    let tag = body.get("tag_name").and_then(|v| v.as_str())?;
    let latest = parse_version(tag)?;
    let current = parse_version(env!("CARGO_PKG_VERSION"))?;
    if latest <= current {
        return None;
    }

    let url = body
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or(RELEASES_PAGE)
        .to_string();
    Some(UpdateInfo {
        version: tag.trim_start_matches('v').to_string(),
        url,
    })
}

/// Parse "v1.4.0" / "1.4" / "2.0.0-beta" → (major, minor, patch). Pre-release
/// and build suffixes are dropped (compared as their base release).
fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let core = s.trim().trim_start_matches('v');
    let core = core.split(['-', '+']).next().unwrap_or(core);
    let mut it = core.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next().unwrap_or("0").parse().ok()?;
    let patch = it.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::parse_version;

    #[test]
    fn parses_common_forms() {
        assert_eq!(parse_version("v1.4.0"), Some((1, 4, 0)));
        assert_eq!(parse_version("1.4"), Some((1, 4, 0)));
        assert_eq!(parse_version("2"), Some((2, 0, 0)));
        assert_eq!(parse_version("2.0.0-beta.1"), Some((2, 0, 0)));
        assert_eq!(parse_version(" v10.2.3 "), Some((10, 2, 3)));
        assert_eq!(parse_version("garbage"), None);
    }

    #[test]
    fn orders_by_precedence() {
        assert!(parse_version("v1.5.0") > parse_version("v1.4.9"));
        assert!(parse_version("v2.0.0") > parse_version("v1.99.99"));
        assert!(parse_version("v1.4.1") > parse_version("v1.4.0"));
        // Equal versions must NOT be treated as an update.
        assert!(!(parse_version("v1.4.0") > parse_version("1.4.0")));
    }
}
