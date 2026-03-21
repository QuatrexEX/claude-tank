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

    pub fn get_usage(&self, org_id: &str) -> Result<UsageData, String> {
        let raw = self.get_json(&format!("/organizations/{}/usage", org_id))?;
        Ok(parse_usage(&raw))
    }
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
