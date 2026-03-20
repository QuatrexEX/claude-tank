# Claude Tank — Your Claude Plan Usage Monitor for Windows

Quatrex 名義の Windows システムトレイ常駐アプリ。
Claude の Pro/Max/Team プランの使用量（5時間制限・7日制限）をリアルタイムでタスクバーに表示する。

**GitHub**: https://github.com/QuatrexEX/claude-tank (private)

## コンセプト

「酸素ボンベの残量計」——Claude はもはや開発者にとって酸素。
タンクの残量が常に見えていれば、安心して呼吸できる。

### ゲージ設計（消費 or 残量の選択式）

ユーザーがどちらか一方のモードを選択する:

- **残量モード（デフォルト）**: 100% → 0% へ減少。タンクが空になるイメージ。緑→黄→赤
- **消費モード**: 0% → 100% へ増加。火が燃え広がるイメージ。青→橙→赤

### タスクバーウィジェット

天気バーのようにタスクバー上に直接常駐。ミニゲージ + 数値をコンパクトに表示。
表示項目（5h, 7d, Opus, Sonnet 等）はユーザーが選択可能。
ホバー時にポップアップパネルが展開し、全項目の詳細を表示する。

## プロジェクト背景

- Anthropic はプラン使用量を取得する**公式 API を提供していない**
- macOS 向けメニューバーアプリやブラウザ拡張は複数存在するが、**Windows システムトレイ向けは皆無**
- ここが Claude Tank の存在意義

## API 調査結果（2025年時点）

### エンドポイント 1: claude.ai 内部 API（推奨・メイン使用）

```
GET https://claude.ai/api/organizations/{orgId}/usage
```

- 認証: `sessionKey` Cookie（ブラウザからコピー）
- Org ID: `lastActiveOrg` Cookie から取得可能
- 複数のブラウザ拡張（claude-counter, claude-usage-monitor 等）が実際に使用中
- レート制限は OAuth エンドポイントより緩い

### エンドポイント 2: OAuth API（Claude Code 向け）

```
GET https://api.anthropic.com/api/oauth/usage
Authorization: Bearer sk-ant-oat01-[TOKEN]
anthropic-beta: oauth-2025-04-20
```

レスポンス例:
```json
{
  "five_hour": { "utilization": 6.0, "resets_at": "2025-11-04T04:59:59Z" },
  "seven_day": { "utilization": 35.0, "resets_at": "2025-11-06T03:59:59Z" },
  "seven_day_opus": { "utilization": 0.0, "resets_at": null },
  "seven_day_sonnet": { "utilization": 1.0, "resets_at": "..." },
  "extra_usage": { "is_enabled": false, "monthly_limit": null }
}
```

- **注意**: 約5回で 429 エラー。リフレッシュトークンでの再取得が必要
- リフレッシュ: `POST https://console.anthropic.com/v1/oauth/token` (client_id: `9d1c250a-e61b-44d9-88ed-5944d1962f5e`)

### 既存ツール一覧（競合調査）

| ツール | 対応 OS | 方式 | 備考 |
|--------|---------|------|------|
| Usage4Claude | macOS | sessionKey Cookie | Swift/SwiftUI |
| claude-counter | ブラウザ拡張 | Cookie + SSE | 精密な utilization 値 |
| claude-usage-monitor (oov) | ブラウザ拡張 | Cookie | Chrome/Firefox/Edge |
| Claude-Usage-Tracker | macOS | sessionKey Cookie | Swift |
| claude-monitor | macOS | sessionKey Cookie | メニューバー |
| **Windows 向けツール** | **なし** | — | **← Claude Tank が埋める** |

## 技術選定

- **言語**: Python 3.11+
- **パッケージング**: PyInstaller（--onefile で単一 exe）
- **システムトレイ**: pystray + Pillow（動的アイコン生成）
- **ダッシュボード UI**: pywebview（組み込み HTML/CSS/JS）
- **API 通信**: requests
- **テーマエンジン**: CSS 変数 + テーマ別スタイルシート

## コーディング規約

- Python: PEP 8 準拠、型ヒント必須
- HTML/CSS/JS: ES2020+、モジュール構成
- exe サイズ: 50MB 以下を目標
- Windows 10 (1903+) / Windows 11 対応

## 多言語化（i18n）

Claude 利用上位5カ国をターゲット:

| 順位 | 国 | 言語 | ロケール |
|------|----|------|---------|
| 1 | アメリカ / イギリス | English | en |
| 2 | 日本 | 日本語 | ja |
| 3 | ドイツ | Deutsch | de |
| 4 | 韓国 | 한국어 | ko |
| 5 | フランス | Français | fr |

- UI テキストは JSON ファイルで管理（`locales/{lang}.json`）
- OS の言語設定を自動検出、フォールバックは English
- v1.0 は English + Japanese、v1.2 以降で全言語対応
