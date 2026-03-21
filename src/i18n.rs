//! Internationalization — loads locale strings for tray/tooltip/menu/dashboard.

use std::collections::HashMap;

/// Get the raw JSON string for a locale (used by popup.rs for HTML injection).
pub fn locale_json(locale: &str) -> String {
    match locale {
        "ja" => include_str!("locales/ja.json").to_string(),
        "de" => include_str!("locales/de.json").to_string(),
        "ko" => include_str!("locales/ko.json").to_string(),
        "fr" => include_str!("locales/fr.json").to_string(),
        _ => String::new(), // English is the default in HTML
    }
}

pub struct Strings {
    map: HashMap<String, String>,
}

impl Strings {
    pub fn load(locale: &str) -> Self {
        let json = match locale {
            "ja" => include_str!("locales/ja.json"),
            "de" => include_str!("locales/de.json"),
            "ko" => include_str!("locales/ko.json"),
            "fr" => include_str!("locales/fr.json"),
            _ => include_str!("locales/en.json"),
        };
        let map: HashMap<String, String> = serde_json::from_str(json).unwrap_or_default();
        Self { map }
    }

    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.map.get(key).map(|s| s.as_str()).unwrap_or(key)
    }
}
