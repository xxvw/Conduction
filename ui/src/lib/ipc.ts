import { invoke } from "@tauri-apps/api/core";

import type { BeatDto } from "@/types/beat";
import type { HotCueDto } from "@/types/hotcue";
import type { DeckId, EqBand, MixerSnapshot } from "@/types/mixer";
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
  loopIn(deck: DeckId, positionSec: number) {
    return call<void>("loop_in", { deck, positionSec });
  },
  loopOut(deck: DeckId, positionSec: number) {
    return call<void>("loop_out", { deck, positionSec });
  },
  loopToggle(deck: DeckId) {
    return call<void>("loop_toggle", { deck });
  },
  loopClear(deck: DeckId) {
    return call<void>("loop_clear", { deck });
  },
  setEq(deck: DeckId, band: EqBand, db: number) {
    return call<void>("set_eq", { deck, band, db });
  },
  setFilter(deck: DeckId, value: number) {
    return call<void>("set_filter", { deck, value });
  },
  setEcho(deck: DeckId, wet: number, timeMs: number, feedback: number) {
    return call<void>("set_echo", { deck, wet, timeMs, feedback });
  },
  setReverb(deck: DeckId, wet: number, room: number) {
    return call<void>("set_reverb", { deck, wet, room });
  },
  setCueSend(deck: DeckId, value: number) {
    return call<void>("set_cue_send", { deck, value });
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
  getResourceStats() {
    return call<ResourceStats>("get_resource_stats");
  },

  // --- Settings (TOML on disk) ---
  getSettings() {
    return call<AppSettings>("get_settings");
  },
  saveSettings(settings: AppSettings) {
    return call<void>("save_settings", { newSettings: settings });
  },
  listAudioDevices() {
    return call<string[]>("list_audio_devices");
  },

  // --- Hot Cues ---
  listHotCues(trackId: string) {
    return call<HotCueDto[]>("list_hot_cues", { trackId });
  },
  setHotCue(trackId: string, slot: number, positionSec: number) {
    return call<void>("set_hot_cue", { trackId, slot, positionSec });
  },
  deleteHotCue(trackId: string, slot: number) {
    return call<void>("delete_hot_cue", { trackId, slot });
  },

  // --- Typed Cues (intro/drop/breakdown 等の構造 Cue) ---
  listCues(trackId: string) {
    return call<CueDto[]>("list_cues", { trackId });
  },
  insertCue(args: InsertCueArgs) {
    return call<CueDto>("insert_cue", { args });
  },
  deleteCue(cueId: string) {
    return call<void>("delete_cue", { cueId });
  },

  // --- Cue 動的マッチング (MixSuggestion 用) ---
  listMatchCandidates(args: MatchQueryArgs) {
    return call<MatchCandidate[]>("list_match_candidates", { args });
  },
  injectDemoCues() {
    return call<number>("inject_demo_cues");
  },

  // --- USB Export (rekordbox-compatible) ---
  exportPreview(destination: string) {
    return call<ExportPreview>("export_preview", { destination });
  },
  exportExecute(destination: string) {
    return call<ExportReport>("export_execute", { destination });
  },

  // --- YouTube (yt-dlp) ---
  ytDlpAvailable() {
    return call<boolean>("yt_dlp_available");
  },
  ytSearch(query: string, limit: number) {
    return call<VideoSearchResult[]>("yt_search", { query, limit });
  },
  ytDownload(url: string, format: AudioFormat, requestId: string) {
    return call<TrackSummary>("yt_download", { url, format, requestId });
  },
};

export interface YtProgressEvent {
  request_id: string;
  raw: string;
  percent: number | null;
  eta_sec: number | null;
  stage: "download" | "postprocess" | "other";
}

export interface YtDoneEvent {
  request_id: string;
  ok: boolean;
}

export type AudioFormat = "m4a" | "mp3" | "opus" | "wav" | "flac";

export type CueTypeId =
  | "hot_cue"
  | "intro_start"
  | "intro_end"
  | "breakdown"
  | "drop"
  | "outro"
  | "custom_hot_cue";

export type MixRoleId = "entry" | "exit";

export interface CueDto {
  id: string;
  track_id: string;
  position_beats: number;
  cue_type: CueTypeId;
  bpm_at_cue: number;
  key_camelot: string;
  energy_level: number;
  phrase_length: number;
  mixable_as: MixRoleId[];
  compatible_energy_min: number;
  compatible_energy_max: number;
  section_start_beats: number | null;
  section_end_beats: number | null;
}

export interface InsertCueArgs {
  track_id: string;
  position_beats: number;
  cue_type: CueTypeId;
  phrase_length: number;
  energy_level?: number;
  mix_roles: MixRoleId[];
  section_end_beats?: number | null;
}

export interface MatchCandidate {
  cue: CueDto;
  track: TrackSummary;
  bpm_score: number;
  key_score: number;
  energy_score: number;
  overall_score: number;
}

export interface MatchQueryArgs {
  bpm: number;
  key_camelot: string;
  energy: number;
  max_bpm_diff?: number;
  limit?: number;
  exclude_track_id?: string;
}

export interface ExportPreview {
  root: string;
  track_count: number;
  estimated_audio_bytes: number;
  tracks_with_beatgrid: number;
  tracks_with_waveform: number;
  total_hot_cues: number;
}

export interface ExportReport {
  tracks_written: number;
  bytes_written: number;
}

export interface VideoSearchResult {
  id: string;
  title: string;
  url: string;
  channel: string;
  duration_sec: number | null;
  thumbnail: string | null;
  view_count: number | null;
}

export interface ResourceStats {
  cpu_percent: number;
  memory_mb: number;
  logical_cores: number;
}

export interface KeybindingEntry {
  action: string;
  key: string;
  label: string;
}

export interface AppSettings {
  keybindings: KeybindingEntry[];
  audio_main_output: string | null;
  audio_cue_output: string | null;
}
