# Conduction — Usage Guide

機能と使い方をまとめたドキュメント。インストール手順は [`install.md`](./install.md) を参照。

---

## 1. Conduction とは

プロのライブ現場向けの **プログラマブル DJ エンジン**。人間の UI 操作・AI 提案・Lua スクリプトが、同じ Conductor 層に対する "指示" として等価に扱われるのが設計思想。

- 2 デッキ + クロスフェーダー + マスター + EQ / フィルタ / FX の標準ミキサー構成
- BPM・キー解析、Hot Cue、ループ、ビートシンク、キーシンク、テンポレンジ切替
- 自動化テンプレート (Visual / Node / Lua) で「次の遷移」を予約・実行
- セットリスト管理 + `.cset` エクスポート/インポート
- YouTube から取り込み (yt-dlp 連携)

---

## 2. 画面構成

トップバーから 6 画面を切り替える。

| 画面 | 役割 |
|------|------|
| **Mix** | 2 デッキ + ミキサー + 自動化トランスポート。本番中はここに居る |
| **Library** | 取り込んだトラックの一覧と検索、デッキへのロード |
| **Templates** | 自動化テンプレートを作る (Visual / Node / Lua の 3 タブ) |
| **Setlist** | プレイ順をピン留めし、トランジションを設計する。`.cset` 入出力 |
| **YouTube** | URL から音源を取り込む (feature: yt-download) |
| **Settings** | キー割り当て、オーディオデバイス、ライブラリパス |

右上の `⌨ Keys` ボタンでショートカット一覧が出る。

---

## 3. 起動して最初にやること

1. **Library 画面**: 楽曲のあるフォルダをスキャンする → BPM / キー / Hot Cue が自動推定される
2. **Mix 画面**: 左のデッキ A に Load… で曲をロード。Play で再生開始
3. ロード直後は波形の解析バーが流れる。終わると拍グリッド/Hot Cue が描画される
4. デッキ B にも別の曲をロードし、`BEAT SYNC` でテンポを合わせる
5. 必要に応じて `KEY SYNC` で Camelot キーを揃える (半音単位の pitch-shift)
6. クロスフェーダーや EQ を触ってミックスする。`MixSuggestion` パネルが次曲候補を提案してくれる

---

## 4. Mix 画面の中身

### 4-1. デッキ (DECK A / DECK B)

- **波形**: 全体波形 (上段) と拡大波形 (下段、ズーム可)。クリックでシーク
- **TEMPO**: ±6 / ±10 / ±16 % のレンジを切替。`MT` で master tempo (keylock) を有効化するとピッチ保存のままテンポだけ動く
- **BEAT SYNC**: 反対デッキの effective BPM (track BPM × playback_speed) に自分の playback_speed を合わせる
- **KEY SYNC**: 反対デッキの Camelot key への最短半音差を pitch_offset に保存
- **CUE ボタン**: PFL (ヘッドフォン送り)。出力デバイスに CUE バスを割り当てておく必要あり (Settings)
- **CH VOLUME**: チャンネルフェーダー (0.0〜2.0)

### 4-2. EQ / FX / Filter (FxPad)

- 各デッキに LOW / MID / HIGH の 3 バンド EQ
- フィルタ (HPF/LPF 兼用ノブ)
- FX (リバーブ / ディレイ等。要件 §6 参照)

### 4-3. Loop (LoopPad)

- `[` で IN、`]` で OUT (どちらも最寄りビートにスナップ)
- `\` で ON/OFF
- `.` で 1 bar 拡張、`,` で 1 bar 縮小

### 4-4. Hot Cue (HotCuePad)

- 数字キー `1`〜`8` でジャンプ
- `Shift + 1〜8` で現在位置を保存 (ビートスナップあり)
- `Alt + 1〜8` で削除

### 4-5. Cue (typed Cue: Drop / Intro / Breakdown / Outro)

トラックの構造を意味付きでマークする。MixSuggestion はここを基準に「次曲のここから入れる」候補を出す。

### 4-6. Crossfader / Master

- クロスフェーダーは −1.0 (A) 〜 +1.0 (B) の連続値
- Center ボタンで 0 に戻す
- Master スライダーは画面上部にも常駐

### 4-7. MixSuggestion パネル

アクティブデッキの BPM / Key / Energy に近い候補を、未ロード側デッキ向けに提案する。

- `Enter` で 1 位を反対デッキにロード + Cue 位置にシーク
- `Esc` で dismiss

### 4-8. Template Launcher / Transport Status

`Template Launcher` で自動化テンプレートを選び「Start」。実行中は `TransportStatusPanel` に進捗が出る。

- `Reverse` チェックで B→A 方向に反転して走らせる (Deck A↔B + クロスフェーダー符号を内部で flip)
- `Shift + Esc` で abort 確認ダイアログ
- 個別ターゲットの上書きは `OverrideControls` から (O = Override / R = Resume / C = Commit)

---

## 5. Templates 画面

Templates 画面では自動化テンプレートを作成・編集できる。3 つのタブで、同じ Template モデルを別々の角度から扱う。

### 5-1. Visual タブ

`AutomationTimeline` 上で、ターゲット別 (crossfader, deck_volume.A, deck_eq_low.A …) に keyframe を打つ。

- ドラッグでキーフレームの位置と値を変更
- カーブタイプ (linear / smooth / ease-in / ease-out) を keyframe 単位で切替
- 直接プレビューが反映される

### 5-2. Node タブ

`react-flow` ベースのノードエディタ。トラックごとに **Source → Target** のノードペアを並べる薄め MVP。

- 複雑な依存を視覚化したいとき向け
- Source ノードは「いま何拍め」「BPM」など、Target ノードは「crossfader = …」のような書き込み先

### 5-3. Script タブ (Lua)

Monaco エディタで Lua スクリプトを書く。Lua は **コードジェネレータ** として扱われ、コンパイル時に Template AST に変換される (オーディオスレッドで Lua は走らない)。

`Cmd/Ctrl + Enter` でコンパイル。エラーは Monaco の inline marker として表示される。

#### 主要 API (global)

```lua
set_duration(bars)            -- テンプレートの全長 (bars 単位)
set_direction("a_to_b" | "b_to_a")
keyframe(target, beat, value, curve)
-- target: "crossfader" | "deck_volume.A" | "deck_eq_low.A" ... 
-- curve : "linear" | "smooth" | "ease_in" | "ease_out"
```

#### Prelude ヘルパ (自動 require 不要)

| 名前 | 概要 |
|------|------|
| `clamp(x, lo, hi)` | クランプ |
| `lerp(a, b, t)` | 線形補間 |
| `smoothstep(a, b, t)` | smoothstep 補間 |
| `each_bar(bars, fn)` | 1 bar ごとに `fn(bar, beat)` を呼ぶイテレータ |
| `each_phrase(bars, fn)` | 4 bar (phrase) ごとに `fn(phrase, beat)` を呼ぶイテレータ |

#### サンプル: 16 bar かけて A → B へクロスフェードしつつ Low を落とす

```lua
set_duration(16)

keyframe("crossfader", 0,  -1.0, "linear")
keyframe("crossfader", 64, 1.0,  "smooth")

each_bar(16, function(bar, beat)
  local t = bar / 16
  keyframe("deck_eq_low.A", beat, lerp(1.0, 0.0, smoothstep(0, 1, t)), "linear")
end)
```

#### Docs パネル

エディタ右上のボタン (もしくは Help アイコン) で **Conduction Script Docs** のスライドインパネルが開く。Overview / Globals / Built-in functions / Prelude / Targets table / Curves table / Examples が入っている。`Esc` で閉じる。

#### ビルトインプリセットの Lua ソース

5 つのビルトインプリセット (linear_16bar, crossfade_outro_intro, eq_swap, filter_sweep, build_drop) はすべて Lua ソース付き。Templates 画面で「View source」を押すと Lua コードが出てくるので、コピーして自分用に改造できる。

### 5-4. ユーザーテンプレートのストレージ

ユーザーが作成したテンプレートは `user_templates` テーブルに保存される (`conduction-library` の SQLite)。Templates 画面の右ペインから:

- Duplicate (複製)
- Rename
- Delete

ができる。プリセットは `preset.*`、ユーザーテンプレートは `user.*` のプレフィックスで識別される。

---

## 6. Setlist 画面

プレイ順とトランジションを設計するための画面。

- 左ペイン: トラック一覧をドラッグして並び替え
- 右ペイン: `SetlistOverview` で曲ごとのトランジション秒数・テンプレート割当
- `.cset` 形式でエクスポート / インポート (他人と共有・別マシンに持ち運び可能)

「Load to Deck」を押すと、現在ハイライトしている曲がデッキ A もしくは B に流し込まれる。

---

## 7. Library 画面

- 取り込み済みトラックの一覧、メタデータ (Title / Artist / BPM / Key / Energy / Length)
- ダブルクリックでアクティブなデッキにロード
- 右クリックメニューから再解析やパスのコピー

スキャンは初回フォルダ指定後、バックグラウンドで進む。BPM / キーは `conduction-analysis` クレートが推定する。

---

## 8. YouTube 画面 (feature: yt-download)

URL を貼って Import を押すと、yt-dlp がローカルにダウンロード → 自動解析 → Library に追加される。

`cargo build --features yt-download` でビルドされたバイナリのみ有効。

---

## 9. Settings 画面

- **Audio output**: マスター / CUE バスにそれぞれ別の出力デバイスを割り当て可能
- **Key bindings**: 後段 (Phase 3b-10) で画面から変更可。現状はハードコードのデフォルト
- **Library path**: トラック取り込みのルート

---

## 10. キーボードショートカット

| キー | 動作 |
|------|------|
| `Space` | 再生 / 一時停止 (active deck) |
| `Q` / `W` | デッキ A / B にフォーカス |
| `←` / `→` | 1 拍 back / fwd (Shift で微小シーク) |
| `I` / `O` | 2 拍 back / fwd |
| `K` / `L` | 4 拍 back / fwd |
| `↑` / `↓` | ズーム in / out |
| `1`〜`8` | Hot Cue 1〜8 にジャンプ |
| `Shift + 1〜8` | 現在位置を Hot Cue として保存 |
| `Alt + 1〜8` | Hot Cue 削除 |
| `[` / `]` / `\` | Loop IN / OUT / Toggle |
| `.` / `,` | Loop +1 / −1 bar |
| `Enter` | MixSuggestion の 1 位を採用 |
| `Esc` | MixSuggestion を dismiss / docs パネル閉じる |
| `Shift + Esc` | 実行中テンプレートを Abort |
| `O` / `R` / `C` | フォーカス中ターゲットの Override / Resume / Commit |
| `Cmd/Ctrl + Enter` | Script エディタでコンパイル実行 |

---

## 11. データの保存先

- SQLite: `~/Library/Application Support/conduction/library.sqlite3` (macOS)
- スキーマバージョンは `schema.rs::CURRENT_SCHEMA_VERSION` で管理 (現在 v5)。起動時に自動マイグレーション
- Waveform / beats / cues / hot cues / setlists / user_templates が同 DB に格納される
- `.cset` ファイルは setlist 全体 + 参照トラックの軽量メタを 1 ファイルにまとめる

---

## 12. アーキテクチャの読み方

UI で何かを操作すると `ui/src/lib/ipc.ts` 経由で Tauri command が呼ばれる → `crates/conduction-app/src/commands.rs` で受け取る → 必要なクレート (audio / conductor / library / script) に委譲する。

オーディオスレッドは `cpal` のコールバックで回り、ステート同期は `arc-swap` + `ringbuf` で行う。10 Hz の mixer snapshot が UI に降りてきて、波形カーソルだけ 60 Hz に補間して描く。

詳しくは [`conduction-requirements.md`](./conduction-requirements.md)。

---

## 13. トラブルシューティング

| 症状 | 対処 |
|------|------|
| 起動しても「audio engine connecting…」のまま | オーディオデバイスを他アプリが専有 / Settings で出力選択 / 一度アプリを終了 |
| ポート 38127 が使用中エラー | `lsof -ti:38127 \| xargs kill` |
| Monaco エディタの補完が空欄 | ブラウザキャッシュ削除 / dev サーバ再起動 |
| BPM/Key が出ない | 解析が走っていない可能性。Library から再解析を試す |
| Lua コンパイルエラー: `attempt to call a nil value (global ...)` | Conduction Script API のサンドボックスは math / string / table のみ許可。OS 系 API は使えない |
| `.cset` インポートで一部曲が無い | 参照先トラックが当該マシンに存在しない。Library 取り込みを先に行う |
