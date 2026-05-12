# Conduction — Install Guide

このドキュメントを Claude (もしくは初見の開発者) に読ませるだけで、リポジトリのクローンからローカル起動・ビルドまで完了できることを目的とする。

対象 OS: **macOS (Apple Silicon / Intel)** をプライマリ。Linux / Windows は注記参照。

---

## 0. このリポについて

- Rust ワークスペース (10 crates) + Tauri v2 + React 18 + TypeScript + Vite 6 の構成
- フロントエンドは `ui/` 配下、Rust 側は `crates/` 配下
- DJ エンジン本体 (オーディオ I/O, BPM 解析, 自動化テンプレート, Lua スクリプト, SQLite ライブラリ) を Tauri アプリにバンドルしてデスクトップ起動する
- 詳細仕様: [`conduction-requirements.md`](./conduction-requirements.md)

---

## 1. 前提ツールのインストール

### 1-1. Rust toolchain (rustup)

`rust-toolchain.toml` で `channel = "stable"` を固定済み。最初に rustup を入れれば、初回 `cargo` 実行時に必要なバージョンが自動で取得される。

```bash
# 未インストールの場合
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# 確認
rustc --version   # 1.82+
cargo --version
```

### 1-2. Node.js (LTS) と npm

Tauri v2 CLI と Vite 6 を動かすために必要。

```bash
# 未インストールの場合 (macOS の例)
brew install node      # もしくは nvm/asdf/volta などお好みで

# 確認 (Node 20+ 推奨)
node --version
npm --version
```

### 1-3. Xcode Command Line Tools (macOS)

`mlua` クレートが `vendored` 機能で Lua 5.4 の C ソースをビルドする。`cc` / `make` / システムヘッダが必要。Tauri の WebView ラッパーも Apple SDK を要求する。

```bash
xcode-select --install   # 既に入っていれば「already installed」と出る
```

### 1-4. (Linux のみ) Tauri 必須パッケージ

参考。本リポは macOS で開発しているが、Linux でも動作する想定。

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
  librsvg2-dev libsoup-3.0-dev build-essential curl wget file pkg-config
```

### 1-5. (Windows のみ) MSVC Build Tools と WebView2

参考。Visual Studio Build Tools (C++ ワークロード) と Microsoft Edge WebView2 Runtime をインストール。

---

## 2. リポジトリの取得

```bash
git clone https://github.com/xxvw/conduction.git
cd conduction
```

(既にクローン済みの場合はこのステップを飛ばす)

---

## 3. 依存パッケージのインストール

### 3-1. Node 側 (Tauri CLI / Monaco / React Flow など)

```bash
cd ui
npm install
cd ..
```

- Tauri CLI は `ui/node_modules/.bin/tauri` に入る (グローバルインストールしないこと)
- 既存 `package-lock.json` を尊重したい場合は `npm ci` を使う

### 3-2. Rust 側

`cargo build` 実行時に自動で依存が取得・コンパイルされる。事前に一度走らせて warm cache しておくと、次回の起動が速い。

```bash
cargo build --workspace
```

初回は 5–10 分かかる (mlua の vendored Lua C ビルド + Tauri / WebKit バインディング)。

---

## 4. 開発モードで起動する

Tauri dev ウィンドウを開く (UI ホットリロード + Rust 側は変更時に自動再ビルド)。

```bash
npm run --prefix ui app:dev
```

裏で動くのは以下:

1. Vite dev server が `ui/` で起動 (ポートは Tauri に通知される)
2. `cargo run` で `conduction-app` クレートをビルドして起動
3. Tauri が WebView を立ち上げ Vite に接続

初回起動は数分かかる。2 回目以降は数十秒程度。

#### よくあるトラブル

- **ポート 38127 が使用中** — 前回の dev プロセスがオーディオサーバを掴んだまま落ちた可能性。次のコマンドで掃除する。
  ```bash
  lsof -ti:38127 | xargs kill
  ```
- **WebView が真っ白** — DevTools (Cmd+Option+I) で console を確認。Vite のポート競合の可能性あり。
- **「audio engine connecting…」のまま** — オーディオデバイスが他のアプリに専有されている、もしくは出力ドライバ未選択。Settings 画面でデバイスを選び直す。

---

## 5. 本番ビルド (.app / .dmg / .exe など)

```bash
npm run --prefix ui app:build
```

成果物:

- macOS: `target/release/bundle/macos/Conduction.app` と `target/release/bundle/dmg/*.dmg`
- Linux: `target/release/bundle/appimage/*.AppImage` など
- Windows: `target/release/bundle/msi/*.msi`

`tsc --noEmit` による型チェック + Vite 本番ビルド + Tauri バンドルがこのコマンドで一括実行される。

### 5-1. 個人開発フィーチャ込みでビルド

YouTube/yt-dlp 連携などのオプションは Cargo feature で切る。

```bash
# yt-dlp 連携入りビルド (Rust 単体)
cargo build --release --features yt-download
```

Tauri バンドルに同梱したい場合は `crates/conduction-app/Cargo.toml` 側で feature を有効化してから `app:build` を回す。

---

## 6. テストと型チェック

```bash
# Rust 全クレートの単体テスト
cargo test --workspace

# TypeScript 型チェック (UI)
cd ui && npx tsc --noEmit && cd ..

# Rust の lint
cargo clippy --workspace --all-targets -- -D warnings

# Rust の format
cargo fmt --all
```

CI は将来追加予定。当面は上記 4 つを手元で回しておけば PR 品質は担保される。

---

## 7. ディレクトリ早見表

```
conduction/
├── install.md                  ← 本ファイル
├── usage.md                    ← 機能と使い方
├── README.md                   ← 概要
├── conduction-requirements.md  ← 詳細仕様
├── Cargo.toml                  ← Rust ワークスペース
├── rust-toolchain.toml         ← Rust バージョン固定
├── crates/
│   ├── conduction-core/        ← データモデル (Track / Cue / Template / Deck)
│   ├── conduction-audio/       ← cpal ベースのオーディオエンジン
│   ├── conduction-analysis/    ← BPM / Key / 楽曲解析
│   ├── conduction-conductor/   ← 自動化テンプレート実行・Cue マッチング
│   ├── conduction-library/     ← SQLite (tracks / cues / setlists / templates)
│   ├── conduction-claude/      ← Claude CLI 連携 [feature: claude-analysis]
│   ├── conduction-download/    ← yt-dlp 連携 [feature: yt-download]
│   ├── conduction-export/      ← .cset 等のシリアライズ
│   ├── conduction-script/      ← Lua → Template コンパイラ
│   └── conduction-app/         ← Tauri アプリ本体 (IPC / commands)
└── ui/
    ├── design-system/          ← CSS 変数とプレビュー
    ├── package.json
    └── src/                    ← React + TS (screens / components / hooks)
```

---

## 8. Claude へのメモ (自動セットアップ時)

このファイルをエージェントが読んだ状態で「セットアップして」と頼まれたら、以下の順で実行すれば良い:

1. `rustc --version` と `node --version` で前提ツールを確認 (無ければユーザーに導入を促す)
2. macOS なら `xcode-select -p` で CLI Tools 有無を確認
3. `cd ui && npm install` を実行
4. `cargo build --workspace` で warm-up
5. ここまで成功したら `npm run --prefix ui app:dev` を **バックグラウンドで** 起動してログを観察
6. 「audio engine connecting…」が消えて UI が描画されたら成功

破壊的操作 (`git reset --hard` 等) と `--no-verify` を使ったコミットは行わない。ロックファイル (`Cargo.lock`, `package-lock.json`) は触らずそのまま使う。
