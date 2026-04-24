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
}

export interface MixerSnapshot {
  crossfader: number;
  master_volume: number;
  deck_a: DeckSnapshot;
  deck_b: DeckSnapshot;
}
