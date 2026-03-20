"""Claude Tank — Entry point.

Launches the system tray, taskbar widget, and polling loop.
Uses pywebview for all GUI windows and pystray for the system tray icon.
"""

from __future__ import annotations

import logging
import sys
import threading

from core import i18n
from core.auth import has_credentials, load_credentials
from core.config import AppConfig
from core.poller import UsagePoller
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
        self._plan = ""

    def run(self) -> None:
        if has_credentials():
            self._start_monitoring()
        else:
            self._show_setup()

        # pywebview event loop (blocks until all windows closed)
        self.widget_mgr.start_webview_loop()

    def _show_setup(self) -> None:
        self.widget_mgr.show_setup(on_complete=self._on_setup_complete)

    def _on_setup_complete(self, session_key: str, org_id: str, plan: str) -> None:
        self._plan = plan
        self.widget_mgr.set_plan(plan)
        self.widget_mgr.close_setup()
        self._start_with_credentials(session_key, org_id)

    def _start_monitoring(self) -> None:
        creds = load_credentials()
        if not creds:
            self._show_setup()
            return
        session_key, org_id = creds
        # Detect plan from API
        from core.api_client import ClaudeAPIClient
        from core.plan_detector import detect_plan
        try:
            client = ClaudeAPIClient(session_key)
            orgs = client.get_organizations()
            self._plan = detect_plan(orgs[0]) if orgs else "Unknown"
        except Exception:
            self._plan = "Unknown"
        self._start_with_credentials(session_key, org_id)

    def _start_with_credentials(self, session_key: str, org_id: str) -> None:
        self.widget_mgr.set_plan(self._plan)

        # Create widget windows
        self.widget_mgr.create_widget()
        self.widget_mgr.create_hover_panel()

        # Start tray in a thread
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

        # Start poller
        self.poller = UsagePoller(
            session_key=session_key,
            org_id=org_id,
            interval_sec=self.config.poll_interval_sec,
            on_update=self._on_usage_update,
            on_error=self._on_poll_error,
        )
        self.poller.start()

    def _on_usage_update(self, data) -> None:
        log.info(
            "Usage updated: 5h=%.0f%% 7d=%.0f%%",
            data.five_hour, data.seven_day,
        )
        self.widget_mgr.update_usage(data)
        if self.tray:
            self.tray.update_usage(data)

    def _on_poll_error(self, error: str, consecutive: int) -> None:
        log.warning("Poll error (%d consecutive): %s", consecutive, error)
        if consecutive >= 3 and self.tray:
            self.tray.set_offline()

    def _on_dashboard(self) -> None:
        self.widget_mgr.show_dashboard()

    def _on_refresh(self) -> None:
        if self.poller:
            threading.Thread(target=self.poller.poll_once, daemon=True).start()

    def _on_update_key(self) -> None:
        self._show_setup()

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
