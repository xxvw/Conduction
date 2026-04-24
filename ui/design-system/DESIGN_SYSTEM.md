# Conduction — Design System

> **Conduct your mix, don't perform it.**
> Conduction は、DJ パフォーマンスを "指揮 (conduct)" するという発想のデスクトップ DJ ソフトウェアです。

---

## 1. Product Context

**Conduction (コンダクション)** は PC 単体で完結する DJ アプリケーション。従来の「ターンテーブルを触り続ける DJ」ではなく、**外部から DJ をコントロールする / 指揮する** という体験を提供します。

### Core concept
- 楽曲にあらかじめ **Cue ポイント + 繋ぎ方テンプレート** を仕込んでおき、プレイ中は「次にどこで、どの繋ぎ方をするか」を選ぶだけで自然なミックスが成立する。
- 演奏 (perform) ではなく、**指揮 (conduct)** する。指揮者がオーケストラに指示を出すように、DJ が 2 つ以上のデッキを統括する。
- ベッドルーム DJ / ホビー DJ が技術面に詰まらず、**選曲と流れの構成** にフォーカスできる。

### Target users
- ベッドルーム DJ（自宅で楽しむ層）
- ホビー DJ（週末のホームパーティーやクラブでたまにプレイする層）
- 初心者〜中級者で「DJ したいけど難しそう」と感じている層

### Products in this system
- **Desktop DJ App (Mac / Windows)** ← 今回の主対象
  - Main DJ View (デッキ + ミキサー + 波形)
  - Library / Track Management
  - Preferences (EQ / FX / System)
  - Login / Onboarding

### Sources
今回は添付資産なし。ゼロから構築した内部向けデザインシステムです。将来的に実装コードが発生したら `/import` で接続し、このシステムを更新してください。

---

## 2. CONTENT FUNDAMENTALS

### 言語 / トーン
- **日英バイリンガル**。主要 UI は日本語を主、英語をサブラベル (micro / overline) として併記する構成を推奨。
- 例: `ライブラリ / LIBRARY`、`ミックスを始める / START MIX`
- 日本語は **です・ます調 は使わない**。体言止め・命令形・名詞句で端的に。
  - ✅ `ミックスを始める` / `トラックを読み込む` / `BPM 同期`
  - ❌ `ミックスを始めましょう` / `トラックを読み込んでください`

### 人称
- ユーザーを指すときは **"あなた"** を避け、動詞から始める命令形ベース。
- Conduction 自身を指すときは一人称を使わない（擬人化しない）。
- 英語側は `you` も使わず、動詞スタート (`Load track`, `Start mix`) を徹底。

### Casing / 表記
- 英語ラベルは **UPPERCASE + トラッキング広め** を overline として使う (`DECK A`, `CUE` など)。本文見出しは Sentence case (`Load track`)。
- 数値 (BPM / Key / Time) は **等幅数字 (tabular nums)** 固定。`128.0 BPM` / `03:42`。
- Key は Camelot 記法 (`8A`, `5B`) を第一候補、クラシカル (`Gm`, `Bb`) を従とする。
- 日本語と英語の間には半角スペースを入れる。`ミックスを 開始`。

### Vibe
- **静かな自信 (quiet confidence)**。マーケティング的な感嘆符や煽り語は使わない。
- モダン / シック / 夜の質感。照明を落としたクラブのブースに近い。
- **絵文字は原則不使用**。どうしても必要な時は単色のアイコン (SVG) に置き換える。

### 具体例

| Context | ✅ Good | ❌ Avoid |
|---|---|---|
| ボタン | `ミックスを始める` | `今すぐスタート！🎧` |
| 空状態 | `トラックがありません` / `Drop tracks here` | `まだ曲が追加されていないようです…` |
| エラー | `BPM の自動検出に失敗` | `すみません、うまくいきませんでした` |
| 成功 | `書き出し完了` | `やったー！書き出せました 🎉` |
| Overline | `NOW PLAYING / 再生中` | `🎵 Now playing 🎵` |

---

## 3. VISUAL FOUNDATIONS

### Overall vibe
**暗い真夜中のミキサー + 蛍光グリーンの点灯灯 + すりガラス越しに見える光**。機材感とソフトウェア感の中間。

### Color
- ベースは **cool-leaning な黒〜濃いスレート**（`--c-ink-0 #050607` 〜 `--c-ink-5 #2E3744`）。純黒は使わず、わずかに青を含む。
- アクセントは **Phosphor Mint `#00F5A0`**（蛍光グリーン）。Deck A / CTA / selected state / シグネチャーにのみ使用し、**決して大面積で塗らない**。
- デッキごとの識別色: A=mint / B=amber / C=blue / D=pink。A と B が最頻出。
- ステータス: success=mint系 / warning=amber / danger=`#FF4A5C` / live=`#FF2D55`。

### Type
- **LINE Seed JP** を全面採用（Thin 100 / Regular 400 / Bold 700 / ExtraBold 800）。幾何学的で丸みがあり、日本語と英語のバランスが良いのでバイリンガル UI に最適。
- 大見出しは **800 + tight tracking**、body は 400、オーバーラインは 700 + ultrawide tracking。
- 数値には **JetBrains Mono** (tabular) を使用。BPM / 時刻 / キー情報は必ず等幅。

### Spacing
4px グリッドベース (`--s-1` = 2px 〜 `--s-12` = 96px)。DJ UI は情報密度が高いため、ホビー寄りとはいえ詰めすぎず、`--s-4` (12px) / `--s-5` (16px) を基本間隔に。

### Backgrounds
- ベース背景は `--bg-app` のフラットカラー。**グラデーションは原則使わない**。
- 代わりに **glassmorphism + 背景側にぼんやりとした波形 / グローの光源** を配置して奥行きを作る。
- 壁紙的なイメージ画像や写真背景は使わない。あくまでプロダクト UI に徹する。

### Glassmorphism （中心的モチーフ）
- Glass サーフェス: `rgba(255,255,255,0.05)` + `backdrop-filter: blur(16px) saturate(140%)`。
- ストロークは `rgba(255,255,255,0.08)` の 1px + 上辺に `inset 0 1px 0 rgba(255,255,255,0.08)` のハイライト（リムライト）。
- **深度を 2 段階** だけ用意 (`.glass` / `.glass-strong`)。無闇に層を増やさない。

### Corner radii
- 小さなチップ / ボタン: **4–8px** (`--r-2` / `--r-3`)
- カード / パネル: **12–16px** (`--r-4` / `--r-5`)
- ヒーロー面 / モーダル: **20–24px** (`--r-6` / `--r-7`)
- **完全な角丸 (pill)** はトグル / タグ / ステータスバッジのみに限定。

### Borders
- ダーク面同士の区切りは 1px `--c-ink-5 (#2E3744)`。
- Glass 面は透明ストローク (`--c-glass-stroke`) + inner-top rim light の組み合わせ。
- フォーカス時は **1.5px の mint ボーダー + ソフトグロー** (`--glow-accent`)。

### Shadow system
- **外側**: 4 段階 (`--shadow-1` → `--shadow-4`)。暗い背景に沈み込む青みがかったブラック (`rgba(0,0,0,0.4)` ベース)。
- **内側**: 必須。`--inner-top` (上辺リムライト) + `--inner-bottom` (下辺の沈み)。ガラス面では両方セットの `--inner-glass`。
- **グロー**: アクセントがアクティブな時のみ発光。常時グローさせない。

### Hover / Press states
- **Hover**: `background: rgba(255,255,255,0.04)` をオーバーレイ (= `--bg-hover`)、or 1 段階明るいインク層へ。
- **Press / Active**: `rgba(255,255,255,0.07)` + **scale(0.98)** + 80ms ease。物理的な「押し込み」感。
- アクセントボタンの hover は **1 stop 明るく** (`--c-accent-hi`)、press は 1 stop 暗く (`--c-accent-lo`)。
- **不透明度の変化は使わない**（グラス面では透明度変化が汚く見える）。色そのものを変える。

### Focus states
- キーボード focus: `outline: none` + `box-shadow: 0 0 0 2px var(--c-ink-1), 0 0 0 3.5px var(--c-accent)`（外側に暗いリング、内側に mint リング）。

### Animation / easing
- 基本イージング: `cubic-bezier(0.2, 0.8, 0.2, 1)` (`--ease-out`)。跳ねすぎない。
- Duration: 80 / 160 / 260 / 420ms の 4 段階。**500ms を超えない**。
- Cross-fade や hover の fade は OK だが、**bounce / spring** は音楽的インタラクション（Cue hit, Play ボタンのパルス）にのみ許可。
- 波形スクロールは linear。リアルタイム挙動のフィーリングを壊さない。
- 装飾的アニメーション（ふわふわ動く背景グラデ等）は禁止。

### Transparency / blur usage
- blur は **前景に対する背景ぼかし** としてのみ。Tooltip / 浮遊パネル / コンテキストメニュー / モーダルのバックドロップに使用。
- 半透明 × 半透明を積み重ねない（読みづらくなる）。

### Cards
- 基本: `background: var(--bg-panel)` + `border: 1px solid var(--stroke-1)` + `border-radius: var(--r-4)` + `--shadow-2`。
- Glass カード: `.glass` クラス。メインビュー上の浮遊 HUD（例: Mix Suggestion Panel）で使用。
- **カード内の左ボーダーアクセント (colored left border)** は使わない（使い古された AI slop パターン）。

### Layout rules
- 上部に titlebar (36px) + toolbar (52px)。下部に transport (72px) が固定。
- メイン領域はサイドバー (library, 248px) + 2 deck + center mixer (320px)。
- フルスクリーン時はタイトルバーを隠せる。

### Imagery mood
- アートワーク以外の写真は基本使わない。
- 波形やスペクトラムの「光」が主要なビジュアル要素。
- Onboarding / marketing 系でもし写真を使う場合は：**クールトーン / 低彩度 / ノイズ or グレイン** のある夜のクラブ写真。温色系は避ける。

---

## 4. ICONOGRAPHY

### 採用アイコンセット
**Lucide Icons** (CDN 経由) を採用。理由:
- 線幅 1.5–2px、シンプルで幾何学的な形状が LINE Seed JP の幾何学的な雰囲気と合う。
- MIT ライセンスで商用利用可、OSS。
- DJ 用途に必要なアイコン（play, pause, skip, volume, sliders, music, disc, headphones 等）が一通り揃っている。

```html
<script src="https://unpkg.com/lucide@latest"></script>
<script>lucide.createIcons();</script>
```

### 代用フラグ
プロダクト固有のアイコン（ターンテーブル / BPM タップ / Cue マーカー等）で Lucide に適切なものがない場合は、**同じストローク規則 (1.75px, line-cap: round)** でカスタム SVG を作成し、`assets/icons/` に配置する。現状は Lucide で代用している。将来的には DJ 機材系アイコンセットへの置き換えを推奨。

### 使用ルール
- サイズは **16 / 20 / 24 / 32px** の 4 段階。
- カラーは `currentColor` を徹底。背景・テキストと同じフローで色を継承。
- 線幅は **1.75px** で統一。`stroke-width="1.75"`。
- **絵文字は原則不使用**。ユーザー設定の DJ 名や曲名に含まれる絵文字はそのまま表示する。
- **Unicode 記号を装飾として使わない** (例: `♪`, `★`, `▶`)。必要なら SVG アイコンで表現。

### ロゴ
ゼロから構築のため、ロゴはプレースホルダーで対応。ワードマーク: **"Conduction"** (LINE Seed JP 800, letter-spacing: -0.02em)。モノグラム: **"C"** をダブル波形で囲んだマーク（assets/logo.svg）。ユーザーが正式ロゴを提供次第差し替え。

---

## 5. Index — File Map

### Root
- `README.md` — 本ファイル
- `colors_and_type.css` — 全カラー & タイポグラフィ変数。全ファイルから `@import` される
- `SKILL.md` — エージェント向けスキル定義
- `index.html` — デザインシステムの入口（ある場合）

### Folders
- `assets/` — ロゴ、アイコン、背景画像など
  - `assets/logo.svg` — Conduction ワードマーク + モノグラム
  - `assets/icons/` — カスタム SVG アイコン（Lucide で足りないもの）
- `fonts/` — ローカルフォント（今回は Google Fonts CDN のみ）
- `preview/` — Design System タブに表示される各カード HTML
- `ui_kits/desktop_app/` — デスクトップ DJ アプリ UI Kit
  - `index.html` — 対話型の Main DJ View
  - `*.jsx` — 再利用可能な React コンポーネント
  - `README.md` — kit 固有の注記

### Manifest (root)
- `README.md` / `SKILL.md` / `colors_and_type.css`
- `assets/logo.svg`, `assets/logo-mark.svg`
- `preview/` — 22 design-system cards (colors / type / spacing / components / brand)
- `ui_kits/desktop_app/` — interactive Main DJ View + React components

### CSS の読み込み
すべての HTML ファイルは `colors_and_type.css` を最初に読み込む:
```html
<link rel="stylesheet" href="../colors_and_type.css" />
```

---

## 6. Caveats / Open Questions

- **ロゴ**: 暫定プレースホルダー。正式ロゴがあれば `assets/logo.svg` を差し替え。
- **アイコン**: Lucide で代用中。DJ ドメイン固有のアイコンは将来追加。
- **音源 / デモ楽曲**: UI Kit では架空のトラックメタデータを表示。実データ接続は未対応。
- **Cue / 繋ぎテンプレートの UX 詳細**: 「次にどこでどう繋ぐかを選ぶ」体験は本システムの中核コンセプト。MVP では Mix Suggestion Panel としてモックしているが、実装時にユーザーテストで詰めるべき領域。
