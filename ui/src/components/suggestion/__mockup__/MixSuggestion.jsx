// MixSuggestion.jsx — The signature "繋ぎ候補" floating HUD
function MixSuggestion({ suggestions, onPick, onDismiss }) {
  return (
    <div style={{
      position: 'absolute', right: 20, top: 80, width: 320, zIndex: 20,
      background: 'rgba(23, 28, 36, 0.72)',
      backdropFilter: 'blur(20px) saturate(160%)',
      WebkitBackdropFilter: 'blur(20px) saturate(160%)',
      border: '1px solid rgba(255,255,255,0.12)',
      borderRadius: 14, padding: 14,
      boxShadow: '0 24px 64px rgba(0,0,0,0.6), inset 0 1px 0 rgba(255,255,255,0.1)',
    }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 10 }}>
        <div className="overline" style={{ color: 'var(--c-accent)' }}>NEXT CUE · 繋ぎ候補</div>
        <button onClick={onDismiss} style={{ background: 'none', border: 'none', color: 'var(--fg-4)', cursor: 'pointer', padding: 2 }}>
          <i data-lucide="x" style={{ width: 14, height: 14 }} />
        </button>
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        {suggestions.map((s, i) => (
          <button key={i} onClick={() => onPick && onPick(s)}
                  className="mix-sugg" style={{
                    display: 'flex', justifyContent: 'space-between', alignItems: 'center',
                    padding: '10px 12px',
                    background: i === 0 ? 'rgba(0,245,160,0.1)' : 'rgba(255,255,255,0.03)',
                    border: `1px solid ${i === 0 ? 'rgba(0,245,160,0.3)' : 'rgba(255,255,255,0.08)'}`,
                    borderRadius: 8, cursor: 'pointer', textAlign: 'left',
                    fontFamily: 'var(--font-sans)',
                  }}>
            <div>
              <div style={{ fontWeight: 700, fontSize: 13, color: i === 0 ? 'var(--fg-1)' : 'var(--fg-2)' }}>{s.name}</div>
              <div style={{ fontSize: 11, color: 'var(--fg-4)', marginTop: 2 }}>{s.detail}</div>
            </div>
            <div className="tabular" style={{ fontSize: 12, color: i === 0 ? 'var(--c-accent)' : 'var(--fg-3)' }}>{s.match}%</div>
          </button>
        ))}
      </div>
      <div style={{ marginTop: 10, paddingTop: 10, borderTop: '1px solid rgba(255,255,255,0.06)', fontSize: 10, color: 'var(--fg-5)', display: 'flex', justifyContent: 'space-between' }}>
        <span>Trigger: <span style={{ fontFamily: 'var(--font-mono)', color: 'var(--fg-3)' }}>⏎ Enter</span></span>
        <span>Dismiss: <span style={{ fontFamily: 'var(--font-mono)', color: 'var(--fg-3)' }}>⎋</span></span>
      </div>
    </div>
  );
}

Object.assign(window, { MixSuggestion });
