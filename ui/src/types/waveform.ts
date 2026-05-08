// Rust 側の conduction_analysis::WaveformPreview と同じ形。

export interface WaveformPreview {
  sample_count: number;
  low: number[];
  mid: number[];
  high: number[];
}
