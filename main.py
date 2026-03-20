"""Claude Tank — Entry point.

Architecture:
1. A pywebview "session window" opens claude.ai — user logs in there.
2. After login, the session window is hidden and reused for API calls
   (via WebViewAPIClient which runs fetch() inside WebView2).
3. System tray + taskbar widget show usage data.
4. No cookies are extracted or stored — the WebView2 manages its own session.
"""

from __future__ import annotations

import json
import logging
import sys
import threading

from core import i18n
from core.config import AppConfig, app_dir
from core.poller import UsagePoller
from core.plan_detector import detect_plan
from tray.tray_app import TrayApp
from ui.widget_manager import WidgetManager

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(name)s] %(levelname)s: %(message)s",
    handlers=[logging.StreamHandler()],
)
log = logging.getLogger("claude-tank")


class App:
    def __init__(self) -> None:
        self.config = AppConfig.load()
        i18n.init(self.config.locale)
        self.widget_mgr = WidgetManager(self.config)
        self.tray: TrayApp | None = None
        self.poller: UsagePoller | None = None

    def run(self) -> None:
        # Always create the session window (navigated to claude.ai)
        # Check if we have a saved org_id (returning user)
        session_file = app_dir() / "session.json"
        has_session = session_file.exists()

        # Create session window — visible for login, or hidden for returning users
        self.widget_mgr.create_session_window(visible=not has_session)
        self.widget_mgr.set_on_login_complete(self._on_login_complete)

        if has_session:
            # Returning user: try to verify session in background
            threading.Thread(target=self._try_resume_session, daemon=True).start()

        # pywebview event loop (blocks)
        self.widget_mgr.start_webview_loop()

    def _try_resume_session(self) -> None:
        """Try to resume an existing session (returning user)."""
        import time
        time.sleep(3)  # Wait for webview to load
        try:
            session_file = app_dir() / "session.json"
            data = json.loads(session_file.read_text(encoding="utf-8"))
            org_id = data.get("org_id", "")

            client = self.widget_mgr.api_client
            if not client:
                from core.webview_api import WebViewAPIClient
                # Need to wait for session window to be ready
                for _ in range(10):
                    if self.widget_mgr._session_win:
                        client = WebViewAPIClient(self.widget_mgr._session_win)
                        break
                    time.sleep(1)

            if not client:
                log.warning("Could not create API client, showing login")
                self.widget_mgr.show_login()
                return

            success, verified_org_id, error = client.test_connection()
            if success:
                log.info("Session resumed successfully")
                self.widget_mgr._api_client = client
                self.widget_mgr._org_id = verified_org_id or org_id

                # Detect plan
                try:
                    orgs = client.get_organizations()
                    plan = detect_plan(orgs[0]) if orgs else "Unknown"
                except Exception:
                    plan = "Unknown"
                self.widget_mgr.set_plan(plan)
                self._start_monitoring(client, verified_org_id or org_id)
            else:
                log.warning("Session expired, showing login: %s", error)
                self.widget_mgr.show_login()
        except Exception as e:
            log.warning("Failed to resume session: %s", e)
            self.widget_mgr.show_login()

    def _on_login_complete(self) -> None:
        """Called after user successfully logs in via the session window."""
        client = self.widget_mgr.api_client
        org_id = self.widget_mgr._org_id
        if client and org_id:
            self._start_monitoring(client, org_id)

    def _start_monitoring(self, client, org_id: str) -> None:
        """Start the widget, tray, and poller."""
        self.widget_mgr.create_widget()
        self.widget_mgr.create_hover_panel()

        # Start tray
        self.tray = TrayApp(
            config=self.config,
            on_dashboard=self._on_dashboard,
            on_refresh=self._on_refresh,
            on_update_key=self._on_update_key,
            on_quit=self._on_quit,
            on_gauge_mode=self._on_gauge_mode,
            on_interval=self._on_interval,
        )
        threading.Thread(target=self.tray.start, daemon=True).start()

        # Start poller using WebView API client
        self.poller = UsagePoller(
            client=client,
            org_id=org_id,
            interval_sec=self.config.poll_interval_sec,
            on_update=self._on_usage_update,
            on_error=self._on_poll_error,
        )
        self.poller.start()

    def _on_usage_update(self, data) -> None:
        log.info("Usage: 5h=%.0f%% 7d=%.0f%%", data.five_hour, data.seven_day)
        self.widget_mgr.update_usage(data)
        if self.tray:
            self.tray.update_usage(data)

    def _on_poll_error(self, error: str, consecutive: int) -> None:
        log.warning("Poll error (%d): %s", consecutive, error)
        if consecutive >= 3 and self.tray:
            self.tray.set_offline()

    def _on_dashboard(self) -> None:
        self.widget_mgr.show_dashboard()

    def _on_refresh(self) -> None:
        if self.poller:
            threading.Thread(target=self.poller.poll_once, daemon=True).start()

    def _on_update_key(self) -> None:
        self.widget_mgr.show_login()

    def _on_quit(self) -> None:
        log.info("Quitting...")
        if self.poller:
            self.poller.stop()
        if self.tray:
            self.tray.stop()
        self.widget_mgr.destroy_all()
        sys.exit(0)

    def _on_gauge_mode(self, mode: str) -> None:
        if self.poller and self.poller.last_data:
            self.widget_mgr.update_usage(self.poller.last_data)

    def _on_interval(self, seconds: int) -> None:
        if self.poller:
            self.poller.set_interval(seconds)


def main() -> None:
    try:
        app = App()
        app.run()
    except KeyboardInterrupt:
        log.info("Interrupted")
    except Exception:
        log.exception("Fatal error")
        sys.exit(1)


if __name__ == "__main__":
    main()
