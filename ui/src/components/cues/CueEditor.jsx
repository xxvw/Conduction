// CueEditor.jsx — Cue points editor for a track
function CueEditor({ track, onClose }) {
  const t = {
    id: 'demo', title: 'Midnight Drive', artist: 'Kaoru',
    bpm: 128.00, key: '8A', duration: 318,
    ...(track || {}),
  };
  const [cues, setCues] = React.useState([
    { id: 'c1', type: 'intro',     name: 'Intro',        bar: 1,   beat: 1, sectionBars: 16, bpm: 128.00, key: '8A', energy: 0.35, phrase: 16, mixable: 'entry', energyRange: [0.2, 0.5] },
    { id: 'c2', type: 'breakdown', name: 'Breakdown',    bar: 33,  beat: 1, sectionBars: 32, bpm: 128.00, key: '8A', energy: 0.5,  phrase: 32, mixable: 'both',  energyRange: [0.3, 0.7] },
    { id: 'c3', type: 'drop',      name: 'Drop · 1st',   bar: 65,  beat: 1, sectionBars: 0,  bpm: 128.00, key: '8A', energy: 0.9,  phrase: 16, mixable: 'entry', energyRange: [0.7, 1.0] },
    { id: 'c4', type: 'custom',    name: 'Loop point A', bar: 97,  beat: 3, sectionBars: 0,  bpm: 128.00, key: '8A', energy: 0.75, phrase: 16, mixable: 'both',  energyRange: [0.5, 0.9] },
    { id: 'c5', type: 'outro',     name: 'Outro',        bar: 113, beat: 1, sectionBars: 32, bpm: 128.00, key: '8A', energy: 0.6,  phrase: 32, mixable: 'exit',  energyRange: [0.3, 0.7] },
  ]);
  const [selected, setSelected] = React.useState('c1');
  const [playhead, setPlayhead] = React.useState(40); // bar
  const totalBars = 128;

  React.useEffect(() => { lucide.createIcons({ attrs: { 'stroke-width': 1.75 } }); });

  const addCue = () => {
    const id = 'c-' + Date.now();
    const c = { id, type: 'custom', name: '新規 Cue', bar: Math.round(playhead), beat: 1, sectionBars: 0, bpm: t.bpm, key: t.key, energy: 0.5, phrase: 16, mixable: 'both', energyRange: [0.3, 0.7] };
    setCues(cs => [...cs, c].sort((a, b) => a.bar - b.bar || a.beat - b.beat));
    setSelected(id);
  };

  const updateCue = (id, c) => setCues(cs => cs.map(x => x.id === id ? c : x));
  const deleteCue = (id) => setCues(cs => cs.filter(c => c.id !== id));

  return (
    <div style={{ flex: 1, display: 'flex', minHeight: 0, background: 'var(--c-ink-1)' }}>
      {/* Main */}
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
        <header style={{
          display: 'flex', alignItems: 'center', gap: 14,
          padding: '14px 20px', borderBottom: '1px solid var(--stroke-1)',
          background: 'linear-gradient(180deg, var(--c-ink-2), var(--c-ink-1))',
        }}>
          {onClose && (
            <button className="icon-btn" onClick={onClose} style={{ width: 28, height: 28 }}>
              <i data-lucide="chevron-left" style={{ width: 15, height: 15 }}/>
            </button>
          )}
          <div style={{ flex: 1, minWidth: 0 }}>
            <div className="overline" style={{ fontSize: 9 }}>Cue エディタ / CUE EDITOR</div>
            <div style={{ display: 'flex', alignItems: 'baseline', gap: 10, marginTop: 2 }}>
              <h2 style={{ fontSize: 20, fontWeight: 700, letterSpacing: 'var(--tracking-tight)', color: 'var(--fg-1)', margin: 0 }}>{t.title}</h2>
              <span style={{ fontSize: 12, color: 'var(--fg-4)' }}>{t.artist}</span>
              <span className="tabular" style={{ fontSize: 11, color: 'var(--fg-5)', marginLeft: 8 }}>
                {t.bpm.toFixed(2)} BPM · {t.key} · {Math.floor(t.duration/60)}:{String(t.duration%60).padStart(2,'0')}
              </span>
            </div>
          </div>
          <span className="tabular" style={{ fontSize: 11, color: 'var(--fg-3)' }}>{cues.length} cues</span>
        </header>

        {/* Waveform with cues */}
        <div style={{ padding: '16px 20px' }}>
          <CueWaveform cues={cues} selected={selected} onSelect={setSelected} totalBars={totalBars} playhead={playhead} setPlayhead={setPlayhead}/>
          {/* Transport */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginTop: 12 }}>
            <button className="btn-cue" style={{ width: 36, height: 36 }}>
              <i data-lucide="play" style={{ width: 13, height: 13, color: 'var(--c-accent)' }}/>
            </button>
            <span className="tabular" style={{ fontSize: 11, color: 'var(--fg-4)', minWidth: 80 }}>
              bar {Math.floor(playhead)} / {totalBars}
            </span>
            <input type="range" min="1" max={totalBars} value={playhead} onChange={e => setPlayhead(parseInt(e.target.value))}
              style={{ flex: 1, accentColor: 'var(--c-accent)' }}/>
          </div>
        </div>

        {/* Bottom action bar */}
        <div style={{ marginTop: 'auto', padding: '12px 20px', borderTop: '1px solid var(--stroke-1)', background: 'var(--c-ink-2)', display: 'flex', gap: 10, alignItems: 'center' }}>
          <button className="btn-xs accent" onClick={addCue}>
            <i data-lucide="plus" style={{ width: 11, height: 11 }}/> ADD CUE @ PLAYHEAD
          </button>
          <button className="btn-xs" onClick={() => selected && deleteCue(selected)}>
            <i data-lucide="trash-2" style={{ width: 11, height: 11 }}/> DELETE SELECTED
          </button>
          <button className="btn-xs">
            <i data-lucide="file-input" style={{ width: 11, height: 11 }}/> IMPORT FROM ANALYSIS
          </button>

          <div style={{ flex: 1 }}/>

          <div style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 10px', borderRadius: 6, background: 'var(--c-accent-soft)', border: '1px solid rgba(79,227,178,0.22)' }}>
            <i data-lucide="sparkles" style={{ width: 11, height: 11, color: 'var(--c-accent)' }}/>
            <span style={{ fontSize: 10, fontWeight: 700, color: 'var(--c-accent-hi)', letterSpacing: 'var(--tracking-wide)', textTransform: 'uppercase' }}>Claude CLI</span>
          </div>
          <button className="btn-xs accent">
            <i data-lucide="wand-2" style={{ width: 11, height: 11 }}/> AUTO-DETECT
          </button>
        </div>
      </div>

      {/* Right: cue list */}
      <aside style={{
        width: 340, flexShrink: 0, borderLeft: '1px solid var(--stroke-1)',
        background: 'var(--c-ink-2)', display: 'flex', flexDirection: 'column', minHeight: 0,
      }}>
        <div style={{ padding: '12px 16px', borderBottom: '1px solid var(--stroke-1)' }}>
          <div className="overline" style={{ fontSize: 9 }}>Cue 一覧 / CUES</div>
          {/* Type legend */}
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4, marginTop: 8 }}>
            {CUE_TYPES.map(c => (
              <div key={c.id} style={{ display: 'flex', alignItems: 'center', gap: 4, padding: '2px 6px', borderRadius: 3, background: 'var(--c-ink-3)' }}>
                <div style={{ width: 6, height: 6, borderRadius: '50%', background: c.color }}/>
                <span style={{ fontSize: 9, color: 'var(--fg-4)', fontWeight: 700, letterSpacing: 'var(--tracking-wide)' }}>{c.en}</span>
              </div>
            ))}
          </div>
        </div>
        <div style={{ flex: 1, overflowY: 'auto', padding: '8px 12px' }}>
          {cues.map(c => (
            <CueListItem key={c.id} cue={c} selected={selected === c.id}
              onSelect={() => setSelected(c.id)}
              onChange={(updated) => updateCue(c.id, updated)}
              onDelete={() => deleteCue(c.id)}/>
          ))}
        </div>
      </aside>
    </div>
  );
}

/* ---------- Waveform with cue overlays ---------- */
function CueWaveform({ cues, selected, onSelect, totalBars, playhead, setPlayhead }) {
  const canvasRef = React.useRef(null);
  const [wrapRef, setWrapRef] = React.useState(null);
  const height = 220;

  React.useEffect(() => {
    const c = canvasRef.current;
    if (!c || !wrapRef) return;
    const rect = wrapRef.getBoundingClientRect();
    const w = rect.width;
    c.width = w * devicePixelRatio;
    c.height = height * devicePixelRatio;
    c.style.width = w + 'px';
    c.style.height = height + 'px';
    const ctx = c.getContext('2d');
    ctx.scale(devicePixelRatio, devicePixelRatio);
    ctx.fillStyle = '#07080A';
    ctx.fillRect(0, 0, w, height);
    const centerY = height / 2;
    for (let x = 0; x < w; x += 2) {
      const seed = x * 13.7;
      const r1 = Math.abs(Math.sin(seed)) * 0.5 + Math.abs(Math.sin(seed * 1.9)) * 0.5;
      const envA = 0.3 + r1 * 0.7;
      const envB = (Math.sin(x / 60) * 0.4 + 0.6);
      const amp = envA * envB * (height / 2 - 16);
      ctx.fillStyle = `rgba(79,227,178,${0.5 + r1 * 0.25})`;
      ctx.fillRect(x, centerY - amp, 1.2, amp * 2);
    }
  }, [wrapRef]);

  const onWaveClick = (e) => {
    if (!wrapRef) return;
    const rect = wrapRef.getBoundingClientRect();
    const x = e.clientX - rect.left;
    setPlayhead(Math.round((x / rect.width) * totalBars));
  };

  return (
    <div ref={setWrapRef} onClick={onWaveClick} style={{ position: 'relative', background: 'var(--c-ink-0)', border: '1px solid var(--stroke-1)', borderRadius: 10, overflow: 'hidden', cursor: 'pointer' }}>
      <canvas ref={canvasRef}/>
      {/* Section shading */}
      {cues.filter(c => c.sectionBars > 0).map(c => {
        const type = CUE_TYPES.find(t => t.id === c.type) || CUE_TYPES[4];
        const left = ((c.bar - 1) / totalBars) * 100;
        const width = (c.sectionBars / totalBars) * 100;
        return (
          <div key={c.id + '-sect'} style={{
            position: 'absolute', left: `${left}%`, width: `${width}%`, top: 0, bottom: 0,
            background: `linear-gradient(180deg, ${type.color}20, ${type.color}08)`,
            borderLeft: `1px solid ${type.color}40`, borderRight: `1px solid ${type.color}40`,
            pointerEvents: 'none',
          }}/>
        );
      })}
      {/* Bar grid */}
      {Array.from({ length: Math.floor(totalBars / 16) + 1 }).map((_, i) => (
        <div key={i} style={{ position: 'absolute', left: `${(i * 16 / totalBars) * 100}%`, top: 0, bottom: 0, borderLeft: '1px solid rgba(255,255,255,0.08)', pointerEvents: 'none' }}>
          <span className="tabular" style={{ position: 'absolute', top: 4, left: 5, fontSize: 9, color: 'var(--fg-5)', fontWeight: 700 }}>{i * 16 + 1}</span>
        </div>
      ))}
      {/* Cue markers */}
      {cues.map(c => {
        const type = CUE_TYPES.find(t => t.id === c.type) || CUE_TYPES[4];
        const left = ((c.bar - 1) / totalBars) * 100;
        const sel = selected === c.id;
        return (
          <div key={c.id}
            onClick={(e) => { e.stopPropagation(); onSelect(c.id); }}
            style={{
              position: 'absolute', left: `${left}%`, top: 0, bottom: 0, width: 2,
              background: type.color,
              boxShadow: sel ? `0 0 12px ${type.color}, 0 0 4px ${type.color}` : `0 0 4px ${type.color}80`,
              cursor: 'pointer', zIndex: sel ? 3 : 2,
            }}>
            <div style={{
              position: 'absolute', top: 6, left: -1,
              padding: '3px 7px', borderRadius: 4,
              background: sel ? type.color : `${type.color}cc`,
              color: 'var(--c-ink-0)',
              fontFamily: 'var(--font-sans)', fontSize: 9, fontWeight: 800,
              letterSpacing: 'var(--tracking-wide)', textTransform: 'uppercase',
              whiteSpace: 'nowrap',
              boxShadow: sel ? 'var(--shadow-2)' : 'none',
              transform: sel ? 'scale(1.05)' : 'scale(1)',
              transition: 'transform var(--dur-2) var(--ease-out)',
            }}>
              {c.name}
            </div>
          </div>
        );
      })}
      {/* Playhead */}
      <div style={{
        position: 'absolute', left: `${(playhead / totalBars) * 100}%`, top: 0, bottom: 0, width: 1,
        background: 'var(--fg-1)', opacity: 0.9, boxShadow: '0 0 4px rgba(255,255,255,0.5)', pointerEvents: 'none', zIndex: 5,
      }}>
        <div style={{ position: 'absolute', top: 0, left: -5, width: 11, height: 9, background: 'var(--fg-1)', clipPath: 'polygon(50% 100%, 0 0, 100% 0)' }}/>
      </div>
    </div>
  );
}

Object.assign(window, { CueEditor });
