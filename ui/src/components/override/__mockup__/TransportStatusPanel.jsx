// TransportStatusPanel.jsx — global template state panel (goes into TopBar / Transport strip)
function TransportStatusPanel({ state, onAbort }) {
  if (!state || !state.running) return null;
  const [confirmAbort, setConfirmAbort] = React.useState(false);
  const { templateName, barsDone, barsTotal, overriddenCount = 0 } = state;
  const progress = barsDone / barsTotal;

  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 12,
      padding: '6px 12px',
      background: 'linear-gradient(180deg, var(--c-glass-3), var(--c-glass-2))',
      border: '1px solid rgba(79,227,178,0.25)',
      borderRadius: 8,
      boxShadow: 'inset 0 1px 0 rgba(255,255,255,0.05)',
      minWidth: 280,
    }}>
      <div style={{ position: 'relative', width: 8, height: 8 }}>
        <div style={{ position: 'absolute', inset: 0, borderRadius: '50%', background: 'var(--c-accent)', boxShadow: '0 0 10px var(--c-accent-glow)' }}/>
        <div style={{ position: 'absolute', inset: -4, borderRadius: '50%', border: '1px solid var(--c-accent)', opacity: 0.4, animation: 'tsPulse 1.2s var(--ease-out) infinite' }}/>
      </div>

      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <span className="overline" style={{ fontSize: 8, color: 'var(--c-accent)' }}>TEMPLATE</span>
          <span style={{ fontSize: 11, fontWeight: 700, color: 'var(--fg-1)', letterSpacing: 'var(--tracking-tight)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
            {templateName}
          </span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 2 }}>
          <div style={{ flex: 1, height: 3, background: 'var(--c-ink-4)', borderRadius: 2, overflow: 'hidden' }}>
            <div style={{ width: `${progress * 100}%`, height: '100%', background: 'linear-gradient(90deg, var(--c-accent), var(--c-accent-hi))' }}/>
          </div>
          <span className="tabular" style={{ fontSize: 10, color: 'var(--fg-3)', minWidth: 48, textAlign: 'right' }}>
            {barsDone}/{barsTotal} bars
          </span>
        </div>
      </div>

      {overriddenCount > 0 && (
        <div style={{
          display: 'flex', alignItems: 'center', gap: 5,
          padding: '3px 8px', borderRadius: 4,
          background: 'rgba(232,145,90,0.12)', border: '1px solid rgba(232,145,90,0.3)',
        }}>
          <i data-lucide="hand" style={{ width: 11, height: 11, color: '#E8915A' }}/>
          <span className="tabular" style={{ fontSize: 10, fontWeight: 700, color: '#E8915A' }}>
            {overriddenCount}
          </span>
          <span style={{ fontSize: 9, color: '#E8915A', letterSpacing: 'var(--tracking-wide)', textTransform: 'uppercase' }}>
            ovr
          </span>
        </div>
      )}

      <button onClick={() => setConfirmAbort(true)}
        style={{
          padding: '4px 9px', borderRadius: 5,
          background: 'rgba(255,74,92,0.08)', border: '1px solid rgba(255,74,92,0.3)',
          color: 'var(--c-danger)', fontFamily: 'var(--font-sans)', fontWeight: 700, fontSize: 9,
          letterSpacing: 'var(--tracking-wide)', textTransform: 'uppercase', cursor: 'pointer',
          display: 'flex', alignItems: 'center', gap: 4,
        }}>
        <i data-lucide="x-octagon" style={{ width: 10, height: 10 }}/> ABORT
      </button>

      {confirmAbort && (
        <ConfirmDialog
          title="テンプレート中断 / Abort template?"
          body={`"${templateName}" を中断します。現在の状態で固定されます。`}
          onCancel={() => setConfirmAbort(false)}
          onConfirm={() => { setConfirmAbort(false); onAbort && onAbort(); }}
        />
      )}

      <style>{`@keyframes tsPulse { 0%,100% { transform: scale(1); opacity: 0.4 } 50% { transform: scale(1.4); opacity: 0 } }`}</style>
    </div>
  );
}

function ConfirmDialog({ title, body, onCancel, onConfirm }) {
  return (
    <div style={{
      position: 'fixed', inset: 0, zIndex: 1000,
      background: 'rgba(7,8,10,0.65)', backdropFilter: 'blur(4px)',
      display: 'flex', alignItems: 'center', justifyContent: 'center',
    }}>
      <div style={{
        width: 360, padding: 20,
        background: 'var(--c-ink-2)', border: '1px solid var(--c-glass-stroke-strong)',
        borderRadius: 14, boxShadow: 'var(--shadow-4)',
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 10 }}>
          <div style={{ width: 32, height: 32, borderRadius: 8, background: 'rgba(255,74,92,0.12)', border: '1px solid rgba(255,74,92,0.3)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
            <i data-lucide="alert-triangle" style={{ width: 16, height: 16, color: 'var(--c-danger)' }}/>
          </div>
          <div style={{ fontSize: 14, fontWeight: 700, color: 'var(--fg-1)', letterSpacing: 'var(--tracking-tight)' }}>
            {title}
          </div>
        </div>
        <div style={{ fontSize: 12, color: 'var(--fg-3)', lineHeight: 1.5 }}>{body}</div>
        <div style={{ display: 'flex', gap: 8, marginTop: 16 }}>
          <button onClick={onCancel} className="btn-ghost" style={{ flex: 1 }}>キャンセル / Cancel</button>
          <button onClick={onConfirm}
            style={{
              flex: 1, height: 34, border: 'none', borderRadius: 8,
              background: 'linear-gradient(180deg, #FF4A5C, #C73340)',
              color: 'white', fontFamily: 'var(--font-sans)', fontWeight: 700, fontSize: 11,
              letterSpacing: 'var(--tracking-wide)', textTransform: 'uppercase', cursor: 'pointer',
              boxShadow: '0 4px 12px rgba(255,74,92,0.35), inset 0 1px 0 rgba(255,255,255,0.15)',
            }}>
            中断 / Abort
          </button>
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { TransportStatusPanel });
