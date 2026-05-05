//! Panic hook — logs to file and shows a MessageBox.
//!
//! Critical for release builds with `windows_subsystem = "windows"` where
//! panics are otherwise completely silent (no console, no error dialog).

use std::fs::OpenOptions;
use std::io::Write;
use std::panic;

pub fn install() {
    panic::set_hook(Box::new(|info| {
        let loc = info.location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "<unknown>".into());

        let payload: &str =
            if let Some(s) = info.payload().downcast_ref::<&str>() { s }
            else if let Some(s) = info.payload().downcast_ref::<String>() { s.as_str() }
            else { "<opaque panic payload>" };

        let summary = format!("Claude Tank crashed at {}\n\n{}", loc, payload);

        write_log(&summary);
        show_message_box(&summary);
    }));
}

fn write_log(summary: &str) {
    let Some(base) = dirs::config_dir() else { return };
    let dir = base.join("Quatrex").join("claude-tank");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("crash.log");
    let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) else { return };
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = writeln!(f, "[unix={}]\n{}\n", ts, summary);
}

fn show_message_box(summary: &str) {
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::core::HSTRING;
    let title = HSTRING::from("Claude Tank — Error");
    let body = HSTRING::from(format!(
        "{}\n\nLog file:\n%APPDATA%\\Quatrex\\claude-tank\\crash.log",
        summary
    ));
    unsafe {
        MessageBoxW(None, &body, &title, MB_OK | MB_ICONERROR);
    }
}
