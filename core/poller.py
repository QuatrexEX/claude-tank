"""Claude Tank — Background polling for usage data."""

from __future__ import annotations

import logging
import threading
import time
from typing import Callable

from core.api_client import ClaudeAPIClient, ClaudeAPIError, UsageData

log = logging.getLogger("claude-tank")


class UsagePoller:
    def __init__(
        self,
        session_key: str,
        org_id: str,
        interval_sec: int = 180,
        on_update: Callable[[UsageData], None] | None = None,
        on_error: Callable[[str, int], None] | None = None,
    ) -> None:
        self._client = ClaudeAPIClient(session_key)
        self._org_id = org_id
        self._interval = interval_sec
        self._on_update = on_update
        self._on_error = on_error
        self._thread: threading.Thread | None = None
        self._stop_event = threading.Event()
        self._consecutive_errors = 0
        self._last_data: UsageData | None = None
        self._backoff_multiplier = 1

    @property
    def last_data(self) -> UsageData | None:
        return self._last_data

    def start(self) -> None:
        if self._thread and self._thread.is_alive():
            return
        self._stop_event.clear()
        self._thread = threading.Thread(target=self._poll_loop, daemon=True)
        self._thread.start()
        log.info("Poller started (interval=%ds)", self._interval)

    def stop(self) -> None:
        self._stop_event.set()
        if self._thread:
            self._thread.join(timeout=5)
        log.info("Poller stopped")

    def poll_once(self) -> UsageData | None:
        try:
            data = self._client.get_usage(self._org_id)
            self._last_data = data
            self._consecutive_errors = 0
            self._backoff_multiplier = 1
            if self._on_update:
                self._on_update(data)
            return data
        except ClaudeAPIError as e:
            self._consecutive_errors += 1
            if e.status_code == 429:
                self._backoff_multiplier = min(self._backoff_multiplier * 2, 8)
                log.warning("Rate limited, backing off (x%d)", self._backoff_multiplier)
            if self._on_error:
                self._on_error(str(e), self._consecutive_errors)
            return None
        except Exception as e:
            self._consecutive_errors += 1
            log.error("Poll error: %s", e)
            if self._on_error:
                self._on_error(str(e), self._consecutive_errors)
            return None

    def set_interval(self, seconds: int) -> None:
        self._interval = seconds

    def _poll_loop(self) -> None:
        # Initial fetch
        self.poll_once()
        while not self._stop_event.is_set():
            wait = self._interval * self._backoff_multiplier
            if self._stop_event.wait(timeout=wait):
                break
            self.poll_once()
