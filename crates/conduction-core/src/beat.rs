use serde::{Deserialize, Serialize};

/// 1 拍のメタデータ。`aubio-rs` のビートトラッキング結果をこの形で保持する。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Beat {
    /// 曲頭からの位置（秒）。
    pub position_sec: f64,
    /// この拍の瞬時 BPM（非線形に変動する曲の記録用、フェーズ3以降で使用）。
    pub instantaneous_bpm: Option<f32>,
    /// ダウンビート（小節頭）かどうか。
    pub is_downbeat: bool,
}

impl Beat {
    pub fn new(position_sec: f64, is_downbeat: bool) -> Self {
        Self {
            position_sec,
            instantaneous_bpm: None,
            is_downbeat,
        }
    }
}
