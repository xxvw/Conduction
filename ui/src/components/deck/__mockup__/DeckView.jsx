// DeckView.jsx — Horizontal deck strip (full-width, A stacked above B)
const { useEffect: useEffectDV, useRef: useRefDV } = React;

function DeckView({ side = 'A', color = '#4FE3B2', track, playing, onTogglePlay, progress = 0.4 }) {
  const wRef = useRefDV(null);

  useEffectDV(() => {
    if (!wRef.current) return;
    // 3-band mirrored, rekordbox-style, scaled to viewport of 1200x100
    const W = 1200, BW = 2, GAP = 1, N = Math.floor(W/(BW+GAP));
    const cy = 50;
    let s = '';
    for (let i = 0; i < N; i++) {
      const x = i * (BW + GAP);
      const low = Math.abs(Math.sin(i*0.11+(side==='B'?0.7:0))*0.65 + Math.sin(i*0.31)*0.3) * 26;
      const mid = Math.abs(Math.sin(i*0.21+1)*0.55 + Math.sin(i*0.49)*0.4) * 18;
      const hi  = Math.abs(Math.sin(i*0.44+2)*0.45 + Math.sin(i*0.88)*0.5) * 10;
      const played = x/W < progress;
      const op = played ? 1 : 0.35;
      const lowC = played ? '#E8935A' : '#5A4840';
      const midC = played ? color : '#3B4453';
      const hiC  = played ? '#8A9BE8' : '#3B4453';
      // upper
      s += `<rect x="${x}" y="${cy-low}" width="${BW}" height="${low}" fill="${lowC}" opacity="${op}"/>`;
      s += `<rect x="${x}" y="${cy-low-mid}" width="${BW}" height="${mid}" fill="${midC}" opacity="${op}"/>`;
      s += `<rect x="${x}" y="${cy-low-mid-hi}" width="${BW}" height="${hi}" fill="${hiC}" opacity="${op*0.9}"/>`;
      // mirror (attenuated)
      const a = 0.75;
      s += `<rect x="${x}" y="${cy}" width="${BW}" height="${low*a}" fill="${lowC}" opacity="${op*0.75}"/>`;
      s += `<rect x="${x}" y="${cy+low*a}" width="${BW}" height="${mid*a}" fill="${midC}" opacity="${op*0.75}"/>`;
      s += `<rect x="${x}" y="${cy+low*a+mid*a}" width="${BW}" height="${hi*a}" fill="${hiC}" opacity="${op*0.65}"/>`;
    }
    wRef.current.innerHTML = s;
  }, [progress, color, side]);

  const playheadX = 1200 * progress;

  return (
    <div style={{
      background: 'linear-gradient(180deg, var(--c-ink-3), var(--c-ink-2))',
      border: '1px solid var(--stroke-1)', borderRadius: 14,
      boxShadow: 'var(--shadow-2), inset 0 1px 0 rgba(255,255,255,0.05)',
      display: 'grid',
      gridTemplateColumns: '240px 1fr 300px',
      overflow: 'hidden',
      minHeight: 0,
    }}>
      {/* Left: deck id + track meta */}
      <div style={{ padding: '12px 14px', display: 'flex', flexDirection: 'column', gap: 8, borderRight: '1px solid rgba(255,255,255,0.04)', justifyContent: 'center' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <span style={{
            fontFamily: 'var(--font-sans)', fontWeight: 800, fontSize: 10, letterSpacing: '0.2em',
            color, background: `${color}18`, border: `1px solid ${color}4D`, borderRadius: 4,
            padding: '3px 8px',
          }}>DECK {side}</span>
          <span className="tabular" style={{ fontSize: 11, color: 'var(--fg-4)' }}>
            {track ? formatTimeDV(track.duration * progress) : '—'} / {track ? formatTimeDV(track.duration) : '—'}
          </span>
        </div>
        <div style={{ minWidth: 0 }}>
          <div style={{ fontWeight: 700, fontSize: 15, color: 'var(--fg-1)', letterSpacing: '-0.01em', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
            {track?.title ?? 'No track loaded'}
          </div>
          <div style={{ fontSize: 11, color: 'var(--fg-4)', marginTop: 1, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
            {track?.artist ?? '—'}
          </div>
        </div>
        <div style={{ display: 'flex', gap: 14, alignItems: 'baseline' }}>
          <div>
            <div style={{ fontSize: 8, fontWeight: 700, letterSpacing: '0.2em', color: 'var(--fg-5)' }}>BPM</div>
            <div className="tabular" style={{ fontSize: 18, fontWeight: 500, color }}>{track?.bpm?.toFixed(1) ?? '—'}</div>
          </div>
          <div>
            <div style={{ fontSize: 8, fontWeight: 700, letterSpacing: '0.2em', color: 'var(--fg-5)' }}>KEY</div>
            <div className="tabular" style={{ fontSize: 18, fontWeight: 500, color: 'var(--c-cue)' }}>{track?.key ?? '—'}</div>
          </div>
          <div>
            <div style={{ fontSize: 8, fontWeight: 700, letterSpacing: '0.2em', color: 'var(--fg-5)' }}>PITCH</div>
            <div className="tabular" style={{ fontSize: 14, fontWeight: 400, color: 'var(--fg-3)' }}>+0.0%</div>
          </div>
        </div>
      </div>

      {/* Middle: waveform */}
      <div style={{ position: 'relative', padding: '6px 8px', background: 'var(--c-ink-1)', boxShadow: 'inset 0 2px 6px rgba(0,0,0,0.5)' }}>
        <svg viewBox="0 0 1200 100" preserveAspectRatio="none" style={{ width: '100%', height: '100%', display: 'block' }}>
          {/* Beat grid */}
          <g>
            {Array.from({length: 41}).map((_, i) => {
              const strong = i % 4 === 0;
              const x = i * 30;
              return <line key={i} x1={x} y1="12" x2={x} y2="96" stroke={`rgba(255,255,255,${strong ? 0.08 : 0.03})`} strokeWidth={strong ? 1 : 0.5}/>;
            })}
          </g>
          <line x1="0" y1="50" x2="1200" y2="50" stroke="rgba(255,255,255,0.04)"/>
          <g ref={wRef} />
          {/* markers */}
          <g fontFamily="JetBrains Mono" fontSize="8" letterSpacing="0.1em">
            <rect x="240" y="6" width="1.5" height="10" fill="var(--c-cue)"/><text x="245" y="13" fill="var(--c-cue)">CUE 1</text>
            <rect x="520" y="6" width="1.5" height="10" fill="var(--c-cue)"/><text x="525" y="13" fill="var(--c-cue)">CUE 2</text>
            <rect x="720" y="6" width="1.5" height="10" fill="var(--c-loop)"/>
            <rect x="840" y="6" width="1.5" height="10" fill="var(--c-loop)"/>
            <rect x="720" y="6" width="120" height="10" fill="rgba(160,137,220,0.1)" stroke="rgba(160,137,220,0.35)" strokeWidth="0.5"/>
            <text x="725" y="13" fill="var(--c-loop)">LOOP · 8</text>
          </g>
          {/* playhead */}
          <line x1={playheadX} y1="12" x2={playheadX} y2="96" stroke="#F5F7FB" strokeWidth="1.2"/>
          <polygon points={`${playheadX-4},12 ${playheadX+4},12 ${playheadX},17`} fill="#F5F7FB"/>
          <polygon points={`${playheadX-4},96 ${playheadX+4},96 ${playheadX},91`} fill="#F5F7FB"/>
        </svg>
      </div>

      {/* Right: transport + hot cues */}
      <div style={{ padding: '10px 12px', display: 'flex', flexDirection: 'column', gap: 8, borderLeft: '1px solid rgba(255,255,255,0.04)', justifyContent: 'center' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <button className="btn-cue" title="Cue">CUE</button>
          <button onClick={onTogglePlay}
            style={{
              width: 54, height: 54, borderRadius: 14, border: `1px solid ${color}99`,
              background: `linear-gradient(180deg, ${color}, ${shade(color,-18)})`,
              color: '#07201A',
              boxShadow: `0 0 20px ${color}55, 0 4px 10px rgba(0,0,0,0.5), inset 0 1px 0 rgba(255,255,255,0.3), inset 0 -1px 0 rgba(0,0,0,0.25)`,
              cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center',
            }}>
            {playing ? (
              <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/></svg>
            ) : (
              <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><polygon points="7 4 20 12 7 20 7 4"/></svg>
            )}
          </button>
          <button className="btn-ghost">SYNC</button>
          <button className="btn-ghost" style={{ color: 'var(--c-loop)' }}>LOOP 8</button>
        </div>
        <div style={{ display: 'flex', gap: 4 }}>
          {[1,2,3,4,5,6,7,8].map(n => (
            <button key={n} className="hot-cue"
              style={{
                background: n <= 3 ? 'rgba(232,184,104,0.1)' : 'rgba(255,255,255,0.03)',
                color: n <= 3 ? 'var(--c-cue)' : 'var(--fg-5)',
                borderColor: n <= 3 ? 'rgba(232,184,104,0.35)' : 'var(--stroke-1)',
              }}>{n}</button>
          ))}
        </div>
      </div>
    </div>
  );
}

function formatTimeDV(sec) {
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60).toString().padStart(2, '0');
  return `${m}:${s}`;
}

function shade(hex, pct) {
  // small helper for gradient bottom color
  const m = /^#?([a-f0-9]{6})$/i.exec(hex || '');
  if (!m) return hex;
  const n = parseInt(m[1], 16);
  const r = Math.max(0, Math.min(255, ((n >> 16) & 255) + pct));
  const g = Math.max(0, Math.min(255, ((n >> 8) & 255) + pct));
  const b = Math.max(0, Math.min(255, (n & 255) + pct));
  return '#' + ((1 << 24) + (r << 16) + (g << 8) + b).toString(16).slice(1);
}

Object.assign(window, { DeckView, shadeDV: shade });
