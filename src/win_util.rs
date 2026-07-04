//! Shared Win32 utilities.

use raw_window_handle::{HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle};
use std::num::NonZeroIsize;
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
    KEY_READ,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_ICONINFORMATION, MB_YESNO, SW_SHOWNORMAL,
};

/// Wrapper to implement HasWindowHandle for a raw HWND isize value.
pub struct WinHandle(pub isize);

impl HasWindowHandle for WinHandle {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, raw_window_handle::HandleError> {
        let h = Win32WindowHandle::new(NonZeroIsize::new(self.0).unwrap());
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(h)) })
    }
}

// ── WebView2 runtime detection ──

/// Evergreen WebView2 Runtime client GUID (registered by the runtime installer).
const WEBVIEW2_CLIENT: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
/// Microsoft's official Evergreen bootstrapper (downloads the runtime installer).
const WEBVIEW2_DOWNLOAD: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

/// True if the Evergreen WebView2 runtime appears installed. Checks the per-user
/// and both per-machine (64-bit and native) registry locations.
pub fn webview2_installed() -> bool {
    let hklm_wow = format!("SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{WEBVIEW2_CLIENT}");
    let hklm = format!("SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{WEBVIEW2_CLIENT}");
    let hkcu = format!("Software\\Microsoft\\EdgeUpdate\\Clients\\{WEBVIEW2_CLIENT}");
    unsafe {
        key_has_version(HKEY_LOCAL_MACHINE, &hklm_wow)
            || key_has_version(HKEY_LOCAL_MACHINE, &hklm)
            || key_has_version(HKEY_CURRENT_USER, &hkcu)
    }
}

/// True if `subkey` holds a non-empty, non-zero "pv" (product version) value.
unsafe fn key_has_version(root: HKEY, subkey: &str) -> bool {
    let sk = HSTRING::from(subkey);
    let mut hkey = HKEY::default();
    if RegOpenKeyExW(root, PCWSTR(sk.as_ptr()), None, KEY_READ, &mut hkey).is_err() {
        return false;
    }
    let mut buf = [0u16; 64];
    let mut size = (buf.len() * 2) as u32; // bytes
    let r = RegQueryValueExW(
        hkey, w!("pv"), None, None,
        Some(buf.as_mut_ptr() as *mut u8), Some(&mut size),
    );
    let _ = RegCloseKey(hkey);
    if r.is_err() || size < 2 {
        return false;
    }
    let len = (size as usize / 2).saturating_sub(1); // drop trailing NUL
    let pv = String::from_utf16_lossy(&buf[..len]);
    !pv.is_empty() && pv != "0.0.0.0"
}

/// Inform the user WebView2 is missing and offer to open the download page.
pub fn prompt_webview2_install(message: &str) {
    let text = HSTRING::from(message);
    let result = unsafe {
        MessageBoxW(None, PCWSTR(text.as_ptr()), w!("Claude Tank"), MB_YESNO | MB_ICONINFORMATION)
    };
    if result == IDYES {
        open_url(WEBVIEW2_DOWNLOAD);
    }
}

/// Open a URL in the user's default browser.
pub fn open_url(url: &str) {
    let file = HSTRING::from(url);
    unsafe {
        let _ = ShellExecuteW(
            None, w!("open"), PCWSTR(file.as_ptr()),
            PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL,
        );
    }
}
