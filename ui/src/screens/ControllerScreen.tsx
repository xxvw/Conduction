import { useCallback, useMemo, useState } from "react";
import { ControllerDeck } from "@/components/controller/ControllerDeck";
import { ControllerMixer } from "@/components/controller/ControllerMixer";
import { ControllerLibrary } from "@/components/controller/ControllerLibrary";
import { ControllerConnections } from "@/components/controller/ControllerConnections";
import { usePerformance } from "@/hooks/usePerformance";
import { performanceIpc } from "@/lib/performance";
import type { DeckId, MixerSnapshot } from "@/types/mixer";
import type { TrackSummary } from "@/types/track";
import "./ControllerScreen.css";

interface ControllerScreenProps {
  status: MixerSnapshot | null;
  tracks: TrackSummary[];
  onLoadToDeck: (deck: DeckId, path: string) => Promise<void> | void;
  onSelectDeck?: (deck: DeckId) => void;
}

export function ControllerScreen({ status, tracks, onLoadToDeck, onSelectDeck }: ControllerScreenProps) {
  const performance = usePerformance();
  const [error, setError] = useState<string | null>(null);
  const [connections, setConnections] = useState<"audio" | "link" | "midi" | null>(null);
  const report = useCallback((message: string) => setError(message), []);
  const tracksByPath = useMemo(() => new Map(tracks.map((track) => [track.path, track])), [tracks]);
  const link = performance.status?.link;
  const midi = performance.status?.midi;
  const audio = status?.audio;
  const config = status?.audio_config ?? performance.status?.audio_config;
  const external = config?.mode === "external";
  const connectedMidi = midi?.connections.filter((connection) => connection.connected).length ?? 0;
  const visibleError = error ?? performance.error ?? status?.output_error ?? performance.status?.last_error ?? link?.last_error ?? midi?.error;
  const route = config ? external
    ? `A ${config.deck_a_pair + 1}/${config.deck_a_pair + 2} · B ${config.deck_b_pair + 1}/${config.deck_b_pair + 2}`
    : `MAIN ${config.main_pair + 1}/${config.main_pair + 2}${config.cue_pair == null && !config.cue_device_name ? "" : ` · CUE ${(config.cue_pair ?? 0) + 1}/${(config.cue_pair ?? 0) + 2}`}`
    : "Waiting for audio engine";

  return <div className="controller-screen">
    <header className="ctl-topbar">
      <div className="ctl-title"><span className="ctl-eyebrow">PERFORMANCE WORKSPACE</span><h1>Controller<span>02 DECKS</span></h1></div>
      <div className="ctl-connection-strip">
        <button className="ctl-connection" onClick={() => setConnections("audio")} data-live={Boolean(audio?.sample_rate && !audio.device_lost) || undefined}>
          <span className="ctl-status-light" /><span><strong>{external ? "EXTERNAL MIX" : "AUDIO OUTPUT"}</strong><small>{audio?.device_lost ? "DEVICE DISCONNECTED" : route}</small></span>
        </button>
        <button className="ctl-connection" onClick={() => setConnections("link")} data-live={Boolean(link?.running) || undefined}>
          <span className="ctl-status-light" /><span><strong>PRO DJ LINK <em>EXPERIMENTAL</em></strong><small>{link?.running ? `${link.devices.length} devices · PLAYER ${link.player_number ?? "—"}${link.master_number ? ` · MASTER ${link.master_number}` : ""}` : "OFFLINE · Configure LAN"}</small></span>
        </button>
        <button className="ctl-connection" onClick={() => setConnections("midi")} data-live={connectedMidi > 0 || undefined}>
          <span className="ctl-status-light" /><span><strong>USB MIDI</strong><small>{connectedMidi ? `${connectedMidi} connected` : "No controller connected"}</small></span>
        </button>
      </div>
    </header>

    {visibleError && <div className="ctl-notice" role="alert"><span>{visibleError}</span>{error && <button className="ctl-button" aria-label="Dismiss controller error" onClick={() => setError(null)}>×</button>}</div>}

    {status ? <div className="ctl-console">
      <ControllerDeck snapshot={status.deck_a} track={status.deck_a.loaded_path ? tracksByPath.get(status.deck_a.loaded_path) ?? null : null} dispatch={performanceIpc.perform} onError={report} onSelectDeck={onSelectDeck} />
      <ControllerMixer status={status} external={external} dispatch={performanceIpc.perform} onError={report} />
      <ControllerDeck snapshot={status.deck_b} track={status.deck_b.loaded_path ? tracksByPath.get(status.deck_b.loaded_path) ?? null : null} dispatch={performanceIpc.perform} onError={report} onSelectDeck={onSelectDeck} />
    </div> : <div className="ctl-awaiting" role="status"><span className="ctl-eyebrow">AUDIO ENGINE</span><h2>Connecting to your decks</h2><p>The shared playback engine will appear here when it is ready.</p></div>}

    <ControllerLibrary tracks={tracks} onLoadToDeck={onLoadToDeck} onError={report} browser={performance.status?.browser} onBrowserChange={performanceIpc.setBrowser} />
    <footer className="ctl-footer">
      <span>{config?.device_name ?? "System audio"}{audio?.sample_rate ? ` · ${(audio.sample_rate / 1000).toFixed(1)} kHz · ${audio.output_channels} outputs` : ""}</span>
      <span>{audio ? `${audio.estimated_latency_ms.toFixed(1)} ms · ${audio.underruns} underruns` : "Audio clock unavailable"}</span>
      <span>{performance.status?.library_preparing ? "Preparing Link library…" : link?.running ? `${link.library_tracks} tracks shared` : "Link library offline"}</span>
    </footer>
    {connections && performance.status && status && <ControllerConnections performance={performance.status} mixer={status} initialTab={connections}
      onClose={() => setConnections(null)} onError={report} onRefresh={performance.refresh} />}
  </div>;
}
