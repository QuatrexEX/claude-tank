use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn app_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("Quatrex").join("claude-tank");
    fs::create_dir_all(&dir).ok();
    dir
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]  // theme/gauge_mode used in future versions
pub struct AppConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_gauge_mode")]
    pub gauge_mode: String,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_sec: u32,
}

fn default_theme() -> String { "cyber".into() }
fn default_gauge_mode() -> String { "remaining".into() }
fn default_poll_interval() -> u32 { 180 }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            gauge_mode: default_gauge_mode(),
            poll_interval_sec: default_poll_interval(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let path = app_dir().join("config.json");
        if let Ok(data) = fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            let config = Self::default();
            config.save().ok();
            config
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = app_dir().join("config.json");
        fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn load_session() -> Option<String> {
        let path = app_dir().join("session.json");
        let data = fs::read_to_string(path).ok()?;
        let v: serde_json::Value = serde_json::from_str(&data).ok()?;
        v["org_id"].as_str().map(|s| s.to_string())
    }

    pub fn save_session(org_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = app_dir().join("session.json");
        fs::write(path, serde_json::json!({"org_id": org_id}).to_string())?;
        Ok(())
    }

    pub fn save_credentials(
        session_key: &str,
        extras: &std::collections::HashMap<String, String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = app_dir().join("credentials.json");
        let data = serde_json::json!({
            "session_key": session_key,
            "extra_cookies": extras,
        });
        fs::write(path, data.to_string())?;
        Ok(())
    }

    pub fn load_credentials() -> Option<(String, std::collections::HashMap<String, String>)> {
        let path = app_dir().join("credentials.json");
        let data = fs::read_to_string(path).ok()?;
        let v: serde_json::Value = serde_json::from_str(&data).ok()?;
        let sk = v["session_key"].as_str()?.to_string();
        let extras: std::collections::HashMap<String, String> = v["extra_cookies"]
            .as_object()
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        Some((sk, extras))
    }
}
