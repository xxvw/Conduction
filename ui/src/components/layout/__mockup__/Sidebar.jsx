// Sidebar.jsx — Primary nav with screen switcher
function Sidebar({ activeScreen = 'mix', onPickScreen }) {
  const screens = [
    { id: 'mix', icon: 'disc-3', label: 'Mix', sub: 'Decks · Live' },
    { id: 'library', icon: 'library', label: 'Library', sub: '842 tracks' },
    { id: 'setlist', icon: 'list-ordered', label: 'Setlist', sub: 'セット · 5 tracks' },
    { id: 'templates', icon: 'layers', label: 'Templates', sub: 'テンプレート' },
    { id: 'beatgrid', icon: 'grid-3x3', label: 'Beatgrid', sub: 'ビート補正' },
    { id: 'cues', icon: 'bookmark-check', label: 'Cues', sub: 'Cue エディタ' },
    { id: 'prep', icon: 'bookmark', label: 'Prepare', sub: '24 in queue' },
    { id: 'history', icon: 'history', label: 'History', sub: null },
    { id: 'settings', icon: 'settings', label: 'Settings', sub: null },
    { id: 'audio', icon: 'volume-2', label: 'Audio', sub: null, parent: 'settings' },
    { id: 'midi', icon: 'sliders-horizontal', label: 'MIDI Mapping', sub: null, parent: 'settings' },
    { id: 'mixing', icon: 'shuffle', label: 'Mixing AI', sub: null, parent: 'settings' },
  ];
  const isSettings = ['settings','audio','midi','mixing','shortcuts','account'].includes(activeScreen);
  const primary = screens.filter(s => !s.parent);
  const subs = screens.filter(s => s.parent === 'settings');

  return (
    <aside style={{
      width: 232, flexShrink: 0, background: 'linear-gradient(180deg, var(--c-ink-1), var(--c-ink-0))',
      borderRight: '1px solid var(--stroke-1)',
      padding: 14, display: 'flex', flexDirection: 'column', gap: 16, overflowY: 'auto',
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '4px 6px' }}>
        <svg viewBox="0 0 64 64" fill="none" style={{ height: 26, width: 26 }}>
          <circle cx="32" cy="32" r="25" stroke="#4FE3B2" strokeWidth="1.5"/>
          <circle cx="32" cy="32" r="25" stroke="rgba(79,227,178,0.15)" strokeWidth="5" fill="none"/>
          <path d="M21 32 Q 26 23, 32 32 T 43 32" stroke="#F5F7FB" strokeWidth="1.75" strokeLinecap="round" fill="none"/>
          <path d="M21 38 Q 26 30, 32 38 T 43 38" stroke="#4FE3B2" strokeWidth="1.25" strokeLinecap="round" fill="none" opacity="0.7"/>
        </svg>
        <div style={{ fontFamily: 'var(--font-display)', fontWeight: 700, fontSize: 17, letterSpacing: '-0.02em', color: 'var(--fg-1)' }}>Conduction</div>
      </div>

      <div>
        <div style={{ fontSize: 9, fontWeight: 700, letterSpacing: '0.2em', color: 'var(--fg-6)', textTransform: 'uppercase', padding: '0 8px 6px' }}>Workspace</div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
          {primary.map(it => (
            <button key={it.id}
              onClick={() => onPickScreen && onPickScreen(it.id)}
              className={`side-btn ${activeScreen === it.id || (it.id==='settings' && isSettings) ? 'active' : ''}`}>
              <i data-lucide={it.icon} style={{ width: 15, height: 15 }}/>
              <span style={{ flex: 1, textAlign: 'left' }}>
                <div>{it.label}</div>
                {it.sub && <div style={{ fontSize: 10, color: 'var(--fg-5)', marginTop: 1 }}>{it.sub}</div>}
              </span>
            </button>
          ))}
        </div>
      </div>

      {isSettings && (
        <div>
          <div style={{ fontSize: 9, fontWeight: 700, letterSpacing: '0.2em', color: 'var(--fg-6)', textTransform: 'uppercase', padding: '0 8px 6px' }}>Settings</div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
            <button onClick={() => onPickScreen('settings')} className={`side-btn ${activeScreen === 'settings' ? 'active' : ''}`} style={{ paddingLeft: 28 }}>
              <span>General</span>
            </button>
            {subs.map(it => (
              <button key={it.id} onClick={() => onPickScreen && onPickScreen(it.id)}
                className={`side-btn ${activeScreen === it.id ? 'active' : ''}`} style={{ paddingLeft: 28 }}>
                <span>{it.label}</span>
              </button>
            ))}
            <button onClick={() => onPickScreen('shortcuts')} className={`side-btn ${activeScreen === 'shortcuts' ? 'active' : ''}`} style={{ paddingLeft: 28 }}>
              <span>Shortcuts</span>
            </button>
            <button onClick={() => onPickScreen('account')} className={`side-btn ${activeScreen === 'account' ? 'active' : ''}`} style={{ paddingLeft: 28 }}>
              <span>Account</span>
            </button>
          </div>
        </div>
      )}

      <div>
        <div style={{ fontSize: 9, fontWeight: 700, letterSpacing: '0.2em', color: 'var(--fg-6)', textTransform: 'uppercase', padding: '0 8px 6px' }}>Sets · セット</div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
          {[
            { label: 'Deep House 128–132', accent: true },
            { label: 'Afterhours · Tokyo' },
            { label: 'Warm-up · 110–122' },
            { label: 'Peak · 132+' },
          ].map(s => (
            <button key={s.label} className={`side-btn ${s.accent ? 'accent' : ''}`}>
              <span style={{ width: 6, height: 6, borderRadius: '50%', background: s.accent ? 'var(--c-accent)' : 'var(--c-ink-6)' }} />
              <span style={{ flex: 1, textAlign: 'left', fontSize: 12 }}>{s.label}</span>
            </button>
          ))}
        </div>
      </div>

      <div style={{ marginTop: 'auto', display: 'flex', alignItems: 'center', gap: 10, padding: 10, borderRadius: 10, background: 'rgba(255,255,255,0.03)', border: '1px solid rgba(255,255,255,0.06)', boxShadow: 'inset 0 1px 0 rgba(255,255,255,0.04)' }}>
        <div style={{ width: 28, height: 28, borderRadius: '50%', background: 'linear-gradient(135deg, #4FE3B2, #8A9BE8)', boxShadow: 'inset 0 1px 0 rgba(255,255,255,0.2)' }} />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--fg-2)', letterSpacing: '-0.005em' }}>User</div>
        </div>
        <i data-lucide="settings" style={{ width: 14, height: 14, color: 'var(--fg-4)' }} />
      </div>
    </aside>
  );
}

Object.assign(window, { Sidebar });
