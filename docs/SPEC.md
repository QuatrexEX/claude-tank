# Claude Tank 仕様書 v1.0

> Quatrex — Your Claude Plan Usage Monitor for Windows

---

## 1. 概要

Claude Tank は、Anthropic Claude の有料プラン（Pro / Max / Team / Enterprise）の
使用量制限をリアルタイムで Windows タスクバー（システムトレイ）に表示する常駐アプリケーションである。

### 1.1 解決する課題

- Claude の使用量制限はWeb版で確認しづらく、突然の制限到達に気付けない
- macOS 向けツールは存在するが、Windows 向けが皆無
- Web版/デスクトップ版/Claude Code で共通のプラン制限を一元監視したい

### 1.2 ターゲットユーザー

- Claude Pro/Max プランの個人ユーザー
- Claude Team/Enterprise のメンバー
- Claude Code を日常的に使用する開発者

### 1.3 プロダクト名

- **正式名称**: Claude Tank
- **リポジトリ**: `claude-tank`
- **exe 名**: `claude-tank.exe`
- **タグライン**: "Your Claude plan usage monitor"
- **コンセプト**: 酸素ボンベの残量計。Claude は開発者の酸素。

---

## 2. 機能要件

### 2.1 初回起動フロー

```
┌──────────────────────────────────────────────────┐
│  🫧 Claude Tank                                  │
│  by Quatrex                                      │
│──────────────────────────────────────────────────│
│                                                  │
│  Claude の使用量をタスクバーに常時表示します。       │
│                                                  │
│  セットアップ:                                     │
│  1. 下のボタンで claude.ai にログイン               │
│  2. ブラウザの開発者ツール (F12) を開く              │
│  3. Application → Cookies → claude.ai             │
│  4. 「sessionKey」の値をコピー                      │
│  5. 下の入力欄に貼り付け                            │
│                                                  │
│  [claude.ai を開く]  ← ボタン押下でブラウザ起動       │
│                                                  │
│  Session Key: [_____________________________]     │
│                                                  │
│  [接続テスト]  [保存して開始]                         │
│──────────────────────────────────────────────────│
│  🔒 Session Key はローカルに暗号化保存されます       │
└──────────────────────────────────────────────────┘
```

- 起動時に `https://claude.ai` を既定ブラウザで自動的に開く
- sessionKey の入力を受け付ける
- 「接続テスト」ボタンで API 疎通を確認
- 成功時に自動的にプラン種別を検出し表示
- sessionKey は Windows DPAPI で暗号化してローカル保存

### 2.2 プラン自動検出

`/api/organizations/{orgId}/usage` のレスポンス構造からプラン種別を自動判定する。

| プラン | 判定基準（想定） |
|--------|-----------------|
| Pro | `five_hour` + `seven_day` が存在、`extra_usage.is_enabled` = false |
| Max (5x) | 上記 + 制限値が Pro の約5倍 |
| Max (20x) | 上記 + 制限値が Pro の約20倍 |
| Team | organization に複数メンバー |
| Enterprise | managed organization フラグ |

※ レスポンスの `utilization` はパーセンテージ（0〜100）。
  プラン種別の正確な判定ロジックは実装時に API レスポンスを見て調整する。

### 2.3 タスクバーウィジェット（メイン表示）

天気ウィジェットやシステムモニター（TrafficMonitor 等）のように、
**タスクバー上に直接ゲージを表示**する常駐ウィジェット。

#### 実装方式

タスクバー領域に埋め込むボーダーレスウィンドウ（AppBar 方式）。
Win32 API `SHAppBarMessage` でタスクバーにドッキングする。

```
 Windows タスクバー
┌──────────────────────────────────────────────────────────────┐
│ [スタート] [アプリ...]          [Claude Tank ウィジェット] [🔔] [時計] │
│                                 ← ここに常駐              │
└──────────────────────────────────────────────────────────────┘
```

フォールバック: AppBar 埋め込みに失敗した場合、タスクバー上端にドッキングする
ボーダーレスの小窓（always-on-top）で代替する。

#### コンパクト表示（通常時）

ユーザーが選択した表示項目のみを横並びで表示する。
高さはタスクバーに合わせ、幅は表示項目数に応じて可変。

```
表示パターン例（表示項目の選択に応じて変化）:

A) 5h のみ:
  ┌─────────────┐
  │ 5h ██░░ 58% │
  └─────────────┘

B) 5h + 7d:
  ┌───────────────────────────┐
  │ 5h ██░░ 58%  7d ██░░ 35% │
  └───────────────────────────┘

C) 全項目:
  ┌─────────────────────────────────────────────────┐
  │ 5h ██░░ 58%  7d ██░░ 35%  Op 0%  So 8%  ⏱ 2:14 │
  └─────────────────────────────────────────────────┘
```

#### 表示項目の選択

右クリックメニューまたは設定画面から ON/OFF 可能:

| 項目 | デフォルト | 説明 |
|------|-----------|------|
| 5h ゲージ | ON | 5時間制限のミニゲージ + % |
| 7d ゲージ | ON | 7日間制限のミニゲージ + % |
| Opus 使用率 | OFF | Opus モデル個別 % |
| Sonnet 使用率 | OFF | Sonnet モデル個別 % |
| リセットタイマー | OFF | 次回リセットまでのカウントダウン |
| プラン名 | OFF | "Max" 等の短縮表示 |

#### ゲージモード（消費 or 残量）

ユーザーが **どちらか一方** を選択する。両方同時表示はしない。

| モード | 表示 | ゲージの動き | 色の変化 |
|--------|------|-------------|---------|
| **消費モード** (CONSUMED) | "58% used" | 0% → 100% へ増加 | 青/シアン → 橙 → 赤（増えるほど危険色） |
| **残量モード** (REMAINING) | "42% left" | 100% → 0% へ減少 | 緑/シアン → 黄 → 赤（減るほど危険色） |

デフォルト: **残量モード**（タンクの残量を見る = 本ツールのコンセプトに合致）

#### ホバー時の詳細パネル

タスクバーウィジェット上にマウスホバーすると、
上方向にポップアップパネルが展開して詳細情報を表示する。

```
  ┌─────────────────────────────────┐
  │  Claude Tank — Max Plan         │
  │─────────────────────────────────│
  │                                 │
  │  5-Hour Limit                   │
  │  ████████████░░░░░░░░░  58%     │
  │  ⏱ Reset in 2h 14m              │
  │                                 │
  │  7-Day Limit                    │
  │  ████░░░░░░░░░░░░░░░░  35%     │
  │  ⏱ Reset in 4d 6h               │
  │                                 │
  │  Opus    ░░░░░░░░░░░░░░  0%    │
  │  Sonnet  █░░░░░░░░░░░░░  8%    │
  │                                 │
  │  Extra Usage: $12.50 / $100     │
  │─────────────────────────────────│
  │  Last updated: 14:28            │
  └─────────────────────────────────┘
  ┌───────────────────────────────┐   ← タスクバー上のウィジェット
  │ 5h ██░░ 42% left  7d 65% left│
  └───────────────────────────────┘
```

パネルの動作:
- ホバー開始: 200ms 遅延後にスライドアップで展開
- ホバー離脱: 300ms 遅延後にフェードアウトで閉じる
- パネル内にマウスが入っている間は閉じない
- テーマに応じたスタイルで描画

#### 右クリックメニュー

```
├── 今すぐ更新
├── ─────────────
├── ゲージモード →  ├── ● 残量表示 (REMAINING)
│                   └── ○ 消費表示 (CONSUMED)
├── 表示項目 →      ├── ☑ 5h ゲージ
│                   ├── ☑ 7d ゲージ
│                   ├── ☐ Opus 使用率
│                   ├── ☐ Sonnet 使用率
│                   ├── ☐ リセットタイマー
│                   └── ☐ プラン名
├── テーマ変更 →    ├── Cyber
│                   ├── Minimal
│                   ├── Kawaii
│                   ├── Neon
│                   └── Classic
├── 更新間隔 →      ├── 1分
│                   ├── 3分（デフォルト）
│                   ├── 5分
│                   └── 10分
├── 言語 →          ├── English
│                   ├── 日本語
│                   ├── Deutsch
│                   ├── 한국어
│                   └── Français
├── ─────────────
├── Session Key を更新
├── Windows 起動時に自動起動
├── ─────────────
├── Quatrex / Claude Tank v1.0
└── 終了
```

### 2.4 システムトレイアイコン（補助）

タスクバーウィジェットと併用。通知とメニューの起点。

- 動的生成アイコン: タンク型。液面の高さ = 残量（残量モード時）or 充填量（消費モード時）
- カラーコード（残量モード基準）:
  - 🟢 緑: 残量 50%〜100% — 余裕あり
  - 🟡 黄: 残量 20%〜50% — 注意
  - 🟠 橙: 残量 5%〜20% — 警告
  - 🔴 赤: 残量 0%〜5% — 制限間近 / 到達

### 2.5 ダッシュボードウィンドウ

トレイアイコンのダブルクリック or メニューから開く。
pywebview で組み込みブラウザウィンドウとして表示（ウィンドウサイズ: 480×720px）。

#### 表示内容

1. **プラン情報ヘッダー**: プラン名、アカウント情報
2. **ゲージ（5h）**: 選択モードのゲージ + リセットカウントダウン
3. **ゲージ（7d）**: 同上
4. **モデル別使用量**: Opus / Sonnet の個別表示（存在する場合）
5. **Extra Usage**: 有効な場合のみ表示（月額上限、使用額）
6. **更新履歴グラフ**: 直近24時間の使用量推移（簡易折れ線）
7. **設定パネル**: ゲージモード切替、表示項目、テーマ、言語

### 2.6 テーマシステム

5種類のテーマを CSS で切り替え可能にする。
テーマはタスクバーウィジェット・ホバーパネル・ダッシュボードすべてに適用される。

#### Theme 1: Cyber（デフォルト）

```
背景:       #0a0a0f（ほぼ黒）
アクセント:  #00ffcc（シアン）、#ff00ff（マゼンタ）
フォント:    monospace 系
ゲージ:      グラデーション付きネオンバー、グロウエフェクト
演出:        スキャンライン、微細なノイズテクスチャ
タンク感:    ホログラフィック液面、デジタルグリッド
```

#### Theme 2: Minimal

```
背景:       #ffffff / #1a1a1a（ライト/ダーク自動切替）
アクセント:  #2563eb（ブルー）
フォント:    system-ui
ゲージ:      フラットな角丸バー、影なし
演出:        なし。情報密度重視
タンク感:    クリーンなプログレスバーのみ
```

#### Theme 3: Kawaii

```
背景:       #fff0f5（ラベンダーブラッシュ）
アクセント:  #ff69b4（ホットピンク）、#87ceeb（スカイブルー）
フォント:    丸ゴシック系
ゲージ:      丸みのあるパステルバー
演出:        使用率に応じた表情アイコン (◉‿◉) → (◉_◉) → (◉︵◉)
タンク感:    かわいいボトル型。泡がぷくぷく
```

#### Theme 4: Neon

```
背景:       #0d0221（ダークパープル）
アクセント:  #f72585 / #4cc9f0 / #7209b7
フォント:    condensed sans
ゲージ:      アニメーションするネオンストライプ
演出:        パルスアニメーション、ボーダーグロウ
タンク感:    電光サイン風。ちらつき
```

#### Theme 5: Classic

```
背景:       #f5f5f0（オフホワイト）
アクセント:  #d97706（アンバー）/ #059669（エメラルド）
フォント:    serif 系
ゲージ:      アナログゲージ風の半円メーター
演出:        レトロ計器風デザイン
タンク感:    真鍮のアナログ圧力計
```

---

## 3. 非機能要件

### 3.1 パフォーマンス

- メモリ使用量: 常駐時 50MB 以下
- CPU 使用率: アイドル時 0.1% 以下
- API ポーリング間隔: デフォルト 3分（設定可能: 1/3/5/10分）
- exe 起動時間: 3秒以内

### 3.2 セキュリティ

- sessionKey は Windows DPAPI（`win32crypt.CryptProtectData`）で暗号化
- 暗号化データはユーザープロファイル内に保存: `%APPDATA%/Quatrex/claude-tank/`
- sessionKey は平文でディスクに書かない
- ログに sessionKey を出力しない
- 通信は HTTPS のみ

### 3.3 互換性

- Windows 10 Version 1903 以降
- Windows 11 全バージョン
- ディスプレイスケーリング: 100%〜200% 対応（DPI Aware）
- マルチモニター対応

### 3.4 配布

- 単一 exe ファイル（PyInstaller --onefile）
- インストーラー不要
- 署名なし（個人プロジェクト）
- SmartScreen 警告は初回のみ許容

---

## 4. アーキテクチャ

### 4.1 全体構成

```
claude-tank.exe (PyInstaller onefile)
├── core/
│   ├── api_client.py      — claude.ai API 通信
│   ├── auth.py            — sessionKey 管理（暗号化/復号）
│   ├── plan_detector.py   — プラン種別自動判定
│   ├── poller.py          — 定期ポーリング（スレッド）
│   └── config.py          — 設定管理（JSON）
├── tray/
│   ├── tray_app.py        — pystray メインループ
│   ├── icon_renderer.py   — 動的タンクアイコン生成（Pillow）
│   └── tooltip.py         — ツールチップ文字列生成
├── widget/
│   ├── taskbar_widget.py  — タスクバー常駐ウィジェット（Win32 AppBar）
│   ├── hover_panel.py     — ホバー時ポップアップパネル
│   └── widget_renderer.py — ミニゲージ描画（GDI+/Pillow）
├── ui/
│   ├── dashboard.py       — pywebview ウィンドウ管理
│   ├── setup_wizard.py    — 初回セットアップ画面
│   └── web/               — 組み込み HTML/CSS/JS
│       ├── index.html     — ダッシュボード
│       ├── setup.html     — セットアップウィザード
│       ├── hover.html     — ホバー詳細パネル
│       ├── css/
│       │   ├── base.css
│       │   ├── widget.css       — タスクバーウィジェット・ホバーパネル
│       │   ├── gauges.css       — ゲージ共通スタイル
│       │   ├── theme-cyber.css
│       │   ├── theme-minimal.css
│       │   ├── theme-kawaii.css
│       │   ├── theme-neon.css
│       │   └── theme-classic.css
│       └── js/
│           ├── app.js       — メインロジック
│           ├── gauges.js    — ゲージ描画・アニメーション
│           ├── hover.js     — ホバーパネル制御
│           ├── charts.js    — 使用量推移グラフ描画
│           └── themes.js    — テーマ切替
├── locales/
│   ├── en.json            — English
│   ├── ja.json            — 日本語
│   ├── de.json            — Deutsch
│   ├── ko.json            — 한국어
│   └── fr.json            — Français
├── assets/
│   └── icon.ico           — ベースタンクアイコン
└── main.py                — エントリーポイント
```

### 4.2 データフロー

```
[claude.ai API]
       │
       ▼ HTTPS (3分間隔)
[api_client.py] ──→ レスポンス解析 ──→ UsageData 生成
       │
       ├──→ [widget] タスクバーウィジェットのミニゲージ再描画
       │         │
       │         └──→ [hover_panel] ホバー時に詳細パネルへデータ渡し
       │
       ├──→ [tray] タンクアイコン液面更新
       │
       └──→ [dashboard] pywebview に JS イベント発火
                │
                ├──→ ゲージ更新（選択モードに応じて消費 or 残量）
                └──→ 推移グラフにデータポイント追加
```

### 4.3 設定ファイル

保存先: `%APPDATA%/Quatrex/claude-tank/config.json`

```json
{
  "version": 1,
  "theme": "cyber",
  "locale": "auto",
  "gauge_mode": "remaining",
  "poll_interval_sec": 180,
  "auto_start": false,
  "window_size": [480, 720],
  "notification_threshold": 80,
  "widget": {
    "show_5h": true,
    "show_7d": true,
    "show_opus": false,
    "show_sonnet": false,
    "show_reset_timer": false,
    "show_plan_name": false
  }
}
```

### 4.4 多言語化（i18n）

OS の `GetUserDefaultUILanguage` から言語を自動検出。
`locale: "auto"` の場合、以下の優先順で決定:

1. OS 言語が対応言語に一致 → その言語
2. 一致しない → English（フォールバック）

ロケールファイル例（`locales/ja.json`）:
```json
{
  "app_name": "Claude Tank",
  "setup_title": "セットアップ",
  "setup_desc": "Claude の使用量をタスクバーに常時表示します。",
  "open_claude": "claude.ai を開く",
  "session_key": "Session Key",
  "test_connection": "接続テスト",
  "save_and_start": "保存して開始",
  "consumed": "消費量",
  "remaining": "残量",
  "reset_in": "リセットまで",
  "plan_pro": "Pro プラン",
  "plan_max": "Max プラン",
  "theme_cyber": "Cyber",
  "theme_minimal": "Minimal",
  "theme_kawaii": "Kawaii",
  "theme_neon": "Neon",
  "theme_classic": "Classic",
  "menu_dashboard": "ダッシュボードを開く",
  "menu_refresh": "今すぐ更新",
  "menu_theme": "テーマ変更",
  "menu_interval": "更新間隔",
  "menu_language": "言語",
  "menu_update_key": "Session Key を更新",
  "menu_autostart": "Windows 起動時に自動起動",
  "menu_quit": "終了",
  "notify_threshold": "使用量が {percent}% に達しました",
  "notify_expired": "Session Key の有効期限が切れました",
  "notify_reset": "使用量がリセットされました"
}
```

---

## 5. API 通信仕様

### 5.1 認証フロー

```
1. ユーザーが sessionKey を入力（例: sk-ant-sid01-xxxxx）
2. GET https://claude.ai/api/organizations
   Headers:
     Cookie: sessionKey=sk-ant-sid01-xxxxx
   → レスポンスから organizations[0].uuid を取得 → orgId
   → organizations[0] からプラン情報を取得
3. orgId + sessionKey を暗号化保存
```

### 5.2 使用量取得

```
GET https://claude.ai/api/organizations/{orgId}/usage
Headers:
  Cookie: sessionKey=sk-ant-sid01-xxxxx
  Accept: application/json
  User-Agent: claude-tank/1.0 (Quatrex)
```

想定レスポンス（実装時に要確認）:
```json
{
  "five_hour": {
    "utilization": 58.0,
    "resets_at": "2025-11-04T14:30:00Z"
  },
  "seven_day": {
    "utilization": 35.0,
    "resets_at": "2025-11-10T00:00:00Z"
  },
  "seven_day_opus": {
    "utilization": 0.0,
    "resets_at": null
  },
  "seven_day_sonnet": {
    "utilization": 8.0,
    "resets_at": "2025-11-10T00:00:00Z"
  },
  "extra_usage": {
    "is_enabled": true,
    "monthly_limit": 100.0,
    "used_credits": 12.50,
    "utilization": 12.5
  }
}
```

### 5.3 エラーハンドリング

| HTTP Status | 意味 | 対応 |
|-------------|------|------|
| 200 | 成功 | データ更新 |
| 401 | sessionKey 無効/期限切れ | トレイ通知 + 再設定促す |
| 403 | アクセス拒否 | トレイ通知 |
| 429 | レート制限 | ポーリング間隔を一時的に2倍に |
| 5xx | サーバーエラー | 次回ポーリングまで待機 |
| Network Error | 接続不可 | オフラインアイコン表示、リトライ継続 |

### 5.4 sessionKey の有効期限

- sessionKey はブラウザセッションに依存（数日〜数週間）
- 期限切れ時はトレイアイコンが灰色に変わり、通知で再設定を促す
- ダッシュボードに「Session Key を更新」ボタンを常設

---

## 6. 通知

| イベント | 通知方法 |
|---------|---------|
| 使用量が閾値超過（デフォルト80%） | Windows トースト通知 |
| 使用量リセット | トースト通知（オプション） |
| sessionKey 期限切れ | トースト通知 + タンクアイコン灰色化 |
| API 接続エラー（3回連続） | トースト通知 |

---

## 7. 開発ロードマップ

### v1.0（MVP）

- [x] CLAUDE.md 作成
- [x] 仕様書作成
- [x] GitHub リポジトリ作成（private）
- [ ] プロジェクトスキャフォールド
- [ ] sessionKey 暗号化保存
- [ ] claude.ai API クライアント
- [ ] プラン自動検出
- [ ] タスクバーウィジェット（ミニゲージ常駐表示）
- [ ] ホバー詳細パネル
- [ ] ゲージモード切替（消費 / 残量）
- [ ] 表示項目の選択 UI
- [ ] システムトレイ常駐（動的タンクアイコン）
- [ ] ダッシュボード UI（Cyber テーマ）
- [ ] 初回セットアップウィザード
- [ ] 多言語化基盤（English + Japanese）
- [ ] PyInstaller ビルド → 単一 exe

### v1.1

- [ ] 残り4テーマ追加（Minimal, Kawaii, Neon, Classic）
- [ ] 使用量推移グラフ
- [ ] Windows 起動時自動起動
- [ ] ポーリング間隔変更 UI

### v1.2

- [ ] 多言語化完了（German, Korean, French 追加）
- [ ] OAuth エンドポイント対応（Claude Code トークン連携）
- [ ] SSE ストリームからの精密 utilization 値取得

### v2.0（将来）

- [ ] 複数アカウント対応
- [ ] Rust/Tauri への移植（exe サイズ最適化）
- [ ] GitHub Releases で自動配布
- [ ] 公開（private → public）

---

## 8. 依存ライブラリ

| パッケージ | 用途 | ライセンス |
|-----------|------|-----------|
| pystray | システムトレイ | MIT/LGPL |
| Pillow | タンクアイコン動的生成 | HPND |
| pywebview | ダッシュボード GUI | BSD-3 |
| requests | HTTP 通信 | Apache 2.0 |
| pywin32 | DPAPI 暗号化、自動起動 | PSF |
| pyinstaller | exe ビルド（dev-only） | GPL (bootloader: Apache) |

---

*Claude Tank — Your Claude plan usage monitor. By Quatrex.*
