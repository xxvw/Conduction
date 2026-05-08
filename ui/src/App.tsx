import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useMemo, useState } from "react";

import "./App.css";
import { HotCuePad } from "@/components/hotcue/HotCuePad";
import { KeyConfigBar } from "@/components/keyconfig/KeyConfigBar";
import { PerfHud } from "@/components/perf/PerfHud";
import { WaveformView } from "@/components/waveform/WaveformView";
import { WaveformZoomView } from "@/components/waveform/WaveformZoomView";
import { useBeats } from "@/hooks/useBeats";
import { useHotCues } from "@/hooks/useHotCues";
import { useInterpolatedPosition } from "@/hooks/useInterpolatedPosition";
import { useKeyBindings } from "@/hooks/useKeyBindings";
import { useMixerStatus } from "@/hooks/useMixerStatus";
import { useShortcuts } from "@/hooks/useShortcuts";
import { useTracks } from "@/hooks/useTracks";
import { useWaveform } from "@/hooks/useWaveform";
import { ipc } from "@/lib/ipc";
import {
  DEFAULT_ZOOM_SEC,
  ZOOM_LEVELS_SEC,
  type ShortcutAction,
} from "@/lib/keybindings";
import { LibraryScreen } from "@/screens/LibraryScreen";
import { SettingsScreen } from "@/screens/SettingsScreen";
import type { DeckId, DeckSnapshot, MixerSnapshot } from "@/types/mixer";
import type { TrackSummary } from "@/types/track";

type Screen = "mix" | "library" | "settings";

const TEMPO_RANGES: readonly [6, 10, 16] = [6, 10, 16] as const;

export function App() {
  const [screen, setScreen] = useState<Screen>("mix");
  const [activeDeck, setActiveDeck] = useState<DeckId>("A");
  const [zoomWindowSec, setZoomWindowSec] = useState<number>(DEFAULT_ZOOM_SEC);
  const status = useMixerStatus(100);
  const tracksHandle = useTracks();
  const keyBindings = useKeyBindings();

  const trackByPath = useMemo(() => {
    const m = new Map<string, TrackSummary>();
    for (const t of tracksHandle.tracks) m.set(t.path, t);
    return m;
  }, [tracksHandle.tracks]);

  // 各デッキにロード中のトラックIDを引き当てる（path → TrackSummary 経由）。
  const deckATrackId = useMemo(() => {
    const p = status?.deck_a.loaded_path;
    return p ? trackByPath.get(p)?.id ?? null : null;
  }, [status?.deck_a.loaded_path, trackByPath]);
  const deckBTrackId = useMemo(() => {
    const p = status?.deck_b.loaded_path;
    return p ? trackByPath.get(p)?.id ?? null : null;
  }, [status?.deck_b.loaded_path, trackByPath]);
  const beatsA = useBeats(deckATrackId);
  const beatsB = useBeats(deckBTrackId);
  const hotCuesA = useHotCues(deckATrackId);
  const hotCuesB = useHotCues(deckBTrackId);

  const handleLoadToDeck = useCallback((deck: DeckId, path: string) => {
    void ipc.loadTrack(deck, path);
    setActiveDeck(deck);
    setScreen("mix");
  }, []);

  // ショートカット: シーク + デッキ切替 + Play/Pause
  const handleShortcut = useCallback(
    (action: ShortcutAction, e: KeyboardEvent) => {
      if (action === "focus-deck-a") {
        setActiveDeck("A");
        return;
      }
      if (action === "focus-deck-b") {
        setActiveDeck("B");
        return;
      }
      if (action === "zoom-in") {
        setZoomWindowSec((current) => {
          const idx = ZOOM_LEVELS_SEC.indexOf(current);
          if (idx <= 0) return ZOOM_LEVELS_SEC[0]!;
          return ZOOM_LEVELS_SEC[idx - 1]!;
        });
        return;
      }
      if (action === "zoom-out") {
        setZoomWindowSec((current) => {
          const idx = ZOOM_LEVELS_SEC.indexOf(current);
          if (idx < 0) return DEFAULT_ZOOM_SEC;
          if (idx >= ZOOM_LEVELS_SEC.length - 1) return ZOOM_LEVELS_SEC[ZOOM_LEVELS_SEC.length - 1]!;
          return ZOOM_LEVELS_SEC[idx + 1]!;
        });
        return;
      }
      if (!status) return;
      const snap = activeDeck === "A" ? status.deck_a : status.deck_b;
      if (!snap.loaded_path) return;

      if (action === "play-pause") {
        if (snap.state === "play") void ipc.pause(activeDeck);
        else void ipc.play(activeDeck);
        return;
      }

      // Hot Cue: 1..8
      if (action.startsWith("hotcue-")) {
        const slot = Number(action.slice("hotcue-".length));
        if (!Number.isFinite(slot) || slot < 1 || slot > 8) return;
        const cuesHandle = activeDeck === "A" ? hotCuesA : hotCuesB;
        if (e.altKey) {
          void cuesHandle.remove(slot);
        } else if (e.shiftKey) {
          void cuesHandle.set(slot, snap.position_sec);
        } else {
          const cue = cuesHandle.get(slot);
          if (cue) void ipc.seek(activeDeck, cue.position_sec);
        }
        return;
      }

      const beatOffset =
        action === "seek-back-1" ? -1 :
        action === "seek-fwd-1"  ? +1 :
        action === "seek-back-2" ? -2 :
        action === "seek-fwd-2"  ? +2 :
        action === "seek-back-4" ? -4 :
        action === "seek-fwd-4"  ? +4 : 0;
      if (beatOffset === 0) return;

      const upper = (snap.duration_sec ?? 0) > 0
        ? Math.max(0, (snap.duration_sec ?? 0) - 0.05)
        : Number.POSITIVE_INFINITY;
      const beatList = activeDeck === "A" ? beatsA : beatsB;

      // Shift なし & ビートグリッドあり → 最も近い拍にスナップしてから N 拍分動く
      if (!e.shiftKey && beatList.length > 0) {
        // 二分探索で「現在位置以上の最初の拍」を見つけ、前後どちらが近いか判定
        let lo = 0;
        let hi = beatList.length;
        while (lo < hi) {
          const mid = (lo + hi) >>> 1;
          if (beatList[mid]!.position_sec < snap.position_sec) lo = mid + 1;
          else hi = mid;
        }
        const next = beatList[lo];
        const prev = lo > 0 ? beatList[lo - 1] : undefined;
        let nearestIdx = lo;
        if (next && prev) {
          const dn = Math.abs(next.position_sec - snap.position_sec);
          const dp = Math.abs(prev.position_sec - snap.position_sec);
          nearestIdx = dp <= dn ? lo - 1 : lo;
        } else if (!next && prev) {
          nearestIdx = lo - 1;
        }
        const targetIdx = Math.max(0, Math.min(beatList.length - 1, nearestIdx + beatOffset));
        const target = Math.max(0, Math.min(upper, beatList[targetIdx]!.position_sec));
        void ipc.seek(activeDeck, target);
        return;
      }

      // Shift あり、または beatList が無い場合 → 相対時間（fine seek）
      const summary = trackByPath.get(snap.loaded_path);
      const bpm = summary?.bpm ?? 0;
      const beatSec = bpm > 0 ? 60 / (bpm * snap.playback_speed) : 0.5;
      const target = Math.max(
        0,
        Math.min(upper, snap.position_sec + beatOffset * beatSec),
      );
      void ipc.seek(activeDeck, target);
    },
    [status, activeDeck, trackByPath, beatsA, beatsB, hotCuesA, hotCuesB],
  );

  useShortcuts({ bindings: keyBindings.bindings, onAction: handleShortcut });

  return (
    <div className="app">
      <header className="topbar">
        <span className="brand">Conduction</span>
        <nav className="nav">
          <button
            className="nav-btn"
            data-active={screen === "mix"}
            onClick={() => setScreen("mix")}
          >
            Mix
          </button>
          <button
            className="nav-btn"
            data-active={screen === "library"}
            onClick={() => setScreen("library")}
          >
            Library
          </button>
          <button
            className="nav-btn"
            data-active={screen === "settings"}
            onClick={() => setScreen("settings")}
          >
            Settings
          </button>
        </nav>
        <div className="spacer" />
        <PerfHud />
        <MasterSlim volume={status?.master_volume ?? 1.0} />
      </header>

      <main className={`main main-${screen}`}>
        {screen === "mix" && (
          <MixScreen
            status={status}
            trackByPath={trackByPath}
            activeDeck={activeDeck}
            onSelectDeck={setActiveDeck}
            zoomWindowSec={zoomWindowSec}
            beatsA={beatsA}
            beatsB={beatsB}
            hotCuesA={hotCuesA}
            hotCuesB={hotCuesB}
          />
        )}
        {screen === "library" && (
          <LibraryScreen
            tracks={tracksHandle.tracks}
            loading={tracksHandle.loading}
            error={tracksHandle.error}
            refresh={tracksHandle.refresh}
            onLoadToDeck={handleLoadToDeck}
          />
        )}
        {screen === "settings" && (
          <SettingsScreen
            bindings={keyBindings.bindings}
            setBinding={keyBindings.setBinding}
            reset={keyBindings.reset}
          />
        )}
      </main>

      {screen === "mix" && (
        <KeyConfigBar
          bindings={keyBindings.bindings}
          activeDeck={activeDeck}
          zoomWindowSec={zoomWindowSec}
        />
      )}
    </div>
  );
}

function MixScreen({
  status,
  trackByPath,
  activeDeck,
  onSelectDeck,
  zoomWindowSec,
  beatsA,
  beatsB,
  hotCuesA,
  hotCuesB,
}: {
  status: MixerSnapshot | null;
  trackByPath: Map<string, TrackSummary>;
  activeDeck: DeckId;
  onSelectDeck: (deck: DeckId) => void;
  zoomWindowSec: number;
  beatsA: import("@/types/beat").BeatDto[];
  beatsB: import("@/types/beat").BeatDto[];
  hotCuesA: ReturnType<typeof useHotCues>;
  hotCuesB: ReturnType<typeof useHotCues>;
}) {
  if (!status) return <p className="hint">audio engine connecting…</p>;
  return (
    <>
      <DeckPanel
        snapshot={status.deck_a}
        trackByPath={trackByPath}
        isActive={activeDeck === "A"}
        onActivate={() => onSelectDeck("A")}
        zoomWindowSec={zoomWindowSec}
        beats={beatsA}
        hotCues={hotCuesA}
      />
      <DeckPanel
        snapshot={status.deck_b}
        trackByPath={trackByPath}
        isActive={activeDeck === "B"}
        onActivate={() => onSelectDeck("B")}
        zoomWindowSec={zoomWindowSec}
        beats={beatsB}
        hotCues={hotCuesB}
      />
      <BusPanel crossfader={status.crossfader} master={status.master_volume} />
    </>
  );
}

function MasterSlim({ volume }: { volume: number }) {
  return (
    <label className="master-slim">
      MASTER
      <input
        type="range"
        min={0}
        max={2}
        step={0.01}
        value={volume}
        onChange={(e) => ipc.setMasterVolume(parseFloat(e.target.value))}
      />
      <span className="value">{volume.toFixed(2)}</span>
    </label>
  );
}

function DeckPanel({
  snapshot,
  trackByPath,
  isActive,
  onActivate,
  zoomWindowSec,
  beats,
  hotCues,
}: {
  snapshot: DeckSnapshot;
  trackByPath: Map<string, TrackSummary>;
  isActive: boolean;
  onActivate: () => void;
  zoomWindowSec: number;
  beats: import("@/types/beat").BeatDto[];
  hotCues: ReturnType<typeof useHotCues>;
}) {
  const deck: DeckId = snapshot.id;
  const loadedTrack = snapshot.loaded_path
    ? trackByPath.get(snapshot.loaded_path) ?? null
    : null;
  const waveform = useWaveform(loadedTrack?.id ?? null);

  const baseBpm = loadedTrack?.bpm ?? 0;
  const effectiveBpm = baseBpm > 0 ? baseBpm * snapshot.playback_speed : 0;

  // mixer snapshot は 10 Hz だが、波形カーソルは 60 Hz で動かしたいので補間する。
  const livePosSec = useInterpolatedPosition(snapshot);

  // overview に表示する Hot Cue 比率（duration 基準）
  const hotCueRatios = useMemo(() => {
    if (!snapshot.duration_sec || snapshot.duration_sec <= 0) return [];
    return hotCues.cues
      .filter((c) => c.position_sec >= 0 && c.position_sec <= snapshot.duration_sec!)
      .map((c) => ({ slot: c.slot, ratio: c.position_sec / snapshot.duration_sec! }));
  }, [hotCues.cues, snapshot.duration_sec]);

  const handleLoad = useCallback(async () => {
    const path = await open({
      multiple: false,
      filters: [
        {
          name: "Audio",
          extensions: ["mp3", "wav", "flac", "aac", "ogg", "m4a"],
        },
      ],
    });
    if (typeof path === "string") {
      await ipc.loadTrack(deck, path);
    }
  }, [deck]);

  const togglePlay = useCallback(() => {
    if (snapshot.state === "play") {
      return ipc.pause(deck);
    }
    return ipc.play(deck);
  }, [deck, snapshot.state]);

  const canPlay = snapshot.loaded_path !== null;
  const filename = snapshot.loaded_path?.split("/").pop() ?? "";
  const positionRatio =
    snapshot.duration_sec && snapshot.duration_sec > 0
      ? livePosSec / snapshot.duration_sec
      : 0;

  return (
    <section
      className="deck"
      data-active={isActive}
      onClick={onActivate}
    >
      <div className="deck-header">
        <div>
          <div className="deck-label" data-id={deck}>
            DECK {deck}
          </div>
          <div className="deck-status-row">
            <span className="state-badge" data-state={snapshot.state}>
              {snapshot.state}
            </span>
            {baseBpm > 0 && (
              <span className="deck-bpm-readout" data-id={deck}>
                <span className="deck-bpm-value">{effectiveBpm.toFixed(2)}</span>
                <span className="deck-bpm-unit">BPM</span>
                {Math.abs(snapshot.playback_speed - 1.0) > 1e-4 && (
                  <span className="deck-bpm-base">({baseBpm.toFixed(2)})</span>
                )}
              </span>
            )}
            {loadedTrack && !waveform && (
              <span
                className="analyzing-bar"
                role="progressbar"
                aria-label="analyzing waveform"
                title="analyzing waveform"
              >
                <span className="analyzing-bar-track" />
                <span className="analyzing-bar-label">ANALYZING</span>
              </span>
            )}
          </div>
        </div>
        <div className="deck-file">
          {snapshot.loaded_path ? (
            <>
              <div>{loadedTrack?.title || filename}</div>
              <em>{loadedTrack?.artist || snapshot.loaded_path}</em>
            </>
          ) : (
            <em>no track loaded</em>
          )}
        </div>
      </div>

      <div className="deck-waveform" data-id={deck}>
        <WaveformView
          waveform={waveform}
          positionRatio={positionRatio}
          hotCueRatios={hotCueRatios}
          height={84}
          onSeekRatio={(r) => {
            if (snapshot.duration_sec && snapshot.duration_sec > 0) {
              void ipc.seek(deck, r * snapshot.duration_sec);
            }
          }}
        />
      </div>
      <div className="deck-waveform deck-waveform-zoom" data-id={deck}>
        <WaveformZoomView
          waveform={waveform}
          beats={beats}
          hotCues={hotCues.cues}
          positionSec={livePosSec}
          durationSec={snapshot.duration_sec ?? 0}
          windowSec={zoomWindowSec}
          height={72}
          onSeekSec={(sec) => void ipc.seek(deck, sec)}
        />
      </div>

      <div className="deck-transport">
        <button className="btn" onClick={handleLoad}>
          Load…
        </button>
        <button
          className="btn"
          data-variant="primary"
          onClick={togglePlay}
          disabled={!canPlay}
        >
          {snapshot.state === "play" ? "Pause" : "Play"}
        </button>
        <button className="btn" onClick={() => ipc.stop(deck)} disabled={!canPlay}>
          Stop
        </button>
        <span className="pos">
          {formatSec(snapshot.position_sec)}
          <span className="sep">/</span>
          {snapshot.duration_sec != null ? formatSec(snapshot.duration_sec) : "--:--"}
          <span className="spd">{snapshot.playback_speed.toFixed(3)}x</span>
        </span>
      </div>

      <div className="control">
        <div className="control-label">
          <span>CH VOLUME</span>
          <span className="value">
            {snapshot.channel_volume.toFixed(2)} ({snapshot.effective_volume.toFixed(2)} eff)
          </span>
        </div>
        <input
          type="range"
          min={0}
          max={2}
          step={0.01}
          value={snapshot.channel_volume}
          onChange={(e) => ipc.setChannelVolume(deck, parseFloat(e.target.value))}
        />
      </div>

      <div className="control">
        <div className="control-label">
          <span>TEMPO</span>
          <span className="value">
            <span className="tempo-pct">
              {snapshot.tempo_adjust >= 0 ? "+" : ""}
              {(snapshot.tempo_adjust * snapshot.tempo_range_percent).toFixed(2)}%
            </span>
            {effectiveBpm > 0 && (
              <span className="tempo-bpm">
                {" · "}
                {effectiveBpm.toFixed(2)} BPM
              </span>
            )}
          </span>
        </div>
        <input
          type="range"
          min={-1}
          max={1}
          step={0.001}
          value={snapshot.tempo_adjust}
          onChange={(e) => ipc.setTempoAdjust(deck, parseFloat(e.target.value))}
        />
        <div className="tempo-ranges">
          {TEMPO_RANGES.map((pct) => (
            <button
              key={pct}
              className="chip"
              aria-pressed={snapshot.tempo_range_percent === pct}
              onClick={() => ipc.setTempoRange(deck, pct)}
            >
              ±{pct}%
            </button>
          ))}
          <button
            className="btn"
            style={{ marginLeft: "auto", padding: "var(--s-2) var(--s-3)" }}
            onClick={() => ipc.setTempoAdjust(deck, 0)}
          >
            Reset
          </button>
        </div>
      </div>

      <HotCuePad
        deckId={deck}
        cues={hotCues.cues}
        currentPositionSec={snapshot.position_sec}
        onJump={(slot) => {
          const cue = hotCues.get(slot);
          if (cue) void ipc.seek(deck, cue.position_sec);
        }}
        onSet={(slot, posSec) => void hotCues.set(slot, posSec)}
        onDelete={(slot) => void hotCues.remove(slot)}
      />
    </section>
  );
}

function BusPanel({ crossfader, master }: { crossfader: number; master: number }) {
  return (
    <section className="bus">
      <div className="control">
        <div className="control-label">
          <span>CROSSFADER</span>
          <span className="value">
            {crossfader >= 0 ? "B " : "A "}
            {Math.abs(crossfader).toFixed(2)}
          </span>
        </div>
        <input
          type="range"
          min={-1}
          max={1}
          step={0.001}
          value={crossfader}
          onChange={(e) => ipc.setCrossfader(parseFloat(e.target.value))}
        />
        <button
          className="btn"
          style={{ alignSelf: "flex-end", padding: "var(--s-2) var(--s-4)" }}
          onClick={() => ipc.setCrossfader(0)}
        >
          Center
        </button>
      </div>
      <div className="control">
        <div className="control-label">
          <span>MASTER</span>
          <span className="value">{master.toFixed(2)}</span>
        </div>
        <input
          type="range"
          min={0}
          max={2}
          step={0.01}
          value={master}
          onChange={(e) => ipc.setMasterVolume(parseFloat(e.target.value))}
        />
      </div>
    </section>
  );
}

function formatSec(sec: number): string {
  if (!Number.isFinite(sec) || sec < 0) sec = 0;
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
}
