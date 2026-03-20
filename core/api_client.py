"""Claude Tank — claude.ai API client."""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from typing import Any

import requests

log = logging.getLogger("claude-tank")

_BASE_URL = "https://claude.ai/api"
_TIMEOUT = 15


@dataclass
class UsageData:
    five_hour: float = 0.0
    five_hour_reset: str | None = None
    seven_day: float = 0.0
    seven_day_reset: str | None = None
    opus: float = 0.0
    opus_reset: str | None = None
    sonnet: float = 0.0
    sonnet_reset: str | None = None
    extra_enabled: bool = False
    extra_monthly_limit: float | None = None
    extra_used: float | None = None
    extra_utilization: float | None = None
    raw: dict[str, Any] = field(default_factory=dict)


class ClaudeAPIError(Exception):
    def __init__(self, status_code: int, message: str = ""):
        self.status_code = status_code
        super().__init__(f"HTTP {status_code}: {message}")


class ClaudeAPIClient:
    def __init__(self, session_key: str) -> None:
        self._session = requests.Session()
        self._session.headers.update({
            "Accept": "application/json",
            "User-Agent": "claude-tank/1.0 (Quatrex)",
        })
        self._session.cookies.set("sessionKey", session_key, domain="claude.ai")

    def get_organizations(self) -> list[dict[str, Any]]:
        resp = self._session.get(
            f"{_BASE_URL}/organizations",
            timeout=_TIMEOUT,
        )
        if resp.status_code != 200:
            raise ClaudeAPIError(resp.status_code, resp.text[:200])
        return resp.json()

    def get_org_id(self) -> str:
        orgs = self.get_organizations()
        if not orgs:
            raise ClaudeAPIError(404, "No organizations found")
        return orgs[0]["uuid"]

    def get_usage(self, org_id: str) -> UsageData:
        resp = self._session.get(
            f"{_BASE_URL}/organizations/{org_id}/usage",
            timeout=_TIMEOUT,
        )
        if resp.status_code != 200:
            raise ClaudeAPIError(resp.status_code, resp.text[:200])
        raw = resp.json()
        return self._parse_usage(raw)

    def test_connection(self) -> tuple[bool, str, str]:
        """Test API connection. Returns (success, org_id, error_message)."""
        try:
            org_id = self.get_org_id()
            self.get_usage(org_id)
            return True, org_id, ""
        except ClaudeAPIError as e:
            return False, "", str(e)
        except requests.RequestException as e:
            return False, "", f"Connection error: {e}"

    @staticmethod
    def _parse_usage(raw: dict[str, Any]) -> UsageData:
        def _get(key: str) -> tuple[float, str | None]:
            block = raw.get(key, {})
            if not block or not isinstance(block, dict):
                return 0.0, None
            return float(block.get("utilization", 0.0)), block.get("resets_at")

        fh_util, fh_reset = _get("five_hour")
        sd_util, sd_reset = _get("seven_day")
        op_util, op_reset = _get("seven_day_opus")
        so_util, so_reset = _get("seven_day_sonnet")

        extra = raw.get("extra_usage", {}) or {}

        return UsageData(
            five_hour=fh_util,
            five_hour_reset=fh_reset,
            seven_day=sd_util,
            seven_day_reset=sd_reset,
            opus=op_util,
            opus_reset=op_reset,
            sonnet=so_util,
            sonnet_reset=so_reset,
            extra_enabled=bool(extra.get("is_enabled")),
            extra_monthly_limit=extra.get("monthly_limit"),
            extra_used=extra.get("used_credits"),
            extra_utilization=extra.get("utilization"),
            raw=raw,
        )
