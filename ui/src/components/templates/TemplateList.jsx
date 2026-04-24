// TemplateList.jsx — 左カラム: テンプレート一覧 + フィルター + 新規/複製/削除
function TemplateList({ templates, activeId, onPick, kind, onKind, onNew, onDup, onDel, onImport, onExport }) {
  const [q, setQ] = React.useState('');
  const filtered = templates.filter(t => t.kind === kind && (q === '' || t.name.toLowerCase().includes(q.toLowerCase())));

  return (
    <aside style={{
      width: 280, flexShrink: 0,
      background: 'linear-gradient(180deg, var(--c-ink-2), var(--c-ink-1))',
      borderRight: '1px solid var(--stroke-1)',
      display: 'flex', flexDirection: 'column', minHeight: 0,
    }}>
      {/* Header */}
      <div style={{ padding: 'var(--s-5) var(--s-5) var(--s-4)', borderBottom: '1px solid var(--stroke-1)' }}>
        <div style={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', marginBottom: 'var(--s-4)' }}>
          <div>
            <div style={{ fontFamily: 'var(--font-display)', fontWeight: 700, fontSize: 16, letterSpacing: 'var(--tracking-tight)', color: 'var(--fg-1)' }}>
              テンプレート
            </div>
            <div className="overline" style={{ fontSize: 9, marginTop: 2 }}>TEMPLATES</div>
          </div>
          <div style={{ display: 'flex', gap: 4 }}>
            <button className="icon-btn" title="インポート" onClick={onImport}>
              <i data-lucide="download" style={{ width: 13, height: 13 }}/>
            </button>
            <button className="icon-btn" title="エクスポート" onClick={onExport}>
              <i data-lucide="upload" style={{ width: 13, height: 13 }}/>
            </button>
          </div>
        </div>

        {/* Kind tabs */}
        <div style={{ display: 'flex', gap: 0, background: 'var(--c-ink-3)', border: '1px solid var(--stroke-1)', borderRadius: 8, padding: 3 }}>
          {[['transition','繋ぎ','TRANSITION'],['setlist','セット','SETLIST']].map(([id, jp, en]) => (
            <button key={id}
              onClick={() => onKind(id)}
              style={{
                flex: 1, padding: '7px 10px', border: 'none', borderRadius: 5,
                background: kind === id ? 'linear-gradient(180deg, var(--c-ink-4), var(--c-ink-3))' : 'transparent',
                boxShadow: kind === id ? 'inset 0 1px 0 rgba(255,255,255,0.06), 0 1px 2px rgba(0,0,0,0.4)' : 'none',
                color: kind === id ? 'var(--fg-1)' : 'var(--fg-4)',
                fontFamily: 'var(--font-sans)', fontWeight: 700, fontSize: 11,
                cursor: 'pointer', display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 1,
                transition: 'all var(--dur-2) var(--ease-out)',
              }}>
              <span>{jp}</span>
              <span style={{ fontSize: 8, letterSpacing: 'var(--tracking-ultrawide)', color: kind === id ? 'var(--c-accent)' : 'var(--fg-5)' }}>{en}</span>
            </button>
          ))}
        </div>

        {/* Search */}
        <div style={{ position: 'relative', marginTop: 'var(--s-4)' }}>
          <i data-lucide="search" style={{ position: 'absolute', left: 10, top: '50%', transform: 'translateY(-50%)', width: 12, height: 12, color: 'var(--fg-5)' }}/>
          <input value={q} onChange={e => setQ(e.target.value)} placeholder="フィルター / Filter"
            style={{
              width: '100%', boxSizing: 'border-box', height: 30,
              background: 'var(--c-ink-3)',
              border: '1px solid var(--stroke-1)', borderRadius: 6, padding: '0 10px 0 28px',
              color: 'var(--fg-2)', fontSize: 12, fontFamily: 'var(--font-sans)', outline: 'none',
              boxShadow: 'inset 0 1px 2px rgba(0,0,0,0.4)',
            }}/>
        </div>
      </div>

      {/* List */}
      <div style={{ flex: 1, overflowY: 'auto', padding: 'var(--s-3) var(--s-3)' }}>
        {/* Presets */}
        <div style={{ padding: '4px 10px 4px' }}>
          <div className="overline" style={{ fontSize: 9, color: 'var(--fg-5)' }}>プリセット / PRESETS</div>
        </div>
        {filtered.filter(t => t.preset).map(t => (
          <TemplateRow key={t.id} t={t} active={activeId === t.id} onPick={() => onPick(t.id)} onDup={() => onDup(t.id)} onDel={() => onDel(t.id)} />
        ))}

        {/* User */}
        <div style={{ padding: '14px 10px 4px' }}>
          <div className="overline" style={{ fontSize: 9, color: 'var(--fg-5)' }}>ユーザー / USER</div>
        </div>
        {filtered.filter(t => !t.preset).length === 0 && (
          <div style={{ padding: '10px 10px', fontSize: 11, color: 'var(--fg-5)' }}>なし / none</div>
        )}
        {filtered.filter(t => !t.preset).map(t => (
          <TemplateRow key={t.id} t={t} active={activeId === t.id} onPick={() => onPick(t.id)} onDup={() => onDup(t.id)} onDel={() => onDel(t.id)} />
        ))}
      </div>

      {/* Bottom: new */}
      <div style={{ padding: 'var(--s-4)', borderTop: '1px solid var(--stroke-1)', background: 'var(--c-ink-2)' }}>
        <button onClick={onNew} style={{
          width: '100%', height: 34, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8,
          background: 'linear-gradient(180deg, var(--c-accent-soft), rgba(79,227,178,0.04))',
          border: '1px solid rgba(79,227,178,0.35)', borderRadius: 8,
          color: 'var(--c-accent-hi)', fontFamily: 'var(--font-sans)', fontSize: 12, fontWeight: 700,
          letterSpacing: 'var(--tracking-wide)', cursor: 'pointer',
          boxShadow: 'inset 0 1px 0 rgba(255,255,255,0.06)',
          transition: 'all var(--dur-2) var(--ease-out)',
        }}>
          <i data-lucide="plus" style={{ width: 14, height: 14 }}/>
          新規作成 / NEW
        </button>
      </div>
    </aside>
  );
}

function TemplateRow({ t, active, onPick, onDup, onDel }) {
  return (
    <div onClick={onPick}
      style={{
        position: 'relative',
        padding: '9px 10px', marginBottom: 2,
        borderRadius: 8, cursor: 'pointer',
        background: active ? 'linear-gradient(180deg, var(--c-glass-3), var(--c-glass-2))' : 'transparent',
        border: `1px solid ${active ? 'rgba(79,227,178,0.22)' : 'transparent'}`,
        boxShadow: active ? 'inset 0 1px 0 rgba(255,255,255,0.04)' : 'none',
        transition: 'background var(--dur-2) var(--ease-out)',
      }}
      onMouseEnter={e => { if (!active) e.currentTarget.style.background = 'var(--bg-hover)'; }}
      onMouseLeave={e => { if (!active) e.currentTarget.style.background = 'transparent'; }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <i data-lucide={t.preset ? (t.kind === 'setlist' ? 'list-ordered' : 'git-commit-horizontal') : 'file'}
           style={{ width: 13, height: 13, color: active ? 'var(--c-accent)' : 'var(--fg-4)', flexShrink: 0 }}/>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: 13, fontWeight: 700, letterSpacing: 'var(--tracking-tight)', color: active ? 'var(--fg-1)' : 'var(--fg-2)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
            {t.name}
          </div>
          {t.nameEn && (
            <div style={{ fontSize: 9, fontWeight: 700, letterSpacing: 'var(--tracking-ultrawide)', color: 'var(--fg-5)', textTransform: 'uppercase', marginTop: 1 }}>
              {t.nameEn}
            </div>
          )}
        </div>
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 6, paddingLeft: 21 }}>
        <span className="tabular" style={{ fontSize: 10, color: active ? 'var(--c-accent)' : 'var(--fg-4)', fontWeight: 500 }}>
          {t.bars} bars
        </span>
        <span style={{ width: 3, height: 3, borderRadius: '50%', background: 'var(--c-ink-6)' }}/>
        {t.tags.map(tag => (
          <span key={tag} style={{
            fontSize: 9, fontWeight: 700, letterSpacing: 'var(--tracking-wide)', textTransform: 'uppercase',
            padding: '2px 6px', borderRadius: 3,
            background: 'var(--c-ink-3)', color: 'var(--fg-4)',
            border: '1px solid var(--stroke-1)',
          }}>{tag}</span>
        ))}
      </div>
      {active && (
        <div style={{ position: 'absolute', right: 6, top: 6, display: 'flex', gap: 2 }}>
          <button className="icon-btn" onClick={(e) => { e.stopPropagation(); onDup(); }} title="複製 / Duplicate">
            <i data-lucide="copy" style={{ width: 11, height: 11 }}/>
          </button>
          {!t.preset && (
            <button className="icon-btn" onClick={(e) => { e.stopPropagation(); onDel(); }} title="削除 / Delete">
              <i data-lucide="trash-2" style={{ width: 11, height: 11 }}/>
            </button>
          )}
        </div>
      )}
    </div>
  );
}

Object.assign(window, { TemplateList });
