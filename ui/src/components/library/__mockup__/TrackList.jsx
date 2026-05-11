// TrackList.jsx — library table
function TrackList({ tracks, onLoad, nowPlayingId }) {
  return (
    <div style={{
      background: 'var(--bg-panel)', border: '1px solid var(--stroke-1)',
      borderRadius: 12, overflow: 'hidden', display: 'flex', flexDirection: 'column',
      flex: 1, minHeight: 0,
    }}>
      <div style={{ display: 'grid', gridTemplateColumns: '32px 1fr 80px 60px 60px 56px 100px', gap: 10, padding: '10px 14px', background: 'rgba(255,255,255,0.02)', borderBottom: '1px solid var(--stroke-1)' }}>
        <span className="overline" style={{ fontSize: 10 }}>#</span>
        <span className="overline" style={{ fontSize: 10 }}>TITLE / ARTIST</span>
        <span className="overline" style={{ fontSize: 10 }}>BPM</span>
        <span className="overline" style={{ fontSize: 10 }}>KEY</span>
        <span className="overline" style={{ fontSize: 10 }}>TIME</span>
        <span className="overline" style={{ fontSize: 10 }}>GENRE</span>
        <span className="overline" style={{ fontSize: 10 }}>LOAD</span>
      </div>
      <div style={{ flex: 1, overflowY: 'auto' }}>
        {tracks.map((t, i) => {
          const isPlaying = t.id === nowPlayingId;
          return (
            <div key={t.id} className={`track-row ${isPlaying ? 'playing' : ''}`}
                 style={{
                   display: 'grid', gridTemplateColumns: '32px 1fr 80px 60px 60px 56px 100px',
                   gap: 10, padding: '10px 14px', borderBottom: '1px solid var(--stroke-1)',
                   alignItems: 'center',
                   background: isPlaying ? 'rgba(0,245,160,0.05)' : 'transparent',
                 }}>
              <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: isPlaying ? 'var(--c-accent)' : 'var(--fg-5)' }}>{isPlaying ? '▶' : String(i+1).padStart(2,'0')}</span>
              <div style={{ minWidth: 0 }}>
                <div style={{ fontSize: 13, fontWeight: 700, color: 'var(--fg-1)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{t.title}</div>
                <div style={{ fontSize: 11, color: 'var(--fg-4)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{t.artist}</div>
              </div>
              <span className="tabular" style={{ fontSize: 13, color: 'var(--fg-2)' }}>{t.bpm.toFixed(1)}</span>
              <span className="tabular" style={{ fontSize: 13, color: 'var(--c-cue)' }}>{t.key}</span>
              <span className="tabular" style={{ fontSize: 12, color: 'var(--fg-4)' }}>{formatTimeDV(t.duration)}</span>
              <span style={{ fontSize: 11, color: 'var(--fg-4)' }}>{t.genre}</span>
              <div style={{ display: 'flex', gap: 4 }}>
                <button className="load-btn" onClick={() => onLoad && onLoad('A', t)} style={{ color: 'var(--c-deck-a)', borderColor: 'rgba(0,245,160,0.35)' }}>A</button>
                <button className="load-btn" onClick={() => onLoad && onLoad('B', t)} style={{ color: 'var(--c-deck-b)', borderColor: 'rgba(255,122,69,0.35)' }}>B</button>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

Object.assign(window, { TrackList });
