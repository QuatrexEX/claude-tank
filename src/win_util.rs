//! Shared Win32 utilities.

use raw_window_handle::{HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle};
use std::num::NonZeroIsize;

/// Wrapper to implement HasWindowHandle for a raw HWND isize value.
pub struct WinHandle(pub isize);

impl HasWindowHandle for WinHandle {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, raw_window_handle::HandleError> {
        let h = Win32WindowHandle::new(NonZeroIsize::new(self.0).unwrap());
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(h)) })
    }
}
