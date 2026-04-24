// Settings.jsx — Settings / Preferences / MIDI / Output screens
function Settings({ screen = 'general' }) {
  const Nav = ({ items, active }) => (
    <aside style={{ width: 200, borderRight: '1px solid var(--stroke-1)', padding: 14, display: 'flex', flexDirection: 'column', gap: 2, background: 'var(--c-ink-1)' }}>
      <div style={{ fontSize: 10, fontWeight: 700, letterSpacing: '0.2em', color: 'var(--fg-5)', textTransform: 'uppercase', padding: '4px 10px 8px' }}>Settings</div>
      {items.map(it => (
        <a key={it.id} href={`#${it.id}`} className={`side-btn ${active === it.id ? 'active' : ''}`} style={{textDecoration:'none'}}>
          <i data-lucide={it.icon} style={{ width: 15, height: 15 }}/>
          <span>{it.label}</span>
        </a>
      ))}
    </aside>
  );
  const items = [
    {id:'general', icon:'settings', label:'General'},
    {id:'audio',   icon:'volume-2',  label:'Audio · 音声'},
    {id:'midi',    icon:'sliders-horizontal', label:'MIDI Mapping'},
    {id:'library', icon:'library',   label:'Library'},
    {id:'mixing',  icon:'shuffle',   label:'Mixing AI'},
    {id:'shortcuts', icon:'command', label:'Shortcuts'},
    {id:'account', icon:'user',      label:'Account'},
  ];

  return (
    <div style={{ flex: 1, display: 'flex', minHeight: 0 }}>
      <Nav items={items} active={screen}/>
      <div style={{ flex: 1, overflowY: 'auto', padding: '28px 36px' }}>
        {screen === 'general' && <General/>}
        {screen === 'audio' && <Audio/>}
        {screen === 'midi' && <MIDI/>}
        {screen === 'library' && <LibraryPref/>}
        {screen === 'mixing' && <MixingAI/>}
        {screen === 'shortcuts' && <Shortcuts/>}
        {screen === 'account' && <Account/>}
      </div>
    </div>
  );
}

// ---- shared bits ----
const SH = {
  group: { background: 'linear-gradient(180deg, var(--c-ink-3), var(--c-ink-2))', border: '1px solid var(--stroke-1)', borderRadius: 12, padding: 18, boxShadow: '0 4px 12px rgba(0,0,0,0.5), inset 0 1px 0 rgba(255,255,255,0.05)', marginBottom: 16 },
  row: { display: 'grid', gridTemplateColumns: '220px 1fr', gap: 16, padding: '14px 0', alignItems: 'center', borderTop: '1px solid rgba(255,255,255,0.04)' },
  label: { fontSize: 13, fontWeight: 600, color: 'var(--fg-1)', letterSpacing: '-0.005em' },
  sub: { fontSize: 11, color: 'var(--fg-4)', marginTop: 2 },
  h: { fontFamily: 'var(--font-display)', fontWeight: 700, fontSize: 22, letterSpacing: '-0.02em', color: 'var(--fg-1)', marginBottom: 4 },
  hSub: { fontSize: 12, color: 'var(--fg-4)', marginBottom: 18 },
  gLabel: { fontSize: 10, fontWeight: 700, letterSpacing: '0.2em', color: 'var(--fg-5)', textTransform: 'uppercase', marginBottom: 10 },
};

function Toggle({ on, accent = 'var(--c-accent)' }) {
  return (
    <span style={{
      width: 34, height: 20, borderRadius: 999, position: 'relative', display: 'inline-block',
      background: on ? 'linear-gradient(180deg, #5CE8BA, #3ACFA0)' : 'linear-gradient(180deg, var(--c-ink-2), var(--c-ink-3))',
      border: on ? 'none' : '1px solid var(--stroke-1)',
      boxShadow: on ? `inset 0 1px 2px rgba(0,0,0,0.3), 0 0 10px rgba(79,227,178,0.25), inset 0 1px 0 rgba(255,255,255,0.2)` : 'inset 0 1px 2px rgba(0,0,0,0.4)',
      transition: 'all 160ms',
    }}>
      <span style={{
        position: 'absolute', top: on ? 2 : 1, left: on ? 'auto' : 2, right: on ? 2 : 'auto',
        width: 16, height: 16, borderRadius: '50%',
        background: on ? 'linear-gradient(180deg, #F5F7FB, #BAC3D2)' : 'linear-gradient(180deg, #5A6578, #3B4453)',
        boxShadow: '0 1px 2px rgba(0,0,0,0.5)',
      }}/>
    </span>
  );
}

function Select({ value, options = [] }) {
  return (
    <div style={{
      height: 34, padding: '0 12px', display: 'inline-flex', alignItems: 'center', gap: 8,
      background: 'linear-gradient(180deg, var(--c-ink-3), var(--c-ink-4))',
      border: '1px solid var(--stroke-1)', borderRadius: 8, color: 'var(--fg-2)',
      fontSize: 12, fontFamily: 'var(--font-sans)', minWidth: 180, justifyContent: 'space-between',
      boxShadow: 'inset 0 1px 0 rgba(255,255,255,0.04)', cursor: 'pointer',
    }}>
      <span>{value}</span>
      <i data-lucide="chevron-down" style={{ width: 13, height: 13, color: 'var(--fg-5)' }}/>
    </div>
  );
}

function Slider({ value = 0.5, color = 'var(--c-accent)', format }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 12, maxWidth: 360 }}>
      <div style={{ flex: 1, height: 4, background: 'var(--c-ink-1)', borderRadius: 999, position: 'relative', boxShadow: 'inset 0 1px 2px rgba(0,0,0,0.5)' }}>
        <div style={{ position: 'absolute', left: 0, top: 0, width: `${value*100}%`, height: '100%', background: `linear-gradient(90deg, var(--c-accent-lo), ${color})`, borderRadius: 999 }}/>
        <div style={{ position: 'absolute', left: `${value*100}%`, top: '50%', width: 14, height: 14, background: 'linear-gradient(180deg, #F5F7FB, #BAC3D2)', borderRadius: '50%', transform: 'translate(-50%,-50%)', boxShadow: '0 2px 4px rgba(0,0,0,0.5), 0 0 0 0.5px rgba(0,0,0,0.6), inset 0 1px 0 rgba(255,255,255,0.4)' }}/>
      </div>
      <span className="tabular" style={{ fontSize: 12, color: 'var(--fg-2)', minWidth: 50, textAlign: 'right' }}>{format ? format(value) : (value*100).toFixed(0)+'%'}</span>
    </div>
  );
}

// ---- General ----
function General() {
  return <div>
    <div style={SH.h}>General · 全般</div>
    <div style={SH.hSub}>Appearance, language and startup behavior.</div>

    <div style={SH.group}>
      <div style={SH.gLabel}>Appearance</div>
      <div style={{...SH.row, borderTop:'none', paddingTop: 0}}>
        <div><div style={SH.label}>Theme</div><div style={SH.sub}>Dark is optimized for club environments.</div></div>
        <div style={{ display: 'flex', gap: 8 }}>
          {['Dark','Dark (Ink)','System'].map((t,i) => (
            <button key={t} className={`chip ${i===0?'active':''}`}>{t}</button>
          ))}
        </div>
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>Accent</div><div style={SH.sub}>Used for Deck A, CTA, focus ring.</div></div>
        <div style={{ display: 'flex', gap: 8 }}>
          {['#4FE3B2','#8A9BE8','#E8915A','#E87098','#E8B868'].map((c,i) => (
            <div key={c} style={{ width: 26, height: 26, borderRadius: '50%', background: c, border: i===0?'2px solid #F5F7FB':'1px solid rgba(255,255,255,0.1)', boxShadow: i===0?`0 0 0 2px ${c}40`:'inset 0 1px 0 rgba(255,255,255,0.15)' }}/>
          ))}
        </div>
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>Language · 言語</div><div style={SH.sub}>UI language</div></div>
        <Select value="日本語 (Japanese)" />
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>Waveform density</div><div style={SH.sub}>Bars per second — denser looks premium.</div></div>
        <Slider value={0.72} format={v => `${Math.round(40 + v*80)} bars/s`} />
      </div>
    </div>

    <div style={SH.group}>
      <div style={SH.gLabel}>Startup</div>
      <div style={{...SH.row, borderTop:'none', paddingTop: 0}}>
        <div><div style={SH.label}>Reopen last session</div><div style={SH.sub}>Restore decks, crossfader, queue.</div></div>
        <Toggle on/>
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>Auto-scan library on launch</div><div style={SH.sub}>Detects new tracks in watch folders.</div></div>
        <Toggle on={false}/>
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>Warn before closing during mix</div></div>
        <Toggle on/>
      </div>
    </div>
  </div>;
}

// ---- Audio ----
function Audio() {
  return <div>
    <div style={SH.h}>Audio · 音声設定</div>
    <div style={SH.hSub}>Output device, sample rate, and headphone monitoring.</div>

    <div style={SH.group}>
      <div style={SH.gLabel}>Output device</div>
      <div style={{...SH.row, borderTop:'none', paddingTop: 0}}>
        <div><div style={SH.label}>Main output</div><div style={SH.sub}>Speakers / house PA.</div></div>
        <Select value="Pioneer DJM-A9 — Channel 1/2" />
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>Headphone cue</div><div style={SH.sub}>For pre-listening next track.</div></div>
        <Select value="Pioneer DJM-A9 — Channel 3/4" />
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>Sample rate</div></div>
        <div style={{ display: 'flex', gap: 8 }}>
          {['44.1','48','88.2','96'].map((r,i) => <button key={r} className={`chip ${i===1?'active':''}`}>{r} kHz</button>)}
        </div>
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>Buffer size</div><div style={SH.sub}>Lower = less latency, more CPU.</div></div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          <Slider value={0.3} format={v => `${[64,128,256,512,1024][Math.floor(v*4.99)]} samples`} />
          <span className="tabular" style={{ fontSize: 11, color: 'var(--c-accent)' }}>2.9 ms</span>
        </div>
      </div>
    </div>

    <div style={SH.group}>
      <div style={SH.gLabel}>Levels</div>
      <div style={{...SH.row, borderTop:'none', paddingTop: 0}}>
        <div><div style={SH.label}>Master limiter</div><div style={SH.sub}>Protects speakers at peaks.</div></div>
        <Toggle on/>
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>Headphone level</div></div>
        <Slider value={0.66} />
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>Booth out</div></div>
        <Slider value={0.5} />
      </div>
    </div>
  </div>;
}

// ---- MIDI ----
function MIDI() {
  const rows = [
    { ctrl: 'Pioneer DDJ-FLX10', ch: 'Ch 1', mapping: 'Deck A · Play/Pause', status: 'Learning…' },
    { ctrl: 'Pioneer DDJ-FLX10', ch: 'Ch 1', mapping: 'Deck A · CUE', status: 'Mapped' },
    { ctrl: 'Pioneer DDJ-FLX10', ch: 'Ch 1', mapping: 'Deck A · Jog wheel', status: 'Mapped' },
    { ctrl: 'Pioneer DDJ-FLX10', ch: 'Ch 2', mapping: 'Deck B · Play/Pause', status: 'Mapped' },
    { ctrl: 'Pioneer DDJ-FLX10', ch: 'X', mapping: 'Crossfader', status: 'Mapped' },
    { ctrl: 'Native Instruments F1', ch: 'Ch 1', mapping: 'Hot cues 1–8', status: 'Mapped' },
    { ctrl: '—', ch: '—', mapping: 'Next Cue · Accept', status: 'Unmapped' },
  ];
  return <div>
    <div style={SH.h}>MIDI Mapping</div>
    <div style={SH.hSub}>Connect a controller, then move a knob to auto-bind.</div>

    <div style={SH.group}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 10 }}>
        <div style={SH.gLabel}>Connected controllers</div>
        <button className="chip active">+ Add MIDI device</button>
      </div>
      <div style={{ display: 'flex', gap: 10 }}>
        {[
          { name: 'Pioneer DDJ-FLX10', status: 'Connected', color: 'var(--c-accent)' },
          { name: 'NI Traktor F1', status: 'Connected', color: 'var(--c-accent)' },
          { name: 'Scan for more…', status: '', color: 'var(--fg-5)' },
        ].map(c => (
          <div key={c.name} style={{ flex: 1, padding: 12, border: '1px solid var(--stroke-1)', borderRadius: 10, background: 'rgba(255,255,255,0.02)' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <i data-lucide="sliders-horizontal" style={{ width: 15, height: 15, color: c.color }}/>
              <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--fg-1)' }}>{c.name}</div>
            </div>
            {c.status && <div style={{ fontSize: 11, color: c.color, marginTop: 4, display:'flex', alignItems:'center', gap:6 }}>
              <span style={{ width: 6, height: 6, borderRadius:'50%', background: c.color, boxShadow:`0 0 6px ${c.color}`}}/>{c.status}
            </div>}
          </div>
        ))}
      </div>
    </div>

    <div style={SH.group}>
      <div style={{...SH.gLabel, marginBottom: 0}}>Bindings</div>
      <div style={{ marginTop: 12, border: '1px solid var(--stroke-1)', borderRadius: 10, overflow: 'hidden' }}>
        <div style={{ display: 'grid', gridTemplateColumns: '1.6fr 0.6fr 1.6fr 1fr 80px', gap: 10, padding: '8px 12px', background: 'rgba(0,0,0,0.25)', borderBottom: '1px solid var(--stroke-1)', fontSize: 9, fontWeight: 700, letterSpacing: '0.2em', color: 'var(--fg-5)', textTransform: 'uppercase' }}>
          <div>Controller</div><div>CH</div><div>Function</div><div>Status</div><div></div>
        </div>
        {rows.map((r,i) => (
          <div key={i} style={{ display: 'grid', gridTemplateColumns: '1.6fr 0.6fr 1.6fr 1fr 80px', gap: 10, padding: '10px 12px', alignItems:'center', borderBottom: i<rows.length-1?'1px solid rgba(255,255,255,0.04)':'none', fontSize: 12 }}>
            <span style={{ color: 'var(--fg-2)' }}>{r.ctrl}</span>
            <span className="tabular" style={{ color: 'var(--fg-4)' }}>{r.ch}</span>
            <span style={{ color: 'var(--fg-1)', fontWeight: 500 }}>{r.mapping}</span>
            <span style={{ color: r.status === 'Mapped' ? 'var(--c-accent)' : r.status === 'Learning…' ? 'var(--c-cue)' : 'var(--fg-5)', fontSize: 11 }}>
              {r.status !== 'Unmapped' && <span style={{ width:6, height:6, borderRadius:'50%', display:'inline-block', background:'currentColor', marginRight:6, boxShadow: r.status === 'Learning…' ? '0 0 6px currentColor' : 'none' }}/>}
              {r.status}
            </span>
            <div style={{ display:'flex', gap: 4 }}>
              <button className="chip" style={{ height: 24, padding: '0 8px', fontSize: 10 }}>Learn</button>
            </div>
          </div>
        ))}
      </div>
    </div>
  </div>;
}

// ---- Library Pref ----
function LibraryPref() {
  return <div>
    <div style={SH.h}>Library</div>
    <div style={SH.hSub}>Where Conduction looks for tracks and how it analyzes them.</div>
    <div style={SH.group}>
      <div style={SH.gLabel}>Watch folders</div>
      {[
        { path: '~/Music/DJ', count: 842, scan: 'Just now' },
        { path: '/Volumes/SSD-A/Sets', count: 318, scan: '12 min ago' },
        { path: '~/Downloads/Promos', count: 47, scan: '2 h ago' },
      ].map(f => (
        <div key={f.path} style={{ display: 'flex', alignItems: 'center', gap: 14, padding: '12px 0', borderTop: '1px solid rgba(255,255,255,0.04)' }}>
          <i data-lucide="folder" style={{ width: 18, height: 18, color: 'var(--c-cue)' }}/>
          <div style={{ flex: 1 }}>
            <div style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--fg-1)' }}>{f.path}</div>
            <div style={{ fontSize: 11, color: 'var(--fg-4)', marginTop: 2 }}>{f.count} tracks · scanned {f.scan}</div>
          </div>
          <button className="chip">Rescan</button>
          <button className="chip" style={{ color: 'var(--c-danger, #E84A5F)', borderColor: 'rgba(232,74,95,0.3)' }}>Remove</button>
        </div>
      ))}
      <div style={{ marginTop: 10 }}>
        <button className="chip active">+ Add folder</button>
      </div>
    </div>

    <div style={SH.group}>
      <div style={SH.gLabel}>Analysis</div>
      <div style={{...SH.row, borderTop:'none', paddingTop: 0}}>
        <div><div style={SH.label}>Auto-analyze on import</div><div style={SH.sub}>BPM, key, beat grid, waveform.</div></div>
        <Toggle on/>
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>Key notation</div></div>
        <div style={{ display: 'flex', gap: 8 }}>
          <button className="chip active">Camelot (8A)</button>
          <button className="chip">Musical (Am)</button>
          <button className="chip">Open Key</button>
        </div>
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>BPM range</div><div style={SH.sub}>Tracks outside range will be flagged.</div></div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <input defaultValue="60" style={{ width: 70, height: 30, background: 'var(--c-ink-3)', border: '1px solid var(--stroke-1)', borderRadius: 6, padding: '0 10px', color: 'var(--fg-1)', fontFamily: 'var(--font-mono)', fontSize: 12 }}/>
          <span style={{ color: 'var(--fg-5)' }}>—</span>
          <input defaultValue="180" style={{ width: 70, height: 30, background: 'var(--c-ink-3)', border: '1px solid var(--stroke-1)', borderRadius: 6, padding: '0 10px', color: 'var(--fg-1)', fontFamily: 'var(--font-mono)', fontSize: 12 }}/>
          <span style={{ color: 'var(--fg-5)', fontSize: 11 }}>BPM</span>
        </div>
      </div>
    </div>
  </div>;
}

// ---- Mixing AI ----
function MixingAI() {
  return <div>
    <div style={SH.h}>Mixing AI · 繋ぎの指揮</div>
    <div style={SH.hSub}>How Conduction suggests and executes the next transition.</div>

    <div style={SH.group}>
      <div style={SH.gLabel}>Autopilot</div>
      <div style={{...SH.row, borderTop:'none', paddingTop: 0}}>
        <div><div style={SH.label}>Enable Next Cue suggestions</div><div style={SH.sub}>Show candidates in the library rail.</div></div>
        <Toggle on/>
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>Auto-mix when queue exhausted</div><div style={SH.sub}>Bridges gaps when you're away.</div></div>
        <Toggle on={false}/>
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>Confidence threshold</div><div style={SH.sub}>Only surface candidates above this match.</div></div>
        <Slider value={0.7} format={v => `${Math.round(60 + v*35)}%`} />
      </div>
    </div>

    <div style={SH.group}>
      <div style={SH.gLabel}>Transition style</div>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 10, marginTop: 4 }}>
        {[
          { name: 'Smooth', desc: 'Long filter fades, phrase-matched', active: true },
          { name: 'Classic', desc: 'EQ trades at bar 8/16/32', active: false },
          { name: 'Sharp', desc: 'Hard cuts on downbeats', active: false },
        ].map(s => (
          <div key={s.name} style={{
            padding: 14, borderRadius: 10,
            background: s.active ? 'linear-gradient(180deg, rgba(79,227,178,0.1), rgba(79,227,178,0.04))' : 'rgba(255,255,255,0.025)',
            border: `1px solid ${s.active ? 'rgba(79,227,178,0.28)' : 'var(--stroke-1)'}`,
            boxShadow: s.active ? 'inset 0 1px 0 rgba(255,255,255,0.05)' : 'none',
          }}>
            <div style={{ fontSize: 13, fontWeight: 700, color: s.active ? 'var(--fg-1)' : 'var(--fg-2)', letterSpacing:'-0.005em' }}>{s.name}</div>
            <div style={{ fontSize: 11, color: 'var(--fg-4)', marginTop: 3 }}>{s.desc}</div>
          </div>
        ))}
      </div>
    </div>

    <div style={SH.group}>
      <div style={SH.gLabel}>Harmonic rules</div>
      <div style={{...SH.row, borderTop:'none', paddingTop: 0}}>
        <div><div style={SH.label}>Respect Camelot wheel</div><div style={SH.sub}>Prefer same/adjacent keys.</div></div>
        <Toggle on/>
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>BPM tolerance</div></div>
        <Slider value={0.35} format={v => `±${Math.round(2 + v*10)}%`} />
      </div>
      <div style={SH.row}>
        <div><div style={SH.label}>Energy progression</div><div style={SH.sub}>Gradually increase energy through the set.</div></div>
        <Toggle on/>
      </div>
    </div>
  </div>;
}

// ---- Shortcuts ----
function Shortcuts() {
  const groups = [
    { name: 'Transport', items: [
      ['Play / Pause Deck A', 'Space'],
      ['Play / Pause Deck B', '⇧ Space'],
      ['Load to A / B', '1 / 2'],
      ['Sync', 'S'],
    ]},
    { name: 'Next Cue', items: [
      ['Accept suggestion', '⏎'],
      ['Skip suggestion', '⇥'],
      ['Toggle Autopilot', '⌘A'],
    ]},
    { name: 'Library', items: [
      ['Focus search', '⌘K'],
      ['Preview in headphones', 'P'],
      ['Star track', '⌘D'],
    ]},
  ];
  return <div>
    <div style={SH.h}>Shortcuts · キーボード</div>
    <div style={SH.hSub}>Hotkeys for performance — learn these first.</div>
    {groups.map(g => (
      <div key={g.name} style={SH.group}>
        <div style={SH.gLabel}>{g.name}</div>
        {g.items.map(([label, keys], i) => (
          <div key={label} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '10px 0', borderTop: i===0?'none':'1px solid rgba(255,255,255,0.04)' }}>
            <span style={{ fontSize: 13, color: 'var(--fg-2)' }}>{label}</span>
            <span style={{
              fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--fg-1)',
              padding: '4px 10px', background: 'rgba(255,255,255,0.04)',
              border: '1px solid rgba(255,255,255,0.08)', borderRadius: 6,
              boxShadow: 'inset 0 -1px 0 rgba(0,0,0,0.3)',
            }}>{keys}</span>
          </div>
        ))}
      </div>
    ))}
  </div>;
}

function Account() {
  return <div>
    <div style={SH.h}>Account · アカウント</div>
    <div style={SH.hSub}>Manage your Conduction subscription.</div>
    <div style={SH.group}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
        <div style={{ width: 56, height: 56, borderRadius: '50%', background: 'linear-gradient(135deg, #4FE3B2, #8A9BE8)', boxShadow: '0 4px 12px rgba(0,0,0,0.4)' }}/>
        <div style={{ flex: 1 }}>
          <div style={{ fontFamily: 'var(--font-display)', fontSize: 20, fontWeight: 700, color: 'var(--fg-1)', letterSpacing: '-0.02em' }}>yukio</div>
          <div style={{ fontSize: 12, color: 'var(--fg-4)' }}>yukio@example.com · Free plan</div>
        </div>
        <button style={{
          height: 38, padding: '0 18px', fontFamily: 'var(--font-sans)', fontWeight: 700, fontSize: 13, letterSpacing: '-0.005em',
          background: 'linear-gradient(180deg, #5CE8BA, #3ACFA0)', color: '#07201A',
          border: '1px solid rgba(79,227,178,0.6)', borderRadius: 8, cursor: 'pointer',
          boxShadow: 'inset 0 1px 0 rgba(255,255,255,0.35), inset 0 -1px 0 rgba(0,0,0,0.25), 0 1px 2px rgba(0,0,0,0.4)',
        }}>Upgrade to Pro</button>
      </div>
    </div>
    <div style={SH.group}>
      <div style={SH.gLabel}>Devices · 3 / 5</div>
      {[
        { n: "Yukio's MacBook Pro", os: 'macOS 14 · this device', current: true },
        { n: "Studio iMac", os: 'macOS 13 · last seen 3 d ago', current: false },
        { n: "Booth MacBook Air", os: 'macOS 14 · last seen 18 h ago', current: false },
      ].map((d,i) => (
        <div key={d.n} style={{ display: 'flex', alignItems: 'center', gap: 14, padding: '12px 0', borderTop: i===0?'none':'1px solid rgba(255,255,255,0.04)' }}>
          <i data-lucide="laptop" style={{ width: 18, height: 18, color: d.current ? 'var(--c-accent)' : 'var(--fg-4)' }}/>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--fg-1)' }}>{d.n} {d.current && <span style={{fontSize:10, color:'var(--c-accent)', marginLeft:8, letterSpacing:'0.18em'}}>THIS DEVICE</span>}</div>
            <div style={{ fontSize: 11, color: 'var(--fg-4)', marginTop: 2 }}>{d.os}</div>
          </div>
          {!d.current && <button className="chip">Sign out</button>}
        </div>
      ))}
    </div>
  </div>;
}

Object.assign(window, { Settings });
