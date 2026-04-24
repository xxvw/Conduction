// App.jsx — Main Conduction DJ view
const TRACKS = [
  { id: 't1', title: 'Midnight Drive', artist: 'Kaoru', bpm: 128.0, key: '8A', duration: 318, genre: 'Deep' },
  { id: 't2', title: 'Neon Reverie', artist: 'Ayame', bpm: 126.5, key: '8A', duration: 362, genre: 'Deep' },
  { id: 't3', title: 'After Tokyo', artist: 'Ren · feat. Mio', bpm: 130.0, key: '9A', duration: 288, genre: 'Tech' },
  { id: 't4', title: 'Glass City', artist: 'Hinata', bpm: 128.0, key: '7A', duration: 402, genre: 'Deep' },
  { id: 't5', title: 'Silhouette', artist: 'Kaoru', bpm: 124.0, key: '6A', duration: 336, genre: 'Deep' },
  { id: 't6', title: 'Under Glow', artist: 'Sora', bpm: 132.0, key: '9A', duration: 298, genre: 'Tech' },
  { id: 't7', title: 'Fade to Indigo', artist: 'Yumi & Tsuki', bpm: 122.0, key: '5A', duration: 418, genre: 'Dub' },
  { id: 't8', title: 'Crosslight', artist: 'Nao', bpm: 128.0, key: '8B', duration: 345, genre: 'Deep' },
];

const SUGGESTIONS = [
  { name: 'Filter Fade · 32 bars', detail: 'drop @ 1:24 → next intro', match: 94 },
  { name: 'Echo Out · 16 bars', detail: '−2 dB tail · Deck A', match: 82 },
  { name: 'Hard Cut · downbeat', detail: 'instant switch at bar 64', match: 71 },
];

const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "deckAColor": "#4FE3B2",
  "deckBColor": "#E8915A",
  "accentColor": "#4FE3B2",
  "showNextCue": true,
  "ambientGlow": true,
  "density": "regular",
  "recording": false,
  "screen": "mix"
}/*EDITMODE-END*/;

function App() {
  const [t, setTweak] = useTweaks(TWEAK_DEFAULTS);

  const [deckA, setDeckA] = React.useState(TRACKS[0]);
  const [deckB, setDeckB] = React.useState(TRACKS[2]);
  const [playingA, setPlayingA] = React.useState(true);
  const [playingB, setPlayingB] = React.useState(false);
  const [progressA, setProgressA] = React.useState(0.42);
  const [progressB, setProgressB] = React.useState(0.08);
  const [crossfade, setCrossfade] = React.useState(0.35);

  React.useEffect(() => {
    const id = setInterval(() => {
      if (playingA) setProgressA(p => Math.min(0.99, p + 0.0008));
      if (playingB) setProgressB(p => Math.min(0.99, p + 0.0008));
    }, 100);
    return () => clearInterval(id);
  }, [playingA, playingB]);

  React.useEffect(() => { lucide.createIcons({ attrs: { 'stroke-width': 1.5 } }); });

  // Push accent colour to CSS so the whole design system follows.
  React.useEffect(() => {
    document.documentElement.style.setProperty('--c-accent', t.accentColor);
    document.documentElement.style.setProperty('--c-deck-a', t.deckAColor);
    document.documentElement.style.setProperty('--c-deck-b', t.deckBColor);
  }, [t.accentColor, t.deckAColor, t.deckBColor]);

  const loadTrack = (side, tr) => {
    if (side === 'A') { setDeckA(tr); setProgressA(0); setPlayingA(false); }
    else { setDeckB(tr); setProgressB(0); setPlayingB(false); }
  };

  const nowPlayingId = playingA ? deckA.id : (playingB ? deckB.id : deckA.id);
  const screen = t.screen;
  const isSettings = ['settings','audio','midi','library-pref','mixing','shortcuts','account'].includes(screen);
  const isFullScreen = ['setlist','templates','beatgrid','cues'].includes(screen);
  const gap = t.density === 'compact' ? 6 : t.density === 'comfy' ? 14 : 10;
  const pad = t.density === 'compact' ? 10 : t.density === 'comfy' ? 18 : 14;

  return (
    <div style={{ height: '100vh', display: 'flex', flexDirection: 'column', background: 'var(--c-ink-0)', color: 'var(--fg-2)', position: 'relative' }}>
      <TopBar masterBpm={deckA.bpm} recording={t.recording} onToggleRec={() => setTweak('recording', !t.recording)} />

      <div style={{ flex: 1, display: 'flex', minHeight: 0 }}>
        <Sidebar activeScreen={screen} onPickScreen={(s) => setTweak('screen', s)} />

        <main style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0, position: 'relative' }}>
          {screen === 'mix' && (
            <>
              {t.ambientGlow && <div className="deck-ambient" />}
              <div style={{ display: 'flex', flexDirection: 'column', gap, padding: pad, position: 'relative', zIndex: 1 }}>
                <DeckView side="A" color={t.deckAColor} track={deckA} playing={playingA} onTogglePlay={() => setPlayingA(!playingA)} progress={progressA} />
                <Mixer crossfade={crossfade} setCrossfade={setCrossfade} />
                <DeckView side="B" color={t.deckBColor} track={deckB} playing={playingB} onTogglePlay={() => setPlayingB(!playingB)} progress={progressB} />
              </div>
              <div style={{ flex: 1, minHeight: 0, padding: `0 ${pad}px ${pad}px`, display: 'flex', flexDirection: 'column', position: 'relative', zIndex: 1 }}>
                <Library tracks={TRACKS} onLoad={loadTrack} nowPlayingId={nowPlayingId} suggestions={SUGGESTIONS} onPickSugg={() => {}} showNextCue={t.showNextCue}/>
              </div>
            </>
          )}

          {screen === 'library' && (
            <div style={{ flex: 1, minHeight: 0, padding: pad }}>
              <Library tracks={TRACKS} onLoad={loadTrack} nowPlayingId={nowPlayingId} suggestions={SUGGESTIONS} onPickSugg={() => {}} showNextCue={t.showNextCue}/>
            </div>
          )}

          {screen === 'setlist' && <Setlist />}
          {screen === 'templates' && <Templates />}
          {screen === 'beatgrid' && <BeatgridCorrection track={deckA} />}
          {screen === 'cues' && <CueEditor track={deckA} />}

          {screen === 'prep' && <EmptyView icon="bookmark" title="Prepare · 準備" desc="Drag tracks here to plan your set. 24 tracks in queue."/>}
          {screen === 'history' && <EmptyView icon="history" title="History · 履歴" desc="Last 30 sessions, recorded and exportable."/>}

          {isSettings && <Settings screen={screen === 'settings' ? 'general' : screen === 'library-pref' ? 'library' : screen} />}
        </main>
      </div>

      <TweaksPanel>
        <TweakSection label="Theme" />
        <TweakColor label="Accent" value={t.accentColor} onChange={v => setTweak('accentColor', v)} />
        <TweakColor label="Deck A" value={t.deckAColor} onChange={v => setTweak('deckAColor', v)} />
        <TweakColor label="Deck B" value={t.deckBColor} onChange={v => setTweak('deckBColor', v)} />
        <TweakToggle label="Ambient glow" value={t.ambientGlow} onChange={v => setTweak('ambientGlow', v)} />

        <TweakSection label="Layout" />
        <TweakRadio label="Density" value={t.density} options={['compact','regular','comfy']} onChange={v => setTweak('density', v)} />
        <TweakToggle label="Show Next Cue rail" value={t.showNextCue} onChange={v => setTweak('showNextCue', v)} />

        <TweakSection label="Screen" />
        <TweakSelect label="View" value={t.screen}
          options={['mix','library','setlist','templates','beatgrid','cues','prep','history','settings','audio','midi','library-pref','mixing','shortcuts','account']}
          onChange={v => setTweak('screen', v)} />

        <TweakSection label="State" />
        <TweakToggle label="Recording" value={t.recording} onChange={v => setTweak('recording', v)} />
      </TweaksPanel>
    </div>
  );
}

function EmptyView({ icon, title, desc }) {
  return (
    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 14, padding: 40 }}>
      <div style={{ width: 64, height: 64, borderRadius: 16, background: 'linear-gradient(180deg, var(--c-ink-3), var(--c-ink-2))', border: '1px solid var(--stroke-1)', display:'flex', alignItems:'center', justifyContent:'center', boxShadow: 'var(--shadow-2)' }}>
        <i data-lucide={icon} style={{ width: 24, height: 24, color: 'var(--c-accent)' }}/>
      </div>
      <div style={{ fontFamily: 'var(--font-display)', fontSize: 22, fontWeight: 700, letterSpacing: '-0.02em', color: 'var(--fg-1)' }}>{title}</div>
      <div style={{ fontSize: 13, color: 'var(--fg-4)', textAlign: 'center', maxWidth: 400 }}>{desc}</div>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
