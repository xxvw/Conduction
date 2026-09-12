import { useEffect, useRef, useState } from "react";
import { useBeats } from "@/hooks/useBeats";
import { useWaveform } from "@/hooks/useWaveform";
import { useInterpolatedPosition } from "@/hooks/useInterpolatedPosition";
import { WaveformView } from "@/components/waveform/WaveformView";
import { WaveformZoomView } from "@/components/waveform/WaveformZoomView";
import { ipc } from "@/lib/ipc";
import type { DeckId, DeckSnapshot } from "@/types/mixer";
import type { TrackSummary } from "@/types/track";
import type { ControllerAction } from "@/types/performance";
import { HoldButton, formatTime } from "./ControllerControls";

interface Props {
  snapshot: DeckSnapshot; track: TrackSummary | null;
  dispatch: (action: ControllerAction) => Promise<void>; onError: (message: string) => void;
  onSelectDeck?: (deck: DeckId) => void;
}

export function ControllerDeck({ snapshot, track, dispatch, onError, onSelectDeck }: Props) {
  const deck = snapshot.id;
  const trackId = snapshot.track_id ?? track?.id ?? null;
  const beats = useBeats(trackId);
  const waveform = useWaveform(trackId);
  const position = useInterpolatedPosition(snapshot);
  const loaded = Boolean(snapshot.loaded_path) && !snapshot.loading;
  const duration = snapshot.duration_sec ?? 0;
  const bpm = snapshot.bpm ?? (track?.bpm ? track.bpm * snapshot.playback_speed : null);
  const cues = (snapshot.hot_cues ?? []).flatMap((position_sec, slot) => position_sec == null ? [] : [{ slot: slot + 1, position_sec }]);
  const loop = snapshot.loop_start_sec == null ? null : { startSec: snapshot.loop_start_sec, endSec: snapshot.loop_end_sec, active: snapshot.loop_active };
  const [padMode, setPadMode] = useState<"play" | "set" | "clear">("play");
  const run = (promise: Promise<unknown>) => { void promise.catch((reason: unknown) => onError(String(reason))); };
  const action = (value: ControllerAction) => run(dispatch(value));
  const beatNumber = snapshot.beat_position == null ? null : Math.floor(snapshot.beat_position) % 4;

  return <section className="ctl-deck" data-deck={deck} aria-label={`Deck ${deck}`} onPointerDownCapture={() => onSelectDeck?.(deck)} onFocusCapture={() => onSelectDeck?.(deck)}>
    <header className="ctl-deck-header">
      <span className="ctl-deck-letter">{deck}</span>
      <div className="ctl-deck-track">
        <span className="ctl-eyebrow">DECK {deck}<span className="ctl-state">{snapshot.loading ? "LOADING" : snapshot.state.toUpperCase()}</span></span>
        <h2 title={track?.title ?? snapshot.loaded_path ?? undefined}>{track?.title ?? (snapshot.loaded_path?.split(/[\\/]/).pop() || "Load your next track")}</h2>
        <p>{track?.artist || (loaded ? "Unknown artist" : "Select a track from the library below")}</p>
      </div>
      <span className="ctl-key" title="Musical key">{track?.key || "—"}</span>
    </header>

    <div className="ctl-deck-readout">
      <div><span className="ctl-eyebrow">BPM</span><strong>{bpm && Number.isFinite(bpm) ? bpm.toFixed(2) : "—"}</strong></div>
      <div className="ctl-beat-count" aria-label={beatNumber == null ? "No beat grid" : `Beat ${beatNumber + 1} of 4`}>
        {[0, 1, 2, 3].map((beat) => <i key={beat} data-lit={beat === beatNumber || undefined} />)}
        <span>{snapshot.sync_enabled ? (snapshot.sync_lost ? "SYNC LOST" : `${snapshot.sync_source?.toUpperCase() ?? "SYNC"} SYNC`) : "FREE"}</span>
      </div>
      <div className="ctl-remaining"><span className="ctl-eyebrow">REMAIN</span><strong>−{formatTime(duration - snapshot.position_sec)}</strong></div>
    </div>

    <div className="ctl-waveforms" aria-label={`Deck ${deck} waveform`}>
      <WaveformZoomView waveform={waveform} beats={beats} hotCues={cues} positionSec={position} durationSec={duration} height={64} loopRange={loop}
        onSeekSec={loaded ? (seconds) => run(ipc.seek(deck, Math.min(duration, seconds))) : undefined} />
      <WaveformView waveform={waveform} positionRatio={duration > 0 ? position / duration : 0} height={30}
        hotCueRatios={cues.map((cue) => ({ slot: cue.slot, ratio: duration > 0 ? cue.position_sec / duration : 0 }))}
        downbeatRatios={duration > 0 ? beats.filter((beat) => beat.is_downbeat).map((beat) => beat.position_sec / duration) : []}
        loopRangeRatio={loop && duration > 0 ? { startRatio: loop.startSec / duration, endRatio: loop.endSec == null ? null : loop.endSec / duration, active: loop.active } : null}
        onSeekRatio={loaded ? (ratio) => run(ipc.seek(deck, ratio * duration)) : undefined} />
      <input className="ctl-sr-only ctl-seek" type="range" aria-label={`Deck ${deck} seek position in seconds`} min={0} max={Math.max(1, duration)} step={0.1}
        value={Math.min(duration, snapshot.position_sec)} disabled={!loaded} onChange={(event) => run(ipc.seek(deck, Number(event.target.value)))} />
    </div>

    <div className="ctl-deck-performance">
      <div className="ctl-jog-column">
        <Jog snapshot={snapshot} dispatch={dispatch} onError={onError} />
        <div className="ctl-nudge" aria-label={`Deck ${deck} pitch bend`}>
          <HoldButton className="ctl-button" disabled={!loaded} onError={onError} onHold={(pressed) => dispatch({ action: "nudge", deck, amount: pressed ? -0.35 : 0 })} aria-label={`Deck ${deck} nudge slower`}>− BEND</HoldButton>
          <HoldButton className="ctl-button" disabled={!loaded} onError={onError} onHold={(pressed) => dispatch({ action: "nudge", deck, amount: pressed ? 0.35 : 0 })} aria-label={`Deck ${deck} nudge faster`}>BEND +</HoldButton>
        </div>
      </div>
      <div className="ctl-tempo">
        <span className="ctl-eyebrow">TEMPO</span>
        <output>{snapshot.tempo_adjust >= 0 ? "+" : ""}{(snapshot.tempo_adjust * snapshot.tempo_range_percent).toFixed(2)}<small>%</small></output>
        <input type="range" className="ctl-tempo-fader" aria-label={`Deck ${deck} tempo`} min={-1} max={1} step={0.001} value={snapshot.tempo_adjust}
          disabled={!loaded} onChange={(event) => action({ action: "tempo", deck, value: (Number(event.target.value) + 1) / 2 })} />
        <select aria-label={`Deck ${deck} tempo range`} value={snapshot.tempo_range_percent} onChange={(event) => run(ipc.setTempoRange(deck, Number(event.target.value) as 6 | 10 | 16))}>
          {[6, 10, 16].map((range) => <option key={range} value={range}>±{range}%</option>)}
        </select>
        <button className="ctl-button" disabled={!loaded} onClick={() => action({ action: "tempo", deck, value: 0.5 })}>RESET</button>
      </div>
    </div>

    <div className="ctl-transport">
      <HoldButton className="ctl-transport-cue" disabled={!loaded} aria-label={`Deck ${deck} Transport CUE`} aria-pressed={snapshot.cue_pressed}
        onHold={(pressed) => dispatch({ action: "cue", deck, pressed })} onError={onError}><span>TRANSPORT</span>CUE</HoldButton>
      <button className="ctl-play" disabled={!loaded} aria-label={`Deck ${deck} ${snapshot.state === "play" ? "pause" : "play"}`} aria-pressed={snapshot.state === "play"}
        onClick={() => action({ action: "play_pause", deck })}>{snapshot.state === "play" ? "Ⅱ" : "▶"}<span>{snapshot.state === "play" ? "PAUSE" : "PLAY"}</span></button>
      <div className="ctl-transport-options">
        <button className="ctl-button" aria-pressed={snapshot.sync_enabled} disabled={!loaded || beats.length < 2} title={beats.length < 2 ? "Analyze a beat grid to enable sync" : "Follow the Link master, or the other deck"}
          onClick={() => action({ action: "sync", deck })}>SYNC</button>
        <button className="ctl-button" aria-pressed={snapshot.key_lock} onClick={() => run(ipc.setKeyLock(deck, !snapshot.key_lock))}>KEY LOCK</button>
      </div>
    </div>

    <div className="ctl-loop" aria-label={`Deck ${deck} loop controls`}>
      <span className="ctl-eyebrow">LOOP</span>
      {([['in', 'IN'], ['out', 'OUT'], ['halve', '½'], ['double', '×2']] as const).map(([operation, label]) =>
        <button key={operation} className="ctl-button" disabled={!loaded || ((operation === "halve" || operation === "double") && !loop?.endSec)}
          onClick={() => action({ action: "loop", deck, operation })} aria-label={`Deck ${deck} loop ${operation}`}>{label}</button>)}
      <button className="ctl-button ctl-loop-toggle" aria-pressed={snapshot.loop_active} disabled={!loaded || !loop?.endSec}
        onClick={() => action({ action: "loop", deck, operation: "toggle" })}>{snapshot.loop_active ? "EXIT" : "RELOOP"}</button>
    </div>
    <div className="ctl-pads-header"><span className="ctl-eyebrow">HOT CUES</span><div className="ctl-segmented" aria-label={`Deck ${deck} pad mode`}>
      {(["play", "set", "clear"] as const).map((mode) => <button className={mode === padMode ? "is-active" : ""} key={mode} aria-pressed={mode === padMode} onClick={() => setPadMode(mode)}>{mode.toUpperCase()}</button>)}
    </div></div>
    <div className="ctl-pads">
      {Array.from({ length: 8 }, (_, slot) => {
        const cue = snapshot.hot_cues?.[slot] ?? null;
        return <button key={slot} className="ctl-pad" disabled={!loaded || !trackId || (padMode === "clear" && cue == null)} data-filled={cue != null || undefined} data-slot={slot}
          aria-label={`Deck ${deck} Hot Cue ${slot + 1}${cue == null ? " empty, set at current position" : ` at ${formatTime(cue)}`}`}
          title="Play/set empty • Shift to replace • Alt to clear"
          onClick={(event) => action({ action: "hot_cue", deck, slot, operation: event.altKey || padMode === "clear" ? "clear" : event.shiftKey || padMode === "set" ? "set" : "trigger_or_set" })}>
          <strong>{slot + 1}</strong><span>{cue == null ? "+" : formatTime(cue)}</span>
        </button>;
      })}
    </div>
    {snapshot.load_error && <p className="ctl-error" role="alert">{snapshot.load_error}</p>}
  </section>;
}

function Jog({ snapshot, dispatch, onError }: Pick<Props, "snapshot" | "dispatch" | "onError">) {
  const holding = useRef(false);
  const lastAngle = useRef(0);
  const pending = useRef(0);
  const frame = useRef(0);
  const callbacks = useRef({ dispatch, onError, deck: snapshot.id });
  callbacks.current = { dispatch, onError, deck: snapshot.id };
  const queue = useRef(Promise.resolve());
  const send = (action: ControllerAction) => {
    queue.current = queue.current.catch(() => {}).then(() => callbacks.current.dispatch(action)).catch((reason: unknown) => callbacks.current.onError(String(reason)));
  };
  const flush = () => {
    cancelAnimationFrame(frame.current); frame.current = 0;
    if (pending.current !== 0) { send({ action: "jog", deck: snapshot.id, delta: pending.current }); pending.current = 0; }
  };
  const release = () => { if (!holding.current) return; flush(); holding.current = false; send({ action: "jog_touch", deck: snapshot.id, touched: false }); };
  const releaseRef = useRef(release); releaseRef.current = release;
  useEffect(() => {
    const reset = () => releaseRef.current();
    window.addEventListener("blur", reset);
    return () => { window.removeEventListener("blur", reset); reset(); cancelAnimationFrame(frame.current); };
  }, []);
  return <button type="button" className="ctl-jog" disabled={!snapshot.loaded_path || snapshot.loading}
    data-playing={snapshot.state === "play" || undefined} data-touched={snapshot.jog_touched || undefined}
    aria-label={`Deck ${snapshot.id} jog wheel; drag to scratch, arrow keys to seek`} title="Drag to scratch • ← / → to jog"
    onPointerDown={(event) => {
      if (event.button !== 0) return;
      event.preventDefault(); event.currentTarget.focus(); event.currentTarget.setPointerCapture(event.pointerId);
      const rect = event.currentTarget.getBoundingClientRect();
      lastAngle.current = Math.atan2(event.clientY - rect.top - rect.height / 2, event.clientX - rect.left - rect.width / 2);
      holding.current = true; send({ action: "jog_touch", deck: snapshot.id, touched: true });
    }}
    onPointerMove={(event) => {
      if (!holding.current) return;
      const rect = event.currentTarget.getBoundingClientRect();
      const angle = Math.atan2(event.clientY - rect.top - rect.height / 2, event.clientX - rect.left - rect.width / 2);
      let difference = angle - lastAngle.current;
      if (difference > Math.PI) difference -= 2 * Math.PI;
      if (difference < -Math.PI) difference += 2 * Math.PI;
      lastAngle.current = angle;
      pending.current += difference / (2 * Math.PI) * 1.8 / 0.002;
      if (!frame.current) frame.current = requestAnimationFrame(flush);
    }}
    onPointerUp={release} onPointerCancel={release} onLostPointerCapture={release} onBlur={release}
    onKeyDown={(event) => {
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
      event.preventDefault(); event.stopPropagation(); send({ action: "jog", deck: snapshot.id, delta: (event.key === "ArrowLeft" ? -1 : 1) * (event.shiftKey ? 250 : 25) });
    }}>
    <span className="ctl-jog-ring" style={{ transform: `rotate(${(snapshot.position_sec / 1.8) * 360}deg)` }}><i /></span>
    <span className="ctl-jog-center"><span>{snapshot.jog_touched ? "SCRATCH" : "VINYL"}</span><strong>{formatTime(snapshot.position_sec)}</strong><small>DECK {snapshot.id}</small></span>
  </button>;
}
