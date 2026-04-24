# Conduction

> Conduct your mix, don't perform it.

プロのライブ現場で使う、プログラマブルで拡張可能な DJ エンジン。
人間の UI 操作・AI・プラグイン・スクリプトが、同じ Conductor 層に対する "指示" として等価に扱われる。

## ステータス

Phase 1 着手中。詳細は [`conduction-requirements.md`](./conduction-requirements.md) の「10. 実装ロードマップ」を参照。

## リポジトリ構成

```
conduction/
├── Cargo.toml                # Rust ワークスペース定義
├── crates/
│   ├── conduction-core/      # データモデル（Track / Cue / Template / Deck）
│   ├── conduction-audio/     # オーディオエンジン（cpal）
│   ├── conduction-analysis/  # BPM / キー検出、楽曲解析
│   ├── conduction-conductor/ # 司令塔（テンプレート実行、Cue マッチング）
│   ├── conduction-library/   # ライブラリ管理、SQLite
│   ├── conduction-claude/    # Claude CLI 連携 [feature: claude-analysis]
│   ├── conduction-download/  # yt-dlp 連携 [feature: yt-download]
│   ├── conduction-script/    # Script API（WebSocket / OSC + Lua）
│   └── conduction-app/       # Tauri アプリ本体
└── ui/
    ├── design-system/        # CSS 変数、アセット、プレビュー
    └── src/                  # React + TypeScript
```

## ビルド

```bash
# 配布版
cargo build --release

# 個人開発ビルド
cargo build --release --features yt-download
```

## ドキュメント

- [要件定義書](./conduction-requirements.md)
- [デザインシステム](./ui/design-system/DESIGN_SYSTEM.md)
