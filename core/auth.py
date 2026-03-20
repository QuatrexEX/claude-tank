"""Claude Tank — Session key encryption via Windows DPAPI."""

from __future__ import annotations

import json
import base64
from pathlib import Path

from core.config import app_dir

_CRED_FILE = "credentials.dat"


def _dpapi_encrypt(data: bytes) -> bytes:
    import win32crypt  # type: ignore[import-untyped]
    _, encrypted = win32crypt.CryptProtectData(
        data, "ClaudeTank", None, None, None, 0
    )
    return encrypted


def _dpapi_decrypt(data: bytes) -> bytes:
    import win32crypt  # type: ignore[import-untyped]
    _, decrypted = win32crypt.CryptUnprotectData(data, None, None, None, 0)
    return decrypted


def save_credentials(session_key: str, org_id: str) -> None:
    payload = json.dumps({
        "session_key": session_key,
        "org_id": org_id,
    }).encode("utf-8")
    encrypted = _dpapi_encrypt(payload)
    path = app_dir() / _CRED_FILE
    path.write_bytes(base64.b64encode(encrypted))


def load_credentials() -> tuple[str, str] | None:
    path = app_dir() / _CRED_FILE
    if not path.exists():
        return None
    try:
        encrypted = base64.b64decode(path.read_bytes())
        decrypted = _dpapi_decrypt(encrypted)
        data = json.loads(decrypted.decode("utf-8"))
        return data["session_key"], data["org_id"]
    except Exception:
        return None


def clear_credentials() -> None:
    path = app_dir() / _CRED_FILE
    if path.exists():
        path.unlink()


def has_credentials() -> bool:
    return (app_dir() / _CRED_FILE).exists()
