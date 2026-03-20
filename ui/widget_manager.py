"""Claude Tank — pywebview window management."""

from __future__ import annotations

import ctypes
import json
import logging
import os
import threading
import webbrowser
from dataclasses import asdict
from pathlib import Path
from typing import Any

import webview  # type: ignore[import-untyped]

from core.api_client import ClaudeAPIClient, UsageData
from core.auth import save_credentials
from core.config import AppConfig
from core.i18n import t
from core.plan_detector import detect_plan

log = logging.getLogger("claude-tank")

_WEB_DIR = Path(__file__).resolve().parent / "web"


class _SetupAPI:
    """JS API exposed to the setup wizard window."""

    def __init__(self, on_complete: callable) -> None:
        self._on_complete = on_complete
        self._org_id = ""
        self._plan = ""

    def open_claude_ai(self) -> None:
        webbrowser.open("https://claude.ai")

    def test_connection(self, session_key: str) -> dict[str, Any]:
        try:
            client = ClaudeAPIClient(session_key)
            success, org_id, error = client.test_connection()
            if success:
                self._org_id = org_id
                orgs = client.get_organizations()
                self._plan = detect_plan(orgs[0]) if orgs else "Unknown"
                return {"success": True, "plan": self._plan}
            return {"success": False, "error": error}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def save_and_start(self, session_key: str) -> None:
        if self._org_id:
            save_credentials(session_key, self._org_id)
            self._on_complete(session_key, self._org_id, self._plan)


class _WidgetAPI:
    """JS API exposed to the widget window for hover detection."""

    def __init__(self, manager: WidgetManager) -> None:
        self._mgr = manager

    def show_hover(self) -> None:
        self._mgr._show_hover_panel()

    def hide_hover(self) -> None:
        self._mgr._hide_hover_panel()


class WidgetManager:
    def __init__(self, config: AppConfig) -> None:
        self._config = config
        self._widget_win: webview.Window | None = None
        self._hover_win: webview.Window | None = None
        self._dashboard_win: webview.Window | None = None
        self._setup_win: webview.Window | None = None
        self._started = threading.Event()
        self._plan_name = ""
        self._last_data: UsageData | None = None
        self._strings: dict[str, str] = {}

    def _load_strings(self) -> dict[str, str]:
        """Load i18n strings as a dict for passing to JS."""
        from core import i18n as i18n_mod
        locale_path = Path(__file__).resolve().parent.parent / "locales" / f"{i18n_mod.current_locale()}.json"
        if locale_path.exists():
            self._strings = json.loads(locale_path.read_text(encoding="utf-8"))
        return self._strings

    def start_webview_loop(self) -> None:
        """Start the pywebview event loop. Must be called from main thread of subprocess."""
        self._started.set()
        webview.start(debug=False)

    def show_setup(self, on_complete: callable) -> None:
        """Show the setup wizard."""
        self._load_strings()
        api = _SetupAPI(on_complete)
        self._setup_win = webview.create_window(
            "Claude Tank — Setup",
            str(_WEB_DIR / "setup.html"),
            width=500,
            height=620,
            resizable=False,
            js_api=api,
            text_select=False,
        )

        def _on_loaded():
            if self._setup_win and self._strings:
                js = f"if(typeof applyStrings==='function')applyStrings({json.dumps(self._strings)})"
                self._setup_win.evaluate_js(js)

        self._setup_win.events.loaded += _on_loaded

    def create_widget(self) -> None:
        """Create the small taskbar widget window."""
        self._load_strings()
        api = _WidgetAPI(self)

        # Get screen size to position widget near taskbar
        try:
            user32 = ctypes.windll.user32
            screen_w = user32.GetSystemMetrics(0)
            screen_h = user32.GetSystemMetrics(1)
        except Exception:
            screen_w, screen_h = 1920, 1080

        widget_w = 320
        widget_h = 44
        x = screen_w - widget_w - 200  # Left of system tray area
        y = screen_h - widget_h - 4    # Bottom edge, above taskbar

        self._widget_win = webview.create_window(
            "Claude Tank",
            str(_WEB_DIR / "widget.html"),
            width=widget_w,
            height=widget_h,
            x=x,
            y=y,
            frameless=True,
            on_top=True,
            transparent=True,
            resizable=False,
            js_api=api,
            text_select=False,
        )

    def create_hover_panel(self) -> None:
        """Create the hover popup panel (hidden initially)."""
        self._hover_win = webview.create_window(
            "Claude Tank — Details",
            str(_WEB_DIR / "hover.html"),
            width=340,
            height=380,
            frameless=True,
            on_top=True,
            transparent=True,
            resizable=False,
            hidden=True,
            text_select=False,
        )

    def show_dashboard(self) -> None:
        """Show or create the dashboard window."""
        if self._dashboard_win:
            try:
                self._dashboard_win.show()
                self._push_to_dashboard()
                return
            except Exception:
                pass

        self._dashboard_win = webview.create_window(
            "Claude Tank — Dashboard",
            str(_WEB_DIR / "index.html"),
            width=480,
            height=720,
            resizable=True,
            text_select=False,
        )

        def _on_loaded():
            self._push_to_dashboard()

        self._dashboard_win.events.loaded += _on_loaded

    def close_setup(self) -> None:
        if self._setup_win:
            self._setup_win.destroy()
            self._setup_win = None

    def set_plan(self, plan: str) -> None:
        self._plan_name = plan

    def update_usage(self, data: UsageData) -> None:
        """Push new usage data to all active windows."""
        self._last_data = data
        self._push_to_widget()
        self._push_to_hover()
        self._push_to_dashboard()

    def _data_as_js_obj(self) -> str:
        if not self._last_data:
            return "null"
        d = self._last_data
        return json.dumps({
            "five_hour": d.five_hour,
            "five_hour_reset": d.five_hour_reset,
            "seven_day": d.seven_day,
            "seven_day_reset": d.seven_day_reset,
            "opus": d.opus,
            "opus_reset": d.opus_reset,
            "sonnet": d.sonnet,
            "sonnet_reset": d.sonnet_reset,
            "extra_enabled": d.extra_enabled,
            "extra_monthly_limit": d.extra_monthly_limit,
            "extra_used": d.extra_used,
            "extra_utilization": d.extra_utilization,
        })

    def _config_as_js_obj(self) -> str:
        cfg = asdict(self._config)
        cfg.pop("_path", None)
        return json.dumps(cfg)

    def _strings_js(self) -> str:
        return json.dumps(self._strings)

    def _push_to_widget(self) -> None:
        if not self._widget_win:
            return
        try:
            js = f"updateWidget({self._data_as_js_obj()},{self._config_as_js_obj()},{self._strings_js()})"
            self._widget_win.evaluate_js(js)
        except Exception:
            pass

    def _push_to_hover(self) -> None:
        if not self._hover_win:
            return
        try:
            js = (f"updateHoverPanel({self._data_as_js_obj()},"
                  f"{self._config_as_js_obj()},"
                  f"{json.dumps(self._plan_name)},"
                  f"{self._strings_js()})")
            self._hover_win.evaluate_js(js)
        except Exception:
            pass

    def _push_to_dashboard(self) -> None:
        if not self._dashboard_win:
            return
        try:
            plan_el = f"var pe=document.getElementById('plan-name');if(pe)pe.textContent={json.dumps(self._plan_name)};"
            js = (f"{plan_el}"
                  f"updateDashboard({self._data_as_js_obj()},"
                  f"{self._config_as_js_obj()},"
                  f"{self._strings_js()})")
            self._dashboard_win.evaluate_js(js)
        except Exception:
            pass

    def _show_hover_panel(self) -> None:
        if not self._hover_win:
            return
        try:
            # Position above the widget
            if self._widget_win:
                x = self._widget_win.x
                y = self._widget_win.y - 390
            else:
                user32 = ctypes.windll.user32
                x = user32.GetSystemMetrics(0) - 360
                y = user32.GetSystemMetrics(1) - 440
            self._hover_win.move(x, y)
            self._hover_win.show()
            self._push_to_hover()
        except Exception as e:
            log.debug("Failed to show hover: %s", e)

    def _hide_hover_panel(self) -> None:
        if self._hover_win:
            try:
                self._hover_win.hide()
            except Exception:
                pass

    def destroy_all(self) -> None:
        for win in (self._widget_win, self._hover_win, self._dashboard_win, self._setup_win):
            if win:
                try:
                    win.destroy()
                except Exception:
                    pass
