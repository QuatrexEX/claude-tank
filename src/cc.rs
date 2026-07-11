//! Claude Code credential bridge.
//!
//! Claude Code stores an OAuth token at `~/.claude/.credentials.json` and keeps
//! it fresh itself. Reusing it lets Claude Tank show usage with no login window
//! when the user already runs Claude Code. Strictly read-only: we never write
//! or refresh this file — that is Claude Code's job. We re-read it on every poll
//! so a background token refresh is picked up automatically.
//!
//! Claude Code often runs inside WSL rather than on Windows. A live token in
//! the Windows home directory always wins — that path is byte-for-byte the
//! pre-WSL behavior. Only when it is missing or expired are the WSL distros'
//! `/home/*/.claude/.credentials.json` probed via the `\\wsl.localhost\` share,
//! taking the freshest `expiresAt` on offer. Note: reading a distro's files
//! starts that distro if it is not already running, which is why the probe is
//! strictly a fallback.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct CcCreds {
    pub access_token: String,
    pub rate_limit_tier: Option<String>,
    pub subscription_type: Option<String>,
}

fn wsl_candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for distro in wsl_distros() {
        let home_root = PathBuf::from(format!(r"\\wsl.localhost\{}\home", distro));
        if let Ok(entries) = std::fs::read_dir(&home_root) {
            for entry in entries.flatten() {
                paths.push(entry.path().join(".claude").join(".credentials.json"));
            }
        }
    }
    paths
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

/// Load the Claude Code OAuth credentials, if present and parseable.
pub fn load() -> Option<CcCreds> {
    let local = dirs::home_dir()
        .and_then(|h| load_file(&h.join(".claude").join(".credentials.json")));

    // A live Windows-local token wins outright, so users who run Claude Code
    // on Windows keep the exact pre-WSL behavior and the WSL VM is never woken.
    match local {
        Some((expires, creds)) if expires > now_ms() => Some(creds),
        local => {
            // No usable local file: probe the WSL distros and take the
            // freshest token, keeping a stale local one as last resort (a 401
            // then drops the caller back to session auth).
            let mut best = local;
            for path in wsl_candidate_paths() {
                if let Some((expires, creds)) = load_file(&path) {
                    if best.as_ref().map_or(true, |(b, _)| expires > *b) {
                        best = Some((expires, creds));
                    }
                }
            }
            best.map(|(_, creds)| creds)
        }
    }
}

fn load_file(path: &Path) -> Option<(f64, CcCreds)> {
    let data = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&data).ok()?;
    let oauth = v.get("claudeAiOauth")?;

    let access_token = oauth.get("accessToken")?.as_str()?.to_string();
    if access_token.is_empty() {
        return None;
    }
    let expires = oauth.get("expiresAt").and_then(|e| e.as_f64()).unwrap_or(0.0);
    let str_field = |k: &str| oauth.get(k).and_then(|s| s.as_str()).map(String::from);
    Some((expires, CcCreds {
        access_token,
        rate_limit_tier: str_field("rateLimitTier"),
        subscription_type: str_field("subscriptionType"),
    }))
}

/// Names of the installed WSL distros, read from the per-user Lxss registry
/// key. Empty when WSL is not installed.
fn wsl_distros() -> Vec<String> {
    use windows::core::{w, PCWSTR, PWSTR};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_READ,
    };

    let mut distros = Vec::new();
    unsafe {
        let mut lxss = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Lxss"),
            None, KEY_READ, &mut lxss,
        ).is_err() {
            return distros;
        }
        for index in 0.. {
            let mut guid = [0u16; 64];
            let mut len = guid.len() as u32;
            if RegEnumKeyExW(
                lxss, index, Some(PWSTR(guid.as_mut_ptr())), &mut len,
                None, None, None, None,
            ).is_err() {
                break;
            }
            let mut sub = HKEY::default();
            if RegOpenKeyExW(lxss, PCWSTR(guid.as_ptr()), None, KEY_READ, &mut sub).is_err() {
                continue;
            }
            let mut buf = [0u16; 256];
            let mut size = (buf.len() * 2) as u32; // bytes
            let r = RegQueryValueExW(
                sub, w!("DistributionName"), None, None,
                Some(buf.as_mut_ptr() as *mut u8), Some(&mut size),
            );
            let _ = RegCloseKey(sub);
            if r.is_ok() && size >= 2 {
                let n = (size as usize / 2).saturating_sub(1); // drop trailing NUL
                distros.push(String::from_utf16_lossy(&buf[..n]));
            }
        }
        let _ = RegCloseKey(lxss);
    }
    distros
}
