# Claude Tank

> Your Claude plan usage monitor for Windows.

**🌐 Language:** English | [日本語](docs/README.ja.md) | [Deutsch](docs/README.de.md) | [한국어](docs/README.ko.md) | [Français](docs/README.fr.md)

---

**Claude Tank** is a lightweight Windows system tray app that monitors your Claude Pro/Max/Team plan usage limits in real-time.

No more surprise rate limits — see your 5-hour and 7-day remaining capacity at a glance.

## Features

- **Dual gauge tray icon** — 5h and 7d remaining shown as color-coded bars
- **Cyber-themed dashboard** — click the tray icon for a tank-style gauge popup
- **Auto-login** — log in to claude.ai in the built-in browser, cookies detected automatically
- **Background polling** — usage updates every 1/3/5 minutes (configurable)
- **Threshold alerts** — independent 5h and 7d alerts when remaining drops below a set level
- **Encrypted credentials** — session key stored with Windows DPAPI
- **Auto-start** — optional launch on Windows startup
- **Multilingual** — English, Japanese, German, Korean, French (auto-detected from OS)
- **Tiny** — ~2MB exe, ~20MB RAM, no installer needed

## Quick Start

1. Download `claude-tank.exe` from [Releases](https://github.com/QuatrexEX/claude-tank/releases)
2. Run it — a browser window opens for claude.ai login
3. Log in normally (Google, email, SSO)
4. Done! The tray icon shows your usage gauges

## How It Works

Claude Tank uses the same internal API as browser extensions like [claude-counter](https://github.com/she-llac/claude-counter) and [Usage4Claude](https://github.com/f-is-h/Usage4Claude):

```
GET https://claude.ai/api/organizations/{orgId}/usage
```

Session cookies are obtained via an embedded WebView2 window — same approach as Usage4Claude on macOS.

## Building from Source

Requirements: [Rust](https://rustup.rs/) (stable)

```bash
cargo build --release
# Output: target/release/claude-tank.exe (~2MB)
```

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Tray icon | [tray-icon](https://crates.io/crates/tray-icon) — pixel-rendered, dynamic |
| Dashboard | [wry](https://crates.io/crates/wry) — WebView2, HTML/CSS/JS |
| HTTP | [ureq](https://crates.io/crates/ureq) |
| Encryption | Windows DPAPI |
| Win32 | [windows](https://crates.io/crates/windows) crate |

## License

[MIT](LICENSE)

## Author

**Quatrex** — [@QuatrexEX](https://github.com/QuatrexEX)
