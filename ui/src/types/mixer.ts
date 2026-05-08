// Rust 側の conduction-app::audio_engine::{DeckSnapshot, MixerSnapshot} と同じ形。

export type DeckId = "A" | "B";

export type DeckState = "idle" | "loaded" | "paused" | "play";

export interface DeckSnapshot {
  id: DeckId;
  state: DeckState;
  loaded_path: string | null;
  channel_volume: number;
  effective_volume: number;
  tempo_range_percent: 6 | 10 | 16;
  tempo_adjust: number;
  playback_speed: number;
  position_sec: number;
  duration_sec: number | null;
  loop_start_sec: number | null;
  loop_end_sec: number | null;
  loop_active: boolean;
  eq_low_db: number;
  eq_mid_db: number;
  eq_high_db: number;
  filter: number;
  echo_wet: number;
  echo_time_ms: number;
  echo_feedback: number;
  reverb_wet: number;
  reverb_room: number;
  cue_send: number;
  has_cue_output: boolean;
}

export type EqBand = "low" | "mid" | "high";

export interface MixerSnapshot {
  crossfader: number;
  master_volume: number;
  deck_a: DeckSnapshot;
  deck_b: DeckSnapshot;
}
