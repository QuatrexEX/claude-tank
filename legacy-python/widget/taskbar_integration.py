"""Claude Tank — Windows taskbar integration.

Uses SHAppBarMessage to register as an Application Desktop Toolbar (AppBar).
The widget docks to the bottom of the screen, just above the taskbar.
Windows automatically adjusts the work area so other windows don't overlap.
"""

from __future__ import annotations

import ctypes
import ctypes.wintypes
import logging
from ctypes import Structure, POINTER, byref, sizeof

log = logging.getLogger("claude-tank")

user32 = ctypes.windll.user32
shell32 = ctypes.windll.shell32

# SHAppBarMessage constants
ABM_NEW = 0x00
ABM_REMOVE = 0x01
ABM_QUERYPOS = 0x02
ABM_SETPOS = 0x03
ABM_GETSTATE = 0x04
ABM_GETTASKBARPOS = 0x05
ABM_ACTIVATE = 0x06

ABE_BOTTOM = 3
ABE_TOP = 1
ABE_LEFT = 0
ABE_RIGHT = 2

# Window message for AppBar notifications
WM_APPBAR_CALLBACK = 0x0401  # WM_USER + 1


class APPBARDATA(Structure):
    _fields_ = [
        ("cbSize", ctypes.wintypes.DWORD),
        ("hWnd", ctypes.wintypes.HWND),
        ("uCallbackMessage", ctypes.wintypes.UINT),
        ("uEdge", ctypes.wintypes.UINT),
        ("rc", ctypes.wintypes.RECT),
        ("lParam", ctypes.wintypes.LPARAM),
    ]


def get_taskbar_rect() -> tuple[int, int, int, int]:
    """Get the taskbar's screen rectangle."""
    abd = APPBARDATA()
    abd.cbSize = sizeof(APPBARDATA)
    shell32.SHAppBarMessage(ABM_GETTASKBARPOS, byref(abd))
    return (abd.rc.left, abd.rc.top, abd.rc.right, abd.rc.bottom)


def get_taskbar_edge() -> int:
    """Get which edge the taskbar is on (ABE_BOTTOM, ABE_TOP, etc.)."""
    abd = APPBARDATA()
    abd.cbSize = sizeof(APPBARDATA)
    shell32.SHAppBarMessage(ABM_GETTASKBARPOS, byref(abd))
    return abd.uEdge


def get_screen_size() -> tuple[int, int]:
    """Get primary screen dimensions."""
    return (
        user32.GetSystemMetrics(0),  # SM_CXSCREEN
        user32.GetSystemMetrics(1),  # SM_CYSCREEN
    )


class AppBarWidget:
    """Docks a window as an AppBar above the taskbar.

    This reserves screen space — other windows won't overlap.
    Much cleaner than trying to embed inside the taskbar.
    """

    def __init__(self) -> None:
        self._hwnd: int = 0
        self._registered = False
        self._bar_height = 32

    def register(self, hwnd: int, height: int = 32) -> bool:
        """Register the window as an AppBar."""
        self._hwnd = hwnd
        self._bar_height = height

        abd = APPBARDATA()
        abd.cbSize = sizeof(APPBARDATA)
        abd.hWnd = hwnd
        abd.uCallbackMessage = WM_APPBAR_CALLBACK

        result = shell32.SHAppBarMessage(ABM_NEW, byref(abd))
        if result:
            self._registered = True
            log.info("AppBar registered (hwnd=%s)", hwnd)
            self._set_position()
            return True
        else:
            log.warning("AppBar registration failed")
            return False

    def _set_position(self) -> None:
        """Position the AppBar at the bottom, above the taskbar."""
        if not self._registered:
            return

        screen_w, screen_h = get_screen_size()
        tb_left, tb_top, tb_right, tb_bottom = get_taskbar_rect()
        edge = get_taskbar_edge()

        abd = APPBARDATA()
        abd.cbSize = sizeof(APPBARDATA)
        abd.hWnd = self._hwnd
        abd.uEdge = edge  # Same edge as taskbar

        if edge == ABE_BOTTOM:
            abd.rc.left = 0
            abd.rc.right = screen_w
            abd.rc.bottom = tb_top
            abd.rc.top = tb_top - self._bar_height
        elif edge == ABE_TOP:
            abd.rc.left = 0
            abd.rc.right = screen_w
            abd.rc.top = tb_bottom
            abd.rc.bottom = tb_bottom + self._bar_height
        else:
            # Left/right taskbar — dock at bottom
            abd.uEdge = ABE_BOTTOM
            abd.rc.left = 0
            abd.rc.right = screen_w
            abd.rc.bottom = screen_h
            abd.rc.top = screen_h - self._bar_height

        # Query position (Windows may adjust it)
        shell32.SHAppBarMessage(ABM_QUERYPOS, byref(abd))

        # Recalculate based on adjusted rect
        if abd.uEdge == ABE_BOTTOM:
            abd.rc.top = abd.rc.bottom - self._bar_height
        elif abd.uEdge == ABE_TOP:
            abd.rc.bottom = abd.rc.top + self._bar_height

        # Set the final position
        shell32.SHAppBarMessage(ABM_SETPOS, byref(abd))

        # Move the actual window
        user32.MoveWindow(
            self._hwnd,
            abd.rc.left, abd.rc.top,
            abd.rc.right - abd.rc.left,
            abd.rc.bottom - abd.rc.top,
            True,
        )

        log.info("AppBar positioned: left=%d top=%d right=%d bottom=%d",
                 abd.rc.left, abd.rc.top, abd.rc.right, abd.rc.bottom)

    def unregister(self) -> None:
        """Remove the AppBar registration and restore screen space."""
        if not self._registered:
            return
        abd = APPBARDATA()
        abd.cbSize = sizeof(APPBARDATA)
        abd.hWnd = self._hwnd
        shell32.SHAppBarMessage(ABM_REMOVE, byref(abd))
        self._registered = False
        log.info("AppBar unregistered")


def position_above_taskbar(hwnd: int, width: int, height: int) -> None:
    """Simple positioning: place widget just above the taskbar
    without reserving screen space (non-AppBar fallback)."""
    tb_left, tb_top, tb_right, tb_bottom = get_taskbar_rect()
    screen_w = user32.GetSystemMetrics(0)

    # Center horizontally, or align right
    x = screen_w - width - 16
    y = tb_top - height - 4

    user32.MoveWindow(hwnd, x, y, width, height, True)
    log.info("Positioned above taskbar: x=%d y=%d", x, y)
