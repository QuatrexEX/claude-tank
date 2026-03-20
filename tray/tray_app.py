"""Claude Tank — System tray application using pystray."""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING, Callable

import pystray
from PIL import Image

from core.config import AppConfig, APP_NAME, APP_VERSION, APP_AUTHOR
from core.i18n import t
from tray.icon_renderer import render_tank_icon, render_offline_icon

if TYPE_CHECKING:
    from core.api_client import UsageData

log = logging.getLogger("claude-tank")


class TrayApp:
    def __init__(
        self,
        config: AppConfig,
        on_dashboard: Callable[[], None] | None = None,
        on_refresh: Callable[[], None] | None = None,
        on_update_key: Callable[[], None] | None = None,
        on_quit: Callable[[], None] | None = None,
        on_gauge_mode: Callable[[str], None] | None = None,
        on_interval: Callable[[int], None] | None = None,
    ) -> None:
        self._config = config
        self._on_dashboard = on_dashboard
        self._on_refresh = on_refresh
        self._on_update_key = on_update_key
        self._on_quit = on_quit
        self._on_gauge_mode = on_gauge_mode
        self._on_interval = on_interval
        self._icon: pystray.Icon | None = None
        self._usage: UsageData | None = None

    def _build_menu(self) -> pystray.Menu:
        cfg = self._config
        return pystray.Menu(
            pystray.MenuItem(t("menu_dashboard"), self._do_dashboard),
            pystray.MenuItem(t("menu_refresh"), self._do_refresh),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem(
                t("menu_gauge_mode"),
                pystray.Menu(
                    pystray.MenuItem(
                        t("menu_gauge_remaining"),
                        lambda: self._set_gauge("remaining"),
                        checked=lambda _: cfg.gauge_mode == "remaining",
                        radio=True,
                    ),
                    pystray.MenuItem(
                        t("menu_gauge_consumed"),
                        lambda: self._set_gauge("consumed"),
                        checked=lambda _: cfg.gauge_mode == "consumed",
                        radio=True,
                    ),
                ),
            ),
            pystray.MenuItem(
                t("menu_interval"),
                pystray.Menu(
                    pystray.MenuItem(
                        t("menu_interval_1m"),
                        lambda: self._set_interval(60),
                        checked=lambda _: cfg.poll_interval_sec == 60,
                        radio=True,
                    ),
                    pystray.MenuItem(
                        t("menu_interval_3m"),
                        lambda: self._set_interval(180),
                        checked=lambda _: cfg.poll_interval_sec == 180,
                        radio=True,
                    ),
                    pystray.MenuItem(
                        t("menu_interval_5m"),
                        lambda: self._set_interval(300),
                        checked=lambda _: cfg.poll_interval_sec == 300,
                        radio=True,
                    ),
                    pystray.MenuItem(
                        t("menu_interval_10m"),
                        lambda: self._set_interval(600),
                        checked=lambda _: cfg.poll_interval_sec == 600,
                        radio=True,
                    ),
                ),
            ),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem(t("menu_update_key"), self._do_update_key),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem(
                f"{APP_AUTHOR} / {APP_NAME} v{APP_VERSION}",
                None,
                enabled=False,
            ),
            pystray.MenuItem(t("menu_quit"), self._do_quit),
        )

    def start(self) -> None:
        icon_img = render_offline_icon()
        self._icon = pystray.Icon(
            "claude-tank",
            icon_img,
            APP_NAME,
            menu=self._build_menu(),
        )
        log.info("Starting system tray")
        self._icon.run_detached()

    def stop(self) -> None:
        if self._icon:
            self._icon.stop()

    def update_usage(self, data: UsageData) -> None:
        self._usage = data
        self._update_icon()
        self._update_tooltip()

    def set_offline(self) -> None:
        if self._icon:
            self._icon.icon = render_offline_icon()
            self._icon.title = f"{APP_NAME} — Offline"

    def _update_icon(self) -> None:
        if not self._icon or not self._usage:
            return
        self._icon.icon = render_tank_icon(
            self._usage.five_hour,
            self._config.gauge_mode,
        )

    def _update_tooltip(self) -> None:
        if not self._icon or not self._usage:
            return
        d = self._usage
        mode = self._config.gauge_mode
        if mode == "remaining":
            v5 = 100.0 - d.five_hour
            v7 = 100.0 - d.seven_day
            label = t("left")
        else:
            v5 = d.five_hour
            v7 = d.seven_day
            label = t("used")
        self._icon.title = (
            f"{APP_NAME}\n"
            f"5h: {v5:.0f}% {label}  |  7d: {v7:.0f}% {label}"
        )

    def _do_dashboard(self) -> None:
        if self._on_dashboard:
            self._on_dashboard()

    def _do_refresh(self) -> None:
        if self._on_refresh:
            self._on_refresh()

    def _do_update_key(self) -> None:
        if self._on_update_key:
            self._on_update_key()

    def _do_quit(self) -> None:
        if self._on_quit:
            self._on_quit()

    def _set_gauge(self, mode: str) -> None:
        self._config.update(gauge_mode=mode)
        self._update_icon()
        self._update_tooltip()
        if self._on_gauge_mode:
            self._on_gauge_mode(mode)

    def _set_interval(self, seconds: int) -> None:
        self._config.update(poll_interval_sec=seconds)
        if self._on_interval:
            self._on_interval(seconds)
