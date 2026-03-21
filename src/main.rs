//! Claude Tank — System tray Claude usage monitor
//!
//! Login: wry WebView2 opens claude.ai/login. User logs in normally.
//!        Cookie is auto-detected via polling (same as Usage4Claude on macOS).
//! Polling: ureq HTTP client with sessionKey cookie.
//! Tray: dual-gauge cyber icon (5h/7d remaining).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod config;
mod tray;

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

enum AppMessage {
    LoginSuccess { org_id: String, five_hour: f64, seven_day: f64 },
    UsageUpdate { five_hour: f64, seven_day: f64 },
    Error(String),
}

fn main() {
    let config = Arc::new(Mutex::new(AppConfig::load()));
    let (tx, rx) = mpsc::channel::<AppMessage>();

    let has_session = try_resume_session(&tx);

    if !has_session {
        let tx_login = tx.clone();
        std::thread::spawn(move || {
            open_login_webview(tx_login);
        });
    }

    let tray = tray::create_tray(config.clone());

    // Main message loop
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let _timer = SetTimer(None, 1, 1000, None);
        let mut msg = MSG::default();

        while GetMessageW(&mut msg, None, 0, 0).into() {
            while let Ok(app_msg) = rx.try_recv() {
                match app_msg {
                    AppMessage::LoginSuccess { org_id, five_hour, seven_day } => {
                        let _ = AppConfig::save_session(&org_id);
                        tray::update_tray(&tray, five_hour, seven_day);
                        let tx_poll = tx.clone();
                        let interval = config.lock().unwrap().poll_interval_sec;
                        std::thread::spawn(move || {
                            poll_loop(tx_poll, org_id, interval);
                        });
                    }
                    AppMessage::UsageUpdate { five_hour, seven_day } => {
                        tray::update_tray(&tray, five_hour, seven_day);
                    }
                    AppMessage::Error(e) => {
                        let _ = tray.set_tooltip(Some(&format!(
                            "Claude Tank\nError: {}", &e[..e.len().min(50)]
                        )));
                    }
                }
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn try_resume_session(tx: &mpsc::Sender<AppMessage>) -> bool {
    let org_id = match AppConfig::load_session() { Some(id) => id, None => return false };
    let (sk, extras) = match AppConfig::load_credentials() { Some(c) => c, None => return false };
    let client = api::ApiClient::new(sk, extras);
    match client.get_usage(&org_id) {
        Ok(data) => {
            let _ = tx.send(AppMessage::LoginSuccess {
                org_id, five_hour: data.five_hour, seven_day: data.seven_day,
            });
            true
        }
        Err(_) => false,
    }
}

fn poll_loop(tx: mpsc::Sender<AppMessage>, org_id: String, interval_sec: u32) {
    let (sk, extras) = match AppConfig::load_credentials() { Some(c) => c, None => return };
    let client = api::ApiClient::new(sk, extras);
    loop {
        std::thread::sleep(Duration::from_secs(interval_sec as u64));
        match client.get_usage(&org_id) {
            Ok(data) => {
                let _ = tx.send(AppMessage::UsageUpdate {
                    five_hour: data.five_hour, seven_day: data.seven_day,
                });
            }
            Err(e) => { let _ = tx.send(AppMessage::Error(e)); }
        }
    }
}

// ──────────────── Login WebView ────────────────

// Thread-local storage for login window state (replaces unsafe static mut)
thread_local! {
    static LOGIN_WV: RefCell<Option<wry::WebView>> = const { RefCell::new(None) };
    static LOGIN_TX: RefCell<Option<mpsc::Sender<AppMessage>>> = const { RefCell::new(None) };
}

fn open_login_webview(tx: mpsc::Sender<AppMessage>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle};
    use std::num::NonZeroIsize;
    use wry::{WebViewBuilder, Rect, dpi::{LogicalPosition, LogicalSize}};
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::core::*;

    struct WinHandle(isize);
    impl HasWindowHandle for WinHandle {
        fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, raw_window_handle::HandleError> {
            let h = Win32WindowHandle::new(NonZeroIsize::new(self.0).unwrap());
            Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(h)) })
        }
    }

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
            .build_as_child(&WinHandle(hwnd.0 as isize))
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

/// Check WebView2 cookies for sessionKey, validate via API
fn try_detect_login(hwnd: windows::Win32::Foundation::HWND) {
    LOGIN_WV.with(|cell| {
        let borrow = cell.borrow();
        let Some(wv) = borrow.as_ref() else { return };

        // Inject banner
        let _ = wv.evaluate_script(BANNER_JS);

        // Check cookies
        let Ok(cookies) = wv.cookies_for_url("https://claude.ai") else { return };
        let Some(session_cookie) = cookies.iter().find(|c| c.name() == "sessionKey") else { return };
        let session_key = session_cookie.value().to_string();

        println!("sessionKey detected! Validating...");

        // Kill timer while validating
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(Some(hwnd), TIMER_ID_COOKIE_POLL);
        }

        let extras = std::collections::HashMap::new();
        let client = api::ApiClient::new(session_key.clone(), extras.clone());

        match client.get_org_id() {
            Ok(org_id) => match client.get_usage(&org_id) {
                Ok(data) => {
                    println!("Login success! 5h={:.0}% 7d={:.0}%", data.five_hour, data.seven_day);
                    let _ = AppConfig::save_credentials(&session_key, &extras);

                    LOGIN_TX.with(|cell| {
                        if let Some(tx) = cell.borrow().as_ref() {
                            let _ = tx.send(AppMessage::LoginSuccess {
                                org_id,
                                five_hour: data.five_hour,
                                seven_day: data.seven_day,
                            });
                        }
                    });

                    unsafe { let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd); }
                }
                Err(e) => {
                    println!("Usage fetch failed: {}. Retrying...", e);
                    restart_cookie_timer(hwnd);
                }
            },
            Err(e) => {
                println!("Org fetch failed: {}. Retrying...", e);
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
