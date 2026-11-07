use std::collections::HashMap;
use std::time::Duration;

const CLAUDE_BASE: &str = "https://claude.ai/api";
const ANTHROPIC_BASE: &str = "https://api.anthropic.com/api";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0.0.0";
const CC_OAUTH_BETA: &str = "oauth-2025-04-20";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Default)]
pub struct UsageData {
    /// `None` when the plan has no 5-hour limit (verified Ultra accounts); the
    /// API then returns `five_hour: null`. Shown as "no limit", never as 100%.
    pub five_hour: Option<f64>,
    pub five_hour_reset: Option<String>,
    pub seven_day: f64,
    pub seven_day_reset: Option<String>,
    pub opus: f64,
    pub sonnet: f64,
}

/// Which credential source an [`ApiClient`] authenticates with.
pub enum AuthSource {
    /// Claude Code OAuth token, read fresh from disk on each request so a
    /// background refresh by Claude Code is picked up automatically.
    ClaudeCode,
    /// claude.ai session cookie captured by the WebView2 login.
    Session {
        session_key: String,
        extra_cookies: HashMap<String, String>,
    },
}

/// Copy descriptor of the active auth source. Lets the poll loop rebuild a
/// client without carrying secrets through the message channel.
#[derive(Clone, Copy, PartialEq)]
pub enum AuthKind {
    ClaudeCode,
    Session,
}

/// Categorized API failure. The poll loop backs off harder on `RateLimited` so
/// the shared Claude Code token isn't hammered while the endpoint is throttling.
#[derive(Debug)]
pub enum ApiError {
    RateLimited,
    Http(u16),
    Other(String),
}

impl ApiError {
    /// HTTP 401 — the OAuth access token expired. Claude Code refreshes it out
    /// of band and we re-read it on the next poll, so recovery is immediate; the
    /// poll loop must not back off on this (that would only delay the recovery).
    pub fn is_auth(&self) -> bool {
        matches!(self, ApiError::Http(401))
    }

    /// Any HTTP 4xx (401 auth expiry, 429 rate limit, …). These are transient
    /// client-side conditions the app recovers from on its own, so the tray
    /// shows a "please wait" reassurance instead of a raw error string.
    pub fn is_client_error(&self) -> bool {
        match self {
            ApiError::RateLimited => true, // 429
            ApiError::Http(code) => (400..500).contains(code),
            ApiError::Other(_) => false,
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::RateLimited => write!(f, "Rate limited (429)"),
            ApiError::Http(code) => write!(f, "HTTP {}", code),
            ApiError::Other(s) => write!(f, "{}", s),
        }
    }
}

impl From<ApiError> for String {
    fn from(e: ApiError) -> String {
        e.to_string()
    }
}

pub struct ApiClient {
    agent: ureq::Agent,
    auth: AuthSource,
}

/// Build a ureq agent configured for Windows Schannel TLS with a global timeout.
///
/// ureq's Config defaults to the Rustls provider even with the rustls feature
/// off, so the provider must be set explicitly to native-tls (Windows Schannel)
/// or the first HTTPS request panics. PlatformVerifier trusts the OS certificate
/// store; the default (bundled WebPki roots) fails Schannel chain building on
/// Windows. Shared by [`ApiClient`] and the GitHub update check.
pub fn build_agent() -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::NativeTls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    // A global timeout keeps a stalled connection from blocking a poll cycle
    // (or startup resume) indefinitely.
    let config = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(REQUEST_TIMEOUT))
        .build();
    ureq::Agent::new_with_config(config)
}

impl ApiClient {
    pub fn new(auth: AuthSource) -> Self {
        Self { agent: build_agent(), auth }
    }

    pub fn session(session_key: String, extra_cookies: HashMap<String, String>) -> Self {
        Self::new(AuthSource::Session { session_key, extra_cookies })
    }

    pub fn claude_code() -> Self {
        Self::new(AuthSource::ClaudeCode)
    }

    fn cookie_header(session_key: &str, extras: &HashMap<String, String>) -> String {
        let mut parts = vec![format!("sessionKey={}", session_key)];
        for (k, v) in extras {
            parts.push(format!("{}={}", k, v));
        }
        parts.join("; ")
    }

    fn get_json(&self, url: &str) -> Result<serde_json::Value, ApiError> {
        let req = self.agent.get(url)
            .header("Accept", "application/json")
            .header("User-Agent", USER_AGENT);

        let req = match &self.auth {
            AuthSource::Session { session_key, extra_cookies } => req
                .header("Cookie", &Self::cookie_header(session_key, extra_cookies))
                .header("anthropic-client-platform", "web_claude_ai"),
            AuthSource::ClaudeCode => {
                let creds = crate::cc::load()
                    .ok_or_else(|| ApiError::Other("Claude Code credentials not found".into()))?;
                req.header("Authorization", &format!("Bearer {}", creds.access_token))
                    .header("anthropic-beta", CC_OAUTH_BETA)
                    .header("anthropic-version", ANTHROPIC_VERSION)
            }
        };

        let mut resp = req.call().map_err(|e| match e {
            ureq::Error::StatusCode(429) => ApiError::RateLimited,
            ureq::Error::StatusCode(code) => ApiError::Http(code),
            other => ApiError::Other(format!("Request failed: {}", other)),
        })?;
        resp.body_mut()
            .read_json()
            .map_err(|e| ApiError::Other(format!("JSON parse error: {}", e)))
    }

    pub fn get_org_id(&self) -> Result<String, String> {
        let body = self.get_json(&format!("{}/organizations", CLAUDE_BASE))?;
        let orgs = body.as_array().ok_or("No organizations found")?;
        pick_org(orgs)
            .and_then(|org| org.get("uuid"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "No organizations found".to_string())
    }

    /// Detect plan type (makes an extra API call). Session auth only.
    pub fn detect_plan(&self) -> Result<String, String> {
        let body = self.get_json(&format!("{}/organizations", CLAUDE_BASE))?;
        let orgs = body.as_array().ok_or("No organizations")?;
        let org = pick_org(orgs).ok_or("No organizations")?;
        Self::detect_plan_from_org(org)
    }

    /// Detect plan from already-fetched organization JSON (avoids extra API call).
    ///
    /// The claude.ai `/organizations` response does not carry a single "plan" field.
    /// The richest signal is `rate_limit_tier` (e.g. "default_claude_max_5x",
    /// "default_claude_max_20x", "default_claude_pro"), which also encodes the Max
    /// multiplier (20x is the plan now sold as Ultra). `capabilities` (e.g. ["chat","claude_max"]) is the fallback.
    pub fn detect_plan_from_org(org: &serde_json::Value) -> Result<String, String> {
        // 1. rate_limit_tier — most precise, distinguishes Max / Ultra.
        if let Some(name) = org.get("rate_limit_tier")
            .and_then(|v| v.as_str())
            .and_then(plan_from_hint)
        {
            return Ok(name);
        }

        // 2. capabilities array — carries claude_max / claude_pro / team / enterprise.
        if let Some(caps) = org.get("capabilities").and_then(|v| v.as_array()) {
            let joined = caps.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(",");
            if let Some(name) = plan_from_hint(&joined) {
                return Ok(name);
            }
        }

        // 3. Explicit billing.plan_type, if a future response shape provides it.
        if let Some(b) = org.get("billing") {
            if let Some(name) = b.get("plan_type")
                .or_else(|| b.get("plan"))
                .and_then(|v| v.as_str())
                .and_then(plan_from_hint)
            {
                return Ok(name);
            }
        }

        Ok("Pro".into()) // Default assumption
    }

    pub fn get_usage(&self, org_id: &str) -> Result<UsageData, ApiError> {
        let url = match &self.auth {
            AuthSource::ClaudeCode => format!("{}/oauth/usage", ANTHROPIC_BASE),
            AuthSource::Session { .. } => format!("{}/organizations/{}/usage", CLAUDE_BASE, org_id),
        };
        let raw = self.get_json(&url)?;
        // A 200 body carrying an `error` object, or one missing both usage
        // blocks, means the shape isn't what we expect. Fail loudly (keeping the
        // last gauge) instead of silently parsing every field to 0% → full tank.
        if raw.get("error").is_some() {
            return Err(ApiError::Other("API returned an error object".into()));
        }
        // `null` counts as absent: a verified Ultra plan returns five_hour: null.
        let present = |k: &str| raw.get(k).is_some_and(|v| !v.is_null());
        if !present("five_hour") && !present("seven_day") {
            return Err(ApiError::Other("Unexpected usage response shape".into()));
        }
        Ok(parse_usage(&raw))
    }
}

/// Derive a plan display name from Claude Code's stored credential fields.
pub fn plan_from_creds(creds: &crate::cc::CcCreds) -> String {
    creds.rate_limit_tier.as_deref().and_then(plan_from_hint)
        .or_else(|| creds.subscription_type.as_deref().and_then(plan_from_hint))
        .unwrap_or_else(|| "Pro".into())
}

/// Choose the claude.ai consumer organization — the one with the "chat"
/// capability — falling back to the first org. Accounts frequently also have
/// API-only orgs (capabilities ["api"]) that must not be mistaken for the
/// subscription org when reading usage or detecting the plan.
fn pick_org(orgs: &[serde_json::Value]) -> Option<&serde_json::Value> {
    orgs.iter()
        .find(|o| o.get("capabilities")
            .and_then(|c| c.as_array())
            .map(|caps| caps.iter().any(|v| v.as_str() == Some("chat")))
            .unwrap_or(false))
        .or_else(|| orgs.first())
}

/// Map a raw hint (rate_limit_tier, capability, or plan_type) to a display name.
/// Returns None when no known plan keyword is present so callers can fall through.
///
/// Uses Claude's current plan names (Oct 2026): the former "Max 20x" is sold as
/// "Ultra" and "Max 5x" simply as "Max". The internal identifiers still say
/// `max_20x` / `max_5x`, so both the old and any future "ultra" form map here.
fn plan_from_hint(raw: &str) -> Option<String> {
    let lower = raw.to_lowercase();
    if lower.contains("enterprise") { Some("Enterprise".into()) }
    else if lower.contains("team") { Some("Team".into()) }
    else if lower.contains("ultra") || (lower.contains("max") && lower.contains("20")) {
        Some("Ultra".into())
    }
    else if lower.contains("max") { Some("Max".into()) }
    else if lower.contains("pro") { Some("Pro".into()) }
    else if lower.contains("free") { Some("Free".into()) }
    else { None }
}

fn parse_usage(raw: &serde_json::Value) -> UsageData {
    fn block(raw: &serde_json::Value, key: &str) -> (f64, Option<String>) {
        match raw.get(key) {
            Some(b) => (
                b.get("utilization").and_then(|v| v.as_f64()).unwrap_or(0.0),
                b.get("resets_at").and_then(|v| v.as_str()).map(String::from),
            ),
            None => (0.0, None),
        }
    }

    // A missing or null five_hour block means the plan has no 5-hour limit.
    let (fh, fh_r) = match raw.get("five_hour").filter(|v| v.is_object()) {
        Some(_) => {
            let (u, r) = block(raw, "five_hour");
            (Some(u), r)
        }
        None => (None, None),
    };
    let (sd, sd_r) = block(raw, "seven_day");
    let (op, _) = block(raw, "seven_day_opus");
    let (so, _) = block(raw, "seven_day_sonnet");

    UsageData {
        five_hour: fh, five_hour_reset: fh_r,
        seven_day: sd, seven_day_reset: sd_r,
        opus: op, sonnet: so,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_usage, plan_from_hint};
    use serde_json::json;

    #[test]
    fn maps_current_plan_names() {
        // Internal identifiers still carry the old Max multipliers.
        assert_eq!(plan_from_hint("default_claude_max_20x").as_deref(), Some("Ultra"));
        assert_eq!(plan_from_hint("default_claude_max_5x").as_deref(), Some("Max"));
        assert_eq!(plan_from_hint("max").as_deref(), Some("Max"));
        assert_eq!(plan_from_hint("default_claude_ultra").as_deref(), Some("Ultra"));
        assert_eq!(plan_from_hint("default_claude_pro").as_deref(), Some("Pro"));
        assert_eq!(plan_from_hint("chat,claude_max").as_deref(), Some("Max"));
        assert_eq!(plan_from_hint("team").as_deref(), Some("Team"));
        assert_eq!(plan_from_hint("enterprise").as_deref(), Some("Enterprise"));
        assert_eq!(plan_from_hint("api"), None);
    }

    #[test]
    fn null_five_hour_means_no_limit() {
        let raw = json!({
            "five_hour": null,
            "seven_day": { "utilization": 42.0, "resets_at": "2026-11-10T00:00:00Z" }
        });
        let d = parse_usage(&raw);
        assert_eq!(d.five_hour, None);
        assert_eq!(d.five_hour_reset, None);
        assert_eq!(d.seven_day, 42.0);
    }

    #[test]
    fn present_five_hour_is_read() {
        let raw = json!({
            "five_hour": { "utilization": 12.5, "resets_at": "2026-11-08T03:00:00Z" },
            "seven_day": { "utilization": 30.0, "resets_at": null }
        });
        let d = parse_usage(&raw);
        assert_eq!(d.five_hour, Some(12.5));
        assert_eq!(d.five_hour_reset.as_deref(), Some("2026-11-08T03:00:00Z"));
    }
}
