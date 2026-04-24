// ScriptEditor.jsx — Lua-like script editor with syntax highlight + API drawer
function ScriptEditor({ template, setTemplate }) {
  const defaultScript = template.script || `-- Conduction transition script
-- テンプレート: ${template.name}
-- 実行時に各 beat/bar でこのスクリプトが評価される

local A = conduction.deck(1)   -- Deck A
local B = conduction.deck(2)   -- Deck B
local xf = conduction.crossfader

-- エントリー: Bar 0 で Deck B をバックグラウンドで再生
conduction.on_bar(0, function()
  B:play()
  B:set_volume(0.0)
  xf:set(0.0)  -- fully on A
end)

-- 16 bars かけて EQ とクロスフェーダーでブレンド
conduction.transition_at(0, 16, function(progress)
  -- progress: 0.0 → 1.0
  A:set_eq("low",  1.0 - progress)
  B:set_eq("low",  progress)
  B:set_volume(progress)
  xf:set(progress * 0.5)
end)

-- Bar 16 でドロップ: クロスフェーダー一気に B 側へ
conduction.on_bar(16, function()
  xf:set(1.0, { curve = "easeOut", duration_beats = 2 })
  A:set_volume(0.0)
end)

-- 終了時に Deck A をクリーンアップ
conduction.on_complete(function()
  A:stop()
end)
`;

  const [src, setSrc] = React.useState(defaultScript);
  const [drawerOpen, setDrawerOpen] = React.useState(true);

  React.useEffect(() => {
    const t = setTimeout(() => setTemplate(p => ({ ...p, script: src })), 400);
    return () => clearTimeout(t);
  }, [src]);

  const highlighted = React.useMemo(() => highlightLua(src), [src]);
  const lineCount = src.split('\n').length;

  return (
    <div style={{ flex: 1, display: 'flex', minHeight: 0, background: 'var(--c-ink-0)' }}>
      {/* Editor */}
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '10px 14px', borderBottom: '1px solid var(--stroke-1)', background: 'var(--c-ink-2)' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <div style={{ width: 8, height: 8, borderRadius: 2, background: '#A089DC' }}/>
            <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--fg-2)', fontWeight: 500 }}>transition.lua</span>
          </div>
          <span className="overline" style={{ fontSize: 9 }}>LUA 5.4</span>
          <div style={{ flex: 1 }}/>
          <button className="btn-xs"><i data-lucide="play" style={{ width: 10, height: 10 }}/> TEST RUN</button>
          <button className="btn-xs" onClick={() => setDrawerOpen(o => !o)}>
            <i data-lucide="book-open" style={{ width: 10, height: 10 }}/> API
          </button>
        </div>

        <div style={{ flex: 1, position: 'relative', overflow: 'auto', fontFamily: 'var(--font-mono)', fontSize: 13, lineHeight: 1.6 }}>
          <div style={{ display: 'flex', minHeight: '100%' }}>
            {/* gutter */}
            <div style={{
              flexShrink: 0, padding: '12px 10px 12px 14px', textAlign: 'right',
              background: 'var(--c-ink-1)', borderRight: '1px solid var(--stroke-1)',
              color: 'var(--fg-5)', userSelect: 'none', minWidth: 44,
            }}>
              {Array.from({ length: lineCount }).map((_, i) => (
                <div key={i} className="tabular" style={{ fontSize: 11 }}>{i + 1}</div>
              ))}
            </div>

            {/* code */}
            <div style={{ flex: 1, position: 'relative', padding: '12px 16px', minWidth: 0 }}>
              <pre
                aria-hidden
                style={{
                  position: 'absolute', inset: '12px 16px', margin: 0,
                  fontFamily: 'var(--font-mono)', fontSize: 13, lineHeight: 1.6,
                  whiteSpace: 'pre-wrap', wordBreak: 'break-word',
                  pointerEvents: 'none', color: 'var(--fg-2)',
                }}
                dangerouslySetInnerHTML={{ __html: highlighted }}
              />
              <textarea
                value={src}
                onChange={e => setSrc(e.target.value)}
                spellCheck={false}
                style={{
                  position: 'relative', zIndex: 1, width: '100%', minHeight: 600,
                  background: 'transparent', border: 'none', outline: 'none', resize: 'none',
                  color: 'transparent', caretColor: 'var(--c-accent)',
                  fontFamily: 'var(--font-mono)', fontSize: 13, lineHeight: 1.6,
                  whiteSpace: 'pre-wrap', wordBreak: 'break-word',
                }}
              />
            </div>
          </div>
        </div>

        {/* status */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '6px 14px', borderTop: '1px solid var(--stroke-1)', background: 'var(--c-ink-2)', fontSize: 10, color: 'var(--fg-5)', fontFamily: 'var(--font-mono)' }}>
          <span>Ln {lineCount}, Col 1</span>
          <span>·</span>
          <span>Lua 5.4</span>
          <span>·</span>
          <span style={{ color: 'var(--c-success)' }}>● 構文 OK</span>
          <div style={{ flex: 1 }}/>
          <span>UTF-8</span>
        </div>
      </div>

      {/* API drawer */}
      {drawerOpen && <ApiDrawer onClose={() => setDrawerOpen(false)}/>}
    </div>
  );
}

const LUA_API = [
  {
    ns: 'conduction.deck(n)', desc: 'デッキ 1/2/3/4 を取得 / Get deck handle',
    methods: [
      { sig: ':play()', desc: '再生開始' },
      { sig: ':stop()', desc: '停止' },
      { sig: ':set_volume(v)', desc: 'v: 0.0 – 1.0' },
      { sig: ':set_eq(band, v)', desc: 'band: "low"|"mid"|"high"' },
      { sig: ':set_fx_send(v)', desc: 'FX バス送り 0.0 – 1.0' },
      { sig: ':seek_to_cue(name)', desc: 'Cue ポイントへ即時移動' },
    ]
  },
  {
    ns: 'conduction.crossfader', desc: 'クロスフェーダー / Crossfader',
    methods: [
      { sig: ':set(v, opts?)', desc: 'v: -1.0 (A) – 1.0 (B)' },
      { sig: ':curve(type)', desc: '"linear"|"sharp"|"smooth"' },
    ]
  },
  {
    ns: 'conduction.on_bar(n, fn)', desc: 'n 小節目でコールバック実行',
    methods: []
  },
  {
    ns: 'conduction.on_beat(n, fn)', desc: 'n 拍目でコールバック実行',
    methods: []
  },
  {
    ns: 'conduction.transition_at(start, bars, fn)', desc: 'start 拍目から bars 小節かけて補間実行。fn(progress)',
    methods: []
  },
  {
    ns: 'conduction.on_complete(fn)', desc: 'テンプレート完了時に実行',
    methods: []
  },
];

function ApiDrawer({ onClose }) {
  const [q, setQ] = React.useState('');
  return (
    <aside style={{
      width: 340, flexShrink: 0, borderLeft: '1px solid var(--stroke-1)',
      background: 'var(--c-ink-2)', display: 'flex', flexDirection: 'column', minHeight: 0,
    }}>
      <div style={{ padding: '10px 14px', borderBottom: '1px solid var(--stroke-1)', display: 'flex', alignItems: 'center', gap: 8 }}>
        <i data-lucide="book-open" style={{ width: 13, height: 13, color: 'var(--c-accent)' }}/>
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: 12, fontWeight: 700, color: 'var(--fg-1)' }}>Lua API リファレンス</div>
          <div className="overline" style={{ fontSize: 8 }}>REFERENCE</div>
        </div>
        <button className="icon-btn" onClick={onClose}><i data-lucide="x" style={{ width: 12, height: 12 }}/></button>
      </div>
      <div style={{ padding: '8px 12px', borderBottom: '1px solid var(--stroke-1)' }}>
        <div style={{ position: 'relative' }}>
          <i data-lucide="search" style={{ position: 'absolute', left: 8, top: '50%', transform: 'translateY(-50%)', width: 11, height: 11, color: 'var(--fg-5)' }}/>
          <input value={q} onChange={e => setQ(e.target.value)} placeholder="search API…"
            style={{ width: '100%', boxSizing: 'border-box', height: 26, background: 'var(--c-ink-3)', border: '1px solid var(--stroke-1)', borderRadius: 5, padding: '0 8px 0 24px', color: 'var(--fg-2)', fontSize: 11, outline: 'none', fontFamily: 'var(--font-mono)' }}/>
        </div>
      </div>
      <div style={{ flex: 1, overflowY: 'auto', padding: 10 }}>
        {LUA_API.filter(e => !q || e.ns.toLowerCase().includes(q.toLowerCase()) || e.desc.toLowerCase().includes(q.toLowerCase())).map((entry, i) => (
          <div key={i} style={{ marginBottom: 14, padding: '10px 12px', background: 'var(--c-ink-3)', borderRadius: 8, border: '1px solid var(--stroke-1)' }}>
            <div style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--c-accent-hi)', fontWeight: 500 }}>{entry.ns}</div>
            <div style={{ fontSize: 10, color: 'var(--fg-4)', marginTop: 3 }}>{entry.desc}</div>
            {entry.methods.length > 0 && (
              <div style={{ marginTop: 8, paddingLeft: 8, borderLeft: '1px solid var(--stroke-1)' }}>
                {entry.methods.map((m, j) => (
                  <div key={j} style={{ marginBottom: 5 }}>
                    <div style={{ fontFamily: 'var(--font-mono)', fontSize: 10, color: 'var(--fg-2)' }}>{m.sig}</div>
                    <div style={{ fontSize: 9, color: 'var(--fg-5)' }}>{m.desc}</div>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </aside>
  );
}

function escapeHtml(s) {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function highlightLua(src) {
  // Token-based; escape first then wrap.
  const escaped = escapeHtml(src);
  const KW = ['local','function','end','if','then','else','elseif','return','for','in','do','while','true','false','nil','and','or','not'];
  let out = escaped;

  // comments
  out = out.replace(/(--[^\n]*)/g, '<span style="color:#5A6578;font-style:italic">$1</span>');
  // strings
  out = out.replace(/(&quot;[^&]*?&quot;|&#39;[^&]*?&#39;)/g, '<span style="color:#4FE3B2">$1</span>');
  // numbers
  out = out.replace(/\b(\d+\.?\d*)\b/g, '<span style="color:#E8B868">$1</span>');
  // conduction namespace
  out = out.replace(/\b(conduction)\b/g, '<span style="color:#E8915A;font-weight:500">$1</span>');
  // keywords
  out = out.replace(new RegExp('\\b(' + KW.join('|') + ')\\b', 'g'),
    '<span style="color:#A089DC;font-weight:500">$1</span>');
  // method calls after :
  out = out.replace(/:([a-zA-Z_][a-zA-Z0-9_]*)/g, ':<span style="color:#8A9BE8">$1</span>');
  return out;
}

Object.assign(window, { ScriptEditor });
