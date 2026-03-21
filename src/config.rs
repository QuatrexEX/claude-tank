use crate::crypto;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const REGISTRY_APP_NAME: &str = "ClaudeTank";

fn app_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("Quatrex").join("claude-tank");
    fs::create_dir_all(&dir).ok();
    dir
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_gauge_mode")]
    pub gauge_mode: String,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_sec: u32,
    #[serde(default)]
    pub locale: String,
    #[serde(default = "default_threshold")]
    pub threshold_5h: u32,
    #[serde(default = "default_threshold")]
    pub threshold_7d: u32,
    #[serde(default)]
    pub auto_start: bool,
}

fn default_theme() -> String { "cyber".into() }
fn default_gauge_mode() -> String { "remaining".into() }
fn default_poll_interval() -> u32 { 180 }
fn default_threshold() -> u32 { 20 } // Alert when remaining drops below 20%

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            gauge_mode: default_gauge_mode(),
            poll_interval_sec: default_poll_interval(),
            locale: String::new(),
            threshold_5h: default_threshold(),
            threshold_7d: default_threshold(),
            auto_start: false,
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

    // ── Session ──

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

    // ── Credentials (DPAPI encrypted) ──

    pub fn save_credentials(
        session_key: &str,
        extras: &HashMap<String, String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let payload = serde_json::json!({
            "session_key": session_key,
            "extra_cookies": extras,
        });
        let json_bytes = payload.to_string().into_bytes();

        match crypto::encrypt(&json_bytes) {
            Ok(encrypted) => {
                let path = app_dir().join("credentials.enc");
                fs::write(&path, &encrypted)?;
                // Remove old plaintext file if exists
                let _ = fs::remove_file(app_dir().join("credentials.json"));
                Ok(())
            }
            Err(_) => {
                // Fallback: save as plaintext (shouldn't happen)
                let path = app_dir().join("credentials.json");
                fs::write(path, payload.to_string())?;
                Ok(())
            }
        }
    }

    pub fn load_credentials() -> Option<(String, HashMap<String, String>)> {
        // Try encrypted first
        let enc_path = app_dir().join("credentials.enc");
        if let Ok(encrypted) = fs::read(&enc_path) {
            if let Ok(decrypted) = crypto::decrypt(&encrypted) {
                if let Ok(json_str) = String::from_utf8(decrypted) {
                    return parse_credentials_json(&json_str);
                }
            }
        }
        // Fallback: try plaintext (migration from old version)
        let json_path = app_dir().join("credentials.json");
        if let Ok(data) = fs::read_to_string(&json_path) {
            if let Some(creds) = parse_credentials_json(&data) {
                // Migrate to encrypted
                let _ = Self::save_credentials(&creds.0, &creds.1);
                return Some(creds);
            }
        }
        None
    }

    // ── Auto-start (Windows Registry) ──

    pub fn set_auto_start(enabled: bool) -> Result<(), String> {
        use windows::Win32::System::Registry::*;
        use windows::core::*;

        let exe_path = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .to_string();

        unsafe {
            let mut key = HKEY::default();
            let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");

            let result = RegOpenKeyExW(HKEY_CURRENT_USER, subkey, None, KEY_SET_VALUE, &mut key);
            if result.is_err() {
                return Err("RegOpenKeyEx failed".into());
            }

            if enabled {
                let value: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
                let name = HSTRING::from(REGISTRY_APP_NAME);
                let result = RegSetValueExW(
                    key, &name, None, REG_SZ,
                    Some(std::slice::from_raw_parts(value.as_ptr() as *const u8, value.len() * 2)),
                );
                if result.is_err() {
                    let _ = RegCloseKey(key);
                    return Err("RegSetValueEx failed".into());
                }
            } else {
                let name = HSTRING::from(REGISTRY_APP_NAME);
                let _ = RegDeleteValueW(key, &name);
            }
            let _ = RegCloseKey(key);
        }
        Ok(())
    }

    /// Resolve the effective locale: if "auto" or empty, detect from OS.
    pub fn effective_locale(&self) -> String {
        if self.locale.is_empty() || self.locale == "auto" {
            Self::detect_locale()
        } else {
            self.locale.clone()
        }
    }

    pub fn detect_locale() -> String {
        let mut buf = [0u16; 85];
        let len = unsafe {
            windows::Win32::Globalization::GetUserDefaultLocaleName(&mut buf)
        };
        if len > 0 {
            let s = String::from_utf16_lossy(&buf[..len as usize - 1]);
            let lang = s.split('-').next().unwrap_or("en");
            // Only return supported locales
            match lang {
                "ja" | "de" | "ko" | "fr" => return lang.to_string(),
                _ => {}
            }
        }
        "en".to_string()
    }
}

fn parse_credentials_json(data: &str) -> Option<(String, HashMap<String, String>)> {
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    let sk = v["session_key"].as_str()?.to_string();
    let extras = v["extra_cookies"]
        .as_object()
        .map(|m| m.iter()
            .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
            .collect())
        .unwrap_or_default();
    Some((sk, extras))
}
