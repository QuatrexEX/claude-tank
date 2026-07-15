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
    pub five_hour: f64,
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
    /// multiplier. `capabilities` (e.g. ["chat","claude_max"]) is the fallback.
    pub fn detect_plan_from_org(org: &serde_json::Value) -> Result<String, String> {
        // 1. rate_limit_tier — most precise, distinguishes Max 5x / 20x.
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
        if raw.get("five_hour").is_none() && raw.get("seven_day").is_none() {
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
fn plan_from_hint(raw: &str) -> Option<String> {
    let lower = raw.to_lowercase();
    if lower.contains("enterprise") { Some("Enterprise".into()) }
    else if lower.contains("team") { Some("Team".into()) }
    else if lower.contains("max") {
        if lower.contains("20") { Some("Max (20x)".into()) }
        else if lower.contains('5') { Some("Max (5x)".into()) }
        else { Some("Max".into()) }
    }
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

    let (fh, fh_r) = block(raw, "five_hour");
    let (sd, sd_r) = block(raw, "seven_day");
    let (op, _) = block(raw, "seven_day_opus");
    let (so, _) = block(raw, "seven_day_sonnet");

    UsageData {
        five_hour: fh, five_hour_reset: fh_r,
        seven_day: sd, seven_day_reset: sd_r,
        opus: op, sonnet: so,
    }
}
