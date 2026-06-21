use std::collections::HashMap;

const BASE_URL: &str = "https://claude.ai/api";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0.0.0";

#[derive(Debug, Clone, Default)]
pub struct UsageData {
    pub five_hour: f64,
    pub five_hour_reset: Option<String>,
    pub seven_day: f64,
    pub seven_day_reset: Option<String>,
    pub opus: f64,
    pub sonnet: f64,
}

pub struct ApiClient {
    session_key: String,
    extra_cookies: HashMap<String, String>,
}

impl ApiClient {
    pub fn new(session_key: String, extra_cookies: HashMap<String, String>) -> Self {
        Self { session_key, extra_cookies }
    }

    fn cookie_header(&self) -> String {
        let mut parts = vec![format!("sessionKey={}", self.session_key)];
        for (k, v) in &self.extra_cookies {
            parts.push(format!("{}={}", k, v));
        }
        parts.join("; ")
    }

    fn get_json(&self, path: &str) -> Result<serde_json::Value, String> {
        let url = format!("{}{}", BASE_URL, path);
        let mut resp = ureq::get(&url)
            .header("Accept", "application/json")
            .header("Cookie", &self.cookie_header())
            .header("User-Agent", USER_AGENT)
            .header("anthropic-client-platform", "web_claude_ai")
            .call()
            .map_err(|e| format!("Request failed: {}", e))?;

        resp.body_mut()
            .read_json()
            .map_err(|e| format!("JSON parse error: {}", e))
    }

    pub fn get_org_id(&self) -> Result<String, String> {
        let body = self.get_json("/organizations")?;
        let orgs = body.as_array().ok_or("No organizations found")?;
        pick_org(orgs)
            .and_then(|org| org.get("uuid"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "No organizations found".to_string())
    }

    /// Detect plan type (makes an extra API call)
    pub fn detect_plan(&self) -> Result<String, String> {
        let body = self.get_json("/organizations")?;
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

    pub fn get_usage(&self, org_id: &str) -> Result<UsageData, String> {
        let raw = self.get_json(&format!("/organizations/{}/usage", org_id))?;
        Ok(parse_usage(&raw))
    }
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
