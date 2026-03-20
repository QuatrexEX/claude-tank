"""Claude Tank — Configuration management."""

from __future__ import annotations

import json
import os
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Any

APP_NAME = "Claude Tank"
APP_VERSION = "1.0.0"
APP_AUTHOR = "Quatrex"

_APP_DIR = Path(os.environ.get("APPDATA", "~")) / "Quatrex" / "claude-tank"


@dataclass
class WidgetConfig:
    show_5h: bool = True
    show_7d: bool = True
    show_opus: bool = False
    show_sonnet: bool = False
    show_reset_timer: bool = False
    show_plan_name: bool = False


@dataclass
class AppConfig:
    version: int = 1
    theme: str = "cyber"
    locale: str = "auto"
    gauge_mode: str = "remaining"  # "remaining" or "consumed"
    poll_interval_sec: int = 180
    auto_start: bool = False
    notification_threshold: int = 80
    widget: WidgetConfig = field(default_factory=WidgetConfig)

    # Not persisted — runtime only
    _path: Path = field(default=_APP_DIR / "config.json", repr=False)

    @classmethod
    def load(cls) -> AppConfig:
        path = _APP_DIR / "config.json"
        if not path.exists():
            cfg = cls()
            cfg.save()
            return cfg
        try:
            raw = json.loads(path.read_text(encoding="utf-8"))
            widget_raw = raw.pop("widget", {})
            widget = WidgetConfig(**{k: v for k, v in widget_raw.items()
                                     if k in WidgetConfig.__dataclass_fields__})
            cfg = cls(
                **{k: v for k, v in raw.items()
                   if k in cls.__dataclass_fields__ and k != "widget"},
                widget=widget,
            )
            cfg._path = path
            return cfg
        except (json.JSONDecodeError, TypeError):
            cfg = cls()
            cfg.save()
            return cfg

    def save(self) -> None:
        self._path.parent.mkdir(parents=True, exist_ok=True)
        data = asdict(self)
        data.pop("_path", None)
        self._path.write_text(json.dumps(data, indent=2, ensure_ascii=False),
                              encoding="utf-8")

    def update(self, **kwargs: Any) -> None:
        for key, value in kwargs.items():
            if key == "widget" and isinstance(value, dict):
                for wk, wv in value.items():
                    if hasattr(self.widget, wk):
                        setattr(self.widget, wk, wv)
            elif hasattr(self, key) and key != "_path":
                setattr(self, key, value)
        self.save()


def app_dir() -> Path:
    _APP_DIR.mkdir(parents=True, exist_ok=True)
    return _APP_DIR
