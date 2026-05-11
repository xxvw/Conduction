// TopBar.jsx — Window title bar + toolbar with master transport info
function TopBar({ masterBpm, recording, onToggleRec }) {
  return (
    <div style={{
      height: 52, background: 'var(--c-ink-1)',
      borderBottom: '1px solid var(--stroke-1)',
      display: 'flex', alignItems: 'center', padding: '0 16px', gap: 16,
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <div className="overline" style={{ fontSize: 10 }}>MASTER</div>
        <div className="tabular" style={{ fontSize: 20, fontWeight: 700, color: 'var(--c-accent)' }}>{masterBpm.toFixed(1)}</div>
        <span className="overline" style={{ fontSize: 10 }}>BPM</span>
      </div>
      <div style={{ width: 1, height: 24, background: 'var(--stroke-1)' }} />
      <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
        <button className="chip">TAP</button>
        <button className="chip active">QUANTIZE</button>
        <button className="chip">KEY LOCK</button>
      </div>

      <div style={{ flex: 1 }} />

      <div style={{ position: 'relative', width: 300 }}>
        <i data-lucide="search" style={{ position: 'absolute', left: 10, top: '50%', transform: 'translateY(-50%)', width: 14, height: 14, color: 'var(--fg-5)' }} />
        <input placeholder="Search library · ライブラリを検索" style={{
          width: '100%', boxSizing: 'border-box',
          background: 'var(--c-ink-2)', border: '1px solid var(--stroke-1)',
          borderRadius: 8, padding: '7px 10px 7px 30px', color: 'var(--fg-2)',
          fontSize: 12, fontFamily: 'var(--font-sans)', outline: 'none',
        }} />
      </div>

      <button onClick={onToggleRec} className="chip" style={{
        color: recording ? '#FF2D55' : 'var(--fg-3)',
        borderColor: recording ? 'rgba(255,45,85,0.4)' : 'var(--stroke-1)',
        background: recording ? 'rgba(255,45,85,0.08)' : undefined,
        boxShadow: recording ? '0 0 14px rgba(255,45,85,0.3)' : undefined,
      }}>
        <span style={{ width: 8, height: 8, borderRadius: '50%', background: recording ? '#FF2D55' : 'var(--fg-5)', display: 'inline-block', marginRight: 6 }} />
        REC
      </button>

      <button className="chip"><i data-lucide="headphones" style={{ width: 14, height: 14 }} /></button>
      <button className="chip"><i data-lucide="settings" style={{ width: 14, height: 14 }} /></button>
    </div>
  );
}

Object.assign(window, { TopBar });
