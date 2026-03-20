"""Claude Tank — API calls via WebView2 fetch().

Uses the pywebview window's browser session to make API calls.
This bypasses Cloudflare entirely because WebView2 is a real browser engine
with valid TLS fingerprints and cookie management.
"""

from __future__ import annotations

import json
import logging
from typing import Any

from core.api_client import UsageData, ClaudeAPIError

log = logging.getLogger("claude-tank")


class WebViewAPIClient:
    """Makes claude.ai API calls through a pywebview window's JS context."""

    def __init__(self, window: Any) -> None:
        self._window = window

    def _fetch_json(self, path: str) -> Any:
        """Execute fetch() inside the WebView2 and return parsed JSON."""
        js = f"""
        (async () => {{
            try {{
                const resp = await fetch('{path}', {{
                    credentials: 'include',
                    headers: {{ 'Accept': 'application/json' }}
                }});
                if (!resp.ok) {{
                    return {{ __error: true, status: resp.status, text: await resp.text() }};
                }}
                return await resp.json();
            }} catch(e) {{
                return {{ __error: true, status: 0, text: e.message }};
            }}
        }})()
        """
        result = self._window.evaluate_js(js)
        if result is None:
            raise ClaudeAPIError(0, "WebView returned null — page may not be loaded")
        if isinstance(result, dict) and result.get("__error"):
            raise ClaudeAPIError(
                result.get("status", 0),
                str(result.get("text", "Unknown error"))[:200],
            )
        return result

    def get_organizations(self) -> list[dict[str, Any]]:
        return self._fetch_json("/api/organizations")

    def get_org_id(self) -> str:
        orgs = self.get_organizations()
        if not orgs:
            raise ClaudeAPIError(404, "No organizations found")
        return orgs[0]["uuid"]

    def get_usage(self, org_id: str) -> UsageData:
        raw = self._fetch_json(f"/api/organizations/{org_id}/usage")
        return self._parse_usage(raw)

    def test_connection(self) -> tuple[bool, str, str]:
        """Test API connection. Returns (success, org_id, error_message)."""
        try:
            org_id = self.get_org_id()
            self.get_usage(org_id)
            return True, org_id, ""
        except ClaudeAPIError as e:
            return False, "", str(e)
        except Exception as e:
            return False, "", f"Error: {e}"

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
