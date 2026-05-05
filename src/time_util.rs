//! ISO 8601 timestamp parsing and "time until" formatting (no chrono dep).

use std::time::{SystemTime, UNIX_EPOCH};

/// Parse an ISO 8601 timestamp and return a compact "Xd Yh" / "Xh Ym" / "Xm" string
/// for the duration from now until that moment. None if past or unparsable.
pub fn time_until(iso: &str) -> Option<String> {
    let target = parse_iso8601(iso)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    let diff = target - now;
    if diff <= 0 { return None; }
    Some(format_duration(diff))
}

fn format_duration(secs: i64) -> String {
    if secs >= 86400 {
        let d = secs / 86400;
        let h = (secs % 86400) / 3600;
        format!("{}d {}h", d, h)
    } else if secs >= 3600 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        format!("{}h {}m", h, m)
    } else if secs >= 60 {
        format!("{}m", secs / 60)
    } else {
        format!("{}s", secs)
    }
}

/// Parse "2026-05-05T18:30:00Z" / "...+00:00" / "...123Z" → unix epoch seconds.
fn parse_iso8601(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 19 { return None; }
    let year: i32 = s.get(0..4)?.parse().ok()?;
    let month: i32 = s.get(5..7)?.parse().ok()?;
    let day: i32 = s.get(8..10)?.parse().ok()?;
    let hour: i64 = s.get(11..13)?.parse().ok()?;
    let minute: i64 = s.get(14..16)?.parse().ok()?;
    let second: i64 = s.get(17..19)?.parse().ok()?;

    // Skip optional .fff
    let rest = &s[19..];
    let mut idx = 0;
    if rest.starts_with('.') {
        idx = 1;
        while idx < rest.len() && rest.as_bytes()[idx].is_ascii_digit() {
            idx += 1;
        }
    }
    let tz = &rest[idx..];
    let offset = parse_tz(tz)?;

    let days = days_from_civil(year, month, day);
    let secs = days * 86400 + hour * 3600 + minute * 60 + second;
    Some(secs - offset)
}

fn parse_tz(tz: &str) -> Option<i64> {
    if tz.is_empty() || tz == "Z" { return Some(0); }
    let bytes = tz.as_bytes();
    let sign = match bytes[0] { b'+' => 1, b'-' => -1, _ => return None };
    let body: String = tz[1..].chars().filter(|c| *c != ':').collect();
    let h: i64 = body.get(0..2)?.parse().ok()?;
    let m: i64 = body.get(2..4).and_then(|x| x.parse().ok()).unwrap_or(0);
    Some(sign * (h * 3600 + m * 60))
}

/// Howard Hinnant's days_from_civil — days since 1970-01-01.
fn days_from_civil(y: i32, m: i32, d: i32) -> i64 {
    let y = y - if m <= 2 { 1 } else { 0 };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as i64;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp as i64 + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era as i64 * 146097 + doe - 719468
}
