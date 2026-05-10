// OverrideControls.jsx — OVR button + Resume/Commit inline controls
//   Use: wrap any automated parameter control. Shows OVR on hover (idle/automated),
//        Resume+Commit while overridden.

function OverrideControls({ state, onOverride, onResume, onCommit, compact = false }) {
  const [hover, setHover] = React.useState(false);

  if (state === 'overridden') {
    return (
      <div style={{ display: 'flex', gap: 4, alignItems: 'center' }}>
        <button onClick={onResume}
          style={ovrBtnStyle('resume', compact)}>
          <i data-lucide="rotate-ccw" style={{ width: 10, height: 10 }}/> RESUME
        </button>
        <button onClick={onCommit}
          style={ovrBtnStyle('commit', compact)}>
          <i data-lucide="check" style={{ width: 10, height: 10 }}/> COMMIT
        </button>
      </div>
    );
  }

  if (state === 'automated') {
    return (
      <button onClick={onOverride}
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        style={{
          padding: compact ? '2px 5px' : '3px 7px',
          borderRadius: 4,
          background: hover ? 'rgba(232,145,90,0.15)' : 'rgba(255,255,255,0.03)',
          border: `1px solid ${hover ? 'rgba(232,145,90,0.45)' : 'rgba(255,255,255,0.08)'}`,
          color: hover ? '#E8915A' : 'var(--fg-5)',
          fontFamily: 'var(--font-mono)', fontSize: 9, fontWeight: 500,
          letterSpacing: 'var(--tracking-wide)', textTransform: 'uppercase',
          cursor: 'pointer',
          opacity: hover ? 1 : 0.5,
          transition: 'all var(--dur-2) var(--ease-out)',
        }}>
        OVR
      </button>
    );
  }

  return null;
}

function ovrBtnStyle(kind, compact) {
  const isResume = kind === 'resume';
  return {
    padding: compact ? '2px 6px' : '3px 8px', borderRadius: 4,
    background: isResume ? 'var(--c-accent-soft)' : 'rgba(255,255,255,0.05)',
    border: `1px solid ${isResume ? 'rgba(79,227,178,0.4)' : 'rgba(255,255,255,0.12)'}`,
    color: isResume ? 'var(--c-accent-hi)' : 'var(--fg-2)',
    fontFamily: 'var(--font-mono)', fontSize: 9, fontWeight: 500,
    letterSpacing: 'var(--tracking-wide)', textTransform: 'uppercase',
    cursor: 'pointer', display: 'inline-flex', alignItems: 'center', gap: 4,
  };
}

// Wrapped parameter — drop-in wrapper for any knob/fader that wants override UX
function OverridableParam({ state, setState, children, label, showKeyHint = false }) {
  const [fadeKey, setFadeKey] = React.useState(0);
  const ref = React.useRef(null);

  const onOverride = () => setState('overridden');
  const onResume = () => setState('automated');
  const onCommit = () => { setState('committed'); setFadeKey(k => k + 1); };

  // keyboard: O/R/C when focused
  React.useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const h = (e) => {
      if (document.activeElement !== el) return;
      if (e.key === 'o' || e.key === 'O') { e.preventDefault(); if (state === 'automated') onOverride(); }
      if (e.key === 'r' || e.key === 'R') { e.preventDefault(); if (state === 'overridden') onResume(); }
      if (e.key === 'c' || e.key === 'C') { e.preventDefault(); if (state === 'overridden') onCommit(); }
    };
    window.addEventListener('keydown', h);
    return () => window.removeEventListener('keydown', h);
  }, [state]);

  return (
    <div ref={ref} tabIndex={0}
      style={{
        position: 'relative', padding: 8, borderRadius: 8,
        outline: 'none',
        boxShadow: state === 'overridden' ? 'inset 0 0 0 1px rgba(232,145,90,0.45), 0 0 12px rgba(232,145,90,0.15)' : 'none',
        transition: 'box-shadow var(--dur-3) var(--ease-out)',
      }}>
      <OverrideIndicator state={state} orientation="top"/>

      {/* label + badge */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 4 }}>
        <span className="overline" style={{ fontSize: 9 }}>{label}</span>
        <OverrideBadge state={state}/>
      </div>

      {/* Control */}
      <div key={fadeKey} style={{
        animation: state === 'committed' && fadeKey > 0 ? 'ovrCommitFade 200ms var(--ease-out)' : 'none',
      }}>
        {children}
      </div>

      {/* Override controls */}
      <div style={{ marginTop: 6, display: 'flex', justifyContent: 'center', minHeight: 20 }}>
        <OverrideControls state={state} onOverride={onOverride} onResume={onResume} onCommit={onCommit}/>
      </div>

      {showKeyHint && (
        <div style={{ marginTop: 4, fontSize: 8, color: 'var(--fg-6)', textAlign: 'center', letterSpacing: 'var(--tracking-wide)', textTransform: 'uppercase', fontFamily: 'var(--font-mono)' }}>
          [O] override · [R] resume · [C] commit
        </div>
      )}

      <style>{`
        @keyframes ovrCommitFade {
          0% { opacity: 0.3; transform: scale(0.98); }
          100% { opacity: 1; transform: scale(1); }
        }
      `}</style>
    </div>
  );
}

Object.assign(window, { OverrideControls, OverridableParam });
