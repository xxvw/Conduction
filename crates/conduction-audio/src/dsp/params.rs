//! デッキ毎の DSP パラメータ。Atomic で共有するため lock-free。
//!
//! UI スレッドが setter で書き換え、オーディオスレッドの DjEffectSource が
//! 数百サンプルごとに getter で読む。f32 を `to_bits()` で u32 化して保持。

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// 全 DSP のパラメータ集合。
pub struct DspParams {
    eq_low_db: AtomicU32,
    eq_mid_db: AtomicU32,
    eq_high_db: AtomicU32,
    filter: AtomicU32,
    echo_wet: AtomicU32,
    echo_time_ms: AtomicU32,
    echo_feedback: AtomicU32,
    reverb_wet: AtomicU32,
    reverb_room: AtomicU32,
}

impl Default for DspParams {
    fn default() -> Self {
        Self {
            eq_low_db: AtomicU32::new(0.0_f32.to_bits()),
            eq_mid_db: AtomicU32::new(0.0_f32.to_bits()),
            eq_high_db: AtomicU32::new(0.0_f32.to_bits()),
            filter: AtomicU32::new(0.0_f32.to_bits()),
            echo_wet: AtomicU32::new(0.0_f32.to_bits()),
            echo_time_ms: AtomicU32::new(375.0_f32.to_bits()),
            echo_feedback: AtomicU32::new(0.4_f32.to_bits()),
            reverb_wet: AtomicU32::new(0.0_f32.to_bits()),
            reverb_room: AtomicU32::new(0.5_f32.to_bits()),
        }
    }
}

macro_rules! atomic_f32_param {
    ($field:ident, $set:ident, $get:ident) => {
        pub fn $set(&self, v: f32) {
            self.$field.store(v.to_bits(), Ordering::Relaxed);
        }
        pub fn $get(&self) -> f32 {
            f32::from_bits(self.$field.load(Ordering::Relaxed))
        }
    };
}

impl DspParams {
    pub fn new_arc() -> Arc<Self> {
        Arc::new(Self::default())
    }

    atomic_f32_param!(eq_low_db, set_eq_low_db, eq_low_db);
    atomic_f32_param!(eq_mid_db, set_eq_mid_db, eq_mid_db);
    atomic_f32_param!(eq_high_db, set_eq_high_db, eq_high_db);
    atomic_f32_param!(filter, set_filter, filter);
    atomic_f32_param!(echo_wet, set_echo_wet, echo_wet);
    atomic_f32_param!(echo_time_ms, set_echo_time_ms, echo_time_ms);
    atomic_f32_param!(echo_feedback, set_echo_feedback, echo_feedback);
    atomic_f32_param!(reverb_wet, set_reverb_wet, reverb_wet);
    atomic_f32_param!(reverb_room, set_reverb_room, reverb_room);
}
