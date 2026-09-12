# Controller 接続と実機検証

Controller は Conduction の 2 デッキを直接演奏する画面です。Pro DJ Link と各 MIDI プリセットは **実験的対応・実機未検証** です。今回の実装作業では対象機材への接続を確認できず、実機での選曲・ロード・音声出力や 60 分の連続演奏試験は実施していません。自動テストの成功を機材の互換性確認として扱わないでください。

## 接続する構成

| 対象 | 選ぶ構成と音声経路 | 現在の範囲 |
| --- | --- | --- |
| CDJ-3000 / CDJ-2000NXS2: 本体で演奏 | Ethernet で Conduction のライブラリを参照。CDJ の音声出力をミキサーへ接続 | 機器検出、選曲・読み込みのプロトコル、拍通信。実機動作は未確認 |
| CDJ-3000 / CDJ-2000NXS2: USB 操作 | CDJ をソフトウェア操作モードにし、MIDI ポートごとに Conduction の A / B を指定。音は Conduction の選択出力から出す | Transport、ジョグ、テンポ、Sync、ループ、Hot Cue、選曲。液晶ミラーリングと LED 出力は対象外 |
| DJM-A9 | USB 音声デバイスを選び、External で A / B を独立出力。既定 MIDI は物理 CH1 → A / CH3 → B | MIDI チャンネル 1、ヘッドホン A を前提。CH2 の曖昧な仕様項目は未割り当て。LED 受信機能なし |
| DDJ-FLX4 | Internal で USB の MAIN / ヘッドホン CUE 出力を指定 | 2 デッキ、ミキサー、対応 LED、共通 Echo / Reverb。Smart CFX / Smart Fader は対象外 |
| DDJ-FLX10 | Internal で USB の MAIN / ヘッドホン CUE 出力を指定 | デッキ 1 / 2、HOT CUE ページ 1、対応 LED。デッキ 3 / 4、STEMS、ジョグ画面は対象外 |

**同じ CDJ では本体演奏と USB デッキ操作を選びます。** NXS2 の取扱説明書にも、本体へ曲をロードするとソフトウェア操作を終了する旨が記載されています。LAN 上の他のプレーヤーと Conduction を同期させながら、別の USB コントローラーで Conduction を操作する構成は検証対象に含めます。[CDJ-2000NXS2 取扱説明書](https://downloads.support.alphatheta.com/manuals/CDJ_2000NXS2_DRI1290A_manual.pdf)

## 音声と接続の設定

1. **Controller → 接続と出力 → Audio** で音声デバイスと方式を選択します。A / B の再生と自動化を停止してから適用します。
2. **Internal:** MAIN と CUE を別ステレオペアへ割り当てます。CUE は別デバイスでも指定できます。ソフトの EQ・フェーダー・クロスフェーダーでミックスし、HEADPHONE CUE と CUE / MASTER バランスで試聴します。
3. **External:** Deck A と Deck B を別ステレオペアへ割り当てます。EQ・フィルタ・音量・クロスフェーダー・マスターとヘッドホンは実機で操作します。デッキ FX は残り、無効なソフトミキサー操作を使う自動化は開始できません。
4. **番号を実機で確認します。** 画面の `1 / 2` はオーディオインターフェースの出力番号です。DJM-A9 の物理 CH1 とは別の番号体系です。A9 の既定 MIDI 割り当てを使う場合、ドライバーと本体の入力選択を合わせ、A の信号が CH1、B が CH3 に届くペアを選びます。機種名だけから固定ペアを決めません。
5. **MIDI** タブで入力・プリセット・必要なら対応 LED 出力を指定します。CDJ を 2 台使う場合は各ポートに A / B を指定します。切断後は明示的に再接続し、フェーダーを現在のソフトの値まで動かして操作を引き継ぎます。
6. **Pro DJ Link** タブでネットワークインターフェース、送信元 A / B、空きプレーヤー番号を選びます。LAN 公開は初期状態でオフです。公開中は登録済み楽曲と生成済みキャッシュが選択ネットワークから読み取り可能になります。

MIDI の機種別割り当てと未対応操作は [conduction-midi](../crates/conduction-midi/README.md)、通信の実装範囲は [conduction-link](../crates/conduction-link/README.md) を参照してください。

## 現在の制限

- Link は公開されたプロトコル解析に基づく独立実装です。CDJ にライブラリが表示され、選曲・同時ロード・拍同期まで完了することは、下記の実機試験で確認する必要があります。表示される `hardware_verified` は未検証のままです。
- 波形は Conduction の解析済み 3 バンド RMS から従来形式の青い波形へ変換します。既存の全体波形を補間するため詳細表示は概算です。高度なカラー / 3 バンド解析タグ、アートワーク、ネイティブ解析ファイルの書き出しには未対応です。
- NFSv2 の範囲を超える 4 GiB 以上の音源、書き込み要求、カタログ外のパスは対象外です。OGG や対象形式・レート・ビット深度に収まらない音源は 44.1 kHz / 16-bit ステレオ WAV へ事前変換します。ALAC も、現在の配信メタデータで M4A を AAC と区別できないため WAV キャッシュを使います。読み込めない曲は公開一覧から除外し、画面に理由を表示します。
- 同期には解析済み拍グリッドが必要です。実際の遅延・拍位相・音声ドロップは計測が必要です。ネットワーク上の機器が見えることだけでは同期完了と判定しません。

## 自動テストと実機試験の区別

ソフトウェアの自動テストは、仮想音声出力によるルーティングと再生時計、MIDI メッセージの再生・LED・操作引き継ぎ、Link のフレーム解析とループバック通信、Unicode メタデータ、NFS の許可ファイル、SQLite 移行・ID 維持、ステレオ変換を確認します。実際の USB ポートや物理 LAN に演奏データを送る試験ではありません。

2026-09-13 の実装時確認（macOS 26.5 / Rust 1.96.0 / Node.js 24.15.0）:

| 確認 | 結果 |
| --- | --- |
| `cargo test --workspace` | 300 件成功、既存のローカル音源依存 2 件は ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | 成功 |
| `cargo fmt --all -- --check` | 成功。stable rustfmt では既存の nightly 専用設定 2 項目が無視される旨の警告あり |
| `cd ui && npm test` | 38 件成功 |
| `cd ui && npm run build` | TypeScript 検査・本番ビルド成功。既存の Monaco を含む大きなチャンクへの警告あり |
| Controller の 1024×640 / 1280×800 / 1920×1080 | 検証用データを使ったブラウザー確認で横溢れ・JavaScript エラーなし。低い画面ではライブラリへ縦スクロール |

Link の初期接続手順全体とメディア初期化応答は、独立した完全な通信記録が得られていないため公開リファレンスの観察情報に基づきます。通信記録から検証した個別のパケット・メタデータ・NFS 処理と区別し、実機の接続からロードまでを確認する必要があります。

```bash
cargo test -p conduction-audio --lib
cargo test -p conduction-midi
cargo test -p conduction-link
cargo test -p conduction-library --lib
cargo test -p conduction-app --lib --no-default-features
```

機材を用意したら、上の構成ごとに次を記録してください。チェック欄は実機確認が済むまで空欄にします。

- [ ] OS とバージョン、Conduction のコミット、機種・台数・各ファームウェア、USB ドライバー、LAN アダプター・スイッチ、サンプルレート、出力ペアを記録。
- [ ] Internal の MAIN / CUE と左右チャンネル、External の A / B → A9 CH1 / CH3 の分離を片側ずつ再生して確認。出力切断時の停止、停止中のみの設定変更、再接続を確認。
- [ ] Play / Transport Cue の押下・解放、ジョグ・スクラッチ、テンポ、Sync、8 Hot Cue、ループ、EQ、フェーダー、対応 FX と LED を確認。MIDI 抜去後に押下状態が残らず、再接続後のフェーダーが急変しないことを確認。
- [ ] CDJ-3000 と CDJ-2000NXS2 の混在 LAN で、日本語の全曲・アーティスト・検索・セットリストから選曲し、2 台で同時ロード。原本形式と変換 OGG、波形、8 Hot Cue の表示を確認。
- [ ] ライブラリ配信と仮想プレーヤーの拍送信を同時実行。マスター交代、A / B の追従、ネットワーク切断時のテンポ維持、復帰、番号競合・空き番号なしを確認。既存プレーヤーの番号を奪わないことを確認。
- [ ] 各構成で **60 分連続再生**。開始時と終了時の拍ずれ、途中の最大ずれ、補正値、音声途切れカウンターと可聴ドロップを記録。CDJ 内部演奏と Conduction の出力を同時録音して比較。
- [ ] 1024×640 / 1280×800 / 1920×1080 で操作・接続設定を確認し、画面を切り替えても再生・MIDI 操作が続くことを確認。

| 実施日 / コミット | OS / ドライバー | 機種・FW / 接続構成 | 60 分 / 最大拍ずれ / ドロップ | 結果・記録へのリンク |
| --- | --- | --- | --- | --- |
| 未実施 | 未記録 | 対象実機の接続なし | 未計測 | 実機未検証 |

仕様の根拠: [AlphaTheta CDJ-3000 MIDI 表](https://downloads.support.alphatheta.com/software_info/dj-players/CDJ-3000/CDJ3000_MIDI_Message_List_E108.pdf)、[DJM-A9 MIDI 表](https://downloads.support.alphatheta.com/midi-mapping/dj-mixers/DJM-A9/DJM-A9_MIDI_Message_List_E_10.pdf)、[FLX4 MIDI 表](https://downloads.support.alphatheta.com/software_info/dj-controllers/DDJ-FLX4/DDJ-FLX4_MIDI_message_List_J1.pdf)、[FLX10 MIDI 表](https://downloads.support.alphatheta.com/software_info/dj-controllers/DDJ-FLX10/DDJ-FLX10_MIDI_Message_List_E1.pdf)、[NXS2 対応音源形式](https://www.pioneerdj.com/en/news/2016/meet-the-new-cdj-2000nxs2-and-djm-900nxs2/)、[DJ Link 通信解析](https://djl-analysis.deepsymmetry.org/djl-analysis/track_metadata.html)。
