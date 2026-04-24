// SetlistTrackBlock.jsx — one track row in the setlist center timeline
function SetlistTrackBlock({ track, index, isPlaying, onRemove, onChangeCue, onSelect, selected }) {
  return (
    <div onClick={onSelect}
      style={{
        position: 'relative',
        display: 'flex', gap: 12, alignItems: 'stretch',
        padding: 12,
        background: isPlaying
          ? 'linear-gradient(90deg, rgba(79,227,178,0.15), rgba(79,227,178,0.04))'
          : selected ? 'var(--c-glass-3)' : 'linear-gradient(180deg, var(--c-ink-3), var(--c-ink-2))',
        border: `1px solid ${isPlaying ? 'rgba(79,227,178,0.4)' : selected ? 'rgba(79,227,178,0.22)' : 'var(--stroke-1)'}`,
        borderRadius: 10,
        boxShadow: isPlaying ? '0 0 14px var(--c-accent-glow), inset 0 1px 0 rgba(255,255,255,0.05)' : 'var(--shadow-1), inset 0 1px 0 rgba(255,255,255,0.03)',
        cursor: 'pointer',
      }}>

      {/* Drag handle */}
      <div style={{ display: 'flex', alignItems: 'center', color: 'var(--fg-6)', cursor: 'grab' }}>
        <i data-lucide="grip-vertical" style={{ width: 14, height: 14 }}/>
      </div>

      {/* Order badge */}
      <div style={{
        width: 36, height: 36, flexShrink: 0, borderRadius: 8,
        background: isPlaying ? 'var(--c-accent)' : 'var(--c-ink-4)',
        color: isPlaying ? 'var(--c-ink-0)' : 'var(--fg-2)',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        fontFamily: 'var(--font-mono)', fontWeight: 700, fontSize: 14,
        border: '1px solid var(--stroke-1)',
        boxShadow: isPlaying ? '0 0 10px var(--c-accent-glow)' : 'inset 0 1px 0 rgba(255,255,255,0.05)',
      }}>
        {String(index + 1).padStart(2, '0')}
      </div>

      {/* Artwork placeholder */}
      <div style={{
        width: 56, height: 56, flexShrink: 0, borderRadius: 8,
        background: `linear-gradient(135deg, ${track.hue1 || '#4FE3B2'}, ${track.hue2 || '#8A9BE8'})`,
        border: '1px solid var(--stroke-1)',
        boxShadow: 'inset 0 1px 0 rgba(255,255,255,0.15), 0 2px 4px rgba(0,0,0,0.4)',
        position: 'relative', overflow: 'hidden',
      }}>
        <div style={{ position: 'absolute', inset: 0, background: 'radial-gradient(circle at 30% 30%, rgba(255,255,255,0.2), transparent 60%)' }}/>
      </div>

      {/* Info */}
      <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', justifyContent: 'center', gap: 4 }}>
        <div style={{ fontSize: 14, fontWeight: 700, color: 'var(--fg-1)', letterSpacing: 'var(--tracking-tight)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {track.title}
        </div>
        <div style={{ fontSize: 11, color: 'var(--fg-4)' }}>{track.artist}</div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 2 }}>
          <span className="tabular" style={{ fontSize: 11, fontWeight: 700, color: 'var(--fg-2)' }}>{track.bpm.toFixed(2)}</span>
          <span style={{ fontSize: 9, color: 'var(--fg-5)', letterSpacing: 'var(--tracking-wide)', fontWeight: 700 }}>BPM</span>
          <div style={{ padding: '2px 6px', background: 'var(--c-accent-soft)', border: '1px solid rgba(79,227,178,0.25)', borderRadius: 3, color: 'var(--c-accent-hi)', fontFamily: 'var(--font-mono)', fontWeight: 700, fontSize: 10 }}>
            {track.key}
          </div>
          <span className="tabular" style={{ fontSize: 10, color: 'var(--fg-5)' }}>
            {Math.floor(track.duration / 60)}:{String(track.duration % 60).padStart(2, '0')}
          </span>
        </div>
      </div>

      {/* Cue selectors */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4, minWidth: 170 }}>
        <CueSelect label="Play from" labelEn="FROM" value={track.fromCue || 'intro-16'} onChange={v => onChangeCue && onChangeCue('fromCue', v)} color="var(--c-accent)"/>
        <CueSelect label="Play until" labelEn="UNTIL" value={track.toCue || 'outro-32'} onChange={v => onChangeCue && onChangeCue('toCue', v)} color="var(--c-deck-b)"/>
      </div>

      {/* Remove */}
      <button className="icon-btn" onClick={(e) => { e.stopPropagation(); onRemove && onRemove(); }}
        style={{ alignSelf: 'flex-start' }}>
        <i data-lucide="x" style={{ width: 12, height: 12 }}/>
      </button>

      {/* Bottom amber connector */}
      <div style={{
        position: 'absolute', left: '50%', bottom: -4,
        width: 14, height: 8, marginLeft: -7,
        background: 'var(--c-deck-b)', borderRadius: '0 0 7px 7px',
        boxShadow: '0 0 6px rgba(232,145,90,0.4)', opacity: 0.85,
      }}/>
    </div>
  );
}

function CueSelect({ label, labelEn, value, onChange, color }) {
  return (
    <div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 5 }}>
        <div style={{ width: 5, height: 5, borderRadius: '50%', background: color }}/>
        <span className="overline" style={{ fontSize: 8 }}>{label} / {labelEn}</span>
      </div>
      <select value={value} onChange={e => onChange(e.target.value)}
        style={{
          width: '100%', marginTop: 3, height: 26,
          background: 'var(--c-ink-3)', border: '1px solid var(--stroke-1)',
          borderRadius: 5, padding: '0 6px', color: 'var(--fg-1)',
          fontFamily: 'var(--font-mono)', fontSize: 10, outline: 'none',
        }}>
        <option value="auto">Auto</option>
        <option value="intro-16">Intro · 16</option>
        <option value="intro-32">Intro · 32</option>
        <option value="breakdown-32">Breakdown · 32</option>
        <option value="drop">Drop</option>
        <option value="outro-32">Outro · 32</option>
      </select>
    </div>
  );
}

Object.assign(window, { SetlistTrackBlock });
