# Conduction

> Conduct your mix, don't perform it.

**Conduction** はライブ現場で使える、プログラマブルで拡張可能な DJ エンジン。人間の UI 操作・AI 提案・プラグイン・Lua スクリプトを、すべて単一の Conductor 層に対する等価な「指示」として扱う。DJ はノブと格闘するのではなく、セットを *振り付け* できる。

- ライセンス: [MIT](./LICENSE)
- ステータス: in-development (1.0 未満) — 最初の公開ビルドのタグは **`in-dev 1.0`**
- 対応プラットフォーム: macOS (主)、Linux、Windows
- 技術スタック: Rust + Tauri v2 + React 18 + TypeScript
- English: [README.md](./README.md)

---

## なぜ Conduction か

ほとんどの DJ ソフトでは自動化が後付けに見える。Ableton のクリップを書く、MIDI コンに割り当てる、ピアノロールと格闘する。Conduction はこれを反転させる。**すべてのパラメータ変化を等価な「指示」として扱う** — フェーダーを触ったのも、Cue にタグを付けたのも、セットリストのトランジションも、Lua の関数呼び出しも、全部同じ Conductor を通る。だから自動化に任せたり、奪い返したりするのは「文脈の切替」であって、別のワークフローではない。

### コア思想

1. **Conductor は一つ、ソースは多数**。フェーダー / AI / スクリプト / Cue マッチングが同じパラメータバスに書き込む。Override / Resume / Commit は普遍的な動詞。
2. **自動化テンプレートはプログラム**。Visual / Node / Lua の 3 つのエディタは、すべて同じ `Template` AST にコンパイルされる。
3. **Cue は型付き**。`Drop` / `Intro` / `Breakdown` / `Outro` には意味がある。だから「次にここでミックスできる曲」はクエリで、推測ではない。
4. **セットリストはドキュメント**。`.cset` でエクスポートし、共演者と共有し、別マシンで開ける。

---

## 機能

### ミキサー
- 2 デッキ + effective BPM 追跡 + master tempo (keylock) + ±6 / ±10 / ±16 % テンポレンジ
- デッキごとに 3 バンド EQ + フィルタ + FX (リバーブ / ディレイ等)
- クロスフェーダー、マスター、チャンネルボリューム、PFL (CUE バス) ルーティング
- ビートスナップ付きループ、bar 単位の伸縮
- トラックごとに Hot Cue 8 個 + 型付き Cue (Drop / Intro / Breakdown / Outro …)

### ライブラリ
- フォルダスキャンによる取り込み、BPM / Camelot キー / Energy 推定、波形キャッシュ
- SQLite で永続化 (現在 schema v5、自動マイグレーション)
- `MixSuggestion` パネルが、アクティブデッキの典型 Cue に対する BPM / Key / Energy 互換性で次曲候補を提案

### 自動化テンプレート
3 つのエディタが同じ `Template` モデルを操作する:

- **Visual timeline** — ターゲットごとに keyframe をドラッグ。`linear` / `smooth` / `ease-in` / `ease-out` カーブ
- **Node editor** — `react-flow` で `Source → Target` ペアをグラフ化
- **Script (Lua)** — 本物のプログラムを書く。Lua はサンドボックスされた *コードジェネレータ* として扱われ、オーディオスレッドでは絶対に走らない

Lua API (抜粋):

```lua
set_duration(16)                     -- bars
set_direction("a_to_b")              -- もしくは "b_to_a"
keyframe("crossfader", 0,  -1.0, "linear")
keyframe("crossfader", 64, 1.0,  "smooth")

each_bar(16, function(bar, beat)
  local t = bar / 16
  keyframe("deck_eq_low.A", beat,
           lerp(1.0, 0.0, smoothstep(0, 1, t)), "linear")
end)
```

Monaco ベースのエディタには inline エラーマーカー、補完 (Conduction API + Prelude ヘルパ + Lua キーワード + `math.*`)、シンタックスハイライト、スライドインのドキュメントパネルが組み込まれている。

5 つのビルトインプリセット (`linear_16bar`, `crossfade_outro_intro`, `eq_swap`, `filter_sweep`, `build_drop`) はすべて Lua ソース付きで、フォークして改造できる。

### セットリスト
- ドラッグ並べ替え、トランジション毎の秒数指定、テンプレート割り当て
- `.cset` エクスポート / インポート (セットリスト本体 + 参照トラックの軽量メタを 1 ファイルにまとめる)
- セットリスト行から直接「Load to Deck」

### YouTube 取り込み (オプション)
- `cargo build --features yt-download` で yt-dlp 連携が有効化される

---

## クイックスタート

> 詳細手順は [`install.md`](./install.md)、機能マニュアルは [`usage.md`](./usage.md)。

```bash
# 1. 前提
#    - rustup (Rust stable、rust-toolchain.toml で固定)
#    - Node.js 20+ と npm
#    - macOS: xcode-select --install   (mlua の vendored Lua ビルド用)
#    - Linux: webkit2gtk / gtk3 / appindicator の dev パッケージ

# 2. クローンとインストール
git clone https://github.com/xxvw/conduction.git
cd conduction
cd ui && npm install && cd ..

# 3. 開発モードで起動
npm run --prefix ui app:dev

# 4. リリースバンドルをビルド (.app / .dmg / .AppImage / .msi)
npm run --prefix ui app:build
```

---

## リポジトリ構成

```
conduction/
├── crates/
│   ├── conduction-core/        データモデル (Track, Cue, Template, Deck)
│   ├── conduction-audio/       オーディオエンジン (cpal)
│   ├── conduction-analysis/    BPM / キー / エネルギー検出
│   ├── conduction-conductor/   テンプレート実行 + Cue マッチング
│   ├── conduction-library/     SQLite 永続化
│   ├── conduction-claude/      Claude CLI 連携 (feature: claude-analysis)
│   ├── conduction-download/    yt-dlp 連携 (feature: yt-download)
│   ├── conduction-export/      .cset シリアライズ
│   ├── conduction-script/      Lua → Template コンパイラ
│   └── conduction-app/         Tauri アプリ (IPC commands)
└── ui/
    ├── design-system/          CSS 変数とプレビュー
    └── src/                    React + TS (screens / components / hooks)
```

---

## ドキュメント

- [`install.md`](./install.md) — LLM エージェントが読んでセットアップを完遂できるレベルの手順書
- [`usage.md`](./usage.md) — 全画面 + 全ショートカット + 全 Lua API
- [`conduction-requirements.md`](./conduction-requirements.md) — 詳細仕様書 (日本語)
- [`ui/design-system/DESIGN_SYSTEM.md`](./ui/design-system/DESIGN_SYSTEM.md) — デザイントークンとコンポーネント

---

## リリース

最初のビルド済みアーティファクトは **`in-dev 1.0`** タグで公開している。macOS 用 `.dmg` / `.app.tar.gz` は [Releases](https://github.com/xxvw/conduction/releases) ページから入手できる。

`Cargo.toml` のバージョン `0.1.0` は、Phase 2 (ライブ FX 自動化 + スクリプト再生制御) 完了で `0.2.0` へ移行する。`1.0.0` の semver bump は、in-dev ではない最初のリリースに予約してある。

---

## コントリビュート

Issue と PR を歓迎する。PR を出す前に:

1. `cargo fmt --all` と `cargo clippy --workspace --all-targets -- -D warnings`
2. `cargo test --workspace`
3. `cd ui && npx tsc --noEmit`

UI に触れる変更については、dev ビルドの before / after スクリーンショットを添付してほしい。

---

## ライセンス

MIT — [`LICENSE`](./LICENSE) を参照。
