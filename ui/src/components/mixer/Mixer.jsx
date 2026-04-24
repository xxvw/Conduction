// Mixer.jsx — Compact horizontal mixer strip between decks
const { useState: useStateMx } = React;

function Knob({ value = 0.5, color = 'var(--fg-3)', label, size = 38 }) {
  const deg = -135 + value * 270;
  const indicator = size / 2 - 5;
  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 3 }}>
      <div style={{
        width: size, height: size, borderRadius: '50%',
        background: 'conic-gradient(from 180deg, #3B4453, #1A1E26, #3B4453, #1A1E26, #3B4453)',
        padding: 2,
        boxShadow: '0 4px 10px rgba(0,0,0,0.55), 0 0 0 0.5px rgba(0,0,0,0.6), inset 0 1px 0 rgba(255,255,255,0.08)',
        position: 'relative',
      }}>
        <div style={{
          width: '100%', height: '100%', borderRadius: '50%',
          background: 'radial-gradient(circle at 30% 25%, #2D3440, #13161C 70%)',
          boxShadow: 'inset 0 1px 2px rgba(255,255,255,0.06), inset 0 -1px 2px rgba(0,0,0,0.5)',
          position: 'relative',
        }}>
          <div style={{
            position: 'absolute', left: '50%', top: 3,
            width: 2, height: 9, background: color, borderRadius: 2,
            transform: `translateX(-50%) rotate(${deg}deg)`,
            transformOrigin: `50% ${indicator}px`,
            boxShadow: `0 0 4px ${color}`,
          }}/>
        </div>
      </div>
      <div style={{ fontSize: 8, fontWeight: 700, letterSpacing: '0.18em', color: 'var(--fg-5)' }}>{label}</div>
    </div>
  );
}

function Fader({ value = 0.7, color = 'var(--c-accent)', height = 100 }) {
  return (
    <div style={{
      width: 26, height, background: 'linear-gradient(180deg, var(--c-ink-0), var(--c-ink-1))',
      border: '1px solid var(--stroke-1)', borderRadius: 5, padding: 3,
      position: 'relative', boxShadow: 'inset 0 2px 6px rgba(0,0,0,0.6), 0 0 0 0.5px rgba(0,0,0,0.4)',
    }}>
      <div style={{ position: 'absolute', left: 3, right: 3, top: 3, bottom: 3, display: 'flex', flexDirection: 'column-reverse', gap: 1.5 }}>
        {Array.from({length: 10}).map((_, i) => {
          const active = (i / 10) < value;
          return <div key={i} style={{ flex: 1, background: active ? `linear-gradient(180deg, ${color}, ${shadeDV(color === 'var(--c-accent)' ? '#4FE3B2' : color, -30)})` : 'rgba(255,255,255,0.03)', borderRadius: 1, boxShadow: active && i > 7 ? `0 0 3px ${color}` : 'none' }} />;
        })}
      </div>
      <div style={{
        position: 'absolute', left: '50%', bottom: `calc(${value * 100}% - 7px)`,
        width: 20, height: 12,
        background: 'linear-gradient(180deg, #828DA2, #3B4453)',
        borderRadius: 3, transform: 'translateX(-50%)',
        boxShadow: '0 2px 4px rgba(0,0,0,0.6), inset 0 1px 0 rgba(255,255,255,0.25), inset 0 -1px 0 rgba(0,0,0,0.3)',
      }}>
        <div style={{ position: 'absolute', left: 3, right: 3, top: 5, height: 1, background: 'rgba(0,0,0,0.5)' }}/>
      </div>
    </div>
  );
}

function Mixer({ crossfade = 0.5, setCrossfade }) {
  return (
    <div style={{
      background: 'linear-gradient(180deg, var(--c-ink-3), var(--c-ink-2))',
      border: '1px solid var(--stroke-1)',
      borderRadius: 14, padding: '10px 16px',
      boxShadow: 'var(--shadow-2), inset 0 1px 0 rgba(255,255,255,0.05)',
      display: 'flex', alignItems: 'center', gap: 18,
    }}>
      <div style={{ fontSize: 8, fontWeight: 700, letterSpacing: '0.2em', color: 'var(--fg-5)', width: 48 }}>MIXER</div>

      {/* EQ Deck A */}
      <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
        <Knob value={0.5} color="var(--c-deck-a)" label="HI" />
        <Knob value={0.5} color="var(--c-deck-a)" label="MID" />
        <Knob value={0.5} color="var(--c-deck-a)" label="LOW" />
        <Knob value={0.7} color="var(--c-deck-a)" label="GAIN" />
      </div>
      <Fader value={0.82} color="#4FE3B2" height={70} />
      <div style={{ width: 1, height: 70, background: 'linear-gradient(180deg, transparent, var(--stroke-1), transparent)' }} />
      <Fader value={0.62} color="#E8915A" height={70} />
      {/* EQ Deck B */}
      <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
        <Knob value={0.5} color="var(--c-deck-b)" label="HI" />
        <Knob value={0.5} color="var(--c-deck-b)" label="MID" />
        <Knob value={0.3} color="var(--c-deck-b)" label="LOW" />
        <Knob value={0.6} color="var(--c-deck-b)" label="GAIN" />
      </div>

      <div style={{ flex: 1 }} />

      {/* Crossfader */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4, alignItems: 'stretch', width: 260 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between' }}>
          <span style={{ fontSize: 8, fontWeight: 700, letterSpacing: '0.2em', color: 'var(--c-deck-a)' }}>A</span>
          <span style={{ fontSize: 8, fontWeight: 700, letterSpacing: '0.2em', color: 'var(--fg-5)' }}>CROSSFADER</span>
          <span style={{ fontSize: 8, fontWeight: 700, letterSpacing: '0.2em', color: 'var(--c-deck-b)' }}>B</span>
        </div>
        <div style={{
          height: 28, background: 'linear-gradient(180deg, var(--c-ink-0), var(--c-ink-1))',
          border: '1px solid var(--stroke-1)', borderRadius: 5, padding: 3,
          position: 'relative', boxShadow: 'inset 0 2px 6px rgba(0,0,0,0.6)', cursor: 'pointer',
        }} onClick={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          setCrossfade && setCrossfade(Math.max(0, Math.min(1, (e.clientX - r.left) / r.width)));
        }}>
          <div style={{ position: 'absolute', left: 3, right: 3, top: '50%', height: 1, background: 'linear-gradient(90deg, transparent, rgba(255,255,255,0.08) 20%, rgba(255,255,255,0.08) 80%, transparent)' }}/>
          <div style={{ position: 'absolute', left: '50%', top: 3, bottom: 3, width: 1, background: 'rgba(255,255,255,0.1)', transform: 'translateX(-50%)' }}/>
          <div style={{
            position: 'absolute', top: 3, bottom: 3, left: `calc(${crossfade * 100}% - 8px)`, width: 16,
            background: 'linear-gradient(180deg, #828DA2, #3B4453)',
            borderRadius: 3,
            boxShadow: '0 2px 4px rgba(0,0,0,0.6), inset 0 1px 0 rgba(255,255,255,0.25), inset 0 -1px 0 rgba(0,0,0,0.3)',
          }}>
            <div style={{ position: 'absolute', left: 3, right: 3, top: '50%', height: 1, background: 'rgba(0,0,0,0.5)', transform: 'translateY(-50%)' }}/>
          </div>
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { Mixer, Knob, Fader });
