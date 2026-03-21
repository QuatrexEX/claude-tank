# Claude Tank

> Your Claude plan usage monitor for Windows.

[日本語](#日本語)

---

**Claude Tank** is a lightweight Windows system tray application that monitors your Claude Pro/Max/Team plan usage limits in real-time.

No more surprise rate limits — see your 5-hour and 7-day remaining capacity at a glance.

## Features

- **Dual gauge tray icon** — 5h and 7d remaining shown as color-coded bars (cyan → gold → magenta → red)
- **Cyber-themed dashboard** — click the tray icon for a tank-style gauge popup
- **Auto-login** — log in to claude.ai in the built-in browser, cookies are detected automatically
- **Background polling** — usage updates every 1/3/5 minutes (configurable)
- **Threshold alerts** — independent 5h and 7d alerts when remaining drops below a set level
- **Encrypted credentials** — session key stored with Windows DPAPI
- **Auto-start** — optional launch on Windows startup
- **Multilingual** — English, Japanese, German, Korean, French (auto-detected from OS)
- **Tiny footprint** — ~2MB exe, ~20MB RAM, no installer needed

## Screenshot

![Dashboard](docs/screenshot.png)

## Quick Start

1. Download `claude-tank.exe` from [Releases](https://github.com/QuatrexEX/claude-tank/releases)
2. Run it — a browser window opens for claude.ai login
3. Log in normally (Google, email, SSO)
4. Done! The tray icon appears with your usage gauges

## How It Works

Claude Tank uses the same internal API that browser extensions like [claude-counter](https://github.com/she-llac/claude-counter) and [Usage4Claude](https://github.com/f-is-h/Usage4Claude) use:

```
GET https://claude.ai/api/organizations/{orgId}/usage
```

The session cookie is obtained by opening claude.ai in an embedded WebView2 window (same approach as Usage4Claude on macOS).

## Building from Source

Requirements: [Rust](https://rustup.rs/) (stable)

```bash
cargo build --release
# Output: target/release/claude-tank.exe (~2MB)
```

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Tray icon | [tray-icon](https://crates.io/crates/tray-icon) (pixel-rendered, dynamic) |
| Dashboard | [wry](https://crates.io/crates/wry) (WebView2, HTML/CSS/JS) |
| HTTP client | [ureq](https://crates.io/crates/ureq) |
| Encryption | Windows DPAPI |
| Win32 API | [windows](https://crates.io/crates/windows) crate |

## License

MIT

## Author

**Quatrex** — [@QuatrexEX](https://github.com/QuatrexEX)

---

# 日本語

> Claude プラン使用量モニター for Windows

**Claude Tank** は、Claude の Pro/Max/Team プランの使用量制限を Windows システムトレイでリアルタイム監視する軽量アプリです。

もう突然の制限到達に驚く必要はありません。5時間制限・7日間制限の残量が常に見えます。

## 機能

- **デュアルゲージ トレイアイコン** — 5h と 7d の残量をカラーバーで表示（シアン→ゴールド→マゼンタ→レッド）
- **サイバーテーマ ダッシュボード** — トレイクリックでタンク型ゲージを表示
- **自動ログイン** — 内蔵ブラウザで claude.ai にログインするだけ。Cookie 自動検出
- **バックグラウンドポーリング** — 1/3/5分間隔で使用量を自動更新
- **しきい値アラート** — 5h と 7d 個別に、残量が設定値を下回ったら警告
- **暗号化保存** — セッションキーを Windows DPAPI で暗号化
- **自動起動** — Windows ログイン時に自動起動（オプション）
- **多言語対応** — 英語、日本語、ドイツ語、韓国語、フランス語（OS 言語を自動検出）
- **軽量** — exe 約 2MB、メモリ約 20MB、インストーラー不要

## 使い方

1. [Releases](https://github.com/QuatrexEX/claude-tank/releases) から `claude-tank.exe` をダウンロード
2. 実行 → claude.ai のログイン画面が開く
3. 普通にログイン（Google、メール、SSO）
4. 完了！ トレイアイコンに使用量ゲージが表示されます

## ソースからビルド

必要: [Rust](https://rustup.rs/)（stable）

```bash
cargo build --release
# 出力: target/release/claude-tank.exe（約 2MB）
```

## ライセンス

MIT

## 作者

**Quatrex** — [@QuatrexEX](https://github.com/QuatrexEX)
