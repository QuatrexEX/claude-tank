"""Claude Tank — Dynamic tray icon renderer using Pillow."""

from __future__ import annotations

from PIL import Image, ImageDraw, ImageFont


def _status_color(pct: float, is_remaining: bool) -> str:
    """Return color based on percentage and gauge mode."""
    if is_remaining:
        if pct > 50:
            return "#22c55e"  # green
        if pct > 20:
            return "#eab308"  # yellow
        if pct > 5:
            return "#f97316"  # orange
        return "#ef4444"      # red
    else:
        if pct < 50:
            return "#22c55e"
        if pct < 80:
            return "#eab308"
        if pct < 95:
            return "#f97316"
        return "#ef4444"


def render_tank_icon(
    utilization: float,
    gauge_mode: str = "remaining",
    size: int = 64,
) -> Image.Image:
    """Render a tank-shaped icon with a fill level indicator.

    Args:
        utilization: 0-100, the consumed percentage.
        gauge_mode: "remaining" or "consumed".
        size: Icon pixel size.
    """
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    pct = utilization
    if gauge_mode == "remaining":
        fill_pct = 100.0 - pct
        display_pct = fill_pct
    else:
        fill_pct = pct
        display_pct = pct

    color = _status_color(display_pct if gauge_mode == "remaining" else pct,
                          gauge_mode == "remaining")

    margin = size // 8
    tank_left = margin
    tank_right = size - margin
    tank_top = margin + size // 8
    tank_bottom = size - margin

    # Tank neck
    neck_w = size // 3
    neck_left = (size - neck_w) // 2
    draw.rectangle(
        [neck_left, margin, neck_left + neck_w, tank_top + 2],
        outline="#888888", fill="#555555", width=1,
    )

    # Tank body outline
    draw.rounded_rectangle(
        [tank_left, tank_top, tank_right, tank_bottom],
        radius=size // 10,
        outline="#aaaaaa",
        width=2,
    )

    # Fill level
    fill_height = int((tank_bottom - tank_top - 4) * fill_pct / 100.0)
    if fill_height > 0:
        fill_top = tank_bottom - 2 - fill_height
        draw.rounded_rectangle(
            [tank_left + 2, fill_top, tank_right - 2, tank_bottom - 2],
            radius=max(1, size // 14),
            fill=color,
        )

    # Percentage text
    try:
        font = ImageFont.truetype("arial.ttf", size // 4)
    except (OSError, IOError):
        font = ImageFont.load_default()

    text = f"{int(display_pct)}"
    bbox = draw.textbbox((0, 0), text, font=font)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    tx = (size - tw) // 2
    ty = tank_top + (tank_bottom - tank_top - th) // 2
    draw.text((tx, ty), text, fill="white", font=font)

    return img


def render_offline_icon(size: int = 64) -> Image.Image:
    """Render a grayed-out tank icon for offline/error state."""
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    margin = size // 8
    tank_top = margin + size // 8
    tank_bottom = size - margin
    tank_left = margin
    tank_right = size - margin

    neck_w = size // 3
    neck_left = (size - neck_w) // 2
    draw.rectangle(
        [neck_left, margin, neck_left + neck_w, tank_top + 2],
        outline="#555555", fill="#333333", width=1,
    )
    draw.rounded_rectangle(
        [tank_left, tank_top, tank_right, tank_bottom],
        radius=size // 10,
        outline="#555555",
        width=2,
    )

    try:
        font = ImageFont.truetype("arial.ttf", size // 4)
    except (OSError, IOError):
        font = ImageFont.load_default()
    draw.text(
        (size // 2 - size // 8, tank_top + (tank_bottom - tank_top) // 3),
        "?", fill="#777777", font=font,
    )
    return img
