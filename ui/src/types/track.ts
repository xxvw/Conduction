// Rust 側の conduction_app::library_state::TrackSummary と同じ形。

export interface TrackSummary {
  id: string;
  path: string;
  title: string;
  artist: string;
  album: string;
  genre: string;
  duration_sec: number;
  bpm: number;
  /** Camelot 表記。例: "8A" */
  key: string;
  energy: number;
  beatgrid_verified: boolean;
  analyzed: boolean;
}
