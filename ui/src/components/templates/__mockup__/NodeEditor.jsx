// NodeEditor.jsx — React Flow style node canvas for template logic
function NodeEditor({ template, setTemplate }) {
  const initialNodes = template.nodes || [
    { id: 'n1', type: 'input', kind: 'Bar', pos: { x: 40, y: 60 }, data: { value: '0' } },
    { id: 'n2', type: 'input', kind: 'Cue Reached', pos: { x: 40, y: 180 }, data: { value: 'drop' } },
    { id: 'n3', type: 'process', kind: 'Branch', pos: { x: 300, y: 120 }, data: { cond: 'energy > 0.7' } },
    { id: 'n4', type: 'output', kind: 'Set Parameter', pos: { x: 560, y: 60 }, data: { target: 'Deck A · Vol', value: '0.0' } },
    { id: 'n5', type: 'output', kind: 'Trigger Cue', pos: { x: 560, y: 200 }, data: { target: 'Deck B · Exit' } },
  ];
  const initialEdges = template.edges || [
    { from: 'n1', to: 'n3' },
    { from: 'n2', to: 'n3' },
    { from: 'n3', to: 'n4' },
    { from: 'n3', to: 'n5' },
  ];
  const [nodes, setNodes] = React.useState(initialNodes);
  const [edges] = React.useState(initialEdges);
  const [drag, setDrag] = React.useState(null);
  const [palette, setPalette] = React.useState(false);

  const TYPE_META = {
    input:   { color: '#8A9BE8', label: '入力 / INPUT', options: ['Beat','Bar','Cue Reached','Parameter Value'] },
    process: { color: '#E8B868', label: '処理 / PROCESS', options: ['Branch','Delay','Transform'] },
    output:  { color: 'var(--c-accent)', label: '出力 / OUTPUT', options: ['Set Parameter','Trigger Cue','Fire Event'] },
  };

  const onMouseDownNode = (e, id) => {
    const node = nodes.find(n => n.id === id);
    setDrag({ id, startX: e.clientX - node.pos.x, startY: e.clientY - node.pos.y });
  };
  React.useEffect(() => {
    if (!drag) return;
    const move = (e) => {
      setNodes(ns => ns.map(n => n.id === drag.id ? { ...n, pos: { x: e.clientX - drag.startX, y: e.clientY - drag.startY } } : n));
    };
    const up = () => setDrag(null);
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', up);
    return () => { window.removeEventListener('mousemove', move); window.removeEventListener('mouseup', up); };
  }, [drag]);

  const addNode = (type, kind) => {
    const id = 'n' + (nodes.length + 1) + Date.now();
    setNodes([...nodes, { id, type, kind, pos: { x: 120 + Math.random() * 200, y: 100 + Math.random() * 150 }, data: {} }]);
    setPalette(false);
  };

  return (
    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minHeight: 0, background: 'var(--c-ink-1)' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '10px 14px', borderBottom: '1px solid var(--stroke-1)', background: 'var(--c-ink-2)' }}>
        <button className="btn-xs accent" onClick={() => setPalette(p => !p)}>
          <i data-lucide="plus" style={{ width: 11, height: 11 }}/> ノード追加 / ADD NODE
        </button>
        <div style={{ width: 1, height: 20, background: 'var(--stroke-1)' }}/>
        <span className="overline" style={{ fontSize: 9 }}>{nodes.length} nodes · {edges.length} edges</span>
        <div style={{ flex: 1 }}/>
        <button className="btn-xs"><i data-lucide="play" style={{ width: 10, height: 10 }}/> SIMULATE</button>
      </div>

      <div style={{ flex: 1, position: 'relative', overflow: 'hidden',
        backgroundImage: 'radial-gradient(circle, rgba(255,255,255,0.04) 1px, transparent 1px)',
        backgroundSize: '16px 16px', backgroundPosition: '0 0',
      }}>
        {/* Edges */}
        <svg style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', pointerEvents: 'none' }}>
          {edges.map((e, i) => {
            const a = nodes.find(n => n.id === e.from);
            const b = nodes.find(n => n.id === e.to);
            if (!a || !b) return null;
            const x1 = a.pos.x + 180, y1 = a.pos.y + 36;
            const x2 = b.pos.x, y2 = b.pos.y + 36;
            const mx = (x1 + x2) / 2;
            const d = `M ${x1} ${y1} C ${mx} ${y1}, ${mx} ${y2}, ${x2} ${y2}`;
            return <path key={i} d={d} stroke="var(--c-ink-6)" strokeWidth="1.5" fill="none" opacity="0.7"/>;
          })}
        </svg>

        {/* Nodes */}
        {nodes.map(n => (
          <NodeCard key={n.id} node={n} meta={TYPE_META[n.type]} onMouseDown={(e) => onMouseDownNode(e, n.id)}/>
        ))}

        {/* Palette */}
        {palette && (
          <div style={{
            position: 'absolute', top: 14, left: 14, zIndex: 20,
            background: 'var(--c-ink-3)', border: '1px solid var(--stroke-1)', borderRadius: 10,
            boxShadow: 'var(--shadow-3)', padding: 10, width: 240,
          }}>
            {Object.entries(TYPE_META).map(([type, m]) => (
              <div key={type} style={{ marginBottom: 10 }}>
                <div className="overline" style={{ fontSize: 9, marginBottom: 4, color: m.color }}>{m.label}</div>
                {m.options.map(k => (
                  <div key={k} onClick={() => addNode(type, k)}
                    style={{
                      padding: '5px 8px', borderRadius: 5, cursor: 'pointer',
                      fontSize: 11, color: 'var(--fg-2)',
                      borderLeft: `2px solid ${m.color}`, marginBottom: 1,
                    }}
                    onMouseEnter={e => e.currentTarget.style.background = 'var(--bg-hover)'}
                    onMouseLeave={e => e.currentTarget.style.background = 'transparent'}>
                    {k}
                  </div>
                ))}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function NodeCard({ node, meta, onMouseDown }) {
  return (
    <div
      onMouseDown={onMouseDown}
      style={{
        position: 'absolute', left: node.pos.x, top: node.pos.y,
        width: 180, background: 'linear-gradient(180deg, var(--c-ink-3), var(--c-ink-2))',
        border: `1px solid var(--stroke-1)`, borderLeft: `3px solid ${meta.color}`,
        borderRadius: 8, boxShadow: 'var(--shadow-2)',
        fontFamily: 'var(--font-sans)', cursor: 'grab', userSelect: 'none',
      }}>
      <div style={{ padding: '7px 10px', borderBottom: '1px solid var(--stroke-1)', display: 'flex', alignItems: 'center', gap: 6 }}>
        <div className="overline" style={{ fontSize: 8, color: meta.color }}>{meta.label.split(' / ')[1]}</div>
        <div style={{ flex: 1 }}/>
        <i data-lucide="grip-vertical" style={{ width: 10, height: 10, color: 'var(--fg-5)' }}/>
      </div>
      <div style={{ padding: 10 }}>
        <div style={{ fontSize: 12, fontWeight: 700, color: 'var(--fg-1)', letterSpacing: 'var(--tracking-tight)' }}>{node.kind}</div>
        {Object.entries(node.data || {}).map(([k, v]) => (
          <div key={k} style={{ display: 'flex', justifyContent: 'space-between', marginTop: 4, fontSize: 10 }}>
            <span style={{ color: 'var(--fg-5)', textTransform: 'uppercase', letterSpacing: 'var(--tracking-wide)' }}>{k}</span>
            <span className="tabular" style={{ color: 'var(--fg-2)' }}>{String(v)}</span>
          </div>
        ))}
      </div>
      {/* ports */}
      <div style={{ position: 'absolute', right: -5, top: 32, width: 10, height: 10, borderRadius: '50%', background: meta.color, border: '1px solid var(--c-ink-1)' }}/>
      {node.type !== 'input' && (
        <div style={{ position: 'absolute', left: -5, top: 32, width: 10, height: 10, borderRadius: '50%', background: 'var(--c-ink-5)', border: '1px solid var(--c-ink-1)' }}/>
      )}
    </div>
  );
}

Object.assign(window, { NodeEditor });
