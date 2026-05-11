// OverrideIndicator.jsx — Status band for Knob/Fader parameters
//   states: 'automated' (blue), 'overridden' (amber), 'committed' (gray), 'idle' (transparent)

function OverrideIndicator({ state = 'idle', orientation = 'top' }) {
  const COLOR = {
    automated:  { bar: '#8A9BE8', glow: 'rgba(138,155,232,0.45)', label: 'AUTO',   jp: '自動' },
    overridden: { bar: '#E8915A', glow: 'rgba(232,145,90,0.55)',  label: 'OVR',    jp: '手動' },
    committed:  { bar: 'var(--c-ink-6)', glow: 'transparent',     label: 'FIXED',  jp: '確定' },
    idle:       { bar: 'transparent', glow: 'transparent',        label: '',       jp: '' },
  }[state];
  if (state === 'idle') return null;
  const horizontal = orientation === 'top' || orientation === 'bottom';
  return (
    <div style={{
      position: 'absolute',
      top: orientation === 'top' ? 0 : orientation === 'bottom' ? 'auto' : 0,
      bottom: orientation === 'bottom' ? 0 : 'auto',
      left: orientation === 'left' ? 0 : 0,
      right: orientation === 'right' ? 0 : 0,
      width: orientation === 'left' || orientation === 'right' ? 3 : '100%',
      height: horizontal ? 3 : '100%',
      background: COLOR.bar,
      boxShadow: state === 'overridden' ? `0 0 6px ${COLOR.glow}` : 'none',
      borderRadius: 1.5,
      pointerEvents: 'none',
      zIndex: 2,
    }}/>
  );
}

function OverrideBadge({ state }) {
  if (state === 'idle') return null;
  const c = state === 'automated' ? '#8A9BE8' : state === 'overridden' ? '#E8915A' : 'var(--c-ink-6)';
  const label = state === 'automated' ? 'AUTO' : state === 'overridden' ? 'OVR' : 'FIXED';
  return (
    <div style={{
      display: 'inline-flex', alignItems: 'center', gap: 3,
      padding: '1px 5px', borderRadius: 3,
      background: `${c}18`, border: `1px solid ${c}55`,
      fontFamily: 'var(--font-mono)', fontSize: 9, fontWeight: 500,
      color: c, letterSpacing: 'var(--tracking-wide)', textTransform: 'uppercase',
    }}>
      {label}
    </div>
  );
}

Object.assign(window, { OverrideIndicator, OverrideBadge });
