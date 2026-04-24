// CueListItem.jsx — expandable cue row with full metadata
const CUE_TYPES = [
  { id: 'intro',     jp: 'イントロ',     en: 'INTRO',     color: 'var(--c-accent)',  icon: 'log-in' },
  { id: 'breakdown', jp: 'ブレイク',     en: 'BREAKDOWN', color: 'var(--c-deck-b)',  icon: 'activity' },
  { id: 'drop',      jp: 'ドロップ',     en: 'DROP',      color: 'var(--c-danger)',  icon: 'flame' },
  { id: 'outro',     jp: 'アウトロ',     en: 'OUTRO',     color: 'var(--c-info)',    icon: 'log-out' },
  { id: 'custom',    jp: 'カスタム',     en: 'CUSTOM',    color: 'var(--c-cue)',     icon: 'bookmark' },
];

function CueListItem({ cue, selected, onSelect, onChange, onDelete }) {
  const [open, setOpen] = React.useState(selected);
  React.useEffect(() => { if (selected) setOpen(true); }, [selected]);

  const type = CUE_TYPES.find(t => t.id === cue.type) || CUE_TYPES[4];

  return (
    <div style={{
      background: selected ? 'var(--c-glass-3)' : 'transparent',
      border: `1px solid ${selected ? 'rgba(79,227,178,0.22)' : 'var(--stroke-1)'}`,
      borderRadius: 8, marginBottom: 4, overflow: 'hidden',
      transition: 'background var(--dur-2) var(--ease-out)',
    }}>
      {/* Summary row */}
      <div onClick={onSelect}
        style={{
          display: 'flex', alignItems: 'center', gap: 10,
          padding: '9px 10px', cursor: 'pointer',
        }}>
        <div style={{ width: 6, height: 22, borderRadius: 2, background: type.color, flexShrink: 0 }}/>
        <i data-lucide={type.icon} style={{ width: 13, height: 13, color: type.color, flexShrink: 0 }}/>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: 12, fontWeight: 700, color: selected ? 'var(--fg-1)' : 'var(--fg-2)', letterSpacing: 'var(--tracking-tight)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
            {cue.name}
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 2 }}>
            <span className="tabular" style={{ fontSize: 10, color: 'var(--fg-4)' }}>{cue.bar}:{cue.beat}</span>
            <span style={{ fontSize: 9, fontWeight: 700, letterSpacing: 'var(--tracking-wide)', color: type.color, textTransform: 'uppercase' }}>{type.en}</span>
          </div>
        </div>
        <button className="icon-btn" onClick={(e) => { e.stopPropagation(); setOpen(o => !o); }}>
          <i data-lucide={open ? 'chevron-up' : 'chevron-down'} style={{ width: 11, height: 11 }}/>
        </button>
      </div>

      {/* Expanded */}
      {open && (
        <div style={{ padding: '2px 12px 12px', borderTop: '1px solid var(--stroke-1)' }}>
          {/* Type picker */}
          <Field label="タイプ" labelEn="TYPE">
            <select value={cue.type} onChange={e => onChange({ ...cue, type: e.target.value })} style={cueSelectStyle}>
              {CUE_TYPES.map(t => <option key={t.id} value={t.id}>{t.jp} / {t.en}</option>)}
            </select>
          </Field>

          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, marginTop: 8 }}>
            <Field label="BPM @ Cue" labelEn="BPM">
              <div className="tabular" style={{ fontSize: 14, color: 'var(--fg-1)', fontWeight: 700 }}>{cue.bpm?.toFixed(2) || '128.00'}</div>
            </Field>
            <Field label="Key @ Cue" labelEn="KEY">
              <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                <div style={{ padding: '3px 7px', background: 'var(--c-accent-soft)', border: '1px solid rgba(79,227,178,0.25)', borderRadius: 4, color: 'var(--c-accent-hi)', fontFamily: 'var(--font-mono)', fontWeight: 700, fontSize: 12 }}>
                  {cue.key || '8A'}
                </div>
                <span style={{ fontSize: 10, color: 'var(--fg-5)' }}>Camelot</span>
              </div>
            </Field>
          </div>

          {/* Energy */}
          <Field label="エネルギー" labelEn="ENERGY LEVEL">
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <div style={{ flex: 1, height: 6, background: 'var(--c-ink-4)', borderRadius: 3, overflow: 'hidden' }}>
                <div style={{
                  width: `${(cue.energy || 0.6) * 100}%`, height: '100%',
                  background: `linear-gradient(90deg, var(--c-accent), ${cue.energy > 0.75 ? 'var(--c-danger)' : 'var(--c-deck-b)'})`,
                }}/>
              </div>
              <span className="tabular" style={{ fontSize: 11, color: 'var(--fg-2)', fontWeight: 700, minWidth: 32 }}>{(cue.energy || 0.6).toFixed(2)}</span>
            </div>
            <input type="range" min="0" max="1" step="0.01" value={cue.energy || 0.6}
              onChange={e => onChange({ ...cue, energy: parseFloat(e.target.value) })}
              style={{ width: '100%', marginTop: 4, accentColor: 'var(--c-accent)' }}/>
          </Field>

          {/* Phrase */}
          <Field label="フレーズ長" labelEn="PHRASE LENGTH">
            <div style={{ display: 'flex', gap: 3 }}>
              {[16, 32, 64].map(n => (
                <button key={n}
                  onClick={() => onChange({ ...cue, phrase: n })}
                  className="btn-xs"
                  style={{
                    flex: 1, background: (cue.phrase || 32) === n ? 'var(--c-accent-soft)' : 'var(--c-ink-3)',
                    borderColor: (cue.phrase || 32) === n ? 'rgba(79,227,178,0.35)' : 'var(--stroke-1)',
                    color: (cue.phrase || 32) === n ? 'var(--c-accent-hi)' : 'var(--fg-3)',
                  }}>
                  {n} bars
                </button>
              ))}
            </div>
          </Field>

          {/* Mixable as */}
          <Field label="使い方" labelEn="MIXABLE AS">
            <div style={{ display: 'flex', gap: 3 }}>
              {['entry','exit','both'].map(m => {
                const active = (cue.mixable || 'both') === m;
                return (
                  <button key={m} onClick={() => onChange({ ...cue, mixable: m })}
                    className="btn-xs" style={{
                      flex: 1, textTransform: 'uppercase',
                      background: active ? 'var(--c-accent-soft)' : 'var(--c-ink-3)',
                      borderColor: active ? 'rgba(79,227,178,0.35)' : 'var(--stroke-1)',
                      color: active ? 'var(--c-accent-hi)' : 'var(--fg-3)',
                    }}>
                    {m === 'entry' ? 'Entry / 入り' : m === 'exit' ? 'Exit / 抜け' : 'Both / 両方'}
                  </button>
                );
              })}
            </div>
          </Field>

          {/* Compatible energy range */}
          <Field label="適合エネルギー帯" labelEn="COMPATIBLE RANGE">
            <EnergyRange value={cue.energyRange || [0.3, 0.8]} onChange={r => onChange({ ...cue, energyRange: r })}/>
          </Field>

          {/* Actions */}
          <div style={{ display: 'flex', gap: 6, marginTop: 10 }}>
            <button className="btn-xs" style={{ flex: 1 }}>
              <i data-lucide="play" style={{ width: 10, height: 10 }}/> PREVIEW
            </button>
            <button className="btn-xs danger" onClick={onDelete}>
              <i data-lucide="trash-2" style={{ width: 10, height: 10 }}/>
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

const cueSelectStyle = {
  width: '100%', height: 28, background: 'var(--c-ink-3)',
  border: '1px solid var(--stroke-1)', borderRadius: 5,
  padding: '0 8px', color: 'var(--fg-2)', fontSize: 11, outline: 'none',
  fontFamily: 'var(--font-sans)',
};

function Field({ label, labelEn, children }) {
  return (
    <div style={{ marginTop: 8 }}>
      <div className="overline" style={{ fontSize: 8, marginBottom: 4 }}>{label} / {labelEn}</div>
      {children}
    </div>
  );
}

function EnergyRange({ value, onChange }) {
  const [lo, hi] = value;
  return (
    <div style={{ position: 'relative', height: 30 }}>
      <div style={{ position: 'absolute', left: 0, right: 0, top: '50%', height: 4, marginTop: -2, background: 'var(--c-ink-4)', borderRadius: 2 }}/>
      <div style={{
        position: 'absolute', left: `${lo * 100}%`, right: `${(1 - hi) * 100}%`,
        top: '50%', height: 4, marginTop: -2,
        background: 'linear-gradient(90deg, var(--c-accent), var(--c-deck-b))',
        borderRadius: 2,
      }}/>
      <input type="range" min="0" max="1" step="0.01" value={lo}
        onChange={e => onChange([Math.min(hi - 0.05, parseFloat(e.target.value)), hi])}
        style={{ position: 'absolute', inset: 0, width: '100%', accentColor: 'var(--c-accent)', background: 'transparent' }}/>
      <input type="range" min="0" max="1" step="0.01" value={hi}
        onChange={e => onChange([lo, Math.max(lo + 0.05, parseFloat(e.target.value))])}
        style={{ position: 'absolute', inset: 0, width: '100%', accentColor: 'var(--c-accent)', background: 'transparent', pointerEvents: 'none' }}/>
      <div style={{ position: 'absolute', right: 0, bottom: -14, fontFamily: 'var(--font-mono)', fontSize: 10, color: 'var(--fg-4)' }}>
        {lo.toFixed(2)} – {hi.toFixed(2)}
      </div>
    </div>
  );
}

Object.assign(window, { CueListItem, CUE_TYPES });
