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
pub struct AppConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default = "default_gauge_mode")]
    pub gauge_mode: String,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_sec: u32,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default = "default_threshold")]
    pub notification_threshold: u32,
}

fn default_version() -> u32 { 1 }
fn default_theme() -> String { "cyber".into() }
fn default_locale() -> String { "auto".into() }
fn default_gauge_mode() -> String { "remaining".into() }
fn default_poll_interval() -> u32 { 180 }
fn default_threshold() -> u32 { 80 }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            theme: default_theme(),
            locale: default_locale(),
            gauge_mode: default_gauge_mode(),
            poll_interval_sec: default_poll_interval(),
            auto_start: false,
            notification_threshold: default_threshold(),
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
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn save_session(&self, org_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = app_dir().join("session.json");
        let json = serde_json::json!({ "org_id": org_id });
        fs::write(path, json.to_string())?;
        Ok(())
    }

    pub fn load_session() -> Option<String> {
        let path = app_dir().join("session.json");
        let data = fs::read_to_string(path).ok()?;
        let v: serde_json::Value = serde_json::from_str(&data).ok()?;
        v["org_id"].as_str().map(|s| s.to_string())
    }
}
