//! Claude Code credential bridge.
//!
//! Claude Code stores an OAuth token at `~/.claude/.credentials.json` and keeps
//! it fresh itself. Reusing it lets Claude Tank show usage with no login window
//! when the user already runs Claude Code. Strictly read-only: we never write
//! or refresh this file — that is Claude Code's job. We re-read it on every poll
//! so a background token refresh is picked up automatically.

use std::path::PathBuf;

pub struct CcCreds {
    pub access_token: String,
    pub rate_limit_tier: Option<String>,
    pub subscription_type: Option<String>,
}

fn credentials_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join(".credentials.json"))
}

/// Load the Claude Code OAuth credentials, if present and parseable.
pub fn load() -> Option<CcCreds> {
    let path = credentials_path()?;
    let data = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&data).ok()?;
    let oauth = v.get("claudeAiOauth")?;

    let access_token = oauth.get("accessToken")?.as_str()?.to_string();
    if access_token.is_empty() {
        return None;
    }
    let str_field = |k: &str| oauth.get(k).and_then(|s| s.as_str()).map(String::from);
    Some(CcCreds {
        access_token,
        rate_limit_tier: str_field("rateLimitTier"),
        subscription_type: str_field("subscriptionType"),
    })
}
