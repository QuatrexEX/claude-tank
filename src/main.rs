//! Claude Tank — System tray Claude usage monitor
//!
//! Login: wry WebView2 opens claude.ai/login. User logs in normally.
//!        Cookie is auto-detected via polling (same as Usage4Claude on macOS).
//! Polling: ureq HTTP client with sessionKey cookie.
//! Tray: dual-gauge cyber icon (5h/7d remaining).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod config;
mod crypto;
mod i18n;
mod popup;
mod tray;
mod win_util;

use api::UsageData;
use config::AppConfig;
use std::cell::RefCell;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TIMER_ID_COOKIE_POLL: usize = 42;
const COOKIE_POLL_MS: u32 = 2000;

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
    LoginSuccess { org_id: String, plan: String, five_hour: f64, seven_day: f64 },
    UsageUpdate(UsageData),
    Error(String),
    TrayClicked { x: i32, y: i32 },
}

fn main() {
    let config = Arc::new(Mutex::new(AppConfig::load()));
    let (tx, rx) = mpsc::channel::<AppMessage>();

    let locale = config.lock().unwrap().effective_locale();
    let strings = Arc::new(i18n::Strings::load(&locale));

    let has_session = try_resume_session(&tx);
    if !has_session {
        let tx_login = tx.clone();
        std::thread::spawn(move || { open_login_webview(tx_login); });
    }

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
        AppMessage::LoginSuccess { org_id, plan, five_hour, seven_day } => {
            *current_plan = plan;
            let data = UsageData { five_hour, seven_day, ..Default::default() };
            let _ = AppConfig::save_session(&org_id);
            tray::update_tray(tray, five_hour, seven_day, current_plan, strings);
            *last_data = Some(data);
            *notified_threshold = false;
            let tx_poll = tx.clone();
            let cfg = config.clone();
            std::thread::spawn(move || { poll_loop(tx_poll, org_id, cfg); });
        }
        AppMessage::UsageUpdate(data) => {
            tray::update_tray(tray, data.five_hour, data.seven_day, current_plan, strings);
            check_threshold(tray, &data, config, current_plan, notified_threshold);
            if let Some(ref p) = popup {
                popup::push_data(p, &data, current_plan);
            }
            *last_data = Some(data);
        }
        AppMessage::Error(e) => {
            let _ = tray.set_tooltip(Some(&format!(
                "Claude Tank\nError: {}", &e[..e.len().min(50)]
            )));
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
) {
    let cfg = config.lock().unwrap();
    let r5 = 100.0 - data.five_hour;
    let r7 = 100.0 - data.seven_day;
    let t5 = cfg.threshold_5h as f64;
    let t7 = cfg.threshold_7d as f64;
    drop(cfg);

    let alert_5h = t5 > 0.0 && r5 <= t5;
    let alert_7d = t7 > 0.0 && r7 <= t7;
    if !*notified && (alert_5h || alert_7d) {
        let mut warn = format!("Claude Tank \u{2014} {}", plan);
        if alert_5h { warn += &format!("\n\u{26A0} 5h: {:.0}% left!", r5); }
        if alert_7d { warn += &format!("\n\u{26A0} 7d: {:.0}% left!", r7); }
        let _ = tray.set_tooltip(Some(&warn));
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
            let dir = dirs::config_dir().unwrap().join("Quatrex").join("claude-tank");
            let _ = std::fs::remove_file(dir.join("credentials.enc"));
            let _ = std::fs::remove_file(dir.join("credentials.json"));
            let _ = std::fs::remove_file(dir.join("session.json"));
            std::process::exit(0);
        }
    }
}

fn try_resume_session(tx: &mpsc::Sender<AppMessage>) -> bool {
    let org_id = match AppConfig::load_session() { Some(id) => id, None => return false };
    let (sk, extras) = match AppConfig::load_credentials() { Some(c) => c, None => return false };
    let client = api::ApiClient::new(sk, extras);
    match client.get_usage(&org_id) {
        Ok(data) => {
            let plan = client.detect_plan().unwrap_or_else(|_| "Pro".into());
            let _ = tx.send(AppMessage::LoginSuccess {
                org_id, plan, five_hour: data.five_hour, seven_day: data.seven_day,
            });
            true
        }
        Err(_) => false,
    }
}

/// Polling loop — reads interval from config each cycle (supports runtime changes)
fn poll_loop(tx: mpsc::Sender<AppMessage>, org_id: String, config: Arc<Mutex<AppConfig>>) {
    let (sk, extras) = match AppConfig::load_credentials() { Some(c) => c, None => return };
    let client = api::ApiClient::new(sk, extras);
    loop {
        let interval = config.lock().map(|c| c.poll_interval_sec).unwrap_or(180);
        std::thread::sleep(Duration::from_secs(interval as u64));
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
        let client = api::ApiClient::new(session_key.clone(), extras.clone());

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
                                org_id, plan,
                                five_hour: data.five_hour, seven_day: data.seven_day,
                            });
                        }
                    });
                    unsafe { let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd); }
                }
                Err(e) => {
                    #[cfg(debug_assertions)]
                    eprintln!("Usage fetch failed: {}. Retrying...", e);
                    restart_cookie_timer(hwnd);
                }
            },
            Err(e) => {
                #[cfg(debug_assertions)]
                eprintln!("Org fetch failed: {}. Retrying...", e);
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
