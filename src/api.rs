use std::collections::HashMap;

const BASE_URL: &str = "https://claude.ai/api";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0.0.0";

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]  // reset/model fields used in future dashboard
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
        body.as_array()
            .and_then(|arr| arr.first())
            .and_then(|org| org.get("uuid"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "No organizations found".to_string())
    }

    /// Detect plan type (makes an extra API call)
    pub fn detect_plan(&self) -> Result<String, String> {
        let body = self.get_json("/organizations")?;
        let org = body.as_array()
            .and_then(|arr| arr.first())
            .ok_or("No organizations")?;
        Self::detect_plan_from_org(org)
    }

    /// Detect plan from already-fetched organization JSON (avoids extra API call)
    pub fn detect_plan_from_org(org: &serde_json::Value) -> Result<String, String> {
        // Check capabilities, billing, or active_flags
        let billing = org.get("billing");
        if let Some(b) = billing {
            let plan = b.get("plan_type")
                .or_else(|| b.get("plan"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !plan.is_empty() {
                return Ok(format_plan_name(plan));
            }
        }

        // Check active_flags or capabilities
        if let Some(caps) = org.get("capabilities").and_then(|v| v.as_array()) {
            let names: Vec<&str> = caps.iter().filter_map(|v| v.as_str()).collect();
            if names.iter().any(|c| c.contains("enterprise")) { return Ok("Enterprise".into()); }
            if names.iter().any(|c| c.contains("team")) { return Ok("Team".into()); }
        }

        // Check rate_limit or settings for plan hints
        if let Some(settings) = org.get("settings") {
            if settings.get("claude_pro_active").and_then(|v| v.as_bool()).unwrap_or(false) {
                return Ok("Pro".into());
            }
        }

        Ok("Pro".into()) // Default assumption
    }

    pub fn get_usage(&self, org_id: &str) -> Result<UsageData, String> {
        let raw = self.get_json(&format!("/organizations/{}/usage", org_id))?;
        Ok(parse_usage(&raw))
    }
}

fn format_plan_name(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("enterprise") { "Enterprise".into() }
    else if lower.contains("team") { "Team".into() }
    else if lower.contains("max") && lower.contains("20") { "Max (20x)".into() }
    else if lower.contains("max") { "Max".into() }
    else if lower.contains("pro") { "Pro".into() }
    else { raw.to_string() }
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
