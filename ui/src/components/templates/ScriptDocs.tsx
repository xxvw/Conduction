// Conduction Script (Lua) のリファレンスドキュメント。
// ScriptEditor 右側のスライドパネルとして表示される。
// 内容は conduction-script の Rust 実装 (lib.rs の PRELUDE / Tauri command の
// add_keyframe / add_track / set_duration) と同期している。実装が変わったら
// ここも更新する必要がある。

interface ScriptDocsProps {
  onClose: () => void;
}

const TARGETS: ReadonlyArray<{ key: string; desc: string }> = [
  { key: "crossfader", desc: "−1 (A 100%) … 0 (中央) … +1 (B 100%)" },
  { key: "master_volume", desc: "0 (無音) … 1 (1×) … 2 (+6dB)" },
  { key: "deck_volume.A / B", desc: "0 … 2 (チャンネルボリューム)" },
  { key: "deck_eq_low.A / B", desc: "−26 (kill) … +6 dB" },
  { key: "deck_eq_mid.A / B", desc: "−26 … +6 dB" },
  { key: "deck_eq_high.A / B", desc: "−26 … +6 dB" },
  { key: "deck_filter.A / B", desc: "−1 (LP) … 0 (off) … +1 (HP)" },
  { key: "deck_echo_wet.A / B", desc: "0 (dry) … 1 (wet)" },
  { key: "deck_reverb_wet.A / B", desc: "0 (dry) … 1 (wet)" },
];

const CURVES: ReadonlyArray<{ key: string; desc: string }> = [
  { key: "linear", desc: "等速。前後をまっすぐ繋ぐ" },
  { key: "ease_in", desc: "ゆっくり始まり、次第に加速 (t²)" },
  { key: "ease_out", desc: "速く始まり、次第に減速 (1 − (1 − t)²)" },
  { key: "ease_in_out", desc: "S字 (両端で減速、中央で加速)" },
  { key: "step", desc: "瞬時にジャンプ (次の keyframe 直前まで現在値保持)" },
  { key: "hold", desc: "step と同じ動作。「無視」を表す UI マーカー" },
];

export function ScriptDocs({ onClose }: ScriptDocsProps) {
  return (
    <aside className="script-docs" aria-label="Conduction Script documentation">
      <header className="script-docs-head">
        <h3>Conduction Script — Reference</h3>
        <button
          type="button"
          className="chip"
          onClick={onClose}
          aria-label="Close docs"
          title="Close (or click × / press Esc)"
        >
          ×
        </button>
      </header>

      <div className="script-docs-body">
        <section>
          <h4>Overview</h4>
          <p>
            Lua スクリプトを書いて Compile すると、AutomationTrack 群が生成され
            Visual / Node エディタにも反映される。実行時 (再生中) には Lua は走らない
            (保存時のみコードを評価して keyframe を構築する)。
          </p>
          <p className="hint">
            <kbd>⌘/Ctrl</kbd> + <kbd>Enter</kbd> で Compile。
          </p>
        </section>

        <section>
          <h4>Globals</h4>
          <dl>
            <dt>
              <code>duration_beats</code>
            </dt>
            <dd>
              テンプレート全長 (拍)。<code>set_duration</code> で変更可能。
              初期値は呼び出し元 (UI の duration_beats フィールド) から渡される。
            </dd>
          </dl>
        </section>

        <section>
          <h4>Built-in functions</h4>
          <dl>
            <dt>
              <code>set_duration(beats: number)</code>
            </dt>
            <dd>テンプレート全長 (拍) を設定する。<code>beats</code> は正の有限数。</dd>

            <dt>
              <code>add_keyframe(target, beat, value [, curve])</code>
            </dt>
            <dd>
              指定ターゲットの該当拍に 1 つの keyframe を追加する。
              <code>curve</code> 省略時は <code>"linear"</code>。
            </dd>

            <dt>
              <code>add_track(target, keyframes_table)</code>
            </dt>
            <dd>
              keyframes をまとめて 1 つの target に追加する。各要素は
              <code>{`{beat = N, value = V, curve = "linear"}`}</code> または
              <code>{`{N, V, "linear"}`}</code>。
            </dd>
          </dl>
        </section>

        <section>
          <h4>Prelude helpers</h4>
          <dl>
            <dt>
              <code>clamp(x, min, max)</code>
            </dt>
            <dd>
              <code>x</code> を <code>min</code>..<code>max</code> に丸める。
            </dd>

            <dt>
              <code>lerp(a, b, t)</code>
            </dt>
            <dd>
              線形補間。<code>t</code> は 0..1 に clamp してから計算。
            </dd>

            <dt>
              <code>smoothstep(a, b, t)</code>
            </dt>
            <dd>
              Hermite smooth (3t² − 2t³) で <code>a</code>..<code>b</code> を補間。
            </dd>

            <dt>
              <code>each_bar(callback)</code>
            </dt>
            <dd>
              <code>0 → duration_beats</code> を 4 拍 (= 1 bar) 刻みで反復し、
              <code>callback(beat, bar_index)</code> を呼ぶ。
            </dd>

            <dt>
              <code>each_phrase(beats_per_phrase, callback)</code>
            </dt>
            <dd>
              指定拍数ごとに <code>callback(beat, phrase_index)</code> を呼ぶ。
            </dd>
          </dl>
        </section>

        <section>
          <h4>Targets</h4>
          <table className="script-docs-table">
            <tbody>
              {TARGETS.map((t) => (
                <tr key={t.key}>
                  <td>
                    <code>{t.key}</code>
                  </td>
                  <td>{t.desc}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        <section>
          <h4>Curves</h4>
          <table className="script-docs-table">
            <tbody>
              {CURVES.map((c) => (
                <tr key={c.key}>
                  <td>
                    <code>"{c.key}"</code>
                  </td>
                  <td>{c.desc}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        <section>
          <h4>Examples</h4>
          <p>32 拍で A → B にゆっくり swap:</p>
          <pre className="script-docs-code">{`set_duration(32)
add_keyframe("crossfader", 0,  -1, "linear")
add_keyframe("crossfader", 32,  1, "ease_in_out")`}</pre>

          <p>各 bar の頭で Filter sweep:</p>
          <pre className="script-docs-code">{`set_duration(16)
each_bar(function(b, bar)
  add_keyframe("deck_filter.A", b, smoothstep(-1, 1, bar / 4))
end)`}</pre>

          <p>add_track で keyframes をまとめて指定:</p>
          <pre className="script-docs-code">{`add_track("deck_eq_low.A", {
  {beat = 0,  value = 0,    curve = "hold"},
  {beat = 16, value = 0,    curve = "ease_in"},
  {beat = 32, value = -26,  curve = "hold"},
})`}</pre>
        </section>
      </div>
    </aside>
  );
}
