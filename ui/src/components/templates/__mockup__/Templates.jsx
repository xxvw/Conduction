// Templates.jsx — Templates screen: left list + right editor
const PRESET_TEMPLATES = [
  {
    id: 'p-long-eq', preset: true, kind: 'transition',
    name: 'Long EQ Mix', nameEn: 'LONG EQ MIX', bars: 32, tags: ['Deep','House'],
    entryCue: 'intro-16', exitCue: 'outro-32',
    tracks: [
      { id: 'a-vol', label: 'Deck A · Volume', sub: 'fader', color: 'var(--c-deck-a)', collapsed: false, keyframes: [
        { beat: 0, value: 1.0, curve: 'linear' },
        { beat: 64, value: 1.0, curve: 'easeOut' },
        { beat: 128, value: 0.0, curve: 'linear' },
      ]},
      { id: 'a-eq-low', label: 'Deck A · EQ Low', sub: 'eq', color: 'var(--c-deck-a)', collapsed: false, keyframes: [
        { beat: 0, value: 0.5, curve: 'linear' },
        { beat: 48, value: 0.5, curve: 'easeOut' },
        { beat: 80, value: 0.0, curve: 'easeOut' },
      ]},
      { id: 'b-vol', label: 'Deck B · Volume', sub: 'fader', color: 'var(--c-deck-b)', collapsed: false, keyframes: [
        { beat: 0, value: 0.0, curve: 'linear' },
        { beat: 64, value: 0.6, curve: 'easeInOut' },
        { beat: 128, value: 1.0, curve: 'linear' },
      ]},
      { id: 'b-eq-low', label: 'Deck B · EQ Low', sub: 'eq', color: 'var(--c-deck-b)', collapsed: false, keyframes: [
        { beat: 0, value: 0.0, curve: 'linear' },
        { beat: 48, value: 0.0, curve: 'easeInOut' },
        { beat: 80, value: 0.5, curve: 'linear' },
      ]},
      { id: 'xf', label: 'Crossfader', sub: 'xf', color: '#E6EAF1', collapsed: false, keyframes: [
        { beat: 0, value: 0.0, curve: 'easeInOut' },
        { beat: 128, value: 1.0, curve: 'linear' },
      ]},
    ],
  },
  {
    id: 'p-quick-cut', preset: true, kind: 'transition',
    name: 'Quick Cut', nameEn: 'QUICK CUT', bars: 4, tags: ['Tech','Peak'],
    entryCue: 'drop', exitCue: 'auto',
    tracks: [
      { id: 'xf', label: 'Crossfader', sub: 'xf', color: '#E6EAF1', collapsed: false, keyframes: [
        { beat: 0, value: 0.0, curve: 'step' },
        { beat: 15, value: 0.0, curve: 'step' },
        { beat: 16, value: 1.0, curve: 'linear' },
      ]},
    ],
  },
  {
    id: 'p-breakdown', preset: true, kind: 'transition',
    name: 'Breakdown Swap', nameEn: 'BREAKDOWN SWAP', bars: 16, tags: ['Deep','Dub'],
    entryCue: 'breakdown-32', exitCue: 'auto',
    tracks: [
      { id: 'a-vol', label: 'Deck A · Volume', sub: 'fader', color: 'var(--c-deck-a)', collapsed: false, keyframes: [
        { beat: 0, value: 1.0, curve: 'easeOut' },
        { beat: 32, value: 0.0, curve: 'linear' },
      ]},
      { id: 'b-vol', label: 'Deck B · Volume', sub: 'fader', color: 'var(--c-deck-b)', collapsed: false, keyframes: [
        { beat: 16, value: 0.0, curve: 'easeIn' },
        { beat: 64, value: 1.0, curve: 'linear' },
      ]},
      { id: 'hp', label: 'FX · HP Filter', sub: 'fx', color: '#A089DC', collapsed: false, keyframes: [
        { beat: 0, value: 0.0, curve: 'easeOut' },
        { beat: 24, value: 0.8, curve: 'easeIn' },
        { beat: 48, value: 0.0, curve: 'linear' },
      ]},
    ],
  },
  {
    id: 'p-echo-out', preset: true, kind: 'transition',
    name: 'Echo Out', nameEn: 'ECHO OUT', bars: 8, tags: ['House','Tech'],
    entryCue: 'outro-32', exitCue: 'auto',
    tracks: [
      { id: 'a-vol', label: 'Deck A · Volume', sub: 'fader', color: 'var(--c-deck-a)', collapsed: false, keyframes: [
        { beat: 0, value: 1.0, curve: 'linear' },
        { beat: 24, value: 0.6, curve: 'easeOut' },
        { beat: 32, value: 0.0, curve: 'linear' },
      ]},
      { id: 'echo', label: 'FX · Echo', sub: 'fx', color: '#A089DC', collapsed: false, keyframes: [
        { beat: 16, value: 0.0, curve: 'easeIn' },
        { beat: 28, value: 1.0, curve: 'easeOut' },
        { beat: 32, value: 1.0, curve: 'hold' },
      ]},
    ],
  },
  {
    id: 'p-instant', preset: true, kind: 'transition',
    name: 'Instant Swap', nameEn: 'INSTANT SWAP', bars: 1, tags: ['Peak'],
    entryCue: 'drop', exitCue: 'auto',
    tracks: [
      { id: 'xf', label: 'Crossfader', sub: 'xf', color: '#E6EAF1', collapsed: false, keyframes: [
        { beat: 0, value: 0.0, curve: 'step' },
        { beat: 4, value: 1.0, curve: 'linear' },
      ]},
    ],
  },
  // User sample
  {
    id: 'u-nagare', preset: false, kind: 'transition',
    name: '流れ Mix', nameEn: 'NAGARE MIX', bars: 24, tags: ['Deep'],
    entryCue: 'intro-16', exitCue: 'auto',
    tracks: [
      { id: 'a-vol', label: 'Deck A · Volume', sub: 'fader', color: 'var(--c-deck-a)', collapsed: false, keyframes: [
        { beat: 0, value: 1.0, curve: 'linear' },
        { beat: 96, value: 0.0, curve: 'easeInOut' },
      ]},
      { id: 'b-vol', label: 'Deck B · Volume', sub: 'fader', color: 'var(--c-deck-b)', collapsed: false, keyframes: [
        { beat: 0, value: 0.0, curve: 'easeIn' },
        { beat: 96, value: 1.0, curve: 'linear' },
      ]},
    ],
  },
  // Setlist samples
  { id: 's-afterhours', preset: true, kind: 'setlist', name: 'Afterhours 3h', nameEn: 'AFTERHOURS 3H', bars: 0, tags: ['Deep'], tracks: [] },
  { id: 's-peak', preset: true, kind: 'setlist', name: 'Peak Hour', nameEn: 'PEAK HOUR', bars: 0, tags: ['Tech'], tracks: [] },
];

function Templates() {
  const [kind, setKind] = React.useState('transition');
  const [templates, setTemplates] = React.useState(PRESET_TEMPLATES);
  const [activeId, setActiveId] = React.useState('p-long-eq');

  const active = templates.find(t => t.id === activeId);
  const setActive = (updater) => {
    setTemplates(ts => ts.map(t => t.id === activeId ? (typeof updater === 'function' ? updater(t) : updater) : t));
  };

  const onNew = () => {
    const id = 'u-' + Date.now();
    const tmpl = {
      id, preset: false, kind,
      name: kind === 'setlist' ? '新規セット' : '新規テンプレート',
      nameEn: 'UNTITLED', bars: 16, tags: ['Custom'],
      entryCue: 'auto', exitCue: 'auto',
      tracks: kind === 'setlist' ? [] : [
        { id: 'a-vol', label: 'Deck A · Volume', sub: 'fader', color: 'var(--c-deck-a)', collapsed: false, keyframes: [
          { beat: 0, value: 1.0, curve: 'linear' }, { beat: 64, value: 0.0, curve: 'easeOut' }
        ]},
        { id: 'b-vol', label: 'Deck B · Volume', sub: 'fader', color: 'var(--c-deck-b)', collapsed: false, keyframes: [
          { beat: 0, value: 0.0, curve: 'easeIn' }, { beat: 64, value: 1.0, curve: 'linear' }
        ]},
      ],
    };
    setTemplates(ts => [...ts, tmpl]); setActiveId(id);
  };
  const onDup = (id) => {
    const src = templates.find(t => t.id === id);
    const copy = { ...JSON.parse(JSON.stringify(src)), id: 'u-' + Date.now(), preset: false, name: src.name + ' のコピー' };
    setTemplates(ts => [...ts, copy]); setActiveId(copy.id);
  };
  const onDel = (id) => {
    setTemplates(ts => ts.filter(t => t.id !== id));
    if (activeId === id) setActiveId(templates.find(t => t.id !== id && t.kind === kind)?.id);
  };

  React.useEffect(() => { lucide.createIcons({ attrs: { 'stroke-width': 1.75 } }); });

  return (
    <div style={{ flex: 1, display: 'flex', minHeight: 0 }}>
      <TemplateList
        templates={templates} activeId={activeId} onPick={setActiveId}
        kind={kind} onKind={setKind}
        onNew={onNew} onDup={onDup} onDel={onDel}
        onImport={() => alert('Import .ctpl')} onExport={() => alert('Export .ctpl')}
      />
      {active ? (
        <TemplateEditor template={active} setTemplate={setActive}/>
      ) : (
        <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--fg-5)' }}>
          テンプレートを選択 / Select a template
        </div>
      )}
    </div>
  );
}

Object.assign(window, { Templates });
