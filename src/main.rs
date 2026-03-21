//! Claude Tank — System tray Claude usage monitor
//!
//! Login: wry WebView2 opens claude.ai/login. User logs in normally
//!        (Google, email, etc.). Cookie is auto-detected via polling
//!        (same approach as Usage4Claude on macOS).
//! No buttons to click. Just log in and it works.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod config;
mod popup;
mod tray;

use config::AppConfig;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

enum AppMessage {
    LoginSuccess { org_id: String, five_hour: f64, seven_day: f64 },
    UsageUpdate { five_hour: f64, seven_day: f64 },
    Error(String),
}

fn main() {
    let config = Arc::new(Mutex::new(AppConfig::load()));
    let (tx, rx) = mpsc::channel::<AppMessage>();

    // Try to resume saved session
    let has_session = try_resume_session(&tx);

    if !has_session {
        // Open login window (blocking on this thread until login completes)
        let tx_login = tx.clone();
        std::thread::spawn(move || {
            open_login_webview(tx_login);
        });
    }

    // Create tray icon
    let tray = tray::create_tray(config.clone());

    // Main loop
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
                        // Start poller
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
            Ok(data) => { let _ = tx.send(AppMessage::UsageUpdate { five_hour: data.five_hour, seven_day: data.seven_day }); }
            Err(e) => { let _ = tx.send(AppMessage::Error(e)); }
        }
    }
}

// ──────────────── Login WebView ────────────────

/// Open WebView2 with claude.ai/login.
/// Poll cookies every second. When sessionKey appears, validate and send LoginSuccess.
/// User just logs in normally — no extra buttons needed.
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
            w!("Claude Tank — Log in to claude.ai"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            100, 100, 1024, 720,
            None, None, Some(instance), None,
        ).unwrap();

        let handle = WinHandle(hwnd.0 as isize);

        let webview = WebViewBuilder::new()
            .with_bounds(Rect {
                position: LogicalPosition::new(0.0, 0.0).into(),
                size: LogicalSize::new(1024.0, 720.0).into(),
            })
            .with_url("https://claude.ai/login")
            // Pretend to be regular Chrome — Google blocks embedded WebViews
            .with_user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            // Let WebView2 handle all navigation and popups naturally
            // (no intervention — Google OAuth needs native popup behavior)
            .build_as_child(&handle)
            .expect("Failed to create WebView2");

        // Inject banner + override window.open to force same-window navigation
        let banner_js = r#"
        (function() {
            if (document.getElementById('ct-banner')) return;
            if (!document.body) return;

            // Banner
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

            // No window.open override — let OAuth popups work naturally
        })();
        "#;

        // Store webview and tx for timer access
        // Use module-level statics (accessed by login_wnd_proc timer)
        LOGIN_WV = Some(webview);
        LOGIN_TX = Some(tx);
        LOGIN_HWND = Some(hwnd);
        BANNER_JS = Some(banner_js.to_string());

        // Start cookie polling timer (every 2 seconds)
        let _ = SetTimer(Some(hwnd), 42, 2000, None);

        // Message loop for login window
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

static mut LOGIN_WV: Option<wry::WebView> = None;
static mut LOGIN_TX: Option<mpsc::Sender<AppMessage>> = None;
static mut LOGIN_HWND: Option<windows::Win32::Foundation::HWND> = None;
static mut BANNER_JS: Option<String> = None;

unsafe extern "system" fn login_wnd_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::Foundation::LRESULT;

    match msg {
        WM_TIMER if wparam.0 == 42 => {
            // Poll cookies from WebView2
            if let Some(wv) = LOGIN_WV.as_ref() {
                // Inject banner
                if let Some(js) = BANNER_JS.as_ref() {
                    let _ = wv.evaluate_script(js);
                }

                // Check cookies for sessionKey
                if let Ok(cookies) = wv.cookies_for_url("https://claude.ai") {
                    for cookie in &cookies {
                        if cookie.name() == "sessionKey" {
                            let session_key = cookie.value().to_string();
                            println!("sessionKey detected! Validating...");

                            // Kill the timer
                            let _ = KillTimer(Some(hwnd), 42);

                            // Validate via API
                            let extras = std::collections::HashMap::new();
                            let client = api::ApiClient::new(session_key.clone(), extras.clone());

                            match client.get_org_id() {
                                Ok(org_id) => {
                                    match client.get_usage(&org_id) {
                                        Ok(data) => {
                                            println!("Login success! 5h={:.0}% 7d={:.0}%", data.five_hour, data.seven_day);
                                            let _ = AppConfig::save_credentials(&session_key, &extras);
                                            if let Some(tx) = LOGIN_TX.as_ref() {
                                                let _ = tx.send(AppMessage::LoginSuccess {
                                                    org_id,
                                                    five_hour: data.five_hour,
                                                    seven_day: data.seven_day,
                                                });
                                            }
                                            // Close login window
                                            let _ = DestroyWindow(hwnd);
                                        }
                                        Err(e) => {
                                            println!("Usage fetch failed: {}. Retrying...", e);
                                            let _ = SetTimer(Some(hwnd), 42, 2000, None);
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("Org fetch failed: {}. Retrying...", e);
                                    let _ = SetTimer(Some(hwnd), 42, 2000, None);
                                }
                            }
                            return LRESULT(0);
                        }
                    }
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            LOGIN_WV = None;
            LOGIN_TX = None;
            LOGIN_HWND = None;
            BANNER_JS = None;
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
