import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useState } from "react";

import "./App.css";
import { FxPad } from "@/components/fx/FxPad";
import { HotCuePad } from "@/components/hotcue/HotCuePad";
import { KeyConfigModal } from "@/components/keyconfig/KeyConfigModal";
import { CuePad } from "@/components/cues/CuePad";
import { LoopPad } from "@/components/loop/LoopPad";
import { TransportStatusPanel } from "@/components/override/TransportStatusPanel";
import { MixSuggestion } from "@/components/suggestion/MixSuggestion";
import { TemplateLauncher } from "@/components/templates/TemplateLauncher";
import { PerfHud } from "@/components/perf/PerfHud";
import { WaveformView } from "@/components/waveform/WaveformView";
import { WaveformZoomView } from "@/components/waveform/WaveformZoomView";
import { useBeats } from "@/hooks/useBeats";
import { useCues } from "@/hooks/useCues";
import { useHotCues } from "@/hooks/useHotCues";
import { useInterpolatedPosition } from "@/hooks/useInterpolatedPosition";
import { useKeyBindings } from "@/hooks/useKeyBindings";
import { useMatchCandidates } from "@/hooks/useMatchCandidates";
import { useMixerStatus } from "@/hooks/useMixerStatus";
import { useShortcuts } from "@/hooks/useShortcuts";
import { useTracks } from "@/hooks/useTracks";
import { useWaveform } from "@/hooks/useWaveform";
import { secondsToBeatIndex, snapToNearestBeat } from "@/lib/beats";
import { shortestSemitoneDiff } from "@/lib/keys";
import { ipc } from "@/lib/ipc";
import {
  DEFAULT_ZOOM_SEC,
  ZOOM_LEVELS_SEC,
  type ShortcutAction,
} from "@/lib/keybindings";
import { LibraryScreen } from "@/screens/LibraryScreen";
import { SettingsScreen } from "@/screens/SettingsScreen";
import { YouTubeScreen } from "@/screens/YouTubeScreen";
import type { DeckId, DeckSnapshot, MixerSnapshot } from "@/types/mixer";
import type { TrackSummary } from "@/types/track";

type Screen = "mix" | "library" | "youtube" | "settings";

const TEMPO_RANGES: readonly [6, 10, 16] = [6, 10, 16] as const;

export function App() {
  const [screen, setScreen] = useState<Screen>("mix");
  const [activeDeck, setActiveDeck] = useState<DeckId>("A");
  const [zoomWindowSec, setZoomWindowSec] = useState<number>(DEFAULT_ZOOM_SEC);
  const [keyHelpOpen, setKeyHelpOpen] = useState<boolean>(false);
  const [suggestionDismissed, setSuggestionDismissed] = useState<boolean>(false);
  const [templatePresets, setTemplatePresets] = useState<
    import("@/lib/ipc").TemplatePreset[]
  >([]);

  useEffect(() => {
    let cancelled = false;
    ipc
      .listTemplatePresets()
      .then((p) => {
        if (!cancelled) setTemplatePresets(p);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);
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
  const cuesA = useCues(deckATrackId);
  const cuesB = useCues(deckBTrackId);

  // ----- MixSuggestion (Cue 動的マッチング) -----
  const activeSnapshot = activeDeck === "A" ? status?.deck_a : status?.deck_b;
  const activeTrackSummary = activeSnapshot?.loaded_path
    ? trackByPath.get(activeSnapshot.loaded_path) ?? null
    : null;
  const activeTrackId = activeDeck === "A" ? deckATrackId : deckBTrackId;
  const oppositeDeck: DeckId = activeDeck === "A" ? "B" : "A";

  const matchCandidates = useMatchCandidates({
    bpm: activeTrackSummary
      ? activeTrackSummary.bpm * (activeSnapshot?.playback_speed ?? 1)
      : 0,
    keyCamelot:
      (activeTrackSummary?.bpm ?? 0) > 0 ? activeTrackSummary?.key ?? "" : "",
    energy: activeTrackSummary?.energy ?? 0.5,
    excludeTrackId: activeTrackId,
    enabled: !!activeTrackSummary && !suggestionDismissed,
  });

  // アクティブトラックが変わると dismiss を解除して再提示
  useEffect(() => {
    setSuggestionDismissed(false);
  }, [activeTrackId]);

  const handlePickCandidate = useCallback(
    async (c: import("@/lib/ipc").MatchCandidate) => {
      try {
        await ipc.loadTrack(oppositeDeck, c.track.path);
        const seekSec =
          (c.cue.position_beats * 60) / Math.max(1, c.cue.bpm_at_cue);
        // ロード直後は decoder が立ち上がる僅かな遅延があるので、念のため await を 1 拍置く
        await new Promise((r) => setTimeout(r, 200));
        await ipc.seek(oppositeDeck, seekSec);
        setSuggestionDismissed(true);
      } catch (e) {
        // eslint-disable-next-line no-console
        console.error("pick candidate failed", e);
      }
    },
    [oppositeDeck],
  );

  // Enter で 1 位選択 / Esc で dismiss (要件 §6.5)
  useEffect(() => {
    if (suggestionDismissed || activeTrackSummary == null) return;
    const onKey = (e: KeyboardEvent) => {
      if (keyHelpOpen) return;
      const t = e.target as HTMLElement | null;
      const tag = t?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (e.key === "Enter") {
        const top = matchCandidates[0];
        if (top) {
          e.preventDefault();
          void handlePickCandidate(top);
        }
      } else if (e.key === "Escape") {
        e.preventDefault();
        setSuggestionDismissed(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [
    matchCandidates,
    suggestionDismissed,
    activeTrackSummary,
    keyHelpOpen,
    handlePickCandidate,
  ]);

  // BEAT SYNC: 反対デッキの effective BPM (track BPM × playback_speed) に
  // 自分の playback_speed を合わせる。tempo_range の上限を超えるなら clamp。
  const handleBeatSync = useCallback(
    (deck: DeckId) => {
      if (!status) return;
      const own = deck === "A" ? status.deck_a : status.deck_b;
      const other = deck === "A" ? status.deck_b : status.deck_a;
      const ownTrack = own.loaded_path ? trackByPath.get(own.loaded_path) : null;
      const otherTrack = other.loaded_path
        ? trackByPath.get(other.loaded_path)
        : null;
      if (!ownTrack || !otherTrack || ownTrack.bpm <= 0 || otherTrack.bpm <= 0) {
        return;
      }
      const otherEffBpm = otherTrack.bpm * other.playback_speed;
      const targetSpeed = otherEffBpm / ownTrack.bpm;
      const maxAdjust = own.tempo_range_percent / 100;
      const adjust = Math.max(-1, Math.min(1, (targetSpeed - 1.0) / maxAdjust));
      void ipc.setTempoAdjust(deck, adjust);
    },
    [status, trackByPath],
  );

  // KEY SYNC: 反対デッキの Camelot key に対する最短半音差を pitch_offset に保存。
  // Phase 2 で実音 pitch-shift に効く。Phase 1 では値の保存だけ。
  const handleKeySync = useCallback(
    (deck: DeckId) => {
      if (!status) return;
      const own = deck === "A" ? status.deck_a : status.deck_b;
      const other = deck === "A" ? status.deck_b : status.deck_a;
      const ownTrack = own.loaded_path ? trackByPath.get(own.loaded_path) : null;
      const otherTrack = other.loaded_path
        ? trackByPath.get(other.loaded_path)
        : null;
      if (!ownTrack || !otherTrack) return;
      const diff = shortestSemitoneDiff(ownTrack.key, otherTrack.key);
      if (diff == null) return;
      void ipc.setPitchOffset(deck, diff);
    },
    [status, trackByPath],
  );

  const handleStartTemplate = useCallback(
    (presetId: string, bpm: number) => {
      void ipc.startTemplatePreset(presetId, bpm);
    },
    [],
  );

  const handleAbortTemplate = useCallback(() => {
    void ipc.abortTemplate();
  }, []);

  // Shift+Esc で実行中テンプレートを Abort (要件 §6.7)
  useEffect(() => {
    if (!status?.template) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && e.shiftKey) {
        e.preventDefault();
        if (window.confirm("Abort template? Current parameters will hold.")) {
          handleAbortTemplate();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [status?.template, handleAbortTemplate]);

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

      const beatList = activeDeck === "A" ? beatsA : beatsB;

      // ループ — 設定位置はビートグリッドにスナップ
      if (action === "loop-in") {
        const snapped = snapToNearestBeat(snap.position_sec, beatList);
        void ipc.loopIn(activeDeck, snapped);
        return;
      }
      if (action === "loop-out") {
        const snapped = snapToNearestBeat(snap.position_sec, beatList);
        void ipc.loopOut(activeDeck, snapped);
        return;
      }
      if (action === "loop-toggle") {
        void ipc.loopToggle(activeDeck);
        return;
      }
      if (action === "loop-extend" || action === "loop-shrink") {
        if (snap.loop_start_sec == null || snap.loop_end_sec == null) return;
        const summaryForLoop = trackByPath.get(snap.loaded_path);
        const bpmForLoop = summaryForLoop?.bpm ?? 0;
        if (bpmForLoop <= 0) return; // BPM未推定時は伸縮できない
        const barSec = (60 / bpmForLoop) * 4;
        const sign = action === "loop-extend" ? +1 : -1;
        const newEnd = snap.loop_end_sec + sign * barSec;
        // 最小幅: start から 1/4 拍以上は確保
        const minEnd = snap.loop_start_sec + (60 / bpmForLoop) * 0.25;
        const clampedEnd = Math.max(minEnd, newEnd);
        // ビートグリッドにスナップ
        void ipc.loopOut(activeDeck, snapToNearestBeat(clampedEnd, beatList));
        return;
      }

      // Hot Cue: 1..8 — 保存位置もビートグリッドにスナップ
      if (action.startsWith("hotcue-")) {
        const slot = Number(action.slice("hotcue-".length));
        if (!Number.isFinite(slot) || slot < 1 || slot > 8) return;
        const cuesHandle = activeDeck === "A" ? hotCuesA : hotCuesB;
        if (e.altKey) {
          void cuesHandle.remove(slot);
        } else if (e.shiftKey) {
          const snapped = snapToNearestBeat(snap.position_sec, beatList);
          void cuesHandle.set(slot, snapped);
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
            data-active={screen === "youtube"}
            onClick={() => setScreen("youtube")}
          >
            YouTube
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
        <button
          className="topbar-icon-btn"
          type="button"
          onClick={() => setKeyHelpOpen(true)}
          aria-label="show keyboard shortcuts"
          title="Keyboard Shortcuts"
        >
          ⌨ Keys
        </button>
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
            cuesA={cuesA}
            cuesB={cuesB}
            onBeatSync={handleBeatSync}
            onKeySync={handleKeySync}
            templatePresets={templatePresets}
            activeBpm={
              activeTrackSummary
                ? activeTrackSummary.bpm * (activeSnapshot?.playback_speed ?? 1)
                : 0
            }
            onStartTemplate={handleStartTemplate}
            onAbortTemplate={handleAbortTemplate}
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
        {screen === "youtube" && (
          <YouTubeScreen
            onImported={() => {
              void tracksHandle.refresh();
            }}
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

      <KeyConfigModal
        open={keyHelpOpen}
        onClose={() => setKeyHelpOpen(false)}
        bindings={keyBindings.bindings}
        activeDeck={activeDeck}
        zoomWindowSec={zoomWindowSec}
      />

      <MixSuggestion
        open={
          screen === "mix" &&
          !suggestionDismissed &&
          activeTrackSummary != null
        }
        targetDeck={oppositeDeck}
        candidates={matchCandidates}
        activeStatus={
          activeTrackSummary
            ? {
                bpm:
                  activeTrackSummary.bpm *
                  (activeSnapshot?.playback_speed ?? 1),
                key: activeTrackSummary.key,
              }
            : null
        }
        onPick={(c) => void handlePickCandidate(c)}
        onDismiss={() => setSuggestionDismissed(true)}
      />
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
  cuesA,
  cuesB,
  onBeatSync,
  onKeySync,
  templatePresets,
  activeBpm,
  onStartTemplate,
  onAbortTemplate,
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
  cuesA: ReturnType<typeof useCues>;
  cuesB: ReturnType<typeof useCues>;
  onBeatSync: (deck: DeckId) => void;
  onKeySync: (deck: DeckId) => void;
  templatePresets: import("@/lib/ipc").TemplatePreset[];
  activeBpm: number;
  onStartTemplate: (presetId: string, bpm: number) => void;
  onAbortTemplate: () => void;
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
        cues={cuesA}
        onBeatSync={() => onBeatSync("A")}
        onKeySync={() => onKeySync("A")}
      />
      <DeckPanel
        snapshot={status.deck_b}
        trackByPath={trackByPath}
        isActive={activeDeck === "B"}
        onActivate={() => onSelectDeck("B")}
        zoomWindowSec={zoomWindowSec}
        beats={beatsB}
        hotCues={hotCuesB}
        cues={cuesB}
        onBeatSync={() => onBeatSync("B")}
        onKeySync={() => onKeySync("B")}
      />
      <BusPanel crossfader={status.crossfader} master={status.master_volume} />
      <div className="transport-bar">
        <TemplateLauncher
          presets={templatePresets}
          currentBpm={activeBpm}
          onStart={onStartTemplate}
        />
        <TransportStatusPanel
          status={status.template}
          onAbort={onAbortTemplate}
        />
      </div>
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
  cues,
  onBeatSync,
  onKeySync,
}: {
  snapshot: DeckSnapshot;
  trackByPath: Map<string, TrackSummary>;
  isActive: boolean;
  onActivate: () => void;
  zoomWindowSec: number;
  beats: import("@/types/beat").BeatDto[];
  hotCues: ReturnType<typeof useHotCues>;
  cues: ReturnType<typeof useCues>;
  onBeatSync: () => void;
  onKeySync: () => void;
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

  // タイプ付き Cue (Drop / Intro / Breakdown 等) を波形に重ねる用に
  // position_beats → 秒 → ratio に変換。
  const typedCueMarkersSec = useMemo(() => {
    const out: Array<{
      type: import("@/lib/ipc").CueTypeId;
      positionSec: number;
      label: string;
    }> = [];
    for (const c of cues.cues) {
      const idx = Math.floor(c.position_beats);
      const fromBeats = beats[idx]?.position_sec;
      const fromBpm =
        c.bpm_at_cue > 0 ? (c.position_beats * 60) / c.bpm_at_cue : null;
      const sec = fromBeats ?? fromBpm;
      if (sec == null || !Number.isFinite(sec) || sec < 0) continue;
      out.push({
        type: c.cue_type,
        positionSec: sec,
        label: cueShortLabel(c.cue_type),
      });
    }
    return out;
  }, [cues.cues, beats]);

  const typedCueMarkersRatio = useMemo(() => {
    if (!snapshot.duration_sec || snapshot.duration_sec <= 0) return [];
    return typedCueMarkersSec
      .filter((c) => c.positionSec <= snapshot.duration_sec!)
      .map((c) => ({
        type: c.type,
        ratio: c.positionSec / snapshot.duration_sec!,
        label: c.label,
      }));
  }, [typedCueMarkersSec, snapshot.duration_sec]);

  // ループ範囲（IN だけ設定 / 完了 両方を扱う）
  const loopRangeSec = useMemo(() => {
    if (snapshot.loop_start_sec == null) return null;
    return {
      startSec: snapshot.loop_start_sec,
      endSec: snapshot.loop_end_sec,
      active: snapshot.loop_active,
    };
  }, [snapshot.loop_start_sec, snapshot.loop_end_sec, snapshot.loop_active]);

  const loopRangeRatio = useMemo(() => {
    if (!loopRangeSec || !snapshot.duration_sec || snapshot.duration_sec <= 0) return null;
    return {
      startRatio: loopRangeSec.startSec / snapshot.duration_sec,
      endRatio:
        loopRangeSec.endSec != null ? loopRangeSec.endSec / snapshot.duration_sec : null,
      active: loopRangeSec.active,
    };
  }, [loopRangeSec, snapshot.duration_sec]);

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
            <button
              className="cue-btn"
              data-active={snapshot.cue_send > 0.5 || undefined}
              data-disabled={!snapshot.has_cue_output || undefined}
              disabled={!snapshot.has_cue_output}
              title={
                snapshot.has_cue_output
                  ? "PFL: Send this deck to Cue (headphone)"
                  : "Cue output not configured (Settings → Audio output → CUE)"
              }
              onClick={() =>
                void ipc.setCueSend(deck, snapshot.cue_send > 0.5 ? 0 : 1)
              }
            >
              CUE
            </button>
            {baseBpm > 0 && (
              <span className="deck-bpm-readout" data-id={deck}>
                <span className="deck-bpm-value">{effectiveBpm.toFixed(2)}</span>
                <span className="deck-bpm-unit">BPM</span>
                {Math.abs(snapshot.playback_speed - 1.0) > 1e-4 && (
                  <span className="deck-bpm-base">({baseBpm.toFixed(2)})</span>
                )}
              </span>
            )}
            {loopRangeSec && (
              <span
                className="loop-badge"
                data-active={loopRangeSec.active || undefined}
                data-armed={loopRangeSec.endSec == null || undefined}
                title={
                  loopRangeSec.endSec == null
                    ? `Loop IN @ ${loopRangeSec.startSec.toFixed(2)}s — press OUT to close`
                    : "loop range"
                }
              >
                {loopRangeSec.endSec == null ? (
                  <>LOOP IN @ {formatSec(loopRangeSec.startSec)}</>
                ) : (
                  <>
                    LOOP
                    {baseBpm > 0 && (
                      <>
                        {" "}
                        {Math.round(
                          ((loopRangeSec.endSec - loopRangeSec.startSec) * baseBpm) / 60 / 4,
                        ) || ""}
                        bar
                      </>
                    )}
                  </>
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
          cueMarkers={typedCueMarkersRatio}
          loopRangeRatio={loopRangeRatio}
          height={64}
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
          cueMarkers={typedCueMarkersSec}
          loopRange={loopRangeSec}
          positionSec={livePosSec}
          durationSec={snapshot.duration_sec ?? 0}
          windowSec={zoomWindowSec}
          height={56}
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
            className="chip sync-chip"
            aria-pressed={snapshot.key_lock}
            onClick={() => void ipc.setKeyLock(deck, !snapshot.key_lock)}
            title="Master Tempo (keylock): change tempo without affecting pitch"
          >
            MT
          </button>
          <button
            className="chip sync-chip"
            onClick={onBeatSync}
            title="Match this deck's effective BPM to the opposite deck"
          >
            BEAT SYNC
          </button>
          <button
            className="chip sync-chip"
            onClick={onKeySync}
            title="Pitch-shift to match opposite deck's Camelot key (semitone)"
          >
            KEY SYNC
            {snapshot.pitch_offset_semitones !== 0 && (
              <span className="chip-sub">
                {" "}
                {snapshot.pitch_offset_semitones > 0 ? "+" : ""}
                {snapshot.pitch_offset_semitones.toFixed(0)}st
              </span>
            )}
          </button>
          <button
            className="btn"
            style={{ marginLeft: "auto", padding: "var(--s-2) var(--s-3)" }}
            onClick={() => ipc.setTempoAdjust(deck, 0)}
          >
            Reset
          </button>
        </div>
      </div>

      <FxPad deckId={deck} snapshot={snapshot} />

      <LoopPad
        deckId={deck}
        loopState={loopRangeSec}
        bpm={baseBpm}
        currentPositionSec={snapshot.position_sec}
        onIn={() =>
          void ipc.loopIn(deck, snapToNearestBeat(snapshot.position_sec, beats))
        }
        onOut={() =>
          void ipc.loopOut(deck, snapToNearestBeat(snapshot.position_sec, beats))
        }
        onToggle={() => void ipc.loopToggle(deck)}
        onClear={() => void ipc.loopClear(deck)}
        onShrink={() => {
          if (!loopRangeSec || loopRangeSec.endSec == null || baseBpm <= 0) return;
          const barSec = (60 / baseBpm) * 4;
          const minEnd = loopRangeSec.startSec + (60 / baseBpm) * 0.25;
          const newEnd = Math.max(minEnd, loopRangeSec.endSec - barSec);
          void ipc.loopOut(deck, snapToNearestBeat(newEnd, beats));
        }}
        onExtend={() => {
          if (!loopRangeSec || loopRangeSec.endSec == null || baseBpm <= 0) return;
          const barSec = (60 / baseBpm) * 4;
          void ipc.loopOut(deck, snapToNearestBeat(loopRangeSec.endSec + barSec, beats));
        }}
      />

      <HotCuePad
        deckId={deck}
        cues={hotCues.cues}
        currentPositionSec={snapshot.position_sec}
        onJump={(slot) => {
          const cue = hotCues.get(slot);
          if (cue) void ipc.seek(deck, cue.position_sec);
        }}
        onSet={(slot, posSec) => void hotCues.set(slot, snapToNearestBeat(posSec, beats))}
        onDelete={(slot) => void hotCues.remove(slot)}
      />

      <CuePad
        cues={cues.cues}
        enabled={loadedTrack != null}
        onAdd={({ cueType, phraseLength, mixRoles }) => {
          if (!loadedTrack) return;
          const beatIdx = secondsToBeatIndex(livePosSec, beats);
          void cues.insert({
            track_id: loadedTrack.id,
            position_beats: beatIdx,
            cue_type: cueType,
            phrase_length: phraseLength,
            mix_roles: mixRoles,
          });
        }}
        onDelete={(cueId) => void cues.remove(cueId)}
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

function cueShortLabel(t: import("@/lib/ipc").CueTypeId): string {
  switch (t) {
    case "drop":
      return "DROP";
    case "intro_start":
      return "IN";
    case "intro_end":
      return "INTRO";
    case "breakdown":
      return "BRK";
    case "outro":
      return "OUT";
    case "hot_cue":
      return "HOT";
    case "custom_hot_cue":
      return "CUE";
  }
}
