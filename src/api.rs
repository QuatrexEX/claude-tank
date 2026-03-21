use serde::Deserialize;
use std::collections::HashMap;

const BASE_URL: &str = "https://claude.ai/api";

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UsageBlock {
    pub utilization: Option<f64>,
    pub resets_at: Option<String>,
}

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

    pub fn get_org_id(&self) -> Result<String, String> {
        let url = format!("{}/organizations", BASE_URL);
        let mut resp = ureq::get(&url)
            .header("Accept", "application/json")
            .header("Cookie", &self.cookie_header())
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0.0.0")
            .header("anthropic-client-platform", "web_claude_ai")
            .call()
            .map_err(|e| format!("Request failed: {}", e))?;

        let body: serde_json::Value = resp.body_mut()
            .read_json()
            .map_err(|e| format!("JSON parse error: {}", e))?;

        if let Some(arr) = body.as_array() {
            if let Some(org) = arr.first() {
                if let Some(uuid) = org.get("uuid").and_then(|v| v.as_str()) {
                    return Ok(uuid.to_string());
                }
            }
        }
        Err("No organizations found".to_string())
    }

    pub fn get_usage(&self, org_id: &str) -> Result<UsageData, String> {
        let url = format!("{}/organizations/{}/usage", BASE_URL, org_id);
        let mut resp = ureq::get(&url)
            .header("Accept", "application/json")
            .header("Cookie", &self.cookie_header())
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0.0.0")
            .header("anthropic-client-platform", "web_claude_ai")
            .call()
            .map_err(|e| format!("Request failed: {}", e))?;

        let raw: serde_json::Value = resp.body_mut()
            .read_json()
            .map_err(|e| format!("JSON parse error: {}", e))?;

        Ok(parse_usage(&raw))
    }
}

fn parse_usage(raw: &serde_json::Value) -> UsageData {
    fn get_block(raw: &serde_json::Value, key: &str) -> (f64, Option<String>) {
        if let Some(block) = raw.get(key) {
            let util = block.get("utilization")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let reset = block.get("resets_at")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            (util, reset)
        } else {
            (0.0, None)
        }
    }

    let (fh, fh_r) = get_block(raw, "five_hour");
    let (sd, sd_r) = get_block(raw, "seven_day");
    let (op, _) = get_block(raw, "seven_day_opus");
    let (so, _) = get_block(raw, "seven_day_sonnet");

    UsageData {
        five_hour: fh,
        five_hour_reset: fh_r,
        seven_day: sd,
        seven_day_reset: sd_r,
        opus: op,
        sonnet: so,
    }
}

/// Parse a cookie string (from user input) into sessionKey + extras
pub fn parse_cookie_input(raw: &str) -> Option<(String, HashMap<String, String>)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with("sk-ant-") {
        return Some((raw.to_string(), HashMap::new()));
    }
    let mut session_key = String::new();
    let mut extras = HashMap::new();
    for pair in raw.split(';') {
        let pair = pair.trim();
        if let Some((name, value)) = pair.split_once('=') {
            let name = name.trim();
            let value = value.trim();
            if name == "sessionKey" {
                session_key = value.to_string();
            } else if ["cf_clearance", "__cf_bm", "lastActiveOrg"].contains(&name) {
                extras.insert(name.to_string(), value.to_string());
            }
        }
    }
    if session_key.is_empty() {
        None
    } else {
        Some((session_key, extras))
    }
}
