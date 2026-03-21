"""Claude Tank — pywebview window management.

Architecture:
- A "session window" (hidden) stays navigated to claude.ai and acts as our
  browser session. All API calls go through this window's fetch() via
  WebViewAPIClient, completely bypassing Cloudflare.
- During setup, the session window is shown so the user can log in.
- After login, it is hidden and reused for background API polling.
"""

from __future__ import annotations

import ctypes
import json
import logging
import threading
import time
from dataclasses import asdict
from pathlib import Path
from typing import Any

import webview  # type: ignore[import-untyped]

from core.api_client import UsageData
from core.config import AppConfig, app_dir
from core.plan_detector import detect_plan
from core.webview_api import WebViewAPIClient

log = logging.getLogger("claude-tank")

_WEB_DIR = Path(__file__).resolve().parent / "web"
_WEBVIEW_STORAGE = str(app_dir() / "webview_data")


class _LoginAPI:
    """JS API exposed to the session/login window."""

    def __init__(self, manager: WidgetManager) -> None:
        self._mgr = manager

    def check_login(self) -> dict[str, Any]:
        """Called from JS to check if user is logged in."""
        return self._mgr._check_login_status()

    def confirm_login(self) -> dict[str, Any]:
        """Called when user clicks 'I'm logged in'."""
        return self._mgr._confirm_login()


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
        self._session_win: webview.Window | None = None
        self._widget_win: webview.Window | None = None
        self._hover_win: webview.Window | None = None
        self._dashboard_win: webview.Window | None = None
        self._plan_name = ""
        self._org_id = ""
        self._last_data: UsageData | None = None
        self._strings: dict[str, str] = {}
        self._on_login_complete: callable | None = None
        self._api_client: WebViewAPIClient | None = None
        self._appbar: Any = None

    def _load_strings(self) -> dict[str, str]:
        from core import i18n as i18n_mod
        locale_path = (Path(__file__).resolve().parent.parent
                       / "locales" / f"{i18n_mod.current_locale()}.json")
        if locale_path.exists():
            self._strings = json.loads(locale_path.read_text(encoding="utf-8"))
        return self._strings

    @property
    def api_client(self) -> WebViewAPIClient | None:
        return self._api_client

    # ──────────────────── Session window (login + API) ────────────────────

    def create_session_window(self, visible: bool = False) -> None:
        """Create the session window navigated to claude.ai.
        If visible=True, user can log in. If hidden, used for background API.
        """
        api = _LoginAPI(self)
        self._session_win = webview.create_window(
            "Claude Tank — Login",
            "https://claude.ai",
            width=1024,
            height=720,
            js_api=api,
            hidden=not visible,
            text_select=True,
        )

        def _on_loaded():
            if self._session_win and visible:
                # Inject a floating "I'm logged in" button
                self._inject_login_button()

        self._session_win.events.loaded += _on_loaded

    def _inject_login_button(self) -> None:
        """Inject UI overlay into the claude.ai page.
        All content is hardcoded static strings — no untrusted data is rendered."""
        js = r"""
        (function() {
            if (document.getElementById('ct-overlay')) return;

            // Banner at top
            var banner = document.createElement('div');
            banner.id = 'ct-overlay';
            banner.style.cssText = 'position:fixed;top:0;left:0;right:0;z-index:99999;' +
                'padding:10px 20px;background:linear-gradient(90deg,#0a0a0f,#1a1a2e);' +
                'color:#00ffcc;font-size:13px;font-family:system-ui;display:flex;' +
                'align-items:center;justify-content:space-between;border-bottom:1px solid #00ffcc40';
            var bannerLeft = document.createElement('span');
            bannerLeft.textContent = '\ud83d\udfe2 Claude Tank \u2014 Log in to claude.ai, then click the green button at bottom-right';
            bannerLeft.style.fontWeight = '600';
            var bannerRight = document.createElement('span');
            bannerRight.textContent = 'Tip: If Google login fails, use email login instead';
            bannerRight.style.cssText = 'font-size:11px;color:#888';
            banner.appendChild(bannerLeft);
            banner.appendChild(bannerRight);
            document.body.appendChild(banner);
            document.body.style.paddingTop = '44px';

            // Floating confirm button
            var btn = document.createElement('div');
            btn.id = 'ct-login-btn';
            btn.style.cssText = 'position:fixed;bottom:20px;right:20px;z-index:99999;' +
                'padding:14px 28px;background:#00ffcc;color:#000;font-weight:700;' +
                'font-size:14px;border-radius:10px;cursor:pointer;font-family:system-ui;' +
                'box-shadow:0 4px 24px rgba(0,255,204,0.35);transition:all 0.2s';
            btn.textContent = '\u25b6 Start Claude Tank';
            btn.onmouseenter = function() { btn.style.transform = 'scale(1.05)'; };
            btn.onmouseleave = function() { btn.style.transform = 'scale(1)'; };
            btn.onclick = function() {
                btn.textContent = 'Connecting...';
                btn.style.background = '#eab308';
                pywebview.api.confirm_login().then(function(r) {
                    if (r.success) {
                        btn.textContent = '\u2713 Connected! Plan: ' + r.plan;
                        btn.style.background = '#22c55e';
                        bannerLeft.textContent = '\u2705 Connected! Starting monitor...';
                    } else {
                        btn.textContent = '\u2717 ' + r.error;
                        btn.style.background = '#ef4444';
                        setTimeout(function() {
                            btn.textContent = '\u25b6 Start Claude Tank';
                            btn.style.background = '#00ffcc';
                        }, 4000);
                    }
                });
            };
            document.body.appendChild(btn);
        })();
        """
        try:
            self._session_win.evaluate_js(js)
        except Exception:
            pass

    def _check_login_status(self) -> dict[str, Any]:
        """Check if the user is logged in by trying an API call."""
        if not self._session_win:
            return {"logged_in": False}
        client = WebViewAPIClient(self._session_win)
        success, org_id, error = client.test_connection()
        return {"logged_in": success, "org_id": org_id, "error": error}

    def _confirm_login(self) -> dict[str, Any]:
        """User confirmed they are logged in. Verify and transition."""
        if not self._session_win:
            return {"success": False, "error": "No session window"}

        # Navigate back to claude.ai if we're on a different domain
        try:
            current_url = self._session_win.get_current_url() or ""
            log.info("confirm_login: current URL = %s", current_url)
            if "claude.ai" not in current_url:
                log.info("Not on claude.ai, navigating back...")
                self._session_win.load_url("https://claude.ai")
                import time
                time.sleep(3)
        except Exception as e:
            log.warning("URL check failed: %s", e)

        client = WebViewAPIClient(self._session_win)
        try:
            log.info("Testing connection...")
            success, org_id, error = client.test_connection()
            if not success:
                return {"success": False, "error": error}
            self._org_id = org_id
            self._api_client = client
            orgs = client.get_organizations()
            self._plan_name = detect_plan(orgs[0]) if orgs else "Unknown"

            # Save org_id for future reference
            config_path = app_dir() / "session.json"
            config_path.write_text(json.dumps({"org_id": org_id}), encoding="utf-8")

            # Hide session window, start monitoring
            threading.Thread(target=self._transition_to_monitoring, daemon=True).start()

            return {"success": True, "plan": self._plan_name}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def _transition_to_monitoring(self) -> None:
        """Hide login window, create widget, start monitoring."""
        time.sleep(1)  # Let the success message show
        if self._session_win:
            self._session_win.hide()
        if self._on_login_complete:
            self._on_login_complete()

    # ──────────────────── Widget + Hover ────────────────────

    def create_widget(self) -> None:
        self._load_strings()
        api = _WidgetAPI(self)

        from widget.taskbar_integration import get_taskbar_rect

        tb_left, tb_top, tb_right, tb_bottom = get_taskbar_rect()
        screen_w = ctypes.windll.user32.GetSystemMetrics(0)

        widget_h = 32
        widget_w = screen_w  # Full width initially, will be adjusted by AppBar

        # Position just above taskbar
        x = 0
        y = max(0, tb_top - widget_h)

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

        def _on_widget_shown():
            """Register as AppBar after window is shown."""
            time.sleep(0.8)
            try:
                hwnd = self._get_native_hwnd(self._widget_win)
                if hwnd:
                    from widget.taskbar_integration import AppBarWidget, position_above_taskbar
                    self._appbar = AppBarWidget()
                    registered = self._appbar.register(hwnd, height=widget_h)
                    if registered:
                        log.info("Widget registered as AppBar")
                    else:
                        log.info("AppBar failed, positioning above taskbar")
                        position_above_taskbar(hwnd, 320, widget_h)
            except Exception as e:
                log.warning("Taskbar integration error: %s", e)

        self._widget_win.events.shown += lambda: threading.Thread(
            target=_on_widget_shown, daemon=True).start()

    @staticmethod
    def _get_native_hwnd(window) -> int:
        """Get the native Win32 HWND from a pywebview window."""
        # Find by window title
        try:
            hwnd = ctypes.windll.user32.FindWindowW(None, window.title)
            if hwnd:
                return hwnd
        except Exception:
            pass
        return 0

    def create_hover_panel(self) -> None:
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
        self._dashboard_win.events.loaded += lambda: self._push_to_dashboard()

    def set_plan(self, plan: str) -> None:
        self._plan_name = plan

    def set_on_login_complete(self, callback: callable) -> None:
        self._on_login_complete = callback

    # ──────────────────── Data push ────────────────────

    def update_usage(self, data: UsageData) -> None:
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

    def show_login(self) -> None:
        """Show the login/session window for re-authentication."""
        if self._session_win:
            self._session_win.show()
            self._session_win.load_url("https://claude.ai")

    def start_webview_loop(self) -> None:
        # private_mode=False: persist cookies between sessions
        # storage_path: store WebView2 data (cookies, cache) locally
        webview.start(
            debug=False,
            private_mode=False,
            storage_path=_WEBVIEW_STORAGE,
        )

    def destroy_all(self) -> None:
        # Unregister AppBar to restore screen space
        if self._appbar:
            try:
                self._appbar.unregister()
            except Exception:
                pass
        for win in (self._widget_win, self._hover_win,
                    self._dashboard_win, self._session_win):
            if win:
                try:
                    win.destroy()
                except Exception:
                    pass
