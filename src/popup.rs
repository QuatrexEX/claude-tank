//! Dashboard popup — created once at startup, shown/hidden on tray click.

use raw_window_handle::{HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle};
use std::num::NonZeroIsize;
use std::sync::mpsc;
use wry::{WebViewBuilder, Rect, dpi::{LogicalPosition, LogicalSize}};
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::core::*;

use crate::config::AppConfig;

const POPUP_W: i32 = 340;
const POPUP_H: i32 = 400;
const CORNER_RADIUS: i32 = 12;

struct WinHandle(isize);
impl HasWindowHandle for WinHandle {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, raw_window_handle::HandleError> {
        let h = Win32WindowHandle::new(NonZeroIsize::new(self.0).unwrap());
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(h)) })
    }
}

pub enum PopupMessage {
    Setting { key: String, value: String },
    Relogin,
    Clear,
}

pub struct Popup {
    pub webview: wry::WebView,
    pub hwnd: HWND,
    pub rx: mpsc::Receiver<PopupMessage>,
}

/// Create the popup at startup (hidden). Call once.
pub fn create_popup() -> Option<Popup> {
    let (tx, rx) = mpsc::channel::<PopupMessage>();

    unsafe {
        let instance: HINSTANCE = GetModuleHandleW(None).ok()?.into();

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(popup_wnd_proc),
            hInstance: instance,
            lpszClassName: w!("ClaudeTankPopup"),
            ..Default::default()
        };
        RegisterClassExW(&wc);

        // Start off-screen, hidden
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            w!("ClaudeTankPopup"),
            w!("Claude Tank"),
            WS_POPUP, // No WS_VISIBLE — hidden
            -1000, -1000, POPUP_W, POPUP_H,
            None, None, Some(instance), None,
        ).ok()?;

        // Rounded corners
        let rgn = windows::Win32::Graphics::Gdi::CreateRoundRectRgn(
            0, 0, POPUP_W + 1, POPUP_H + 1, CORNER_RADIUS, CORNER_RADIUS,
        );
        if !rgn.is_invalid() {
            windows::Win32::Graphics::Gdi::SetWindowRgn(hwnd, Some(rgn), true);
        }
        {
            use windows::Win32::Graphics::Dwm::*;
            let pref = DWM_WINDOW_CORNER_PREFERENCE(2);
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE,
                &pref as *const _ as _, std::mem::size_of_val(&pref) as u32);
        }

        let handle = WinHandle(hwnd.0 as isize);

        // Build HTML with locale and config embedded
        let config = AppConfig::load();
        let locale = if config.locale.is_empty() || config.locale == "auto" {
            AppConfig::detect_locale()
        } else {
            config.locale.clone()
        };
        let locale_json = load_locale_json(&locale);
        let config_json = serde_json::to_string(&config).unwrap_or_default();

        let base_html = include_str!("dashboard.html");
        // Inject locale + config before closing </body>
        let init_script = format!(
            "<script>document.addEventListener('DOMContentLoaded',function(){{applyLocale({});loadConfig({})}});</script>",
            if locale_json.is_empty() { "null".to_string() } else { locale_json },
            config_json,
        );
        let html = base_html.replace("</body>", &format!("{}</body>", init_script));

        let webview = WebViewBuilder::new()
            .with_bounds(Rect {
                position: LogicalPosition::new(0.0, 0.0).into(),
                size: LogicalSize::new(POPUP_W as f64, POPUP_H as f64).into(),
            })
            .with_background_color((10, 10, 15, 255))
            .with_html(&html)
            .with_ipc_handler(move |msg| {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(msg.body()) {
                    match val.get("type").and_then(|v| v.as_str()).unwrap_or("") {
                        "setting" => {
                            let key = val["key"].as_str().unwrap_or("").to_string();
                            let value = if let Some(s) = val["value"].as_str() { s.to_string() }
                                else if let Some(b) = val["value"].as_bool() { b.to_string() }
                                else { val["value"].to_string() };
                            let _ = tx.send(PopupMessage::Setting { key, value });
                        }
                        "relogin" => { let _ = tx.send(PopupMessage::Relogin); }
                        "clear" => { let _ = tx.send(PopupMessage::Clear); }
                        _ => {}
                    }
                }
            })
            .build_as_child(&handle)
            .ok()?;

        // Config + locale are already embedded in the HTML
        Some(Popup { webview, hwnd, rx })
    }
}

/// Show the popup near (tray_x, tray_y), or hide if already visible.
pub fn toggle_popup(popup: &Popup, tray_x: i32, tray_y: i32) {
    unsafe {
        let visible = IsWindowVisible(popup.hwnd).as_bool();
        if visible {
            let _ = ShowWindow(popup.hwnd, SW_HIDE);
        } else {
            let x = (tray_x - POPUP_W / 2).max(0);
            let y = (tray_y - POPUP_H - 8).max(0);
            let _ = SetWindowPos(popup.hwnd, Some(HWND_TOPMOST),
                x, y, POPUP_W, POPUP_H, SWP_SHOWWINDOW);
            let _ = ShowWindow(popup.hwnd, SW_SHOW);
            let _ = SetForegroundWindow(popup.hwnd);
        }
    }
}

/// Push usage data to the dashboard JS
pub fn push_data(popup: &Popup, data: &crate::api::UsageData, plan: &str) {
    let js = format!(
        "document.getElementById('plan').textContent='{}';updateDashboard({})",
        plan,
        serde_json::json!({
            "five_hour": data.five_hour,
            "five_hour_reset": data.five_hour_reset,
            "seven_day": data.seven_day,
            "seven_day_reset": data.seven_day_reset,
            "opus": data.opus,
            "sonnet": data.sonnet,
        })
    );
    let _ = popup.webview.evaluate_script(&js);
}

fn load_locale_json(locale: &str) -> String {
    let json = match locale {
        "ja" => include_str!("locales/ja.json"),
        "de" => include_str!("locales/de.json"),
        "ko" => include_str!("locales/ko.json"),
        "fr" => include_str!("locales/fr.json"),
        _ => return String::new(), // English is the default in HTML
    };
    json.to_string()
}

unsafe extern "system" fn popup_wnd_proc(
    hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ACTIVATE => {
            if wparam.0 & 0xFFFF == 0 {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
