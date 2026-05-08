import { invoke } from "@tauri-apps/api/core";

import type { BeatDto } from "@/types/beat";
import type { DeckId, MixerSnapshot } from "@/types/mixer";
import type { TrackSummary } from "@/types/track";
import type { WaveformPreview } from "@/types/waveform";

// すべての invoke を console.debug でログ。問題切り分け中。
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  // eslint-disable-next-line no-console
  console.debug("[ipc] →", cmd, args ?? {});
  try {
    const r = await invoke<T>(cmd, args);
    // eslint-disable-next-line no-console
    console.debug("[ipc] ←", cmd, "ok");
    return r;
  } catch (e) {
    // eslint-disable-next-line no-console
    console.error("[ipc] ✗", cmd, e);
    throw e;
  }
}

export const ipc = {
  // --- Mixer / Deck ---
  loadTrack(deck: DeckId, path: string) {
    return call<void>("load_track", { deck, path });
  },
  play(deck: DeckId) {
    return call<void>("play", { deck });
  },
  pause(deck: DeckId) {
    return call<void>("pause", { deck });
  },
  stop(deck: DeckId) {
    return call<void>("stop", { deck });
  },
  seek(deck: DeckId, positionSec: number) {
    return call<void>("seek_deck", { deck, positionSec });
  },
  setCrossfader(position: number) {
    return call<void>("set_crossfader", { position });
  },
  setChannelVolume(deck: DeckId, volume: number) {
    return call<void>("set_channel_volume", { deck, volume });
  },
  setMasterVolume(volume: number) {
    return call<void>("set_master_volume", { volume });
  },
  setTempoAdjust(deck: DeckId, adjust: number) {
    return call<void>("set_tempo_adjust", { deck, adjust });
  },
  setTempoRange(deck: DeckId, percent: 6 | 10 | 16) {
    return call<void>("set_tempo_range", { deck, percent });
  },
  getStatus() {
    return call<MixerSnapshot>("get_status");
  },

  // --- Library ---
  importTrack(path: string) {
    return call<TrackSummary>("import_track", { path });
  },
  listTracks() {
    return call<TrackSummary[]>("list_tracks");
  },
  deleteTrack(id: string) {
    return call<void>("delete_track", { id });
  },

  // --- Analysis / Waveform ---
  analyzeTrack(id: string) {
    return call<WaveformPreview>("analyze_track", { id });
  },
  getWaveform(id: string) {
    return call<WaveformPreview | null>("get_waveform", { id });
  },
  getTrackBeats(id: string) {
    return call<BeatDto[]>("get_track_beats", { id });
  },
};
