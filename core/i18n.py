"""Claude Tank — Internationalization."""

from __future__ import annotations

import ctypes
import json
from pathlib import Path
from typing import Any

_LOCALES_DIR = Path(__file__).resolve().parent.parent / "locales"
_SUPPORTED = ("en", "ja", "de", "ko", "fr")
_strings: dict[str, str] = {}
_current_locale: str = "en"


def _detect_os_language() -> str:
    try:
        lang_id = ctypes.windll.kernel32.GetUserDefaultUILanguage()  # type: ignore[union-attr]
        primary = lang_id & 0x3FF
        mapping = {
            0x09: "en",  # English
            0x11: "ja",  # Japanese
            0x07: "de",  # German
            0x12: "ko",  # Korean
            0x0C: "fr",  # French
        }
        return mapping.get(primary, "en")
    except Exception:
        return "en"


def init(locale: str = "auto") -> str:
    global _strings, _current_locale
    if locale == "auto":
        locale = _detect_os_language()
    if locale not in _SUPPORTED:
        locale = "en"
    _current_locale = locale
    path = _LOCALES_DIR / f"{locale}.json"
    if not path.exists():
        path = _LOCALES_DIR / "en.json"
    try:
        _strings = json.loads(path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, FileNotFoundError):
        _strings = {}
    return _current_locale


def t(key: str, **kwargs: Any) -> str:
    text = _strings.get(key, key)
    if kwargs:
        for k, v in kwargs.items():
            text = text.replace(f"{{{k}}}", str(v))
    return text


def current_locale() -> str:
    return _current_locale
