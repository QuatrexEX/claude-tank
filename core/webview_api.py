"""Claude Tank — API calls via WebView2 fetch().

Uses the pywebview window's browser session to make API calls.
This bypasses Cloudflare entirely because WebView2 is a real browser engine.

IMPORTANT: pywebview's evaluate_js() returns {} for Promises unless
a callback function is passed as the second argument. We use
threading.Event to synchronize the async callback with the caller.
"""

from __future__ import annotations

import json
import logging
import threading
from typing import Any

from core.api_client import UsageData, ClaudeAPIError

log = logging.getLogger("claude-tank")


class WebViewAPIClient:
    """Makes claude.ai API calls through a pywebview window's JS context."""

    def __init__(self, window: Any) -> None:
        self._window = window

    def _fetch_json(self, path: str) -> Any:
        """Execute fetch() inside the WebView2 and return parsed JSON.
        Uses evaluate_js with a callback to properly resolve the Promise.
        """
        url = f"https://claude.ai{path}" if path.startswith("/") else path
        log.info("WebViewAPI fetch: %s", url)

        js = f"""
        new Promise(function(resolve, reject) {{
            fetch('{url}', {{
                credentials: 'include',
                headers: {{
                    'Accept': 'application/json',
                    'Content-Type': 'application/json',
                    'anthropic-client-platform': 'web_claude_ai'
                }}
            }})
            .then(function(resp) {{
                return resp.text().then(function(body) {{
                    return {{ status: resp.status, ok: resp.ok, url: resp.url, body: body }};
                }});
            }})
            .then(function(data) {{
                resolve(data);
            }})
            .catch(function(err) {{
                resolve({{ status: 0, ok: false, url: '{url}', body: err.message }});
            }});
        }})
        """

        result_holder: list[Any] = []
        event = threading.Event()

        def callback(result: Any) -> None:
            result_holder.append(result)
            event.set()

        self._window.evaluate_js(js, callback)

        # Wait for the Promise to resolve (max 20 seconds)
        if not event.wait(timeout=20):
            log.error("WebViewAPI timeout for %s", url)
            raise ClaudeAPIError(0, "Request timed out")

        raw = result_holder[0] if result_holder else None
        log.debug("WebViewAPI raw result: %s", str(raw)[:300])

        if raw is None:
            raise ClaudeAPIError(0, "WebView returned null")

        if isinstance(raw, dict):
            status = raw.get("status", 0)
            ok = raw.get("ok", False)
            body = raw.get("body", "")
            resp_url = raw.get("url", url)

            if not ok:
                log.error("WebViewAPI error: status=%s url=%s body=%s",
                          status, resp_url, str(body)[:200])
                raise ClaudeAPIError(status, str(body)[:200])

            try:
                parsed = json.loads(body)
                log.info("WebViewAPI success for %s (status=%s)", url, status)
                return parsed
            except json.JSONDecodeError as e:
                log.error("WebViewAPI JSON parse error: %s body=%s", e, str(body)[:200])
                raise ClaudeAPIError(status, f"JSON parse error: {body[:100]}")
        else:
            log.error("WebViewAPI unexpected result type: %s", type(raw).__name__)
            raise ClaudeAPIError(0, f"Unexpected result: {str(raw)[:200]}")

    def get_organizations(self) -> list[dict[str, Any]]:
        return self._fetch_json("/api/organizations")

    def get_org_id(self) -> str:
        orgs = self.get_organizations()
        if not orgs:
            raise ClaudeAPIError(404, "No organizations found")
        if isinstance(orgs, list):
            return orgs[0]["uuid"]
        # Some responses might be a dict with a different structure
        if isinstance(orgs, dict):
            if "uuid" in orgs:
                return orgs["uuid"]
            # Try to find uuid in nested structure
            log.info("Orgs dict keys: %s", list(orgs.keys()))
            raise ClaudeAPIError(404, f"Unexpected org format: {list(orgs.keys())}")
        raise ClaudeAPIError(404, f"Unexpected org type: {type(orgs).__name__}")

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
