// SetlistOverview.jsx — right rail: stats + BPM curve + Camelot map + energy curve
function SetlistOverview({ tracks }) {
  const totalSec = tracks.reduce((s, t) => s + t.duration, 0);
  const mins = Math.floor(totalSec / 60);
  const secs = totalSec % 60;

  return (
    <aside style={{
      width: 320, flexShrink: 0, background: 'var(--c-ink-2)',
      borderLeft: '1px solid var(--stroke-1)',
      display: 'flex', flexDirection: 'column', minHeight: 0,
    }}>
      {/* Summary */}
      <section style={{ padding: '16px 18px', borderBottom: '1px solid var(--stroke-1)' }}>
        <div className="overline" style={{ fontSize: 9 }}>セット概要 / OVERVIEW</div>
        <div style={{ marginTop: 10, display: 'flex', alignItems: 'baseline', gap: 8 }}>
          <span className="tabular" style={{ fontSize: 30, fontWeight: 700, color: 'var(--fg-1)', letterSpacing: 'var(--tracking-tight)' }}>
            {mins}:{String(secs).padStart(2,'0')}
          </span>
          <span style={{ fontSize: 11, color: 'var(--fg-4)' }}>total · {tracks.length} tracks</span>
        </div>
        <div style={{ display: 'flex', gap: 12, marginTop: 10 }}>
          <Stat label="BPM" labelEn="TEMPO" value={`${Math.min(...tracks.map(t => t.bpm)).toFixed(0)}–${Math.max(...tracks.map(t => t.bpm)).toFixed(0)}`}/>
          <Stat label="Key" labelEn="CAMELOT" value={`${uniqKeys(tracks).length}`}/>
          <Stat label="遷移" labelEn="TRANSITIONS" value={tracks.length - 1}/>
        </div>
      </section>

      {/* BPM curve */}
      <section style={{ padding: '14px 18px', borderBottom: '1px solid var(--stroke-1)' }}>
        <div className="overline" style={{ fontSize: 9 }}>BPM カーブ / BPM CURVE</div>
        <BpmCurve tracks={tracks}/>
      </section>

      {/* Camelot wheel */}
      <section style={{ padding: '14px 18px', borderBottom: '1px solid var(--stroke-1)' }}>
        <div className="overline" style={{ fontSize: 9 }}>Key 遷移 / KEY PATH</div>
        <CamelotWheel tracks={tracks}/>
      </section>

      {/* Energy curve */}
      <section style={{ padding: '14px 18px' }}>
        <div className="overline" style={{ fontSize: 9 }}>エネルギー / ENERGY CURVE</div>
        <EnergyCurve tracks={tracks}/>
      </section>
    </aside>
  );
}

function Stat({ label, labelEn, value }) {
  return (
    <div style={{ flex: 1 }}>
      <div className="overline" style={{ fontSize: 8 }}>{label} / {labelEn}</div>
      <div className="tabular" style={{ fontSize: 14, fontWeight: 700, color: 'var(--fg-1)', marginTop: 2 }}>{value}</div>
    </div>
  );
}

function uniqKeys(tracks) { return [...new Set(tracks.map(t => t.key))]; }

function BpmCurve({ tracks }) {
  const h = 80;
  const w = 280;
  const minB = Math.min(...tracks.map(t => t.bpm)) - 1;
  const maxB = Math.max(...tracks.map(t => t.bpm)) + 1;
  const range = maxB - minB || 1;
  const points = tracks.map((t, i) => {
    const x = tracks.length === 1 ? w/2 : (i / (tracks.length - 1)) * w;
    const y = h - ((t.bpm - minB) / range) * (h - 16) - 8;
    return { x, y, bpm: t.bpm };
  });
  const path = points.map((p, i) => (i === 0 ? 'M' : 'L') + ` ${p.x} ${p.y}`).join(' ');
  return (
    <svg width={w} height={h + 20} style={{ marginTop: 8 }}>
      {/* grid */}
      {[0, 0.5, 1].map(f => (
        <line key={f} x1="0" y1={h * f + 4} x2={w} y2={h * f + 4} stroke="var(--stroke-1)" strokeWidth="1" opacity="0.4"/>
      ))}
      <path d={path + ` L ${w} ${h + 4} L 0 ${h + 4} Z`} fill="var(--c-accent)" opacity="0.1"/>
      <path d={path} stroke="var(--c-accent)" strokeWidth="1.5" fill="none"/>
      {points.map((p, i) => (
        <g key={i}>
          <circle cx={p.x} cy={p.y} r={3} fill="var(--c-accent)"/>
          <text x={p.x} y={h + 16} fill="var(--fg-4)" fontSize="9" fontFamily="JetBrains Mono" textAnchor="middle">
            {p.bpm.toFixed(0)}
          </text>
        </g>
      ))}
    </svg>
  );
}

function CamelotWheel({ tracks }) {
  const size = 180;
  const cx = size / 2, cy = size / 2;
  const rOuter = 78, rInner = 48;
  const keys = [];
  for (let i = 1; i <= 12; i++) keys.push({ k: `${i}A`, outer: true, angle: (i - 1) * 30 - 90 });
  for (let i = 1; i <= 12; i++) keys.push({ k: `${i}B`, outer: false, angle: (i - 1) * 30 - 90 });

  const posFor = (key) => {
    const entry = keys.find(x => x.k === key);
    if (!entry) return { x: cx, y: cy };
    const r = entry.outer ? rOuter : rInner;
    const rad = entry.angle * Math.PI / 180;
    return { x: cx + Math.cos(rad) * r, y: cy + Math.sin(rad) * r };
  };

  const positions = tracks.map(t => ({ ...posFor(t.key), key: t.key }));

  return (
    <svg width={size} height={size} style={{ display: 'block', margin: '8px auto 0' }}>
      {/* rings */}
      <circle cx={cx} cy={cy} r={rOuter + 12} fill="var(--c-ink-3)" stroke="var(--stroke-1)"/>
      <circle cx={cx} cy={cy} r={rInner - 12} fill="var(--c-ink-1)" stroke="var(--stroke-1)"/>
      {/* labels */}
      {keys.map(k => {
        const r = k.outer ? rOuter : rInner;
        const rad = k.angle * Math.PI / 180;
        const x = cx + Math.cos(rad) * r;
        const y = cy + Math.sin(rad) * r;
        return (
          <text key={k.k} x={x} y={y + 3} fill="var(--fg-5)" fontSize="8" fontFamily="JetBrains Mono" textAnchor="middle" fontWeight="700">
            {k.k}
          </text>
        );
      })}
      {/* path */}
      {positions.length > 1 && (
        <polyline
          points={positions.map(p => `${p.x},${p.y}`).join(' ')}
          stroke="var(--c-accent)" strokeWidth="1.5" fill="none" strokeLinejoin="round"
          strokeDasharray="3 3" opacity="0.8"/>
      )}
      {positions.map((p, i) => (
        <circle key={i} cx={p.x} cy={p.y} r={i === 0 ? 5 : 4}
          fill={i === 0 ? 'var(--c-accent)' : 'var(--c-deck-b)'}
          stroke="var(--c-ink-0)" strokeWidth="1.5"/>
      ))}
      {/* center */}
      <text x={cx} y={cy + 3} fill="var(--fg-4)" fontSize="9" fontFamily="LINE Seed JP" textAnchor="middle" fontWeight="700">
        {tracks.length}
      </text>
    </svg>
  );
}

function EnergyCurve({ tracks }) {
  const h = 80;
  const w = 280;
  // simulated energy
  const points = tracks.map((t, i) => {
    const x = tracks.length === 1 ? w/2 : (i / (tracks.length - 1)) * w;
    const e = t.energy || (0.3 + 0.5 * Math.sin(i * 1.2) + i * 0.05);
    const y = h - Math.max(0.1, Math.min(1, e)) * (h - 8) - 4;
    return { x, y };
  });
  const path = points.map((p, i) => (i === 0 ? 'M' : 'L') + ` ${p.x} ${p.y}`).join(' ');
  return (
    <svg width={w} height={h + 8} style={{ marginTop: 8 }}>
      <defs>
        <linearGradient id="enGrad" x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" stopColor="var(--c-accent)"/>
          <stop offset="1" stopColor="var(--c-deck-b)"/>
        </linearGradient>
      </defs>
      <path d={path + ` L ${w} ${h + 4} L 0 ${h + 4} Z`} fill="url(#enGrad)" opacity="0.15"/>
      <path d={path} stroke="url(#enGrad)" strokeWidth="1.5" fill="none"/>
      {points.map((p, i) => (
        <circle key={i} cx={p.x} cy={p.y} r={3} fill="var(--c-deck-b)"/>
      ))}
    </svg>
  );
}

Object.assign(window, { SetlistOverview });
