import { invoke } from "@tauri-apps/api/core";

import type { DeckId, MixerSnapshot } from "@/types/mixer";
import type { TrackSummary } from "@/types/track";

export const ipc = {
  // --- Mixer / Deck ---
  loadTrack(deck: DeckId, path: string) {
    return invoke<void>("load_track", { deck, path });
  },
  play(deck: DeckId) {
    return invoke<void>("play", { deck });
  },
  pause(deck: DeckId) {
    return invoke<void>("pause", { deck });
  },
  stop(deck: DeckId) {
    return invoke<void>("stop", { deck });
  },
  setCrossfader(position: number) {
    return invoke<void>("set_crossfader", { position });
  },
  setChannelVolume(deck: DeckId, volume: number) {
    return invoke<void>("set_channel_volume", { deck, volume });
  },
  setMasterVolume(volume: number) {
    return invoke<void>("set_master_volume", { volume });
  },
  setTempoAdjust(deck: DeckId, adjust: number) {
    return invoke<void>("set_tempo_adjust", { deck, adjust });
  },
  setTempoRange(deck: DeckId, percent: 6 | 10 | 16) {
    return invoke<void>("set_tempo_range", { deck, percent });
  },
  getStatus() {
    return invoke<MixerSnapshot>("get_status");
  },

  // --- Library ---
  importTrack(path: string) {
    return invoke<TrackSummary>("import_track", { path });
  },
  listTracks() {
    return invoke<TrackSummary[]>("list_tracks");
  },
  deleteTrack(id: string) {
    return invoke<void>("delete_track", { id });
  },
};
