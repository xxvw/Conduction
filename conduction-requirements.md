# Conduction 要件定義書

プロのライブ現場で使う、プログラマブルで拡張可能なDJエンジン。

---

## 1. プロダクト概要

### 名称
**Conduction**

### コンセプト
DJを外部からコントロール可能にする、"指揮"の発想に基づくDJソフトウェア。人間のUI操作・AI・プラグイン・スクリプトが、同じ`Conductor`層に対する"指示"として等価に扱われる。

名称の由来は、動詞 "conduct"（指揮する）および、Butch Morrisが提唱した即興演奏を指揮するメソッド "Conduction"。

### タグライン
**Conduct your mix, don't perform it.**

### 差別化ポイント
- **Cue動的マッチング**：現在のデッキ状態から、繋げられるCue候補をリアルタイムにライブラリ全体から絞り込む
- **テンプレート駆動の繋ぎ**：遷移方法を事前にテンプレート化し、現場で選択して実行
- **プログラマブル**：人間の操作・スクリプト・AIが同じインターフェイスでConductorを操る

### ターゲットユーザー
プロDJのライブ現場を中核に、ホビー〜プロまで幅広く対応。情報密度は高めでよいが、現場の緊張下でも誤操作しにくい設計を優先。

---

## 2. 技術スタック

### オーディオコア（Rust）
- `cpal` — クロスプラットフォームのオーディオI/O（CoreAudio / WASAPI / ASIO / ALSA）
- `rodio` + `Symphonia` — 再生抽象化とフォーマットデコード（MP3, WAV, FLAC, Vorbis, AAC）
- `rubato` — タイムストレッチ / ピッチシフト

### 楽曲解析（Rust）
- `aubio-rs` — BPM検出、ビートトラッキング、オンセット検出、ピッチ検出、クロマグラム計算
- 自前実装：Krumhansl-Schmucklerアルゴリズムによるキー検出（クロマベクトル × キープロファイル相関）
- Claude CLI（`claude -p`）連携 — 楽曲構造（イントロ/ブレイク/ドロップ/アウトロ）の推定

### GUI
- `Tauri` — デスクトップアプリケーションフレームワーク
- **React + TypeScript** — フロントエンド
- `Framer Motion` — 起動アニメーション、UIアニメーション
- Canvas/WebGL — 波形描画
- `React Flow` — ノードエディタ

### タイポグラフィ・アイコン
- `LINE Seed JP` — 主UIフォント（Google Fonts CDN、Thin 100 / Regular 400 / Bold 700 / ExtraBold 800）
- `JetBrains Mono` — 数値・時刻・BPM表示用（Google Fonts CDN、Regular 400 / Medium 500 / Bold 700）
- `Lucide Icons` — アイコンセット（1.75px stroke、currentColor、CDN経由）

### データ永続化・設定
- `rusqlite` + SQLite — ライブラリ、Cue、テンプレート保存
- `serde` / `serde_json` — シリアライズ
- `toml` — 設定ファイル、テンプレートファイル形式
- `directories` — OS規約準拠のパス解決

### スレッド・並行処理
- `ringbuf` — lock-freeリングバッファ（Conductor → Audio Engine）
- `arc-swap` — atomicなArc入れ替え
- `crossbeam` — 並行ユーティリティ

### スクリプト
- `mlua` — Lua スクリプトエンジン統合（外部スクリプトAPI用）
- TypeScript型定義の配布（Lua API を TypeScript でも記述可能に）

### ロギング・診断
- `tracing` + `tracing-subscriber` — 構造化ログ
- `tracing-appender` — ログローテーション
- オーディオスレッド用 non-blockingアペンダ

### 外部連携
- Claude CLI（`claude -p`）— 楽曲構造の事前解析、ビートグリッドのAIアシスト補正
- `yt-dlp`（外部バイナリ呼び出し、feature flag管理）

---

## 3. システムアーキテクチャ

### レイヤー構成
```
┌─────────────────────────────────┐
│   UI Layer (React + Tauri)      │  ユーザー操作、アニメーション、エディタ
└─────────────┬───────────────────┘
              │ Tauri IPC
┌─────────────▼───────────────────┐
│   Conductor Layer (Rust)        │  テンプレート実行、Cueマッチング、指示の集約点
└─────────────┬───────────────────┘   ← Script API、Claude CLI、将来のプラグイン
              │ lock-free channel
┌─────────────▼───────────────────┐
│   Audio Engine (Rust realtime)  │  再生、ミックス、エフェクト
└─────────────┬───────────────────┘
              │ cpal
┌─────────────▼───────────────────┐
│   OS Audio (CoreAudio / WASAPI) │
└─────────────────────────────────┘
```

### 設計原則
- Audio EngineはUIからIPC経由で独立し、リアルタイム性を保証
- すべての指示入力（人間のUI、スクリプト、Claude解析結果、将来のプラグイン）はConductor層に集約
- Conductor層で指示を具体的な状態変更に翻訳してAudio Engineに伝達

### 実行エンジンの2層構造
```
Conductor Thread (約100Hz ≒ 512samples @ 48kHz)
  - テンプレートのキーフレーム補間
  - ターゲット値の計算
  ↓ lock-free channel (ringbuf)
Audio Thread (48kHz サンプル単位)
  - 1-pole lowpass filterでスムージング
  - パラメータを実際の音に適用
  - クリックノイズ防止
```

---

## 4. プロジェクト構造

Rustワークスペース + Tauri のハイブリッド構成。

```
conduction/
├── Cargo.toml                     # ワークスペース定義
├── crates/
│   ├── conduction-core/           # データモデル（Track, Cue, Template, Deck）
│   ├── conduction-audio/          # オーディオエンジン（cpal直結、realtime）
│   ├── conduction-analysis/       # BPM / キー検出、楽曲解析
│   ├── conduction-conductor/      # 司令塔（テンプレート実行、Cueマッチング）
│   ├── conduction-library/        # ライブラリ管理、SQLite
│   ├── conduction-claude/         # Claude CLI連携 [feature: claude-analysis]
│   ├── conduction-download/       # yt-dlp連携 [feature: yt-download]
│   ├── conduction-script/         # Script API（WebSocket / OSC + Lua）
│   └── conduction-app/            # Tauriアプリ本体
└── ui/                            # React + TypeScript (Tauriのfrontend)
    ├── design-system/
    │   ├── colors_and_type.css    # CSS変数システム（全画面から @import）
    │   └── assets/                # ロゴ・カスタムアイコン
    ├── src/
    │   ├── components/
    │   │   ├── layout/            # App, Sidebar, TopBar, TweaksPanel
    │   │   ├── deck/              # DeckView
    │   │   ├── mixer/             # Mixer, Knob, Fader
    │   │   ├── library/           # Library, TrackList
    │   │   ├── suggestion/        # MixSuggestion (Cue動的マッチング)
    │   │   ├── templates/         # Templates, TemplateList, TemplateEditor,
    │   │   │                      # AutomationTimeline, NodeEditor, ScriptEditor
    │   │   ├── setlist/           # Setlist, SetlistTrackBlock,
    │   │   │                      # SetlistTransitionBlock, SetlistOverview
    │   │   ├── beatgrid/          # BeatgridCorrection
    │   │   ├── cues/              # CueEditor, CueListItem
    │   │   ├── override/          # OverrideIndicator, OverrideControls,
    │   │   │                      # TransportStatusPanel
    │   │   └── settings/          # Settings
    │   ├── stores/
    │   ├── hooks/
    │   └── types/
    └── package.json
```

---

## 5. オーディオ仕様

### 基本仕様
- **サンプルレート**：44.1kHz / 48kHz 両対応（ユーザー設定、デバイスの能力を判定して自動選択）
- **内部処理**：32-bit float
- **バッファサイズ**：設定可能（デフォルト512サンプル、低レイテンシ用に128〜256を選択可能）

### 出力構成
- **メイン出力バス**（Main Out）：観客に流す音
- **モニタリング出力バス**（Cue / Monitor）：DJがヘッドホンで聴く音
- 各デッキから両バスへの独立ルーティング（PFL / Pre-Fader Listen 方式）
- モニタリング音量は独立調整可能

### デバイス設定
- メイン出力とモニタリング出力は、**別々の物理デバイスにも、同一デバイスにも設定可能**
- 同一デバイスに設定した場合：内部で2バスをミキシングしてステレオ出力（または左右チャンネルに分離するオプション）
- 別デバイスに設定した場合：cpalで2ストリームを並行実行

### レイテンシ目標
- 操作から音への反応：20ms以下
- オーディオコールバック自体：5ms以下

---

## 6. 機能要件（MVP）

### 6.1 デッキ（2デッキ構成）
- 再生、停止、一時停止、シーク
- テンポ調整（±6% / ±10% / ±16% のレンジ選択可能）
- キーロック（テンポ変更時にピッチを維持）
- 波形表示（全体波形 + ビート単位のズーム波形、3バンドミラー表示）
- ビートグリッド表示
- 各デッキから `Main` / `Cue` バスへのルーティング切替
- Hot Cue 8個（1-8のキーバインド）
- SYNC、LOOP 8機能

### 6.2 ミキサー
- 3バンドEQ（Low / Mid / High、各バンド -∞ 〜 +6dB）
- チャンネルボリューム / チャンネルゲイン
- クロスフェーダー（カーブ設定可能：Linear / Smooth / Sharp）
- 基本FX（MVPスコープ）：
  - High-pass / Low-pass フィルタ
  - エコー / ディレイ
- ヘッドホン CUE ボタン（デッキごとのPFL切替）

### 6.3 Cueシステム
以下のCueタイプを統合的に扱う：
- **基本Cue（Hot Cue）**：位置マーカー
- **タイプ付きCue**：`IntroStart` / `IntroEnd` / `Breakdown` / `Drop` / `Outro` / `CustomHotCue`
- **セクションCue**：開始点 + 範囲（例：32小節のブレイク区間）

Cueに付随するメタデータ：
- 位置（拍数）
- その地点でのBPM / キー / エネルギー
- フレーズ長（16 / 32 / 64小節など）
- 繋ぎでの役割（`Entry` / `Exit` / `Both`）
- 互換エネルギー範囲

### 6.4 楽曲解析・自動Cue検出
事前解析のみ実行（ライブ中のリアルタイム解析は対象外）。

#### 解析パイプライン
1. **基礎解析**（`aubio-rs`）：BPM、ビートグリッド、オンセット、クロマグラム
2. **キー検出**（自前実装）：Krumhansl-Schmucklerアルゴリズムでクロマベクトルと24キー（長調12 + 短調12）のプロファイル相関計算
3. **エネルギー解析**（自前）：RMS包絡線、スペクトル重心による時系列エネルギー推定
4. **構造推定**（Claude CLI連携）：基礎解析結果を構造化テキスト化し`claude -p`で「この曲のイントロ/ビルドアップ/ドロップ/ブレイク/アウトロのセクション区切りをJSONで返して」と問い合わせ、結果をCueとして保存

#### ビートグリッド検証・補正
プロライブ用途では、検出精度の不足が致命的になるため、**自動検出 + 手動補正**を必須工程とする。

- 自動検出の結果をUI上に可視化
- 最初のダウンビート位置（1拍目）をドラッグで調整可能
- BPM値を0.01刻みで微調整可能（±0.01 / ±0.1 / ±1.0 のボタン）
- Tap Tempo機能
- 試聴プレビュー（メトロノーム音を重ねて再生）
- 検証完了までライブラリで「未検証」ステータス（プレイ中の誤再生を防ぐ）

#### AIアシスト補正（Claude CLI検出時のみ有効）
- 起動時に`claude`コマンドの存在をランタイム検出
- 検出された場合、補正UIに「AIアシスト」ボタンが追加される
- ユーザーが怪しい箇所を指定 → Claude CLIに周辺の解析データを渡して再推定を依頼
- `claude`コマンド未インストール時はボタン非表示、基本機能のみ動作

### 6.5 Cue動的マッチング（Conductionのコア機能）
- 現在のデッキ状態（BPM、キー、現在再生位置、エネルギー）から、ライブラリ全体で繋げられるCueを**動的にフィルタリング**
- フィルタ条件：
  - BPM距離（閾値内）
  - キー互換性（Camelotホイール準拠）
  - エネルギー互換性
  - 繋ぎロール（`Entry`可能なCue）
- マッチング結果はスコアリングして候補リストとしてUIに常時提示（マッチ率％表示）
- 再生中にリアルタイム更新
- UI：`MixSuggestion`コンポーネント（"Next Cue · 繋ぎ候補" フローティングHUD）
- キーボード：`Enter` でトリガー、`Esc` でDismiss

### 6.6 テンプレートシステム
**内部モデル**は1つ。3つの編集モードで同じデータを扱う。

#### テンプレートの単位
- **遷移テンプレート**（Transition）：A→Bの繋ぎ方を定義（例：32小節のロングEQミックス）
- **セットリストテンプレート**（Setlist）：複数曲を連続で繋ぐ（曲指定 + 各遷移のテンプレート指定）
- 両者を同じデータモデルで扱う（セットリストは遷移テンプレートのシーケンス）

#### テンプレートの内容
- Duration（拍数）
- Automation Track：各パラメータ（EQ、ボリューム、FX）の時系列曲線
- Trigger Track：特定時点でのアクション（再生開始、Cueジャンプなど）
- Entry Cue / Exit Cue 指定

#### 時間単位
拍数ベースをメインとし、特定イベントは絶対時間（秒）も許容。
- `Beats(f64)` — 0拍目から何拍目か
- `Seconds(f64)` — 絶対時間
- `BeatsFromEnd(f64)` — 終了から逆算

#### 曲線タイプ（6種類）
`Linear` / `EaseIn` / `EaseOut` / `EaseInOut` / `Step` / `Hold`

#### パラメータアドレッシング
ハイブリッド方式：
- **内蔵パラメータ**（型安全enum）：`DeckVolume`, `DeckEq{band}`, `DeckFxSend`, `Crossfader`, `FxParameter` 等
- **拡張**（文字列）：プラグイン・スクリプト用の `Custom(String)`

#### 編集モード（切替可能）
1. **Visual**（メイン編集モード）— DAWのオートメーションに近いタイムラインUI
2. **Node** — 条件分岐やロジック記述用（React Flow）
3. **Script** — Lua（mlua経由で実行）、Monaco エディタ + API リファレンスドロワー

※ 同時編集はしない（モード切替のみ）。整合性維持のため。

#### スナップ
`Off / 1/4 beat / 1/2 beat / Beat / 2 beats / 4 beats / Bar / Phrase`

#### プレビュー
- **Dry run**（音なし）：ターゲット値の変化を数値でモニタリング
- **With audio**：実際に2デッキに曲をロードして、テンプレートを適用した音を聴く
- プレビュー中も編集可能（DAW的挙動）

#### プリセットテンプレート（5種類同梱）
- **Long EQ Mix**（32 bars）— 標準的な長めEQミックス、テクノ・ハウス向け
- **Quick Cut**（4 bars）— 短い切り替え、HipHop・ドロップ合わせ向け
- **Breakdown Swap**（16 bars）— ブレイクダウン区間で入れ替え、EDM・フェス向け
- **Echo Out**（8 bars）— エコーをかけてフェードアウト、アンビエント・セット区切り向け
- **Instant Swap**（1 bar）— 緊急用の即時切り替え、失敗リカバリー向け

### 6.7 繋ぎの実行とOverride挙動

#### 通常実行
- **繋ぎ開始直前**にテンプレートを選択する方式
- 選択後、テンプレートのタイムラインに沿って自動で各パラメータが動く
- 繋ぎ実行中のテンプレート差し替えはMVP対象外

#### Override（手動介入）
各パラメータ（Knob / Fader / Crossfader）ごとに独立した状態管理。

**状態**（4種類）：
- `Automated`（青 #8A9BE8）— テンプレートが制御中
- `Overridden`（オレンジ #E8915A）— 手動操作中
- `Committed`（グレー）— 手動値で確定
- `Idle`（透明）— テンプレート非実行中

**操作フロー**：
1. 明示的な「Override」ボタンクリックで `Automated → Overridden`
2. 手動操作
3. 選択肢：
   - **Resume**：1〜2拍でスムーズに自動化に復帰（Glide Back、デフォルト）
   - **Commit**：手動値で確定（そのパラメータだけ自動終了、他は継続）
4. オプション：**Relative Mode**（差分維持）— 上級者向けに切替可能

**キーボードショートカット**：
- `O` — Override開始（フォーカス中のパラメータ）
- `R` — Resume
- `C` — Commit
- `Shift + Esc` — Abort Template（全体中断）

**誤操作防止**：
- Override開始は明示的クリックのみ（タッチ判定でのLatched方式は不採用）
- Commit時は200msのフェード確認アニメーション
- Abort Template は確認ダイアログ必須

#### グローバル状態表示
- Transport領域に `TransportStatusPanel` を配置
- 現在実行中テンプレート名、進捗バー、Override中パラメータ数カウンター
- Abort Template ボタン（確認ダイアログ付き）

### 6.8 ライブラリ管理
- ローカルファイル管理（対応フォーマット：MP3, WAV, FLAC, AAC, OGG）
- SQLiteでメタデータ保存（タグ、BPM、キー、Cue、解析結果）
- フォルダ監視（追加された楽曲の自動インポート）
- 検索・フィルタ（BPMレンジ、キー、タグ、エネルギー）
- **YouTube / SoundCloud ダウンロード**：`yt-dlp` 統合、feature flag `yt-download` で制御（個人用ビルドのみ、配布版はコード自体を含まない）

### 6.9 Script API
外部からConductor層に指示を送るためのAPI。
- **WebSocket API**：JSON形式のコマンド送受信
- **OSC API**：Open Sound Controlプロトコル対応
- **Lua API**：内蔵Luaエンジンで直接スクリプト実行、テンプレートのロジック記述
- API仕様は公開、外部ツール（MaxMSP、TouchDesignerなど）との連携を想定
- セキュリティ：デフォルトはlocalhost限定、ネットワーク経由は明示的に有効化

#### Lua API 例
```lua
-- デッキ操作
conduction.deck(1):play()
conduction.deck(1):set_tempo(128.0)
conduction.deck(1):jump_to_cue("drop")

-- 繋ぎ実行
conduction.transition("long_eq_mix", { from = 1, to = 2 })

-- Cue検索
local candidates = conduction.library:find_compatible_cues({
  bpm = 128,
  key = "8A",
  role = "entry"
})

-- イベントフック
conduction.on("beat", function(beat_num)
  -- ビートごとに呼ばれる
end)
```

同じAPIをTypeScript型定義としても配布（Lua LSP + 型定義）。

### 6.10 画面構成

| 画面ID | 画面名 | 主要機能 |
|---|---|---|
| `mix` | Mix（メイン） | 2デッキ + Mixer + Library + MixSuggestion |
| `library` | Library | 全楽曲管理、検索、フィルタ |
| `setlist` | Setlist | セットリスト編集、各遷移のテンプレート指定 |
| `templates` | Templates | テンプレート作成・編集（Visual/Node/Script） |
| `beatgrid` | Beatgrid | ビートグリッド検証・補正、AIアシスト |
| `cues` | Cues | Cueポイント編集、メタデータ管理 |
| `prep` | Prepare | セット準備、キュー |
| `history` | History | 過去セッション履歴 |
| `settings` | Settings | 一般設定、Audio、MIDI、Mixing AI、Shortcuts、Account |

### 6.11 セットリスト機能
- ライブラリから曲をドラッグ&ドロップで追加
- 各曲間に遷移テンプレート指定
- テンポ遷移モード：`HoldSource` / `MatchTarget` / `LinearBlend` / `MasterTempo`
- Cue動的マッチングから候補上位3件を自動提示
- セット全体のBPMカーブ・キー遷移マップ・エネルギーカーブをオーバービュー表示
- `.cset` ファイルとしてエクスポート/インポート
- Rehearse機能（セット全体の高速プレビュー）

---

## 7. MVPから除外する項目

次期以降で対応：
- プラグインシステム（動的ロード、サードパーティ拡張）
- AI自動DJ（完全自動で繋ぎ続ける）
- 4デッキ以上の対応
- MIDI / 外部コントローラ連携
- 既存DJソフトからのライブラリインポート（rekordbox、Serato、Traktor）
- 既存ソフトへの指示出力（Traktor等の外部制御）
- 高度エフェクト（リバーブ、ディストーション、グラニュラー等）
- モバイル対応（iOS / Android）
- ストリーミングサービス連携（Beatport、TIDAL等）
- 楽曲類似度による推薦（bliss-rs統合）

---

## 8. 非機能要件

### パフォーマンス
- オーディオコールバックレイテンシ：5ms以下
- UI → オーディオ反応：20ms以下
- 楽曲ロード：3秒以内（10分の楽曲）
- ライブラリ検索：1秒以内（10,000曲規模）

### 安定性
- 再生中のクラッシュ禁止（Audio Engineの耐障害性）
- UI側クラッシュ時もオーディオは継続するアーキテクチャ
- 最後の再生状態をsnapshotし、クラッシュ後の復元を可能に

### クロスプラットフォーム
- **優先順位**：macOS → Windows → Linux
- Macファーストで開発、基本機能が動いた段階でWindows / Linux対応
- cpal + Tauri の選択により、UI・オーディオとも同一コードベースで対応可能

### 配布
- スタンドアロン配布（依存関係を内包）
- macOS: `.app` バンドル
- Windows: `.exe`（MSI インストーラ）
- Linux: AppImage

---

## 9. Feature Flags

Cargoのfeatures機能で管理（ビルド時制御）。

```toml
[features]
default = ["core", "claude-analysis"]
core = []
claude-analysis = []         # Claude CLI連携（ビルドに含める）※ランタイム検出と併用
yt-download = []             # YouTube/SoundCloudダウンロード（個人用のみ）
beatport-integration = []    # 将来：正規ストリーミング連携
plugin-system = []           # 将来：プラグインシステム
```

配布版ビルド：
```bash
cargo build --release
```

個人開発ビルド：
```bash
cargo build --release --features yt-download
```

### ランタイム検出との関係
- `claude-analysis` featureはClaude CLI連携コードをバイナリに含めるかどうかを制御
- 含めた上で、実行時に`claude`コマンドの存在を検出し、動的に機能ON/OFFを切り替え
- 配布版では`claude-analysis`を含めておき、ユーザーが`claude`をインストール済みかどうかで機能が現れる/消える

---

## 10. 実装ロードマップ

### フェーズ1：基盤（最小再生）
- `conduction-core`：データモデル骨格
- `conduction-audio`：1曲再生、ゲイン調整
- 設定ファイル（TOML）、ロギング（tracing）の基盤構築
- **完了条件**：1曲を再生して音が出る、ログが正しく出力される

### フェーズ2：2デッキ + ミキサー
- 2デッキ並行再生
- クロスフェーダー、3バンドEQ、基本FX
- テンポ調整、キーロック
- Tauri UI骨格、波形表示（DeckView, Mixer）
- メイン出力 / モニタリング出力のルーティング
- **完了条件**：手動で2曲をミックスできる

### フェーズ3：Cue + ライブラリ
- Cueの打ち込み・保存（SQLite）
- 楽曲解析（BPM / キー検出 / クロマグラム / Krumhansl-Schmuckler）
- ビートグリッド手動補正UI（BeatgridCorrection画面）
- Cueエディタ（CueEditor画面）
- ライブラリUI、検索・フィルタ
- **完了条件**：Cueから再生、曲管理ができる、ビートグリッド検証フロー成立

### フェーズ4：Conductor + テンプレート（Conductionの核）
- テンプレートデータモデル
- テンプレート実行エンジン（2層ティック構造）
- Cue動的マッチング（MixSuggestion）
- Visual編集UI（AutomationTimeline）
- Override挙動（OverrideControls, OverrideIndicator, TransportStatusPanel）
- **完了条件**：テンプレートを選んで2曲を自動で繋げる、手動介入も可能

### フェーズ5：自動化と拡張
- Claude CLI連携（ランタイム検出、自動Cue検出、ビートグリッドAIアシスト）
- Script API（WebSocket / OSC / Lua）
- NodeEditor、ScriptEditor
- Setlist画面、SetlistOverview
- クラッシュレポート・セッション復元機能
- **完了条件**：MVPスコープ完了

各フェーズ終了時に動作確認可能な状態を維持する（段階的デリバリー）。

---

## 11. データモデル概要

### Track
```
Track {
  id: Uuid
  path: PathBuf
  title, artist, album, genre: String
  duration: Duration
  bpm: f32
  key: Key              // Camelot形式保持
  energy: f32           // 0.0-1.0
  cues: Vec<Cue>
  beatgrid: Vec<Beat>
  beatgrid_verified: bool   // 手動検証済みフラグ
  analyzed_at: Option<DateTime>
}
```

### Cue
```
Cue {
  id: Uuid
  track_id: Uuid
  position: f64         // 拍数
  type: CueType
  section: Option<Range<f64>>
  
  // マッチング用
  bpm_at_cue: f32
  key_at_cue: Key
  energy_level: f32
  phrase_length: u32
  
  // 繋ぎ設定
  mixable_as: Set<MixRole>    // Entry / Exit / Both
  compatible_energy: Range<f32>
}
```

### Template
```
Template {
  id: Uuid
  name: String
  kind: TemplateKind          // Transition / Setlist
  duration: TimePosition      // 全体の長さ（通常は拍数）
  tracks: Vec<AutomationTrack>
  triggers: Vec<TriggerEvent>
  entry_cue_type: Option<CueType>
  exit_cue_type: Option<CueType>
  setlist: Option<SetlistData> // Setlistのときだけ使う
}

AutomationTrack {
  target: ParameterTarget
  keyframes: Vec<AutomationKeyframe>
}

AutomationKeyframe {
  position: TimePosition
  value: f32
  curve: CurveType            // Linear/EaseIn/EaseOut/EaseInOut/Step/Hold
}

ParameterTarget {
  BuiltIn(BuiltInTarget)      // DeckVolume, DeckEq, Crossfader, etc.
  Custom(String)              // プラグイン・スクリプト拡張用
}
```

### SetlistData
```
SetlistData {
  entries: Vec<SetlistEntry>
}

SetlistEntry {
  track_id: Uuid
  play_from_cue: Option<CueId>
  play_until_cue: Option<CueId>
  transition_to_next: Option<TransitionSpec>
}

TransitionSpec {
  template_id: Uuid
  entry_cue: Option<CueId>
  exit_cue: Option<CueId>
  tempo_transition: TempoTransition
}

TempoTransition {
  mode: TempoMode             // HoldSource/MatchTarget/LinearBlend/MasterTempo
  curve: CurveType
}
```

### Deck State
```
DeckState {
  track: Option<Track>
  position: f64                // 拍数
  playing: bool
  bpm: f32
  key: Key
  tempo_adjust: f32            // -0.16 ~ +0.16
  key_lock: bool
  main_send: f32               // 0.0-1.0
  cue_send: f32                // 0.0-1.0
}
```

### AutomationState（Override管理）
```
AutomationState {
  target: ParameterTarget
  mode: AutomationMode
  resume_strategy: ResumeStrategy  // デフォルトはGlideBack
}

AutomationMode {
  Automated
  Overridden { user_value: f32, override_started_at: BeatPosition }
  Committed { fixed_value: f32 }
  Idle
}

ResumeStrategy {
  GlideBack { duration_beats: f64 }   // デフォルト：1-2拍
  Relative                             // 差分維持モード
}
```

---

## 12. 用語集

- **Conduction**：本プロダクト名。動詞 "conduct"（指揮する）に由来
- **Conductor**：Conductionの司令塔レイヤー。すべての指示を集約し、Audio Engineに翻訳して伝える
- **Cue**：楽曲内の位置マーカー、およびそれに付随する構造的メタデータ
- **テンプレート**：繋ぎ方の設計図。オートメーション曲線とトリガーの集合
- **Cue動的マッチング**：現在のデッキ状態から、ライブラリ全体で繋げられるCueをリアルタイム絞り込み
- **遷移テンプレート**（Transition）：A→Bの繋ぎ単位のテンプレート
- **セットリストテンプレート**（Setlist）：複数曲を連続で繋ぐテンプレート
- **メイン出力 / Main**：観客用のオーディオ出力
- **モニタリング出力 / Cue**：DJヘッドホン用のプリキュー出力
- **PFL**：Pre-Fader Listen。フェーダー前段での信号をモニタリングする方式
- **Override**：テンプレート実行中の手動介入。明示的ボタン操作で開始
- **Resume**：Override後に自動化に復帰する操作（Glide Back / Relative）
- **Commit**：Override値で確定し、そのパラメータのテンプレート制御を終了する操作
- **Abort Template**：テンプレート全体を中断する操作（確認ダイアログあり）
- **Krumhansl-Schmucklerアルゴリズム**：クロマベクトルと24キーのプロファイルの相関計算による音楽キー検出手法
- **Camelotホイール**：DJ用途のキー互換性を示す円形の記法（8A, 5B 形式）

---

## 13. 運用・保守

### 設定ファイル
- 形式：TOML（コメント可、末尾カンマ問題なし、人間可読）
- ユーザー設定：`config.toml`
- テンプレート：`.ctpl`拡張子（TOMLベース）
- セットリスト：`.cset`拡張子（TOMLベース）

### ユーザーデータの保存場所
`directories` クレートでOS規約に準拠したパスを解決。

| OS | 設定 | データベース | キャッシュ | ログ |
|---|---|---|---|---|
| macOS | `~/Library/Application Support/Conduction/config.toml` | 同ディレクトリ/library.db | `~/Library/Caches/Conduction/` | `~/Library/Logs/Conduction/` |
| Windows | `%APPDATA%\Conduction\config.toml` | 同ディレクトリ\library.db | `%LOCALAPPDATA%\Conduction\cache\` | `%APPDATA%\Conduction\logs\` |
| Linux | XDG Base Directory準拠 | | | |

### ロギング
- `tracing` + `tracing-subscriber` で構造化ログ
- 出力先：コンソール + ローテーションするファイル（日次、7日保持）
- オーディオスレッドからは non-blocking アペンダ経由（リアルタイム性を保護）
- ログレベル：設定ファイルで`trace/debug/info/warn/error`を切替

### クラッシュレポート・セッション復元
- 方針：**ローカル保存のみ。外部送信なし**
- クラッシュ時：スタックトレース + 直近のログを`crashes/`配下に保存
- 再生中のセッション状態を定期的にsnapshot → `last_session.toml`
- 次回起動時、未処理のクラッシュレポートがあれば「前回クラッシュから復元しますか？」とプロンプト
- Sentry等の外部送信は将来のオプトイン機能として検討

---

## 14. 未決定・今後詰める項目

- ビートグリッド検出アルゴリズムの細部（aubioのどのパラメータを使うか、複数手法の比較）
- Lua API の完全な仕様書（フェーズ5で詳細化）
- プラグインAPIの具体仕様（次期）
- ネットワーク経由のScript API使用時のセキュリティ（認証トークン設計）
- ライセンス方針（OSS公開 / プロプライエタリ / デュアルライセンス）
- プレビュー用同梱楽曲のライセンス確認
- MIDIマッピングUIの詳細（Settings配下に placeholder あり）

---

## 15. UI実装ガイドライン

### 15.1 デザインシステムファイル配置
```
ui/design-system/
  colors_and_type.css      # 全CSS変数定義（全画面から @import）
  assets/logo.svg          # Conductionワードマーク + モノグラム
  assets/logo-mark.svg     # アイコン単体
  assets/icons/            # カスタムSVGアイコン（Lucideで代用できないもの）
```

すべてのHTML/コンポーネントは先頭で `colors_and_type.css` をインポートする。

### 15.2 コンテンツ・トーン規則

**言語**：日英バイリンガル。主要UIは日本語主 + 英語サブラベル（overline）。
- 例：`ライブラリ / LIBRARY`、`繋ぎ候補 / NEXT CUE`

**日本語の文体**：
- **「です・ます調」は使わない**。命令形・体言止め・名詞句で端的に
- ✅ `ミックスを始める` / `トラックを読み込む` / `BPM 同期`
- ❌ `ミックスを始めましょう` / `トラックを読み込んでください`

**人称**：
- ユーザーを指すときの "あなた" は使わない。動詞スタート
- Conduction自身を指すときも一人称を使わない（擬人化しない）

**表記ルール**：
- 英語ラベルは UPPERCASE + トラッキング広め（overline）
- 数値（BPM / Key / 時刻）は **等幅数字（tabular nums）** 固定
- Key は Camelot記法（`8A`, `5B`）を第一候補、クラシカル（`Gm`, `Bb`）を従
- 日本語と英語の間には半角スペース

**Vibe**：静かな自信（quiet confidence）。マーケティング的な感嘆符や煽り語は使わない。絵文字原則不使用、Unicode記号（♪★▶）を装飾として使わない。

### 15.3 カラートークン

すべて `colors_and_type.css` の CSS 変数として定義済み。新規カラーは勝手に追加しない。

#### ニュートラルスケール（12段階、クール系チャコール）
```
--c-ink-0:   #07080A   /* deepest */
--c-ink-1:   #0C0E12   /* app bg */
--c-ink-2:   #13161C   /* panel bg */
--c-ink-3:   #1A1E26   /* raised panel */
--c-ink-4:   #232833   /* field bg / hover */
--c-ink-5:   #2D3440   /* border / divider */
--c-ink-11:  #F5F7FB   /* fg highest */
```

#### アクセント（Phosphor Mint）
```
--c-accent:     #4FE3B2   /* muted mint, premium feel */
--c-accent-hi:  #7CEEC7   /* hover stop */
--c-accent-lo:  #2BB38A   /* press stop */
--c-accent-glow: rgba(79, 227, 178, 0.28)
```

**アクセントは大面積で塗らない**。CTA、selected state、シグネチャーにのみ使用。

#### デッキ識別色
```
--c-deck-a:  #4FE3B2   /* Deck A — mint */
--c-deck-b:  #E8915A   /* Deck B — amber */
--c-deck-c:  #8A9BE8   /* Deck C — dusted blue (将来用) */
--c-deck-d:  #E87098   /* Deck D — rose (将来用) */
```

#### その他
```
--c-cue:       #E8B868   /* Cueマーカー warm gold */
--c-loop:      #A089DC   /* Loop muted violet */
--c-success:   #00E08A
--c-warning:   #FFB840
--c-danger:    #FF4A5C
--c-info:      #3EA8FF
--c-live:      #FF2D55   /* recording / live */
```

#### Override 4状態色（6.7参照）
- Automated: `#8A9BE8`（dusted blue）
- Overridden: `#E8915A`（amber）
- Committed: `var(--c-ink-6)`（グレー）
- Idle: transparent

### 15.4 タイポグラフィ

```
--font-sans:    "LINE Seed JP", -apple-system, BlinkMacSystemFont, "Hiragino Sans", sans-serif
--font-display: "LINE Seed JP", system-ui, sans-serif
--font-mono:    "JetBrains Mono", "SF Mono", Menlo, ui-monospace, monospace
```

#### サイズスケール
```
--fs-display: 56px   /* hero */
--fs-h1:      40px
--fs-h2:      28px
--fs-h3:      22px
--fs-h4:      18px
--fs-body:    14px
--fs-small:   12px
--fs-micro:   10px
```

#### 等幅数字（必須使用箇所）
- BPM表示
- 時刻表示（00:00形式）
- キー表示
- マッチ率（94%）
- パラメータ数値

```css
.tabular, .bpm, .time {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}
```

### 15.5 スペーシング（4pxグリッド）

```
--s-1: 2px  --s-2: 4px   --s-3: 8px   --s-4: 12px
--s-5: 16px --s-6: 20px  --s-7: 24px  --s-8: 32px
--s-9: 40px --s-10: 56px --s-11: 72px --s-12: 96px
```

**基本間隔**：`--s-4` (12px) / `--s-5` (16px)。DJ UIは情報密度が高いが、詰めすぎない。

### 15.6 角丸（Radii）

```
--r-2: 4px     /* 小チップ・パネル内ボタン */
--r-3: 8px     /* デフォルトコントロール */
--r-4: 12px    /* カード */
--r-5: 16px    /* 大カード */
--r-6: 20px    /* hero / モーダル */
--r-7: 24px
--r-full: 999px
```

**完全な角丸（pill）はトグル・タグ・ステータスバッジのみ**。

### 15.7 Glassmorphism

**階層は2段階のみ**（無闇に層を増やさない）：
- `.glass` — 標準 glass サーフェス
  - `background: rgba(255, 255, 255, 0.05)`
  - `backdrop-filter: blur(16px) saturate(140%)`
- `.glass-strong` — 強化版（フローティングHUD、モーダル）
  - `background: rgba(255, 255, 255, 0.10)`
  - `backdrop-filter: blur(28px) saturate(160%)`

**リムライト**：`inset 0 1px 0 rgba(255,255,255,0.08)` を必須で付ける。

### 15.8 シャドウシステム

```
--shadow-1: 浮遊感わずか、1px以下
--shadow-2: 標準カード
--shadow-3: フローティングパネル・ドロワー
--shadow-4: モーダル・最前面

--inner-top:    inset 0 1px 0 rgba(255,255,255,0.06)
--inner-bottom: inset 0 -1px 0 rgba(0,0,0,0.4)
--inner-glass:  両方 + 左右エッジハイライト

--glow-accent:  0 0 0 1px var(--c-accent), 0 0 16px 0 var(--c-accent-glow)
--glow-live:    0 0 0 1px var(--c-live), 0 0 20px 0 rgba(255,45,85,0.4)
```

**常時グローは禁止**。アクセントがアクティブな時のみ発光。

### 15.9 アニメーション

**イージング**：
```
--ease-out:    cubic-bezier(0.2, 0.8, 0.2, 1)   /* 基本 */
--ease-in-out: cubic-bezier(0.6, 0, 0.2, 1)
--ease-spring: cubic-bezier(0.34, 1.56, 0.64, 1) /* 音楽的インタラクションのみ */
```

**デュレーション**：80 / 160 / 260 / 420ms の4段階。**500msを超えない**。

**禁止事項**：
- 装飾的アニメーション（ふわふわ動く背景グラデ等）
- 500ms超の長いトランジション
- spring/bounceをUI一般に使うこと（Cue hit、Play ボタンのパルス等の音楽的インタラクションにのみ許可）

**波形スクロールはlinear**（リアルタイム挙動を壊さない）。

### 15.10 レイアウト定数

```
--titlebar-h:  36px   /* 上部タイトルバー */
--toolbar-h:   52px   /* ツールバー */
--transport-h: 72px   /* 下部トランスポート（固定） */
--mixer-w:     320px  /* ミキサー幅（将来縦レイアウト時） */
--sidebar-w:   248px  /* サイドバー */
```

**Mix画面レイアウト**：
- 上部：TopBar（titlebar 36px + toolbar 52px）
- 左：Sidebar（232px）
- 中央：Deck A（横長ストリップ）→ Mixer → Deck B の縦スタック
- 下部：Library
- フルスクリーン時はタイトルバーを隠せる

### 15.11 主要コンポーネント（提供済み .jsx）

#### レイアウト
- `App.jsx` — ルート、画面切替
- `Sidebar.jsx` — ナビゲーション
- `TopBar.jsx` — 上部バー
- `tweaks-panel.jsx` — 開発用テーマ切替

#### Mix画面
- `DeckView.jsx` — 横長デッキストリップ
- `Mixer.jsx` + `Knob` + `Fader` — ミキサー
- `MixSuggestion.jsx` — 繋ぎ候補HUD
- `Library.jsx` + `TrackList.jsx` — ライブラリ

#### Templates画面
- `Templates.jsx` — メインコンテナ
- `TemplateList.jsx` — 左カラム（テンプレート一覧）
- `TemplateEditor.jsx` — 右エディタ（モード切替ヘッダ + PreviewPanel）
- `AutomationTimeline.jsx` — Visualモード（DAW風タイムライン）
- `NodeEditor.jsx` — Nodeモード
- `ScriptEditor.jsx` — Scriptモード

#### Setlist画面
- `Setlist.jsx` — メインコンテナ
- `SetlistTrackBlock.jsx` — 曲ブロック
- `SetlistTransitionBlock.jsx` — 遷移テンプレート指定
- `SetlistOverview.jsx` — 右パネル（BPMカーブ・キー遷移・エネルギー）

#### Beatgrid画面
- `BeatgridCorrection.jsx` — ビートグリッド検証・補正

#### Cues画面
- `CueEditor.jsx` — メインコンテナ
- `CueListItem.jsx` — Cue行アイテム

#### Override
- `OverrideIndicator.jsx` — ステータス帯（knob/fader周囲の色帯）
- `OverrideControls.jsx` — OVR/Resume/Commitボタン
- `TransportStatusPanel.jsx` — テンプレート実行状態（Transport領域）

#### Settings
- `Settings.jsx` — 設定画面（General / Audio / MIDI / Mixing AI / Shortcuts / Account）

### 15.12 実装上の注意

**CSS変数のみを使用**。ハードコードの色値は、デザインシステム未定義分のみ（実装時は定義追加を検討）。

**既存コンポーネントの再利用**を優先。新規コンポーネントを作る前に、既存で代用できないか確認。

**不透明度の変化は使わない**（グラス面では汚く見える）。色そのものを変える。

**フォーカスリング**：
```css
outline: none;
box-shadow: 0 0 0 2px var(--c-ink-1), 0 0 0 3.5px var(--c-accent);
```

---

*Document version: 0.3*  
*Updated: デザインシステム統合、UI実装ガイドライン追加、Override挙動の確定、画面構成の明確化*
