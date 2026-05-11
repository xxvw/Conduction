// AutomationTimeline.jsx — DAW-style automation timeline (Visual mode)
// Tracks list (left), beat ruler (top), per-track lanes with keyframes & curves.

const CURVE_TYPES = ['linear', 'easeOut', 'easeIn', 'easeInOut', 'step', 'hold'];

const SNAPS = [
  { id: 'off', label: 'OFF', beats: 0 },
  { id: '1/4', label: '1/4', beats: 0.25 },
  { id: '1/2', label: '1/2', beats: 0.5 },
  { id: 'beat', label: 'BEAT', beats: 1 },
  { id: '2beats', label: '2', beats: 2 },
  { id: '4beats', label: '4', beats: 4 },
  { id: 'bar', label: 'BAR', beats: 4 },
  { id: 'phrase', label: 'PHRASE', beats: 32 },
];

function AutomationTimeline({ template, setTemplate, playhead, setPlayhead, playing }) {
  const totalBeats = template.bars * 4;
  const [pxPerBeat, setPxPerBeat] = React.useState(28);
  const [snap, setSnap] = React.useState('beat');
  const [selected, setSelected] = React.useState(new Set()); // "trackId:kfIdx"
  const [marquee, setMarquee] = React.useState(null);
  const [drag, setDrag] = React.useState(null);
  const [ctx, setCtx] = React.useState(null); // right click menu
  const scrollRef = React.useRef(null);
  const contentRef = React.useRef(null);

  const snapBeats = SNAPS.find(s => s.id === snap)?.beats || 0;
  const TRACK_NAME_W = 140;
  const LANE_H = 36;
  const RULER_H = 36;
  const contentW = totalBeats * pxPerBeat;

  const snapValue = (b) => snapBeats > 0 ? Math.round(b / snapBeats) * snapBeats : b;

  const addKeyframe = (trackId, beat, value = 0.5) => {
    setTemplate(prev => ({
      ...prev,
      tracks: prev.tracks.map(t => t.id === trackId
        ? { ...t, keyframes: [...t.keyframes, { beat: snapValue(beat), value, curve: 'easeOut' }].sort((a, b) => a.beat - b.beat) }
        : t)
    }));
  };

  const moveKeyframe = (trackId, idx, dBeat, dVal) => {
    setTemplate(prev => ({
      ...prev,
      tracks: prev.tracks.map(t => {
        if (t.id !== trackId) return t;
        const kfs = t.keyframes.map((k, i) => i === idx
          ? { ...k, beat: Math.max(0, Math.min(totalBeats, snapValue(k.beat + dBeat))), value: Math.max(0, Math.min(1, k.value + dVal)) }
          : k);
        return { ...t, keyframes: kfs };
      })
    }));
  };

  const deleteSelected = () => {
    setTemplate(prev => ({
      ...prev,
      tracks: prev.tracks.map(t => {
        const kept = t.keyframes.filter((_, i) => !selected.has(`${t.id}:${i}`));
        return { ...t, keyframes: kept };
      })
    }));
    setSelected(new Set());
  };

  const setCurve = (trackId, idx, curve) => {
    setTemplate(prev => ({
      ...prev,
      tracks: prev.tracks.map(t => t.id === trackId
        ? { ...t, keyframes: t.keyframes.map((k, i) => i === idx ? { ...k, curve } : k) }
        : t)
    }));
  };

  const toggleCollapse = (trackId) => {
    setTemplate(prev => ({ ...prev, tracks: prev.tracks.map(t => t.id === trackId ? { ...t, collapsed: !t.collapsed } : t) }));
  };

  const removeTrack = (trackId) => {
    setTemplate(prev => ({ ...prev, tracks: prev.tracks.filter(t => t.id !== trackId) }));
  };

  // Keyboard shortcuts
  React.useEffect(() => {
    const h = (e) => {
      if ((e.key === 'Delete' || e.key === 'Backspace') && selected.size > 0 && !e.target.matches('input,textarea')) {
        e.preventDefault(); deleteSelected();
      }
    };
    window.addEventListener('keydown', h);
    return () => window.removeEventListener('keydown', h);
  });

  // Double-click lane to add
  const onLaneDblClick = (e, trackId) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const beat = (e.clientX - rect.left) / pxPerBeat;
    const value = 1 - ((e.clientY - rect.top) / LANE_H);
    addKeyframe(trackId, beat, Math.max(0, Math.min(1, value)));
  };

  // Drag keyframe
  const onKfMouseDown = (e, trackId, idx) => {
    e.stopPropagation();
    const key = `${trackId}:${idx}`;
    if (e.shiftKey) {
      setSelected(prev => {
        const n = new Set(prev);
        n.has(key) ? n.delete(key) : n.add(key);
        return n;
      });
    } else if (!selected.has(key)) {
      setSelected(new Set([key]));
    }
    setDrag({ trackId, idx, startX: e.clientX, startY: e.clientY });
  };

  React.useEffect(() => {
    if (!drag) return;
    const move = (e) => {
      const dBeat = (e.clientX - drag.startX) / pxPerBeat;
      const dVal = -(e.clientY - drag.startY) / LANE_H;
      moveKeyframe(drag.trackId, drag.idx, dBeat, dVal);
      setDrag(d => d ? { ...d, startX: e.clientX, startY: e.clientY } : null);
    };
    const up = () => setDrag(null);
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', up);
    return () => { window.removeEventListener('mousemove', move); window.removeEventListener('mouseup', up); };
  }, [drag, pxPerBeat]);

  // Right click on keyframe
  const onKfContext = (e, trackId, idx) => {
    e.preventDefault(); e.stopPropagation();
    setCtx({ x: e.clientX, y: e.clientY, trackId, idx });
  };

  React.useEffect(() => {
    if (!ctx) return;
    const h = () => setCtx(null);
    window.addEventListener('click', h);
    return () => window.removeEventListener('click', h);
  }, [ctx]);

  return (
    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minHeight: 0, background: 'var(--c-ink-1)' }}>
      {/* Toolbar */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '10px 14px', borderBottom: '1px solid var(--stroke-1)', background: 'var(--c-ink-2)' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span className="overline" style={{ fontSize: 9 }}>Snap</span>
          <div className="tab-strip" style={{ padding: 2 }}>
            {SNAPS.map(s => (
              <button key={s.id} className={snap === s.id ? 'active' : ''} onClick={() => setSnap(s.id)}
                style={{ padding: '4px 8px', fontSize: 9 }}>{s.label}</button>
            ))}
          </div>
        </div>

        <div style={{ width: 1, height: 20, background: 'var(--stroke-1)' }}/>

        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span className="overline" style={{ fontSize: 9 }}>Zoom</span>
          <button className="btn-xs" onClick={() => setPxPerBeat(p => Math.max(10, p - 6))}>
            <i data-lucide="minus" style={{ width: 10, height: 10 }}/>
          </button>
          <span className="tabular" style={{ fontSize: 10, color: 'var(--fg-4)', minWidth: 30, textAlign: 'center' }}>{pxPerBeat}px</span>
          <button className="btn-xs" onClick={() => setPxPerBeat(p => Math.min(80, p + 6))}>
            <i data-lucide="plus" style={{ width: 10, height: 10 }}/>
          </button>
        </div>

        <div style={{ flex: 1 }}/>

        {selected.size > 0 && (
          <>
            <span className="tabular" style={{ fontSize: 11, color: 'var(--fg-3)' }}>{selected.size} selected</span>
            <button className="btn-xs danger" onClick={deleteSelected}>
              <i data-lucide="trash-2" style={{ width: 10, height: 10 }}/> DELETE
            </button>
          </>
        )}
      </div>

      {/* Timeline body */}
      <div ref={scrollRef} style={{ flex: 1, overflow: 'auto', position: 'relative' }}>
        <div ref={contentRef} style={{ display: 'flex', flexDirection: 'column', minWidth: TRACK_NAME_W + contentW }}>
          {/* Ruler */}
          <Ruler totalBeats={totalBeats} pxPerBeat={pxPerBeat} trackNameW={TRACK_NAME_W} height={RULER_H} />

          {/* Tracks */}
          {template.tracks.map(track => (
            <div key={track.id} style={{ display: 'flex', borderBottom: '1px solid var(--stroke-1)' }}>
              {/* Track name */}
              <TrackHeader
                track={track} width={TRACK_NAME_W} height={track.collapsed ? 18 : LANE_H}
                onToggle={() => toggleCollapse(track.id)}
                onRemove={() => removeTrack(track.id)}
              />

              {/* Lane */}
              {!track.collapsed ? (
                <Lane
                  track={track} totalBeats={totalBeats} pxPerBeat={pxPerBeat} height={LANE_H}
                  selected={selected}
                  onDblClick={(e) => onLaneDblClick(e, track.id)}
                  onKfMouseDown={onKfMouseDown}
                  onKfContext={onKfContext}
                />
              ) : (
                <div style={{ flex: 1, height: 18, background: 'var(--c-ink-2)', opacity: 0.6 }}/>
              )}
            </div>
          ))}

          {/* Add track row */}
          <AddTrackRow template={template} setTemplate={setTemplate} width={TRACK_NAME_W} contentW={contentW}/>

          {/* Playhead */}
          <div style={{
            position: 'absolute', top: 0, bottom: 0,
            left: TRACK_NAME_W + playhead * pxPerBeat,
            width: 1, background: 'var(--c-accent)',
            boxShadow: `0 0 6px var(--c-accent-glow)`,
            pointerEvents: 'none', zIndex: 10,
          }}>
            <div style={{
              position: 'absolute', top: 0, left: -6, width: 13, height: 13,
              background: 'var(--c-accent)', clipPath: 'polygon(50% 100%, 0 0, 100% 0)',
            }}/>
          </div>
        </div>
      </div>

      {/* Bottom: Cues + duration */}
      <BottomBar template={template} setTemplate={setTemplate}/>

      {/* Context menu */}
      {ctx && (
        <div style={{
          position: 'fixed', left: ctx.x, top: ctx.y, zIndex: 1000,
          background: 'var(--c-ink-3)', border: '1px solid var(--stroke-1)', borderRadius: 8,
          boxShadow: 'var(--shadow-3)', padding: 4, minWidth: 160,
        }}>
          <div className="overline" style={{ padding: '6px 10px 4px', fontSize: 9 }}>カーブ / CURVE</div>
          {CURVE_TYPES.map(c => (
            <div key={c} onClick={() => { setCurve(ctx.trackId, ctx.idx, c); setCtx(null); }}
              style={{
                padding: '6px 10px', borderRadius: 5, fontSize: 12, cursor: 'pointer',
                color: 'var(--fg-2)', display: 'flex', alignItems: 'center', gap: 8,
              }}
              onMouseEnter={e => e.currentTarget.style.background = 'var(--bg-hover)'}
              onMouseLeave={e => e.currentTarget.style.background = 'transparent'}>
              <CurveIcon type={c}/>
              <span style={{ textTransform: 'capitalize' }}>{c}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/* ---------- Ruler ---------- */
function Ruler({ totalBeats, pxPerBeat, trackNameW, height }) {
  const marks = [];
  for (let b = 0; b <= totalBeats; b++) {
    const isBar = b % 4 === 0;
    const isPhrase = b % 16 === 0;
    marks.push({ beat: b, isBar, isPhrase });
  }
  return (
    <div style={{ display: 'flex', position: 'sticky', top: 0, zIndex: 5, background: 'var(--c-ink-2)', borderBottom: '1px solid var(--stroke-1)' }}>
      <div style={{ width: trackNameW, height, flexShrink: 0, borderRight: '1px solid var(--stroke-1)' }}/>
      <div style={{ position: 'relative', height, flex: 1 }}>
        {marks.map(m => (
          <div key={m.beat} style={{
            position: 'absolute', left: m.beat * pxPerBeat, top: 0, bottom: 0,
            borderLeft: `1px solid ${m.isPhrase ? 'var(--c-ink-7)' : m.isBar ? 'var(--c-ink-6)' : 'var(--c-ink-5)'}`,
            opacity: m.isPhrase ? 1 : m.isBar ? 0.8 : 0.35,
          }}>
            {m.isBar && (
              <span className="tabular" style={{
                position: 'absolute', left: 4, top: 4, fontSize: 10,
                color: m.isPhrase ? 'var(--fg-2)' : 'var(--fg-4)',
                fontWeight: m.isPhrase ? 700 : 400,
              }}>
                {m.beat === 0 ? '1' : m.beat / 4 + 1}
              </span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

/* ---------- Track header ---------- */
function TrackHeader({ track, width, height, onToggle, onRemove }) {
  const accent = track.color || 'var(--fg-4)';
  return (
    <div style={{
      width, height, flexShrink: 0, display: 'flex', alignItems: 'center', gap: 6,
      padding: '0 8px', background: 'var(--c-ink-2)',
      borderRight: '1px solid var(--stroke-1)',
      borderLeft: `2px solid ${accent}`,
    }}>
      <button className="icon-btn" onClick={onToggle} style={{ width: 16, height: 16 }}>
        <i data-lucide={track.collapsed ? 'chevron-right' : 'chevron-down'} style={{ width: 11, height: 11 }}/>
      </button>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 11, fontWeight: 700, color: 'var(--fg-2)', letterSpacing: 'var(--tracking-tight)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {track.label}
        </div>
        {!track.collapsed && track.sub && (
          <div className="tabular" style={{ fontSize: 9, color: 'var(--fg-5)', marginTop: 1 }}>{track.sub}</div>
        )}
      </div>
      {!track.collapsed && (
        <button className="icon-btn" onClick={onRemove} title="削除">
          <i data-lucide="x" style={{ width: 10, height: 10 }}/>
        </button>
      )}
    </div>
  );
}

/* ---------- Lane ---------- */
function Lane({ track, totalBeats, pxPerBeat, height, selected, onDblClick, onKfMouseDown, onKfContext }) {
  const w = totalBeats * pxPerBeat;
  const accent = track.color || '#8A9BE8';

  // Build curve path
  const pathD = React.useMemo(() => {
    if (track.keyframes.length === 0) return '';
    const kfs = track.keyframes;
    let d = `M 0 ${height - kfs[0].value * height}`;
    d += ` L ${kfs[0].beat * pxPerBeat} ${height - kfs[0].value * height}`;
    for (let i = 0; i < kfs.length - 1; i++) {
      const a = kfs[i], b = kfs[i + 1];
      const x1 = a.beat * pxPerBeat, y1 = height - a.value * height;
      const x2 = b.beat * pxPerBeat, y2 = height - b.value * height;
      if (a.curve === 'step' || a.curve === 'hold') {
        d += ` L ${x2} ${y1} L ${x2} ${y2}`;
      } else if (a.curve === 'linear') {
        d += ` L ${x2} ${y2}`;
      } else {
        // bezier approximation for ease variants
        const dx = x2 - x1;
        let c1x = x1 + dx * 0.5, c1y = y1, c2x = x1 + dx * 0.5, c2y = y2;
        if (a.curve === 'easeOut') { c1x = x1 + dx * 0.15; c2x = x1 + dx * 0.5; c1y = y1; c2y = y2; }
        if (a.curve === 'easeIn')  { c1x = x1 + dx * 0.5;  c2x = x1 + dx * 0.85; c1y = y1; c2y = y2; }
        if (a.curve === 'easeInOut') { c1x = x1 + dx * 0.4; c2x = x1 + dx * 0.6; c1y = y1; c2y = y2; }
        d += ` C ${c1x} ${c1y}, ${c2x} ${c2y}, ${x2} ${y2}`;
      }
    }
    const last = kfs[kfs.length - 1];
    d += ` L ${w} ${height - last.value * height}`;
    return d;
  }, [track.keyframes, pxPerBeat, height, w]);

  return (
    <div onDoubleClick={onDblClick}
      style={{
        flex: 1, height, position: 'relative',
        background: `repeating-linear-gradient(90deg,
          transparent 0, transparent ${pxPerBeat - 1}px,
          rgba(255,255,255,0.025) ${pxPerBeat - 1}px, rgba(255,255,255,0.025) ${pxPerBeat}px)`,
        borderRight: '1px solid var(--stroke-1)',
        cursor: 'crosshair',
      }}>
      {/* Bar highlights */}
      {Array.from({length: Math.ceil(totalBeats / 4)}).map((_, i) => (
        <div key={i} style={{
          position: 'absolute', left: i * 4 * pxPerBeat, top: 0, bottom: 0,
          borderLeft: `1px solid ${i % 4 === 0 ? 'rgba(255,255,255,0.10)' : 'rgba(255,255,255,0.05)'}`,
        }}/>
      ))}

      {/* Curve */}
      <svg width={w} height={height} style={{ position: 'absolute', left: 0, top: 0, pointerEvents: 'none' }}>
        <path d={pathD} stroke={accent} strokeWidth="1.5" fill="none" opacity="0.85"/>
        <path d={pathD + ` L ${w} ${height} L 0 ${height} Z`} fill={accent} opacity="0.06"/>
      </svg>

      {/* Keyframes */}
      {track.keyframes.map((kf, i) => {
        const key = `${track.id}:${i}`;
        const isSel = selected.has(key);
        return (
          <div key={i}
            onMouseDown={(e) => onKfMouseDown(e, track.id, i)}
            onContextMenu={(e) => onKfContext(e, track.id, i)}
            style={{
              position: 'absolute',
              left: kf.beat * pxPerBeat - 5,
              top: height - kf.value * height - 5,
              width: 10, height: 10, borderRadius: '50%',
              background: isSel ? 'var(--c-accent)' : accent,
              border: `1px solid ${isSel ? 'var(--c-accent-hi)' : 'rgba(255,255,255,0.25)'}`,
              boxShadow: isSel ? '0 0 8px var(--c-accent-glow)' : '0 1px 2px rgba(0,0,0,0.5)',
              cursor: 'grab', zIndex: 2,
            }}/>
        );
      })}
    </div>
  );
}

/* ---------- Add track ---------- */
function AddTrackRow({ template, setTemplate, width, contentW }) {
  const [open, setOpen] = React.useState(false);
  const existing = new Set(template.tracks.map(t => t.id));
  const options = [
    { id: 'a-vol', label: 'Deck A · Volume', sub: 'fader', color: 'var(--c-deck-a)' },
    { id: 'a-eq-low', label: 'Deck A · EQ Low', sub: 'eq', color: 'var(--c-deck-a)' },
    { id: 'a-eq-mid', label: 'Deck A · EQ Mid', sub: 'eq', color: 'var(--c-deck-a)' },
    { id: 'a-eq-high', label: 'Deck A · EQ High', sub: 'eq', color: 'var(--c-deck-a)' },
    { id: 'a-fx', label: 'Deck A · FX Send', sub: 'fx', color: 'var(--c-deck-a)' },
    { id: 'b-vol', label: 'Deck B · Volume', sub: 'fader', color: 'var(--c-deck-b)' },
    { id: 'b-eq-low', label: 'Deck B · EQ Low', sub: 'eq', color: 'var(--c-deck-b)' },
    { id: 'b-eq-mid', label: 'Deck B · EQ Mid', sub: 'eq', color: 'var(--c-deck-b)' },
    { id: 'b-eq-high', label: 'Deck B · EQ High', sub: 'eq', color: 'var(--c-deck-b)' },
    { id: 'b-fx', label: 'Deck B · FX Send', sub: 'fx', color: 'var(--c-deck-b)' },
    { id: 'xf', label: 'Crossfader', sub: 'xf', color: '#E6EAF1' },
    { id: 'hp', label: 'FX · HP Filter', sub: 'fx', color: '#A089DC' },
    { id: 'echo', label: 'FX · Echo', sub: 'fx', color: '#A089DC' },
  ].filter(o => !existing.has(o.id));

  const add = (o) => {
    setTemplate(prev => ({
      ...prev,
      tracks: [...prev.tracks, { id: o.id, label: o.label, sub: o.sub, color: o.color, keyframes: [], collapsed: false }]
    }));
    setOpen(false);
  };

  return (
    <div style={{ display: 'flex', position: 'relative' }}>
      <div style={{ width, height: 28, flexShrink: 0, background: 'var(--c-ink-2)', borderRight: '1px solid var(--stroke-1)', display: 'flex', alignItems: 'center', padding: '0 8px' }}>
        <button className="btn-xs accent" onClick={() => setOpen(o => !o)} style={{ width: '100%', justifyContent: 'center' }}>
          <i data-lucide="plus" style={{ width: 10, height: 10 }}/> TRACK
        </button>
      </div>
      <div style={{ flex: 1, height: 28, background: 'var(--c-ink-1)' }}/>
      {open && (
        <div style={{
          position: 'absolute', left: 8, top: 32, zIndex: 20,
          background: 'var(--c-ink-3)', border: '1px solid var(--stroke-1)', borderRadius: 8,
          boxShadow: 'var(--shadow-3)', padding: 4, minWidth: 220, maxHeight: 300, overflowY: 'auto',
        }}>
          {options.length === 0 && <div style={{ padding: 8, fontSize: 11, color: 'var(--fg-5)' }}>全トラック追加済 / all added</div>}
          {options.map(o => (
            <div key={o.id} onClick={() => add(o)}
              style={{ padding: '6px 10px', borderRadius: 5, cursor: 'pointer', fontSize: 12, color: 'var(--fg-2)', borderLeft: `2px solid ${o.color}` }}
              onMouseEnter={e => e.currentTarget.style.background = 'var(--bg-hover)'}
              onMouseLeave={e => e.currentTarget.style.background = 'transparent'}>
              {o.label}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/* ---------- Bottom bar: cues + duration ---------- */
function BottomBar({ template, setTemplate }) {
  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 16,
      padding: '10px 16px', borderTop: '1px solid var(--stroke-1)',
      background: 'var(--c-ink-2)',
    }}>
      <CueField label="Entry Cue" labelEn="ENTRY" value={template.entryCue} onChange={v => setTemplate(p => ({...p, entryCue: v}))} color="var(--c-accent)"/>
      <CueField label="Exit Cue" labelEn="EXIT" value={template.exitCue} onChange={v => setTemplate(p => ({...p, exitCue: v}))} color="var(--c-deck-b)"/>

      <div style={{ flex: 1 }}/>

      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end' }}>
          <div className="overline" style={{ fontSize: 9 }}>全体尺 / DURATION</div>
          <div className="tabular" style={{ fontSize: 15, fontWeight: 700, color: 'var(--fg-1)' }}>
            {template.bars} <span style={{ fontSize: 10, color: 'var(--fg-4)', fontWeight: 400 }}>bars</span>
            <span style={{ fontSize: 10, color: 'var(--fg-5)', fontWeight: 400, marginLeft: 6 }}>·</span>
            <span style={{ fontSize: 11, color: 'var(--fg-4)', fontWeight: 400, marginLeft: 6 }}>
              {(template.bars * 4 * 60 / 128).toFixed(1)}s @ 128
            </span>
          </div>
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
          <button className="icon-btn" onClick={() => setTemplate(p => ({...p, bars: p.bars + 4}))}>
            <i data-lucide="chevron-up" style={{ width: 11, height: 11 }}/>
          </button>
          <button className="icon-btn" onClick={() => setTemplate(p => ({...p, bars: Math.max(1, p.bars - 4)}))}>
            <i data-lucide="chevron-down" style={{ width: 11, height: 11 }}/>
          </button>
        </div>
      </div>
    </div>
  );
}

function CueField({ label, labelEn, value, onChange, color }) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
        <div style={{ width: 7, height: 7, borderRadius: '50%', background: color }}/>
        <span className="overline" style={{ fontSize: 9 }}>{label} / {labelEn}</span>
      </div>
      <select value={value} onChange={e => onChange(e.target.value)}
        style={{
          background: 'var(--c-ink-3)', border: '1px solid var(--stroke-1)', borderRadius: 6,
          padding: '5px 8px', color: 'var(--fg-1)', fontFamily: 'var(--font-mono)',
          fontSize: 11, outline: 'none', minWidth: 160,
        }}>
        <option value="auto">Auto-detect</option>
        <option value="intro-16">Intro · 16 bars</option>
        <option value="breakdown-32">Breakdown · 32 bars</option>
        <option value="drop">Drop @ 1:24</option>
        <option value="outro-32">Outro · 32 bars</option>
        <option value="custom">Custom cue…</option>
      </select>
    </div>
  );
}

function CurveIcon({ type }) {
  const paths = {
    linear: 'M 2 14 L 14 2',
    easeOut: 'M 2 14 Q 2 2, 14 2',
    easeIn: 'M 2 14 Q 14 14, 14 2',
    easeInOut: 'M 2 14 C 8 14, 8 2, 14 2',
    step: 'M 2 14 L 8 14 L 8 2 L 14 2',
    hold: 'M 2 2 L 14 2',
  };
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
      <path d={paths[type]} stroke="var(--c-accent)" strokeWidth="1.5" strokeLinecap="round" fill="none"/>
    </svg>
  );
}

Object.assign(window, { AutomationTimeline });
