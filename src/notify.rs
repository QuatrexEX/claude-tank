//! Lightweight balloon/toast notifications via Shell_NotifyIcon.
//!
//! `tray-icon` 0.19 exposes no notification API and keeps the (hwnd, uID) of its
//! shell icon private, so we register our own hidden notify icon purely to fire
//! NIF_INFO balloons. Everything runs on the main thread (the same thread that
//! pumps the message loop), so no extra synchronization is needed and the hidden
//! window's messages are dispatched by the main loop to DefWindowProcW.
//!
//! The icon is added with NIS_HIDDEN so no second static icon appears in the
//! tray; on Windows 10/11 the NIF_INFO balloon still surfaces as a toast.

use std::cell::RefCell;
use windows::core::w;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_STATE, NIIF_WARNING, NIM_ADD, NIM_MODIFY,
    NIS_HIDDEN, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

const NOTIFY_UID: u32 = 0x0C7A;

thread_local! {
    static NOTIFIER: RefCell<Option<HWND>> = const { RefCell::new(None) };
}

/// The notifier window has no logic of its own; defer everything to the OS.
unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Register the hidden notify icon. Call once, on the main thread, before the
/// message loop. A failure just disables toasts (tooltips still work).
pub fn init() {
    unsafe {
        let instance: HINSTANCE = match GetModuleHandleW(None) {
            Ok(h) => h.into(),
            Err(_) => return,
        };
        let cls = w!("ClaudeTankNotify");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            lpszClassName: cls,
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE::default(), cls, w!("ClaudeTankNotify"),
            WS_OVERLAPPED, 0, 0, 0, 0, None, None, Some(instance), None,
        ) {
            Ok(h) => h,
            Err(_) => return,
        };

        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: NOTIFY_UID,
            uFlags: NIF_ICON | NIF_STATE,
            dwState: NIS_HIDDEN,
            dwStateMask: NIS_HIDDEN,
            hIcon: LoadIconW(None, IDI_INFORMATION).unwrap_or_default(),
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        NOTIFIER.with(|c| *c.borrow_mut() = Some(hwnd));
    }
}

/// Show a warning balloon. No-op if [`init`] was not called or failed.
pub fn show(title: &str, body: &str) {
    NOTIFIER.with(|c| {
        let Some(hwnd) = *c.borrow() else { return };
        unsafe {
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: NOTIFY_UID,
                uFlags: NIF_INFO,
                dwInfoFlags: NIIF_WARNING,
                ..Default::default()
            };
            fill_wide(&mut nid.szInfoTitle, title);
            fill_wide(&mut nid.szInfo, body);
            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    });
}

/// Copy `s` as UTF-16 into `dst`, always leaving a trailing NUL.
fn fill_wide(dst: &mut [u16], s: &str) {
    let mut i = 0;
    for u in s.encode_utf16() {
        if i + 1 >= dst.len() {
            break;
        }
        dst[i] = u;
        i += 1;
    }
    dst[i] = 0;
}
