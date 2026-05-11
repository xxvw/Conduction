// TemplateEditor.jsx — right side editor wrapper with mode tabs + preview panel
function TemplateEditor({ template, setTemplate }) {
  const [mode, setMode] = React.useState('visual');
  const [playing, setPlaying] = React.useState(false);
  const [playhead, setPlayhead] = React.useState(0);
  const [dryRun, setDryRun] = React.useState(true);
  const totalBeats = template.bars * 4;

  React.useEffect(() => {
    if (!playing) return;
    const id = setInterval(() => {
      setPlayhead(p => {
        const next = p + 0.1;
        if (next >= totalBeats) { setPlaying(false); return 0; }
        return next;
      });
    }, 60);
    return () => clearInterval(id);
  }, [playing, totalBeats]);

  return (
    <section style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0, background: 'var(--c-ink-1)', position: 'relative' }}>
      {/* Header */}
      <header style={{
        display: 'flex', alignItems: 'center', gap: 14,
        padding: '12px 18px', borderBottom: '1px solid var(--stroke-1)',
        background: 'linear-gradient(180deg, var(--c-ink-2), var(--c-ink-1))',
      }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="overline" style={{ fontSize: 9 }}>{template.preset ? 'プリセット / PRESET' : 'ユーザー / USER TEMPLATE'} · {template.bars} BARS</div>
          <input value={template.name} onChange={e => setTemplate(p => ({ ...p, name: e.target.value }))}
            style={{
              background: 'transparent', border: 'none', outline: 'none',
              fontFamily: 'var(--font-display)', fontWeight: 700, fontSize: 20,
              letterSpacing: 'var(--tracking-tight)', color: 'var(--fg-1)',
              width: '100%', padding: 0, marginTop: 2,
            }}/>
        </div>

        <div className="tab-strip">
          {[['visual','Visual'],['node','Node'],['script','Script']].map(([id, label]) => (
            <button key={id} className={mode === id ? 'active' : ''} onClick={() => setMode(id)}>{label}</button>
          ))}
        </div>

        <div style={{ width: 1, height: 24, background: 'var(--stroke-1)' }}/>

        <button className="btn-xs"><i data-lucide="save" style={{ width: 10, height: 10 }}/> SAVE</button>
      </header>

      {/* Body */}
      {mode === 'visual' && (
        <AutomationTimeline template={template} setTemplate={setTemplate} playhead={playhead} setPlayhead={setPlayhead} playing={playing}/>
      )}
      {mode === 'node' && <NodeEditor template={template} setTemplate={setTemplate}/>}
      {mode === 'script' && <ScriptEditor template={template} setTemplate={setTemplate}/>}

      {/* Floating preview */}
      <PreviewPanel
        playing={playing} setPlaying={setPlaying}
        playhead={playhead} setPlayhead={setPlayhead}
        totalBeats={totalBeats}
        dryRun={dryRun} setDryRun={setDryRun}
      />
    </section>
  );
}

function PreviewPanel({ playing, setPlaying, playhead, setPlayhead, totalBeats, dryRun, setDryRun }) {
  return (
    <div style={{
      position: 'absolute', top: 72, right: 18, zIndex: 50,
      width: 320, padding: 12,
      background: 'linear-gradient(180deg, rgba(26,30,38,0.92), rgba(19,22,28,0.92))',
      border: '1px solid var(--c-glass-stroke-strong)',
      borderRadius: 12,
      boxShadow: 'var(--shadow-3)',
      backdropFilter: 'blur(var(--blur-md)) saturate(140%)',
      WebkitBackdropFilter: 'blur(var(--blur-md)) saturate(140%)',
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 10 }}>
        <div style={{ width: 6, height: 6, borderRadius: '50%', background: playing ? 'var(--c-accent)' : 'var(--fg-5)', boxShadow: playing ? '0 0 8px var(--c-accent-glow)' : 'none' }}/>
        <div className="overline" style={{ fontSize: 9, color: playing ? 'var(--c-accent)' : 'var(--fg-4)' }}>
          プレビュー / PREVIEW
        </div>
        <div style={{ flex: 1 }}/>
        <span className="tabular" style={{ fontSize: 10, color: 'var(--fg-4)' }}>
          {playhead.toFixed(1)} / {totalBeats}
        </span>
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 10 }}>
        <button onClick={() => setPlaying(p => !p)}
          style={{
            width: 40, height: 40, borderRadius: 10, border: 'none',
            background: playing
              ? 'linear-gradient(180deg, var(--c-accent-hi), var(--c-accent))'
              : 'linear-gradient(180deg, var(--c-ink-4), var(--c-ink-3))',
            color: playing ? 'var(--c-ink-0)' : 'var(--fg-1)',
            cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center',
            boxShadow: playing ? '0 0 12px var(--c-accent-glow), inset 0 1px 0 rgba(255,255,255,0.2)' : 'var(--shadow-1), inset 0 1px 0 rgba(255,255,255,0.06)',
            transition: 'all var(--dur-2) var(--ease-out)',
          }}>
          <i data-lucide={playing ? 'pause' : 'play'} style={{ width: 16, height: 16, fill: playing ? 'currentColor' : 'none' }}/>
        </button>
        <button className="btn-cue" onClick={() => { setPlaying(false); setPlayhead(0); }} style={{ width: 40, height: 40, borderRadius: 10 }}>
          <i data-lucide="square" style={{ width: 13, height: 13, color: 'var(--fg-3)' }}/>
        </button>

        <div style={{ flex: 1, height: 40, display: 'flex', alignItems: 'center', padding: '0 10px', background: 'var(--c-ink-3)', border: '1px solid var(--stroke-1)', borderRadius: 10 }}>
          <input type="range" min="0" max={totalBeats} step="0.1" value={playhead} onChange={e => setPlayhead(parseFloat(e.target.value))}
            style={{ flex: 1, accentColor: 'var(--c-accent)' }}/>
        </div>
      </div>

      {/* Mode toggle */}
      <div style={{ display: 'flex', gap: 4, padding: 3, background: 'var(--c-ink-3)', border: '1px solid var(--stroke-1)', borderRadius: 6 }}>
        <button onClick={() => setDryRun(true)} style={modeBtn(dryRun)}>Dry run <span style={{fontSize:9,color:'var(--fg-5)'}}>音なし</span></button>
        <button onClick={() => setDryRun(false)} style={modeBtn(!dryRun)}>With audio <span style={{fontSize:9,color:'var(--fg-5)'}}>実演</span></button>
      </div>
    </div>
  );
}
function modeBtn(active) {
  return {
    flex: 1, padding: '5px 8px', border: 'none', borderRadius: 4,
    background: active ? 'linear-gradient(180deg, var(--c-ink-4), var(--c-ink-3))' : 'transparent',
    color: active ? 'var(--fg-1)' : 'var(--fg-4)',
    fontFamily: 'var(--font-sans)', fontWeight: 700, fontSize: 10, cursor: 'pointer',
    display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 5,
    boxShadow: active ? 'inset 0 1px 0 rgba(255,255,255,0.06), 0 1px 2px rgba(0,0,0,0.4)' : 'none',
    letterSpacing: 'var(--tracking-wide)', textTransform: 'uppercase',
  };
}

Object.assign(window, { TemplateEditor });
