import type { DeckId, EqBand } from "@/types/mixer";

/** Wire contract shared with conduction-midi::ControllerAction. Absolute values are 0..1. */
export type ControllerAction =
  | { action: "play_pause" | "sync" | "headphone_cue" | "load_selected"; deck: DeckId }
  | { action: "cue"; deck: DeckId; pressed: boolean }
  | { action: "jog_touch"; deck: DeckId; touched: boolean }
  | { action: "jog"; deck: DeckId; delta: number }
  | { action: "nudge"; deck: DeckId; amount: number }
  | { action: "tempo" | "filter" | "fader"; deck: DeckId; value: number }
  | { action: "eq"; deck: DeckId; band: EqBand; value: number }
  | { action: "master" | "crossfader" | "headphone_volume" | "headphone_mix"; value: number }
  | { action: "hot_cue"; deck: DeckId; slot: number; operation: "trigger_or_set" | "set" | "clear" }
  | { action: "loop"; deck: DeckId; operation: "in" | "out" | "toggle" | "exit" | "halve" | "double" | "reloop" }
  | { action: "browse"; delta: number }
  | { action: "fx"; deck: DeckId; parameter: string; value: number };

export interface AudioOutputConfig {
  device_name: string | null; cue_device_name: string | null; sample_rate: number | null;
  buffer_frames?: number | null;
  mode: "internal" | "external";
  /** Zero-based first channel of the stereo pair, displayed as 1 / 2 etc. */
  main_pair: number; cue_pair: number | null; deck_a_pair: number; deck_b_pair: number;
  headphone_mix: number; headphone_volume: number;
}

export interface AudioOutputStatus {
  sample_rate: number; output_channels: number; elapsed_frames: number; underruns: number;
  device_lost: boolean; error: string | null; estimated_latency_ms: number;
  peak_main: [number, number]; peak_cue: [number, number]; peak_decks: [[number, number], [number, number]];
}
export interface AudioOutputDescriptor { name: string; is_default: boolean; max_output_channels: number; sample_rates: number[] }

export interface LinkConfig {
  enabled: boolean; interface_ip: string; broadcast_ip: string; mac_address: number[];
  source_deck: DeckId; latency_ms: number; library_enabled: boolean; preferred_player: number | null;
}
export interface LinkInterface { name: string; ip: string; broadcast: string; mac_address: number[] }
export interface LinkDevice {
  device_number: number; name: string; ip: string; kind: number; bpm: number | null;
  playing: boolean; synced: boolean; master: boolean; beat: number; track_id: number | null; last_seen_micros: number;
}
export interface LinkStatus {
  running: boolean; hardware_verified: boolean; capability: string;
  player_number: number | null; source_number: number; master_number: number | null;
  devices: LinkDevice[]; library_tracks: number; last_error: string | null;
}

export type MidiMessage = { kind: "note" | "control_change"; channel: number; number: number } | { kind: "pitch_bend"; channel: number };
export type MidiControl = { control: string; deck?: DeckId; band?: string; slot?: number; operation?: string; parameter?: string };
export type MidiInputMode = { mode: "button" | "gate" | "trigger" | "absolute7" | "pitch_bend14" }
  | { mode: "absolute14"; lsb: number }
  | { mode: "relative"; encoding: "offset64" | "twos_complement" | "sign_magnitude"; step: number };
export interface MidiBinding { message: MidiMessage; control: MidiControl; mode: MidiInputMode; pickup: boolean }
export interface MidiConfig { input_port: string; output_port: string | null; profile: string; deck: DeckId | null; bindings: MidiBinding[] }
export interface MidiConnection { config: MidiConfig; profile_name: string; connected: boolean; error: string | null }
export interface MidiStatus { connections: MidiConnection[]; dropped_messages: number; error: string | null }
export interface MidiDevices { inputs: { id: string; name: string }[]; outputs: { id: string; name: string }[] }
export interface MidiProfile {
  id: string; name: string; experimental: boolean; sources: string[]; warnings: string[];
  bindings: MidiBinding[];
  leds: { message: MidiMessage; control: MidiControl; on_value: number; off_value: number }[];
}
export interface PerformanceBrowser { query: string; selected_track_id: string | null; playlist_id: string | null }
export interface PerformanceStatus {
  audio: AudioOutputStatus; audio_config: AudioOutputConfig;
  link: LinkStatus; link_config: LinkConfig; midi: MidiStatus;
  browser: PerformanceBrowser; last_error: string | null; library_preparing: boolean;
  catalog_issues: { track_id: string; title: string; message: string }[];
}
