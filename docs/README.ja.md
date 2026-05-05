# Claude Tank

<img src="img/icon.png" alt="Claude Tank Icon" width="64">

> Claude プラン使用量モニター for Windows

**🌐 言語:** [English](../README.md) | 日本語 | [Deutsch](README.de.md) | [한국어](README.ko.md) | [Français](README.fr.md)

---

**Claude Tank** は、Claude の Pro/Max/Team プランの使用量制限を Windows システムトレイでリアルタイム監視する軽量アプリです。

もう突然の制限到達に驚く必要はありません。5時間制限・7日間制限の残量が常に見えます。

## 機能

- **デュアルゲージ トレイアイコン** — 5h と 7d の残量をカラーバーで表示
- **サイバーテーマ ダッシュボード** — トレイクリックでタンク型ゲージを表示
- **自動ログイン** — 内蔵ブラウザで claude.ai にログインするだけ。Cookie 自動検出
- **バックグラウンドポーリング** — 1/3/5分間隔で使用量を自動更新（設定可能）
- **しきい値アラート** — 5h と 7d 個別に、残量が設定値を下回ったら警告
- **暗号化保存** — セッションキーを Windows DPAPI で暗号化
- **自動起動** — Windows ログイン時に自動起動（オプション）
- **多言語対応** — 英語、日本語、ドイツ語、韓国語、フランス語（OS 言語を自動検出）
- **軽量** — exe 約 2MB、メモリ約 20MB、インストーラー不要

## 動作要件

- Windows 10 (1903+) または Windows 11
- [Microsoft Edge WebView2 ランタイム](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) — Windows 11 では通常プリインストール済みですが、必ずあるとは限りません。起動に失敗する場合はリンク先の **Evergreen Standalone Installer** をインストールしてください。

## 使い方

1. [Releases](https://github.com/QuatrexEX/claude-tank/releases) から `claude-tank.exe` をダウンロード
2. 実行 → claude.ai のログイン画面が開く
3. 普通にログイン（Google、メール、SSO）
4. 完了！ トレイアイコンに使用量ゲージが表示されます

## 仕組み

Claude Tank は [claude-counter](https://github.com/she-llac/claude-counter) や [Usage4Claude](https://github.com/f-is-h/Usage4Claude) と同じ claude.ai 内部 API を使用します:

```
GET https://claude.ai/api/organizations/{orgId}/usage
```

セッション Cookie は WebView2 ウィンドウ経由で取得します（macOS の Usage4Claude と同じ方式）。

## ソースからビルド

必要: [Rust](https://rustup.rs/)（stable）

```bash
cargo build --release
# 出力: target/release/claude-tank.exe（約 2MB）
```

## ライセンス

[MIT](../LICENSE)

## 作者

**Quatrex** — [@QuatrexEX](https://github.com/QuatrexEX)
