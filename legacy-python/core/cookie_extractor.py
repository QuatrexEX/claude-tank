"""Claude Tank — Automatic cookie extraction from browsers.

Strategy (in order):
1. Firefox: Direct SQLite read (no encryption, no admin, instant)
2. pywebview: User logs in inside the app (fallback)
3. Manual: User pastes cookie string (last resort)
"""

from __future__ import annotations

import glob
import logging
import os
import shutil
import sqlite3
import tempfile
from dataclasses import dataclass

log = logging.getLogger("claude-tank")


@dataclass
class ExtractedCookies:
    session_key: str
    extra_cookies: dict[str, str]
    source: str  # "firefox", "webview", "manual"


def extract_from_firefox() -> ExtractedCookies | None:
    """Read claude.ai cookies directly from Firefox's SQLite database.
    Firefox does NOT encrypt cookies, so this works without admin rights.
    """
    profiles_dir = os.path.expandvars(r"%APPDATA%\Mozilla\Firefox\Profiles")
    if not os.path.isdir(profiles_dir):
        log.info("Firefox profiles directory not found")
        return None

    profiles = glob.glob(os.path.join(profiles_dir, "*"))
    for profile in profiles:
        db_path = os.path.join(profile, "cookies.sqlite")
        if not os.path.exists(db_path):
            continue

        try:
            # Firefox locks the DB while running — copy to a temp file
            tmp = tempfile.mktemp(suffix=".sqlite")
            shutil.copy2(db_path, tmp)

            conn = sqlite3.connect(tmp)
            cursor = conn.execute(
                "SELECT name, value FROM moz_cookies "
                "WHERE host LIKE '%claude.ai%'"
            )
            cookies = {row[0]: row[1] for row in cursor.fetchall()}
            conn.close()
            os.unlink(tmp)

            session_key = cookies.pop("sessionKey", "")
            if not session_key:
                continue

            # Keep useful extra cookies
            extras = {}
            for name in ("cf_clearance", "__cf_bm", "lastActiveOrg",
                         "CH-prefers-color-scheme", "activitySessionId"):
                if name in cookies:
                    extras[name] = cookies[name]

            log.info("Extracted cookies from Firefox profile: %s",
                     os.path.basename(profile))
            return ExtractedCookies(
                session_key=session_key,
                extra_cookies=extras,
                source="firefox",
            )
        except Exception as e:
            log.debug("Failed to read Firefox profile %s: %s", profile, e)
            try:
                os.unlink(tmp)
            except Exception:
                pass
            continue

    log.info("No claude.ai cookies found in any Firefox profile")
    return None


def extract_from_cookie_string(raw: str) -> ExtractedCookies | None:
    """Parse a manually pasted cookie header string or bare sessionKey."""
    raw = raw.strip()
    if not raw:
        return None

    # Bare sessionKey value
    if raw.startswith("sk-ant-"):
        return ExtractedCookies(
            session_key=raw,
            extra_cookies={},
            source="manual",
        )

    # Full cookie header: "name=value; name=value; ..."
    session_key = ""
    extras: dict[str, str] = {}
    for pair in raw.split(";"):
        pair = pair.strip()
        if "=" not in pair:
            continue
        name, _, value = pair.partition("=")
        name = name.strip()
        value = value.strip()
        if name == "sessionKey":
            session_key = value
        elif name in ("cf_clearance", "__cf_bm", "lastActiveOrg",
                       "CH-prefers-color-scheme", "activitySessionId"):
            extras[name] = value

    if not session_key:
        return None

    return ExtractedCookies(
        session_key=session_key,
        extra_cookies=extras,
        source="manual",
    )


def try_auto_extract() -> ExtractedCookies | None:
    """Try all automatic extraction methods in order."""
    # 1. Firefox
    result = extract_from_firefox()
    if result:
        return result

    # Future: add more browsers here

    return None
