// Library.jsx — Library panel with integrated Next Cue rail
function Library({ tracks, onLoad, nowPlayingId, suggestions, onPickSugg, showNextCue = true }) {
  const [q, setQ] = React.useState('');
  const [activeChip, setActiveChip] = React.useState('All');
  return (
    <div style={{ display: 'grid', gridTemplateColumns: showNextCue ? '1fr 320px' : '1fr', gap: 14, flex: 1, minHeight: 0 }}>
      {/* Main library */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 10, minHeight: 0 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <h3 style={{ fontSize: 15, fontWeight: 700, margin: 0, letterSpacing: '-0.01em', color: 'var(--fg-1)' }}>Library</h3>
          <span style={{ fontSize: 10, fontWeight: 700, letterSpacing: '0.2em', color: 'var(--fg-5)', textTransform: 'uppercase' }}>{tracks.length} tracks · Deep House</span>
          <div style={{ flex: 1 }} />
          <div style={{ position: 'relative', width: 220 }}>
            <i data-lucide="search" style={{ position: 'absolute', left: 10, top: '50%', transform: 'translateY(-50%)', width: 13, height: 13, color: 'var(--fg-5)' }} />
            <input value={q} onChange={e => setQ(e.target.value)} placeholder="Filter…" style={{
              width: '100%', boxSizing: 'border-box', height: 30,
              background: 'linear-gradient(180deg, var(--c-ink-3), var(--c-ink-4))',
              border: '1px solid var(--stroke-1)', borderRadius: 6, padding: '0 10px 0 28px',
              color: 'var(--fg-2)', fontSize: 12, fontFamily: 'var(--font-sans)', outline: 'none',
              boxShadow: 'inset 0 1px 2px rgba(0,0,0,0.4)',
            }}/>
          </div>
          <div style={{ display: 'flex', gap: 4 }}>
            {['All','Deep House','Techno','128–132'].map(c =>
              <button key={c} className={`chip ${activeChip===c?'active':''}`} onClick={() => setActiveChip(c)}>{c}</button>
            )}
          </div>
        </div>

        <TrackList tracks={tracks} onLoad={onLoad} nowPlayingId={nowPlayingId} />
      </div>

      {/* Right rail: Next Cue */}
      {showNextCue && (<div style={{
        background: 'linear-gradient(180deg, var(--c-ink-3), var(--c-ink-2))',
        border: '1px solid var(--stroke-1)',
        borderRadius: 12, padding: 14,
        boxShadow: 'var(--shadow-2), inset 0 1px 0 rgba(255,255,255,0.05)',
        display: 'flex', flexDirection: 'column', gap: 10, minHeight: 0,
      }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <div style={{ width: 6, height: 6, borderRadius: '50%', background: 'var(--c-accent)', boxShadow: '0 0 8px var(--c-accent-glow)' }}/>
            <div style={{ fontSize: 10, fontWeight: 700, letterSpacing: '0.2em', color: 'var(--c-accent)', textTransform: 'uppercase' }}>Next Cue · 繋ぎ候補</div>
          </div>
          <span style={{ fontSize: 10, color: 'var(--fg-5)', fontFamily: 'var(--font-mono)' }}>{suggestions.length} candidates</span>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 6, minHeight: 0, overflowY: 'auto' }}>
          {suggestions.map((s, i) => (
            <button key={i} onClick={() => onPickSugg && onPickSugg(s)} className="mix-sugg" style={{
              display: 'flex', justifyContent: 'space-between', alignItems: 'center',
              padding: '11px 12px', textAlign: 'left', cursor: 'pointer', fontFamily: 'var(--font-sans)',
              background: i === 0 ? 'linear-gradient(180deg, rgba(79,227,178,0.1), rgba(79,227,178,0.04))' : 'rgba(255,255,255,0.025)',
              border: `1px solid ${i === 0 ? 'rgba(79,227,178,0.28)' : 'rgba(255,255,255,0.06)'}`,
              borderRadius: 10,
              boxShadow: i === 0 ? 'inset 0 1px 0 rgba(255,255,255,0.06)' : 'none',
            }}>
              <div style={{ minWidth: 0 }}>
                <div style={{ fontWeight: 700, fontSize: 13, color: i === 0 ? 'var(--fg-1)' : 'var(--fg-2)', letterSpacing: '-0.005em' }}>{s.name}</div>
                <div style={{ fontSize: 11, color: i === 0 ? 'var(--fg-3)' : 'var(--fg-4)', marginTop: 2 }}>{s.detail}</div>
              </div>
              <div className="tabular" style={{ fontSize: 12, fontWeight: 500, color: i === 0 ? 'var(--c-accent-hi)' : 'var(--fg-4)' }}>{s.match}%</div>
            </button>
          ))}
        </div>

        <div style={{ marginTop: 'auto', paddingTop: 10, borderTop: '1px solid rgba(255,255,255,0.05)', display: 'flex', justifyContent: 'space-between', fontSize: 10, color: 'var(--fg-5)' }}>
          <span>Trigger: <span style={{ fontFamily: 'var(--font-mono)', color: 'var(--fg-3)' }}>⏎</span></span>
          <span>Skip: <span style={{ fontFamily: 'var(--font-mono)', color: 'var(--fg-3)' }}>⇥</span></span>
          <span>Auto: <span style={{ color: 'var(--c-accent)' }}>ON</span></span>
        </div>
      </div>)}
    </div>
  );
}

Object.assign(window, { Library });
