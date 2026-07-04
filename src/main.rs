//! Claude Tank — System tray Claude usage monitor
//!
//! Login: wry WebView2 opens claude.ai/login. User logs in normally.
//!        Cookie is auto-detected via polling (same as Usage4Claude on macOS).
//! Polling: ureq HTTP client with sessionKey cookie.
//! Tray: dual-gauge cyber icon (5h/7d remaining).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod cc;
mod config;
mod crypto;
mod i18n;
mod notify;
mod panic_hook;
mod popup;
mod time_util;
mod tray;
mod win_util;

use api::UsageData;
use config::AppConfig;
use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TIMER_ID_COOKIE_POLL: usize = 42;
const COOKIE_POLL_MS: u32 = 2000;

/// Bumped on every successful login. A poll_loop thread exits once it sees a
/// newer generation, so re-login never leaves a stale loop polling in parallel.
static POLL_GENERATION: AtomicUsize = AtomicUsize::new(0);

const BANNER_JS: &str = r#"
(function() {
    if (document.getElementById('ct-banner')) return;
    if (!document.body) return;
    var b = document.createElement('div');
    b.id = 'ct-banner';
    b.style.cssText = 'position:fixed;top:0;left:0;right:0;z-index:99999;' +
        'padding:8px 16px;background:linear-gradient(90deg,#0a0a0f,#12121a);' +
        'color:#00ffcc;font-size:12px;font-family:Consolas,monospace;text-align:center;' +
        'font-weight:500;border-bottom:1px solid #00ffcc40;letter-spacing:0.5px;' +
        'text-shadow:0 0 8px #00ffcc40';
    b.textContent = '\u25c8 CLAUDE TANK \u2014 Log in normally. Connection auto-detected. \u25c8';
    document.body.appendChild(b);
    document.body.style.paddingTop = '32px';
})();
"#;

const CHROME_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

pub enum AppMessage {
    LoginSuccess { org_id: String, plan: String, data: UsageData, auth: api::AuthKind },
    UsageUpdate(UsageData),
    Error(String),
    TrayClicked { x: i32, y: i32 },
}

fn main() {
    panic_hook::install();
    let config = Arc::new(Mutex::new(AppConfig::load()));
    let (tx, rx) = mpsc::channel::<AppMessage>();

    let locale = config.lock().unwrap().effective_locale();
    let strings = Arc::new(i18n::Strings::load(&locale));

    // The WebView2 runtime is needed for the login window and dashboard (the
    // tray gauge itself works without it). Warn once if it is missing.
    if !win_util::webview2_installed() {
        win_util::prompt_webview2_install(strings.get("webview2_missing"));
    }
    notify::init();

    // Resolve saved auth off the main thread so the tray appears instantly, even
    // on a slow connection. Falls through to the login window if nothing resumes.
    let tx_boot = tx.clone();
    std::thread::spawn(move || {
        if !try_resume_session(&tx_boot) {
            open_login_webview(tx_boot);
        }
    });

    let tray = tray::create_tray(config.clone(), tx.clone(), &strings);
    let mut current_plan = String::from("Pro");
    let mut last_data: Option<UsageData> = None;
    let popup = popup::create_popup();
    let mut notified_threshold = false;

    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let _timer = SetTimer(None, 1, 500, None);
        let mut msg = MSG::default();

        while GetMessageW(&mut msg, None, 0, 0).into() {
            while let Ok(app_msg) = rx.try_recv() {
                handle_app_message(
                    app_msg, &tray, &config, &strings, &tx, &popup,
                    &mut current_plan, &mut last_data, &mut notified_threshold,
                );
            }

            // Process popup settings
            let popup_msgs: Vec<_> = popup.as_ref()
                .map(|p| p.rx.try_iter().collect()).unwrap_or_default();
            for pmsg in popup_msgs {
                handle_popup_message(pmsg, &config, &popup, &tx);
            }

            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn handle_app_message(
    msg: AppMessage,
    tray: &tray_icon::TrayIcon,
    config: &Arc<Mutex<AppConfig>>,
    strings: &Arc<i18n::Strings>,
    tx: &mpsc::Sender<AppMessage>,
    popup: &Option<popup::Popup>,
    current_plan: &mut String,
    last_data: &mut Option<UsageData>,
    notified_threshold: &mut bool,
) {
    match msg {
        AppMessage::LoginSuccess { org_id, plan, data, auth } => {
            *current_plan = plan;
            // Claude Code auth carries no org_id and needs no session file.
            if !org_id.is_empty() {
                let _ = AppConfig::save_session(&org_id);
            }
            tray::update_tray(tray, &data, current_plan, strings);
            *last_data = Some(data);
            *notified_threshold = false;
            // Supersede any previous poll loop so re-login doesn't double-poll.
            let generation = POLL_GENERATION.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
            let tx_poll = tx.clone();
            let cfg = config.clone();
            std::thread::spawn(move || { poll_loop(tx_poll, org_id, cfg, generation, auth); });
        }
        AppMessage::UsageUpdate(data) => {
            tray::update_tray(tray, &data, current_plan, strings);
            check_threshold(tray, &data, config, current_plan, notified_threshold, strings);
            if let Some(ref p) = popup {
                popup::push_data(p, &data, current_plan);
            }
            *last_data = Some(data);
        }
        AppMessage::Error(e) => {
            // Truncate on a char boundary — byte slicing can split a multibyte
            // sequence (localized OS / server errors) and panic.
            let short: String = e.chars().take(50).collect();
            let _ = tray.set_tooltip(Some(&format!("Claude Tank\nError: {}", short)));
        }
        AppMessage::TrayClicked { x, y } => {
            if let Some(ref p) = popup {
                popup::toggle_popup(p, x, y);
                if let Some(ref data) = last_data {
                    popup::push_data(p, data, current_plan);
                }
            }
        }
    }
}

fn check_threshold(
    tray: &tray_icon::TrayIcon,
    data: &UsageData,
    config: &Arc<Mutex<AppConfig>>,
    plan: &str,
    notified: &mut bool,
    strings: &i18n::Strings,
) {
    let cfg = config.lock().unwrap();
    let r5 = 100.0 - data.five_hour;
    let r7 = 100.0 - data.seven_day;
    let t5 = cfg.threshold_5h as f64;
    let t7 = cfg.threshold_7d as f64;
    drop(cfg);

    let left = strings.get("tray_left");
    let alert_5h = t5 > 0.0 && r5 <= t5;
    let alert_7d = t7 > 0.0 && r7 <= t7;
    if !*notified && (alert_5h || alert_7d) {
        let title = format!("Claude Tank \u{2014} {}", plan);
        let mut lines: Vec<String> = Vec::new();
        if alert_5h { lines.push(format!("5h: {:.0}% {}", r5, left)); }
        if alert_7d { lines.push(format!("7d: {:.0}% {}", r7, left)); }
        let body = lines.join("\n");
        // Tooltip persists on hover; the balloon grabs attention once per crossing.
        let _ = tray.set_tooltip(Some(&format!("{}\n{}", title, body)));
        notify::show(&title, &body);
        *notified = true;
    }
    if !alert_5h && !alert_7d {
        *notified = false;
    }
}

fn handle_popup_message(
    pmsg: popup::PopupMessage,
    config: &Arc<Mutex<AppConfig>>,
    popup: &Option<popup::Popup>,
    tx: &mpsc::Sender<AppMessage>,
) {
    use windows::Win32::UI::WindowsAndMessaging::*;
    match pmsg {
        popup::PopupMessage::Setting { key, value } => {
            if let Ok(mut c) = config.lock() {
                match key.as_str() {
                    "poll_interval_sec" => c.poll_interval_sec = value.parse().unwrap_or(180),
                    "threshold_5h" => c.threshold_5h = value.parse().unwrap_or(20),
                    "threshold_7d" => c.threshold_7d = value.parse().unwrap_or(20),
                    "auto_start" => {
                        let enabled = value == "true";
                        c.auto_start = enabled;
                        let _ = AppConfig::set_auto_start(enabled);
                    }
                    _ => {}
                }
                let _ = c.save();
            }
        }
        popup::PopupMessage::Relogin => {
            if let Some(ref p) = popup {
                unsafe { let _ = ShowWindow(p.hwnd, SW_HIDE); }
            }
            let tx_login = tx.clone();
            std::thread::spawn(move || { open_login_webview(tx_login); });
        }
        popup::PopupMessage::Clear => {
            let dir = config::app_dir();
            let _ = std::fs::remove_file(dir.join("credentials.enc"));
            let _ = std::fs::remove_file(dir.join("credentials.json"));
            let _ = std::fs::remove_file(dir.join("session.json"));
            std::process::exit(0);
        }
    }
}

/// Resume without a login window, preferring Claude Code's OAuth token and
/// falling back to a saved claude.ai session cookie. Returns false if neither
/// works, so the caller opens the WebView2 login.
fn try_resume_session(tx: &mpsc::Sender<AppMessage>) -> bool {
    // 1. Claude Code OAuth — works if the user already runs Claude Code.
    if let Some(creds) = cc::load() {
        let client = api::ApiClient::claude_code();
        match client.get_usage("") {
            Ok(data) => {
                let plan = api::plan_from_creds(&creds);
                #[cfg(debug_assertions)]
                eprintln!("Resumed via Claude Code. Plan={} 5h={:.0}% 7d={:.0}%",
                    plan, data.five_hour, data.seven_day);
                let _ = tx.send(AppMessage::LoginSuccess {
                    org_id: String::new(), plan, data, auth: api::AuthKind::ClaudeCode,
                });
                return true;
            }
            #[cfg(debug_assertions)]
            Err(_e) => eprintln!("Claude Code auth failed: {}. Trying saved session...", _e),
            #[cfg(not(debug_assertions))]
            Err(_) => {}
        }
    }
    // 2. Saved claude.ai session cookie from a previous WebView2 login.
    if let (Some(org_id), Some((sk, extras))) =
        (AppConfig::load_session(), AppConfig::load_credentials())
    {
        let client = api::ApiClient::session(sk, extras);
        if let Ok(data) = client.get_usage(&org_id) {
            let plan = client.detect_plan().unwrap_or_else(|_| "Pro".into());
            let _ = tx.send(AppMessage::LoginSuccess {
                org_id, plan, data, auth: api::AuthKind::Session,
            });
            return true;
        }
    }
    false
}

/// Polling loop. Rebuilds the client for its auth source, then re-reads the
/// interval every second so a runtime interval change (or a superseding login
/// that bumps `POLL_GENERATION`) both take effect promptly.
fn poll_loop(
    tx: mpsc::Sender<AppMessage>,
    org_id: String,
    config: Arc<Mutex<AppConfig>>,
    generation: usize,
    auth: api::AuthKind,
) {
    let client = match auth {
        api::AuthKind::ClaudeCode => api::ApiClient::claude_code(),
        api::AuthKind::Session => match AppConfig::load_credentials() {
            Some((sk, extras)) => api::ApiClient::session(sk, extras),
            None => return,
        },
    };
    loop {
        let mut elapsed = 0u32;
        loop {
            if POLL_GENERATION.load(Ordering::SeqCst) != generation { return; }
            let interval = config.lock().map(|c| c.poll_interval_sec).unwrap_or(180);
            if elapsed >= interval { break; }
            std::thread::sleep(Duration::from_secs(1));
            elapsed += 1;
        }
        match client.get_usage(&org_id) {
            Ok(data) => { let _ = tx.send(AppMessage::UsageUpdate(data)); }
            Err(e) => { let _ = tx.send(AppMessage::Error(e)); }
        }
    }
}

// ──────────────── Login WebView ────────────────

thread_local! {
    static LOGIN_WV: RefCell<Option<wry::WebView>> = const { RefCell::new(None) };
    static LOGIN_TX: RefCell<Option<mpsc::Sender<AppMessage>>> = const { RefCell::new(None) };
}

fn open_login_webview(tx: mpsc::Sender<AppMessage>) {
    use wry::{WebViewBuilder, Rect, dpi::{LogicalPosition, LogicalSize}};
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::core::*;

    unsafe {
        let instance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(login_wnd_proc),
            hInstance: instance,
            lpszClassName: w!("ClaudeTankLogin"),
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("ClaudeTankLogin"),
            w!("Claude Tank \u{2014} Log in to claude.ai"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            100, 100, 1024, 720,
            None, None, Some(instance), None,
        ).unwrap();

        let webview = WebViewBuilder::new()
            .with_bounds(Rect {
                position: LogicalPosition::new(0.0, 0.0).into(),
                size: LogicalSize::new(1024.0, 720.0).into(),
            })
            .with_url("https://claude.ai/login")
            .with_user_agent(CHROME_USER_AGENT)
            .build_as_child(&win_util::WinHandle(hwnd.0 as isize))
            .expect("Failed to create WebView2");

        LOGIN_WV.with(|cell| *cell.borrow_mut() = Some(webview));
        LOGIN_TX.with(|cell| *cell.borrow_mut() = Some(tx));
        let _ = SetTimer(Some(hwnd), TIMER_ID_COOKIE_POLL, COOKIE_POLL_MS, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn try_detect_login(hwnd: windows::Win32::Foundation::HWND) {
    LOGIN_WV.with(|cell| {
        let borrow = cell.borrow();
        let Some(wv) = borrow.as_ref() else { return };

        let _ = wv.evaluate_script(BANNER_JS);

        let Ok(cookies) = wv.cookies_for_url("https://claude.ai") else { return };
        let Some(session_cookie) = cookies.iter().find(|c| c.name() == "sessionKey") else { return };
        let session_key = session_cookie.value().to_string();

        #[cfg(debug_assertions)]
        eprintln!("sessionKey detected! Validating...");

        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(Some(hwnd), TIMER_ID_COOKIE_POLL);
        }

        let extras = std::collections::HashMap::new();
        let client = api::ApiClient::session(session_key.clone(), extras.clone());

        match client.get_org_id() {
            Ok(org_id) => match client.get_usage(&org_id) {
                Ok(data) => {
                    let plan = client.detect_plan().unwrap_or_else(|_| "Pro".into());
                    #[cfg(debug_assertions)]
                    eprintln!("Login success! Plan={} 5h={:.0}% 7d={:.0}%", plan, data.five_hour, data.seven_day);
                    let _ = AppConfig::save_credentials(&session_key, &extras);
                    LOGIN_TX.with(|cell| {
                        if let Some(tx) = cell.borrow().as_ref() {
                            let _ = tx.send(AppMessage::LoginSuccess {
                                org_id, plan, data, auth: api::AuthKind::Session,
                            });
                        }
                    });
                    unsafe { let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd); }
                }
                Err(_e) => {
                    #[cfg(debug_assertions)]
                    eprintln!("Usage fetch failed: {}. Retrying...", _e);
                    restart_cookie_timer(hwnd);
                }
            },
            Err(_e) => {
                #[cfg(debug_assertions)]
                eprintln!("Org fetch failed: {}. Retrying...", _e);
                restart_cookie_timer(hwnd);
            }
        }
    });
}

fn restart_cookie_timer(hwnd: windows::Win32::Foundation::HWND) {
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::SetTimer(
            Some(hwnd), TIMER_ID_COOKIE_POLL, COOKIE_POLL_MS, None,
        );
    }
}

unsafe extern "system" fn login_wnd_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::Foundation::LRESULT;
    match msg {
        WM_TIMER if wparam.0 == TIMER_ID_COOKIE_POLL => {
            try_detect_login(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            LOGIN_WV.with(|cell| *cell.borrow_mut() = None);
            LOGIN_TX.with(|cell| *cell.borrow_mut() = None);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
