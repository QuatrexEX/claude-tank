use wry::{WebViewBuilder, Rect, dpi::{LogicalPosition, LogicalSize}};
use raw_window_handle::{HasWindowHandle, RawWindowHandle, WindowHandle, Win32WindowHandle};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::core::*;
use std::num::NonZeroIsize;

const POPUP_W: u32 = 380;
const POPUP_H: u32 = 460;
const CLASS_NAME: PCWSTR = w!("ClaudeTankPopup");

/// Wrapper to implement HasWindowHandle for a raw HWND
struct WinHandle(isize);

impl HasWindowHandle for WinHandle {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, raw_window_handle::HandleError> {
        let h = Win32WindowHandle::new(NonZeroIsize::new(self.0).unwrap());
        let raw = RawWindowHandle::Win32(h);
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

/// Create the popup window with WebView2.
pub fn create_popup() -> HWND {
    unsafe {
        let instance: HINSTANCE = GetModuleHandleW(None).unwrap().into();

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(popup_wnd_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let x = screen_w - POPUP_W as i32 - 16;
        let y = screen_h - POPUP_H as i32 - 60;

        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            CLASS_NAME,
            w!("Claude Tank"),
            WS_POPUP,
            x, y,
            POPUP_W as i32, POPUP_H as i32,
            None, None, Some(instance), None,
        ).unwrap();

        // Wrap HWND for wry
        let handle = WinHandle(hwnd.0 as isize);
        let html = include_str!("../src/index.html");

        let _webview = WebViewBuilder::new()
            .with_bounds(Rect {
                position: LogicalPosition::new(0.0, 0.0).into(),
                size: LogicalSize::new(POPUP_W as f64, POPUP_H as f64).into(),
            })
            .with_html(html)
            .build_as_child(&handle)
            .expect("Failed to create WebView2");

        // Keep webview alive by leaking it (it lives for the app lifetime)
        std::mem::forget(_webview);

        hwnd
    }
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
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
