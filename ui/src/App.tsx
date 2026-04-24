import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useState } from "react";

import "./App.css";
import { useMixerStatus } from "@/hooks/useMixerStatus";
import { ipc } from "@/lib/ipc";
import { LibraryScreen } from "@/screens/LibraryScreen";
import type { DeckId, DeckSnapshot, MixerSnapshot } from "@/types/mixer";

type Screen = "mix" | "library";

const TEMPO_RANGES: readonly [6, 10, 16] = [6, 10, 16] as const;

export function App() {
  const [screen, setScreen] = useState<Screen>("mix");
  const status = useMixerStatus(100);

  const handleLoadToDeck = useCallback((deck: DeckId, path: string) => {
    void ipc.loadTrack(deck, path);
    setScreen("mix");
  }, []);

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
        </nav>
        <div className="spacer" />
        <MasterSlim volume={status?.master_volume ?? 1.0} />
      </header>

      <main className={screen === "mix" ? "main main-mix" : "main main-library"}>
        {screen === "mix" ? (
          <MixScreen status={status} />
        ) : (
          <LibraryScreen onLoadToDeck={handleLoadToDeck} />
        )}
      </main>
    </div>
  );
}

function MixScreen({ status }: { status: MixerSnapshot | null }) {
  if (!status) return <p className="hint">audio engine connecting…</p>;
  return (
    <>
      <DeckPanel snapshot={status.deck_a} />
      <DeckPanel snapshot={status.deck_b} />
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

function DeckPanel({ snapshot }: { snapshot: DeckSnapshot }) {
  const deck: DeckId = snapshot.id;

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

  return (
    <section className="deck">
      <div className="deck-header">
        <div>
          <div className="deck-label" data-id={deck}>
            DECK {deck}
          </div>
          <div style={{ marginTop: "var(--s-2)" }}>
            <span className="state-badge" data-state={snapshot.state}>
              {snapshot.state}
            </span>
          </div>
        </div>
        <div className="deck-file">
          {snapshot.loaded_path ? (
            <>
              <div>{filename}</div>
              <em>{snapshot.loaded_path}</em>
            </>
          ) : (
            <em>no track loaded</em>
          )}
        </div>
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
            {snapshot.tempo_adjust >= 0 ? "+" : ""}
            {(snapshot.tempo_adjust * snapshot.tempo_range_percent).toFixed(2)}%
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
