// Setlist.jsx — main setlist editor screen
function Setlist() {
  const [tracks, setTracks] = React.useState([
    { id: 't1', title: 'Opening Fog',     artist: 'Arata',   bpm: 124.00, key: '8A', duration: 342, energy: 0.35, hue1: '#4FE3B2', hue2: '#6BA3E8' },
    { id: 't2', title: 'Midnight Drive',  artist: 'Kaoru',   bpm: 126.00, key: '9A', duration: 318, energy: 0.55, hue1: '#8A9BE8', hue2: '#A089DC' },
    { id: 't3', title: 'Kōri / 氷',       artist: 'Riku',    bpm: 128.00, key: '9A', duration: 296, energy: 0.7,  hue1: '#E8915A', hue2: '#E84A5C' },
    { id: 't4', title: 'Neon Gardens',    artist: 'Mei',     bpm: 128.00, key: '10A',duration: 310, energy: 0.85, hue1: '#E84A5C', hue2: '#A089DC' },
    { id: 't5', title: 'Last Train West', artist: 'Sora',    bpm: 124.00, key: '10A',duration: 328, energy: 0.65, hue1: '#8A9BE8', hue2: '#4FE3B2' },
  ]);
  const [transitions, setTransitions] = React.useState({
    't1-t2': { template: 'Long EQ Mix',     bars: 32, tempoMode: 'LinearBlend',  candidates: [{name:'Long EQ Mix',bars:32,match:94},{name:'Breakdown Swap',bars:16,match:81},{name:'Echo Out',bars:8,match:68}] },
    't2-t3': { template: 'Breakdown Swap',  bars: 16, tempoMode: 'MatchTarget', candidates: [{name:'Breakdown Swap',bars:16,match:92},{name:'Long EQ Mix',bars:32,match:78},{name:'Quick Cut',bars:4,match:52}] },
    't3-t4': { template: 'Quick Cut',       bars: 4,  tempoMode: 'HoldSource',   candidates: [{name:'Quick Cut',bars:4,match:89},{name:'Echo Out',bars:8,match:72},{name:'Long EQ Mix',bars:32,match:55}] },
    't4-t5': { template: 'Echo Out',        bars: 8,  tempoMode: 'LinearBlend',  candidates: [{name:'Echo Out',bars:8,match:91},{name:'Long EQ Mix',bars:32,match:74},{name:'Quick Cut',bars:4,match:48}] },
  });
  const [selectedId, setSelectedId] = React.useState('t1');
  const [playingIdx, setPlayingIdx] = React.useState(1);

  React.useEffect(() => { lucide.createIcons({ attrs: { 'stroke-width': 1.75 } }); });

  const moveTrack = (from, to) => {
    if (from === to) return;
    const next = [...tracks];
    const [m] = next.splice(from, 1);
    next.splice(to, 0, m);
    setTracks(next);
  };

  const removeTrack = (id) => setTracks(ts => ts.filter(t => t.id !== id));
  const updateTrack = (id, patch) => setTracks(ts => ts.map(t => t.id === id ? { ...t, ...patch } : t));
  const updateTransition = (key, patch) => setTransitions(t => ({ ...t, [key]: { ...t[key], ...patch } }));

  return (
    <div style={{ flex: 1, display: 'flex', minHeight: 0, background: 'var(--c-ink-1)' }}>
      {/* Main timeline */}
      <section style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
        <header style={{
          display: 'flex', alignItems: 'center', gap: 14,
          padding: '14px 20px', borderBottom: '1px solid var(--stroke-1)',
          background: 'linear-gradient(180deg, var(--c-ink-2), var(--c-ink-1))',
        }}>
          <div style={{ flex: 1 }}>
            <div className="overline" style={{ fontSize: 9 }}>セットリスト / SETLIST</div>
            <div style={{ display: 'flex', alignItems: 'baseline', gap: 10, marginTop: 2 }}>
              <h2 style={{ fontSize: 20, fontWeight: 700, letterSpacing: 'var(--tracking-tight)', color: 'var(--fg-1)', margin: 0 }}>
                Shibuya Afterhours
              </h2>
              <span style={{ fontSize: 12, color: 'var(--fg-4)' }}>2026.04.27 · Club Conductor</span>
            </div>
          </div>
          <button className="btn-xs"><i data-lucide="upload" style={{ width: 11, height: 11 }}/> LOAD .CSET</button>
          <button className="btn-xs"><i data-lucide="download" style={{ width: 11, height: 11 }}/> EXPORT .CSET</button>
          <button className="btn-xs accent"><i data-lucide="play-circle" style={{ width: 11, height: 11 }}/> REHEARSE</button>
        </header>

        <div style={{ flex: 1, overflowY: 'auto', padding: '16px 20px' }}>
          {tracks.map((track, i) => (
            <React.Fragment key={track.id}>
              <SetlistTrackBlock
                track={track} index={i}
                isPlaying={i === playingIdx}
                selected={track.id === selectedId}
                onSelect={() => setSelectedId(track.id)}
                onRemove={() => removeTrack(track.id)}
                onChangeCue={(f, v) => updateTrack(track.id, { [f]: v })}
              />
              {i < tracks.length - 1 && (
                <SetlistTransitionBlock
                  transition={transitions[`${track.id}-${tracks[i+1].id}`]}
                  fromTrack={track}
                  toTrack={tracks[i+1]}
                  onChangeTemplate={(name, bars) => updateTransition(`${track.id}-${tracks[i+1].id}`, { template: name, bars })}
                  onChangeTempoMode={v => updateTransition(`${track.id}-${tracks[i+1].id}`, { tempoMode: v })}
                />
              )}
            </React.Fragment>
          ))}

          {/* Add track drop zone */}
          <div style={{
            marginTop: 12, padding: '20px 14px',
            border: '1px dashed var(--stroke-2)', borderRadius: 10,
            display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8,
            color: 'var(--fg-5)', fontSize: 12,
            background: 'rgba(79,227,178,0.02)',
          }}>
            <i data-lucide="plus-circle" style={{ width: 14, height: 14 }}/>
            ライブラリからトラックをドロップ / Drop tracks from library
          </div>
        </div>
      </section>

      <SetlistOverview tracks={tracks}/>
    </div>
  );
}

Object.assign(window, { Setlist });
