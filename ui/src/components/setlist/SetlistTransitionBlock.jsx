// SetlistTransitionBlock.jsx — transition between two track blocks

function SetlistTransitionBlock({ transition, fromTrack, toTrack, onChangeTemplate, onChangeTempoMode, onEditInTemplates }) {
  const t = transition || { template: 'Long EQ Mix', bars: 32, tempoMode: 'LinearBlend', candidates: [
    { name: 'Long EQ Mix', bars: 32, match: 94 },
    { name: 'Breakdown Swap', bars: 16, match: 82 },
    { name: 'Echo Out', bars: 8, match: 71 },
  ]};

  const TEMPO_MODES = [
    { id: 'HoldSource',  jp: '元を維持',   en: 'HOLD SOURCE' },
    { id: 'MatchTarget', jp: '次に合わせ', en: 'MATCH TARGET' },
    { id: 'LinearBlend', jp: '直線補間',   en: 'LINEAR BLEND' },
    { id: 'MasterTempo', jp: 'マスター',   en: 'MASTER TEMPO' },
  ];

  const avgBpm = fromTrack && toTrack ? (fromTrack.bpm + toTrack.bpm) / 2 : 128;
  const estSeconds = (t.bars * 4 * 60 / avgBpm).toFixed(1);

  return (
    <div style={{
      position: 'relative',
      margin: '4px 24px', padding: '10px 14px',
      background: 'linear-gradient(180deg, rgba(232,145,90,0.05), rgba(232,145,90,0.01))',
      border: '1px dashed rgba(232,145,90,0.35)',
      borderRadius: 10,
      display: 'flex', gap: 12, alignItems: 'center',
    }}>
      {/* Connector top */}
      <div style={{ position: 'absolute', top: -5, left: '50%', width: 1, height: 8, background: 'var(--c-deck-b)', opacity: 0.5 }}/>
      <div style={{ position: 'absolute', bottom: -5, left: '50%', width: 1, height: 8, background: 'var(--c-deck-b)', opacity: 0.5 }}/>

      <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexShrink: 0 }}>
        <div style={{
          width: 32, height: 32, borderRadius: 8,
          background: 'rgba(232,145,90,0.12)', border: '1px solid rgba(232,145,90,0.3)',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
        }}>
          <i data-lucide="git-commit-horizontal" style={{ width: 14, height: 14, color: 'var(--c-deck-b)' }}/>
        </div>
        <div>
          <div className="overline" style={{ fontSize: 8, color: 'var(--c-deck-b)' }}>遷移 / TRANSITION</div>
          <div style={{ fontSize: 12, fontWeight: 700, color: 'var(--fg-1)', marginTop: 1 }}>{t.template}</div>
        </div>
      </div>

      {/* Candidates */}
      <div style={{ display: 'flex', gap: 5, flex: 1, minWidth: 0, flexWrap: 'wrap' }}>
        {t.candidates.map((c, i) => {
          const active = c.name === t.template;
          return (
            <button key={c.name} onClick={() => onChangeTemplate && onChangeTemplate(c.name, c.bars)}
              style={{
                padding: '4px 10px', borderRadius: 5,
                background: active ? 'rgba(232,145,90,0.18)' : 'var(--c-ink-3)',
                border: `1px solid ${active ? 'rgba(232,145,90,0.45)' : 'var(--stroke-1)'}`,
                color: active ? '#E8915A' : 'var(--fg-3)',
                fontFamily: 'var(--font-sans)', fontWeight: 700, fontSize: 10,
                cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 5,
              }}>
              <span>{c.name}</span>
              <span className="tabular" style={{ fontSize: 9, color: active ? '#E8915A' : 'var(--fg-5)' }}>{c.match}%</span>
            </button>
          );
        })}
        <button className="btn-xs" onClick={onEditInTemplates}>
          <i data-lucide="settings-2" style={{ width: 10, height: 10 }}/> カスタム
        </button>
      </div>

      {/* Tempo mode */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
        <span className="overline" style={{ fontSize: 8 }}>テンポ / TEMPO</span>
        <select value={t.tempoMode} onChange={e => onChangeTempoMode && onChangeTempoMode(e.target.value)}
          style={{
            height: 24, background: 'var(--c-ink-3)', border: '1px solid var(--stroke-1)',
            borderRadius: 5, padding: '0 6px', color: 'var(--fg-1)',
            fontFamily: 'var(--font-sans)', fontSize: 10, outline: 'none', minWidth: 130,
          }}>
          {TEMPO_MODES.map(m => <option key={m.id} value={m.id}>{m.jp} / {m.en}</option>)}
        </select>
      </div>

      {/* Time estimate */}
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', flexShrink: 0 }}>
        <span className="overline" style={{ fontSize: 8 }}>尺 / LENGTH</span>
        <span className="tabular" style={{ fontSize: 13, fontWeight: 700, color: 'var(--fg-1)', marginTop: 1 }}>
          {t.bars} <span style={{ fontSize: 9, color: 'var(--fg-5)', fontWeight: 400 }}>bars</span>
        </span>
        <span className="tabular" style={{ fontSize: 9, color: 'var(--fg-5)', marginTop: 1 }}>
          ≈ {estSeconds}s @ {avgBpm.toFixed(0)}
        </span>
      </div>
    </div>
  );
}

Object.assign(window, { SetlistTransitionBlock });
