// BeatgridCorrection.jsx — BPM & beat grid manual verification
function BeatgridCorrection({ track, onClose, onConfirm }) {
  const t = {
    id: 'demo', title: 'Midnight Drive', artist: 'Kaoru',
    detectedBpm: 128.02, detectedConfidence: 0.87, detectedDownbeatMs: 142,
    ...(track || {}),
    // ensure analysis fields always exist: fall back to bpm + sensible defaults
    detectedBpm: (track && (track.detectedBpm ?? track.bpm)) ?? 128.02,
    detectedConfidence: (track && track.detectedConfidence) ?? 0.87,
    detectedDownbeatMs: (track && track.detectedDownbeatMs) ?? 142,
  };

  const [bpm, setBpm] = React.useState(t.detectedBpm);
  const [offset, setOffset] = React.useState(t.detectedDownbeatMs);
  const [zoom, setZoom] = React.useState(8); // bars visible
  const [scroll, setScroll] = React.useState(0); // bar offset
  const [status, setStatus] = React.useState('未検証');
  const [playing, setPlaying] = React.useState(false);
  const [met, setMet] = React.useState(true);
  const [metVol, setMetVol] = React.useState(0.4);
  const [loop, setLoop] = React.useState(8);
  const [aiDiff, setAiDiff] = React.useState(null);
  const waveRef = React.useRef(null);
  const [dragOffset, setDragOffset] = React.useState(null);

  const beatsTotal = zoom * 4;
  const BPM_DIFF_AI = { bpm: 128.00, offset: 138 };

  React.useEffect(() => { lucide.createIcons({ attrs: { 'stroke-width': 1.75 } }); });

  const tap = (() => {
    let taps = [];
    return () => {
      const now = performance.now();
      taps = taps.filter(x => now - x < 2500);
      taps.push(now);
      if (taps.length >= 3) {
        const intervals = [];
        for (let i = 1; i < taps.length; i++) intervals.push(taps[i] - taps[i - 1]);
        const avg = intervals.reduce((a, b) => a + b, 0) / intervals.length;
        setBpm(parseFloat((60000 / avg).toFixed(2)));
      }
    };
  })();

  const bpmAdjust = (delta) => setBpm(b => parseFloat((b + delta).toFixed(2)));
  const offAdjust = (delta) => setOffset(o => o + delta);

  return (
    <div style={{ flex: 1, display: 'flex', minHeight: 0, background: 'var(--c-ink-1)' }}>
      {/* Main */}
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
        {/* Header */}
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
            <div className="overline" style={{ fontSize: 9 }}>ビートグリッド補正 / BEATGRID CORRECTION</div>
            <div style={{ display: 'flex', alignItems: 'baseline', gap: 10, marginTop: 2 }}>
              <h2 style={{ fontSize: 20, fontWeight: 700, letterSpacing: 'var(--tracking-tight)', color: 'var(--fg-1)', margin: 0 }}>
                {t.title}
              </h2>
              <span style={{ fontSize: 12, color: 'var(--fg-4)' }}>{t.artist}</span>
            </div>
          </div>
          <StatusBadge status={status}/>
        </header>

        {/* Waveform */}
        <div style={{ padding: '16px 20px 10px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 10 }}>
            <span className="overline" style={{ fontSize: 9 }}>波形 / WAVEFORM</span>
            <div style={{ flex: 1 }}/>
            <span className="overline" style={{ fontSize: 9 }}>ズーム</span>
            <div className="tab-strip" style={{ padding: 2 }}>
              {[4, 8, 16].map(n => (
                <button key={n} className={zoom === n ? 'active' : ''} onClick={() => setZoom(n)} style={{ padding: '4px 10px', fontSize: 9 }}>
                  {n} bars
                </button>
              ))}
            </div>
          </div>
          <Waveform
            ref={waveRef}
            height={240} beatsTotal={beatsTotal} bpm={bpm} offset={offset} scroll={scroll}
            onOffsetDragStart={() => setDragOffset({ startX: 0, startOffset: offset })}
            onOffsetDrag={(dx) => setOffset(o => Math.max(-200, Math.min(400, (dragOffset?.startOffset || offset) + dx)))}
            onOffsetDragEnd={() => setDragOffset(null)}
            playing={playing} playhead={0}
          />
          {/* Scrollbar */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginTop: 8 }}>
            <i data-lucide="move-horizontal" style={{ width: 13, height: 13, color: 'var(--fg-5)' }}/>
            <input type="range" min="0" max="200" value={scroll} onChange={e => setScroll(parseInt(e.target.value))}
              style={{ flex: 1, accentColor: 'var(--c-accent)' }}/>
            <span className="tabular" style={{ fontSize: 11, color: 'var(--fg-4)' }}>bar {scroll + 1} – {scroll + zoom}</span>
          </div>
        </div>

        {/* Numeric controls */}
        <div style={{ padding: '10px 20px', borderTop: '1px solid var(--stroke-1)', display: 'flex', gap: 24, alignItems: 'center' }}>
          {/* BPM */}
          <NumericStepper label="BPM" labelEn="BPM"
            value={bpm.toFixed(2)} unit=""
            steps={[-1, -0.1, -0.01, 0.01, 0.1, 1]}
            onStep={d => bpmAdjust(d)}
            big/>

          {/* Downbeat offset */}
          <NumericStepper label="Downbeat Offset" labelEn="DOWNBEAT"
            value={offset} unit="ms"
            steps={[-10, -1, 1, 10]}
            onStep={d => offAdjust(d)}
          />

          {/* Tap tempo */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            <div className="overline" style={{ fontSize: 9 }}>Tap Tempo / TAP</div>
            <button onMouseDown={tap}
              style={{
                padding: '11px 20px', border: '1px solid rgba(79,227,178,0.35)', borderRadius: 10,
                background: 'linear-gradient(180deg, var(--c-accent-soft), rgba(79,227,178,0.03))',
                color: 'var(--c-accent-hi)', fontFamily: 'var(--font-sans)', fontWeight: 700, fontSize: 12,
                letterSpacing: 'var(--tracking-wide)', textTransform: 'uppercase',
                cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 8,
              }}>
              <i data-lucide="drum" style={{ width: 14, height: 14 }}/> TAP
            </button>
          </div>
        </div>

        {/* Preview controls */}
        <div style={{ padding: '10px 20px 16px', borderTop: '1px solid var(--stroke-1)', display: 'flex', alignItems: 'center', gap: 16, background: 'var(--c-ink-2)' }}>
          <button onClick={() => setPlaying(p => !p)}
            style={{
              width: 48, height: 48, borderRadius: 12, border: 'none',
              background: playing
                ? 'linear-gradient(180deg, var(--c-accent-hi), var(--c-accent))'
                : 'linear-gradient(180deg, var(--c-ink-4), var(--c-ink-3))',
              color: playing ? 'var(--c-ink-0)' : 'var(--fg-1)',
              cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center',
              boxShadow: playing ? '0 0 14px var(--c-accent-glow)' : 'var(--shadow-1)',
            }}>
            <i data-lucide={playing ? 'pause' : 'play'} style={{ width: 18, height: 18, fill: playing ? 'currentColor' : 'none' }}/>
          </button>

          {/* Metronome */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <button onClick={() => setMet(m => !m)}
              className="btn-xs"
              style={{
                height: 36, padding: '0 14px',
                background: met ? 'var(--c-accent-soft)' : 'var(--c-ink-3)',
                borderColor: met ? 'rgba(79,227,178,0.35)' : 'var(--stroke-1)',
                color: met ? 'var(--c-accent-hi)' : 'var(--fg-3)',
              }}>
              <i data-lucide="metronome" style={{ width: 13, height: 13 }}/>
              メトロノーム / METRONOME
            </button>
            <input type="range" min="0" max="1" step="0.05" value={metVol} onChange={e => setMetVol(parseFloat(e.target.value))}
              disabled={!met} style={{ width: 100, accentColor: 'var(--c-accent)', opacity: met ? 1 : 0.4 }}/>
            <span className="tabular" style={{ fontSize: 10, color: 'var(--fg-5)', minWidth: 32 }}>{Math.round(metVol * 100)}%</span>
          </div>

          <div style={{ width: 1, height: 24, background: 'var(--stroke-1)' }}/>

          {/* Loop */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <span className="overline" style={{ fontSize: 9 }}>ループ / LOOP</span>
            <div className="tab-strip" style={{ padding: 2 }}>
              {[4, 8, 16].map(n => (
                <button key={n} className={loop === n ? 'active' : ''} onClick={() => setLoop(n)} style={{ padding: '4px 10px', fontSize: 9 }}>
                  {n}
                </button>
              ))}
            </div>
          </div>

          <div style={{ flex: 1 }}/>

          {/* Warning */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 12px', background: 'rgba(255,184,64,0.08)', border: '1px solid rgba(255,184,64,0.25)', borderRadius: 8 }}>
            <i data-lucide="alert-triangle" style={{ width: 13, height: 13, color: 'var(--c-warning)' }}/>
            <span style={{ fontSize: 11, color: 'var(--c-warning)', fontWeight: 500 }}>
              未検証トラックは本番再生不可 / Locked for live
            </span>
          </div>
        </div>
      </div>

      {/* Right panel */}
      <aside style={{
        width: 320, flexShrink: 0, borderLeft: '1px solid var(--stroke-1)',
        background: 'var(--c-ink-2)', display: 'flex', flexDirection: 'column', minHeight: 0,
      }}>
        {/* Detection summary */}
        <section style={{ padding: '16px 18px', borderBottom: '1px solid var(--stroke-1)' }}>
          <div className="overline" style={{ fontSize: 9 }}>自動検出結果 / AUTO-DETECTED</div>
          <div style={{ marginTop: 10, display: 'flex', flexDirection: 'column', gap: 10 }}>
            <Row label="BPM" labelEn="BPM" value={t.detectedBpm.toFixed(2)} mono/>
            <Row label="信頼度" labelEn="CONFIDENCE"
              value={<ConfidenceBar v={t.detectedConfidence}/>} />
            <Row label="Downbeat" labelEn="DOWNBEAT" value={`${t.detectedDownbeatMs} ms`} mono/>
          </div>

          {/* Diff from current */}
          {(Math.abs(bpm - t.detectedBpm) > 0.01 || Math.abs(offset - t.detectedDownbeatMs) > 0) && (
            <div style={{ marginTop: 12, padding: '8px 10px', background: 'rgba(79,227,178,0.06)', border: '1px solid rgba(79,227,178,0.2)', borderRadius: 6 }}>
              <div className="overline" style={{ fontSize: 8, color: 'var(--c-accent)' }}>補正量 / DELTA</div>
              <div style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--fg-2)', marginTop: 3 }}>
                BPM {(bpm - t.detectedBpm).toFixed(2) > 0 ? '+' : ''}{(bpm - t.detectedBpm).toFixed(2)} ·
                Off {offset - t.detectedDownbeatMs > 0 ? '+' : ''}{offset - t.detectedDownbeatMs}ms
              </div>
            </div>
          )}
        </section>

        {/* Validation status */}
        <section style={{ padding: '16px 18px', borderBottom: '1px solid var(--stroke-1)' }}>
          <div className="overline" style={{ fontSize: 9 }}>検証ステータス / VALIDATION</div>
          <div style={{ marginTop: 10, display: 'flex', flexDirection: 'column', gap: 4 }}>
            {['未検証', '検証中', '検証完了'].map(s => (
              <button key={s} onClick={() => setStatus(s)}
                style={{
                  display: 'flex', alignItems: 'center', gap: 8,
                  padding: '8px 10px', borderRadius: 6,
                  background: status === s ? 'var(--c-glass-3)' : 'transparent',
                  border: `1px solid ${status === s ? 'var(--stroke-1)' : 'transparent'}`,
                  color: status === s ? 'var(--fg-1)' : 'var(--fg-4)',
                  fontFamily: 'var(--font-sans)', fontSize: 12, fontWeight: status === s ? 700 : 400,
                  cursor: 'pointer', textAlign: 'left',
                }}>
                <div style={{
                  width: 8, height: 8, borderRadius: '50%',
                  background: s === '検証完了' ? 'var(--c-accent)' : s === '検証中' ? 'var(--c-warning)' : 'var(--c-ink-6)',
                  boxShadow: status === s && s === '検証完了' ? '0 0 8px var(--c-accent-glow)' : 'none',
                }}/>
                <span>{s}</span>
              </button>
            ))}
          </div>
        </section>

        {/* AI assist */}
        <section style={{ padding: '16px 18px', borderBottom: '1px solid var(--stroke-1)' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 8 }}>
            <i data-lucide="sparkles" style={{ width: 12, height: 12, color: 'var(--c-accent)' }}/>
            <div className="overline" style={{ fontSize: 9, color: 'var(--c-accent)' }}>CLAUDE CLI 検出</div>
          </div>
          <button onClick={() => setAiDiff(aiDiff ? null : BPM_DIFF_AI)}
            style={{
              width: '100%', padding: '9px 12px',
              background: 'linear-gradient(180deg, var(--c-accent-soft), rgba(79,227,178,0.03))',
              border: '1px solid rgba(79,227,178,0.28)',
              borderRadius: 8, color: 'var(--c-accent-hi)',
              fontFamily: 'var(--font-sans)', fontWeight: 700, fontSize: 12,
              letterSpacing: 'var(--tracking-tight)',
              cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8,
            }}>
            <i data-lucide="wand-2" style={{ width: 13, height: 13 }}/>
            AI アシスト補正
          </button>

          {aiDiff && (
            <div style={{ marginTop: 10, padding: 10, background: 'var(--c-ink-3)', border: '1px solid var(--stroke-1)', borderRadius: 8 }}>
              <div className="overline" style={{ fontSize: 8, marginBottom: 6 }}>推定結果 / PROPOSED</div>
              <div style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--fg-2)', lineHeight: 1.8 }}>
                <div>BPM: <s style={{ color: 'var(--fg-5)' }}>{bpm.toFixed(2)}</s> → <span style={{ color: 'var(--c-accent)' }}>{aiDiff.bpm.toFixed(2)}</span></div>
                <div>Off: <s style={{ color: 'var(--fg-5)' }}>{offset}ms</s> → <span style={{ color: 'var(--c-accent)' }}>{aiDiff.offset}ms</span></div>
              </div>
              <div style={{ display: 'flex', gap: 6, marginTop: 8 }}>
                <button className="btn-xs accent" style={{ flex: 1 }} onClick={() => { setBpm(aiDiff.bpm); setOffset(aiDiff.offset); setAiDiff(null); }}>
                  <i data-lucide="check" style={{ width: 10, height: 10 }}/> ACCEPT
                </button>
                <button className="btn-xs" style={{ flex: 1 }} onClick={() => setAiDiff(null)}>
                  <i data-lucide="x" style={{ width: 10, height: 10 }}/> REJECT
                </button>
              </div>
            </div>
          )}
        </section>

        {/* Confirm */}
        <div style={{ marginTop: 'auto', padding: '14px 18px', borderTop: '1px solid var(--stroke-1)' }}>
          <button onClick={() => { setStatus('検証完了'); onConfirm && onConfirm({ bpm, offset }); }}
            disabled={status === '検証完了'}
            style={{
              width: '100%', height: 44, borderRadius: 10, border: 'none', cursor: status === '検証完了' ? 'default' : 'pointer',
              background: status === '検証完了'
                ? 'linear-gradient(180deg, rgba(79,227,178,0.2), rgba(79,227,178,0.1))'
                : 'linear-gradient(180deg, var(--c-accent-hi), var(--c-accent))',
              color: status === '検証完了' ? 'var(--c-accent-hi)' : 'var(--c-ink-0)',
              fontFamily: 'var(--font-sans)', fontWeight: 800, fontSize: 13,
              letterSpacing: 'var(--tracking-wide)', textTransform: 'uppercase',
              boxShadow: status === '検証完了' ? 'none' : '0 4px 16px var(--c-accent-glow), inset 0 1px 0 rgba(255,255,255,0.2)',
              display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8,
            }}>
            <i data-lucide={status === '検証完了' ? 'check-circle-2' : 'check'} style={{ width: 16, height: 16 }}/>
            {status === '検証完了' ? '検証済み' : '検証完了'}
          </button>
        </div>
      </aside>
    </div>
  );
}

/* ---------- Waveform canvas ---------- */
const Waveform = React.forwardRef(({ height, beatsTotal, bpm, offset, scroll, playing, playhead, onOffsetDragStart, onOffsetDrag, onOffsetDragEnd }, ref) => {
  const canvasRef = React.useRef(null);
  const [drag, setDrag] = React.useState(null);

  React.useEffect(() => {
    const c = canvasRef.current;
    if (!c) return;
    const rect = c.parentElement.getBoundingClientRect();
    const w = rect.width;
    c.width = w * devicePixelRatio;
    c.height = height * devicePixelRatio;
    c.style.width = w + 'px';
    c.style.height = height + 'px';
    const ctx = c.getContext('2d');
    ctx.scale(devicePixelRatio, devicePixelRatio);
    // BG
    ctx.fillStyle = '#0C0E12';
    ctx.fillRect(0, 0, w, height);
    // Waveform
    const centerY = height / 2;
    ctx.fillStyle = '#2BB38A';
    ctx.globalAlpha = 0.55;
    for (let x = 0; x < w; x += 2) {
      // deterministic pseudo-random based on scroll + x
      const seed = (x * 17.31 + scroll * 97 + 0.5);
      const r1 = Math.abs(Math.sin(seed)) * 0.5 + Math.abs(Math.sin(seed * 2.3)) * 0.5;
      const r2 = Math.abs(Math.sin(seed * 1.7)) * 0.3 + Math.abs(Math.sin(seed * 3.1)) * 0.2;
      const envA = 0.4 + r1 * 0.5 + r2 * 0.3;
      const envB = (Math.sin((x + scroll * 40) / 55) * 0.4 + 0.6);
      const amp = envA * envB * (height / 2 - 10);
      ctx.fillRect(x, centerY - amp, 1.2, amp * 2);
    }
    ctx.globalAlpha = 1;
    // center line
    ctx.strokeStyle = 'rgba(255,255,255,0.08)';
    ctx.lineWidth = 1;
    ctx.beginPath(); ctx.moveTo(0, centerY); ctx.lineTo(w, centerY); ctx.stroke();
  }, [height, beatsTotal, scroll]);

  return (
    <div style={{ position: 'relative', background: 'var(--c-ink-0)', border: '1px solid var(--stroke-1)', borderRadius: 10, overflow: 'hidden' }}>
      <canvas ref={canvasRef} />
      {/* Grid overlay */}
      <GridOverlay beatsTotal={beatsTotal} offset={offset} height={height} onDownbeatDragStart={onOffsetDragStart} onDownbeatDrag={onOffsetDrag} onDownbeatDragEnd={onOffsetDragEnd}/>
      {/* Ruler */}
      <div style={{ position: 'absolute', top: 0, left: 0, right: 0, height: 20, background: 'linear-gradient(180deg, rgba(0,0,0,0.5), transparent)', pointerEvents: 'none' }}>
        {Array.from({ length: Math.ceil(beatsTotal / 4) + 1 }).map((_, i) => (
          <span key={i} className="tabular" style={{
            position: 'absolute', left: `${(i * 4 / beatsTotal) * 100}%`,
            top: 3, fontSize: 9, color: 'var(--fg-4)', fontWeight: 700, paddingLeft: 4,
          }}>{i + 1}</span>
        ))}
      </div>
    </div>
  );
});

function GridOverlay({ beatsTotal, offset, height, onDownbeatDragStart, onDownbeatDrag, onDownbeatDragEnd }) {
  const [ref, setRef] = React.useState(null);
  const [drag, setDrag] = React.useState(null);
  const w = ref?.clientWidth || 1;
  const pxPerBeat = w / beatsTotal;
  const offsetPx = (offset / 1000) * pxPerBeat * 2; // exaggerated for demo

  React.useEffect(() => {
    if (!drag) return;
    const move = e => onDownbeatDrag(e.clientX - drag.startX);
    const up = () => { onDownbeatDragEnd(); setDrag(null); };
    window.addEventListener('mousemove', move); window.addEventListener('mouseup', up);
    return () => { window.removeEventListener('mousemove', move); window.removeEventListener('mouseup', up); };
  }, [drag]);

  return (
    <div ref={setRef} style={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}>
      {/* beat lines */}
      {Array.from({ length: beatsTotal + 1 }).map((_, i) => {
        const isBar = i % 4 === 0;
        const isPhrase = i % 16 === 0;
        return (
          <div key={i} style={{
            position: 'absolute', top: 20, bottom: 0,
            left: i * pxPerBeat + offsetPx,
            borderLeft: `${isPhrase ? 2 : isBar ? 1 : 1}px solid ${isPhrase ? 'rgba(79,227,178,0.6)' : isBar ? 'rgba(255,255,255,0.45)' : 'rgba(255,255,255,0.15)'}`,
          }}/>
        );
      })}
      {/* downbeat marker */}
      <div
        onMouseDown={(e) => { onDownbeatDragStart && onDownbeatDragStart(); setDrag({ startX: e.clientX }); }}
        style={{
          position: 'absolute', top: 20, bottom: 0, left: offsetPx, width: 3,
          background: 'var(--c-danger)', cursor: 'ew-resize', pointerEvents: 'auto',
          boxShadow: '0 0 8px rgba(255,74,92,0.5)', zIndex: 5,
        }}>
        <div style={{ position: 'absolute', top: -6, left: -7, width: 17, height: 14,
          background: 'var(--c-danger)', clipPath: 'polygon(50% 100%, 0 0, 100% 0)',
        }}/>
      </div>
    </div>
  );
}

/* ---------- Helpers ---------- */
function NumericStepper({ label, labelEn, value, unit, steps, onStep, big }) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
      <div className="overline" style={{ fontSize: 9 }}>{label} / {labelEn}</div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <div className="tabular" style={{
          fontSize: big ? 34 : 22, fontWeight: 700, color: 'var(--fg-1)',
          letterSpacing: 'var(--tracking-tight)', minWidth: big ? 130 : 90,
        }}>
          {value}
          {unit && <span style={{ fontSize: big ? 14 : 11, color: 'var(--fg-4)', fontWeight: 400, marginLeft: 4 }}>{unit}</span>}
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: `repeat(${Math.ceil(steps.length/2)}, auto)`, gap: 3 }}>
          {steps.map(s => (
            <button key={s} className="btn-xs" onClick={() => onStep(s)} style={{ height: 22, padding: '0 8px', fontSize: 10 }}>
              {s > 0 ? '+' : ''}{s}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

function StatusBadge({ status }) {
  const c = status === '検証完了' ? 'var(--c-accent)' : status === '検証中' ? 'var(--c-warning)' : 'var(--c-warning)';
  const bg = status === '検証完了' ? 'rgba(79,227,178,0.08)' : 'rgba(255,184,64,0.08)';
  const en = status === '検証完了' ? 'VERIFIED' : status === '検証中' ? 'REVIEWING' : 'UNVERIFIED';
  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 8,
      padding: '6px 12px', borderRadius: 20,
      background: bg, border: `1px solid ${c.replace('var(--c-', 'rgba(').slice(0, -1)}`,
    }}>
      <i data-lucide={status === '検証完了' ? 'check-circle-2' : 'alert-triangle'} style={{ width: 13, height: 13, color: c }}/>
      <div style={{ display: 'flex', flexDirection: 'column', lineHeight: 1 }}>
        <span style={{ fontSize: 11, fontWeight: 700, color: c }}>{status}</span>
        <span style={{ fontSize: 8, fontWeight: 700, letterSpacing: 'var(--tracking-ultrawide)', color: c, opacity: 0.7, marginTop: 1 }}>{en}</span>
      </div>
    </div>
  );
}

function Row({ label, labelEn, value, mono }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
      <div>
        <div style={{ fontSize: 11, color: 'var(--fg-3)' }}>{label}</div>
        <div className="overline" style={{ fontSize: 8, marginTop: 1 }}>{labelEn}</div>
      </div>
      <div className={mono ? 'tabular' : ''} style={{ fontSize: 15, fontWeight: 700, color: 'var(--fg-1)' }}>{value}</div>
    </div>
  );
}

function ConfidenceBar({ v }) {
  const pct = Math.round(v * 100);
  const color = v > 0.85 ? 'var(--c-accent)' : v > 0.6 ? 'var(--c-warning)' : 'var(--c-danger)';
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
      <div style={{ width: 80, height: 4, background: 'var(--c-ink-4)', borderRadius: 2, overflow: 'hidden' }}>
        <div style={{ width: `${pct}%`, height: '100%', background: color }}/>
      </div>
      <span className="tabular" style={{ fontSize: 12, fontWeight: 700, color }}>{pct}%</span>
    </div>
  );
}

Object.assign(window, { BeatgridCorrection });
