// Rust 側 conduction_app::commands::HotCueDto と同じ。

export interface HotCueDto {
  slot: number;       // 1..=8
  position_sec: number;
}
