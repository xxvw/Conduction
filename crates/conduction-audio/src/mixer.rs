use crate::deck::{Deck, DeckId};
use crate::device::OutputDevice;
use crate::error::AudioResult;

/// クロスフェーダーのカーブ形状（要件 6.2）。
///
/// Phase 2a では `Linear` のみ厳密実装。`Smooth` / `Sharp` はプレースホルダとして
/// `Linear` にフォールバックする（Phase 2c で実装）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrossfaderCurve {
    Linear,
    Smooth,
    Sharp,
}

impl Default for CrossfaderCurve {
    fn default() -> Self {
        Self::Linear
    }
}

/// マスターボリューム許容範囲（0.0〜2.0）。
pub const MASTER_VOLUME_MIN: f32 = 0.0;
pub const MASTER_VOLUME_MAX: f32 = 2.0;

/// クロスフェーダー位置の範囲。`-1.0` = Full A、`+1.0` = Full B、`0.0` = Center。
pub const CROSSFADER_MIN: f32 = -1.0;
pub const CROSSFADER_MAX: f32 = 1.0;

/// 2 デッキミキサー。
///
/// 観客用（Main）出力への合流、クロスフェーダー / マスターボリュームの適用を担当する。
/// `Mixer` は `Deck` を所有し、ボリューム状態の更新時に各 `Deck` の実効ボリュームを再計算する。
pub struct Mixer {
    deck_a: Deck,
    deck_b: Deck,

    /// -1.0 (Full A) .. 0.0 (Center) .. 1.0 (Full B)
    crossfader: f32,
    crossfader_curve: CrossfaderCurve,

    master_volume: f32,
}

impl Mixer {
    pub fn new(device: &OutputDevice, cue_device: Option<&OutputDevice>) -> AudioResult<Self> {
        let deck_a = Deck::new(DeckId::A, device, cue_device)?;
        let deck_b = Deck::new(DeckId::B, device, cue_device)?;
        let mut mixer = Self {
            deck_a,
            deck_b,
            crossfader: 0.0,
            crossfader_curve: CrossfaderCurve::Linear,
            master_volume: 1.0,
        };
        mixer.recompute();
        Ok(mixer)
    }

    pub fn set_cue_send(&mut self, id: DeckId, value: f32) {
        self.deck(id).set_cue_send(value);
    }

    pub fn deck_a(&mut self) -> &mut Deck {
        &mut self.deck_a
    }
    pub fn deck_b(&mut self) -> &mut Deck {
        &mut self.deck_b
    }

    pub fn deck(&mut self, id: DeckId) -> &mut Deck {
        match id {
            DeckId::A => &mut self.deck_a,
            DeckId::B => &mut self.deck_b,
        }
    }

    // --- Crossfader ---

    pub fn set_crossfader(&mut self, pos: f32) {
        self.crossfader = pos.clamp(CROSSFADER_MIN, CROSSFADER_MAX);
        self.recompute();
    }

    pub fn crossfader(&self) -> f32 {
        self.crossfader
    }

    pub fn set_crossfader_curve(&mut self, curve: CrossfaderCurve) {
        self.crossfader_curve = curve;
        self.recompute();
    }

    pub fn crossfader_curve(&self) -> CrossfaderCurve {
        self.crossfader_curve
    }

    // --- Master ---

    pub fn set_master_volume(&mut self, v: f32) {
        self.master_volume = v.clamp(MASTER_VOLUME_MIN, MASTER_VOLUME_MAX);
        self.recompute();
    }

    pub fn master_volume(&self) -> f32 {
        self.master_volume
    }

    // --- Channel volume passthroughs（recompute triggering） ---

    pub fn set_channel_volume(&mut self, id: DeckId, v: f32) {
        match id {
            DeckId::A => self.deck_a.set_channel_volume(v),
            DeckId::B => self.deck_b.set_channel_volume(v),
        }
        self.recompute();
    }

    /// 状態変化時に全デッキの実効ボリュームを再計算して反映する。
    fn recompute(&mut self) {
        let (side_a, side_b) = crossfader_sides(self.crossfader, self.crossfader_curve);
        let eff_a = self.deck_a.channel_volume() * side_a * self.master_volume;
        let eff_b = self.deck_b.channel_volume() * side_b * self.master_volume;
        self.deck_a.apply_effective_volume(eff_a);
        self.deck_b.apply_effective_volume(eff_b);
    }
}

/// クロスフェーダー位置 `pos` （-1..1）とカーブから、
/// A / B 両サイドのゲイン係数（0..1）を返す。
///
/// Linear: center (pos=0) で両方 1.0、端で片側が 0.0 になる。
fn crossfader_sides(pos: f32, curve: CrossfaderCurve) -> (f32, f32) {
    let pos = pos.clamp(CROSSFADER_MIN, CROSSFADER_MAX);
    match curve {
        // Phase 2c で独自実装予定。現状は Linear にフォールバック。
        CrossfaderCurve::Linear | CrossfaderCurve::Smooth | CrossfaderCurve::Sharp => {
            let side_a = if pos <= 0.0 { 1.0 } else { 1.0 - pos };
            let side_b = if pos >= 0.0 { 1.0 } else { 1.0 + pos };
            (side_a, side_b)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn linear_center_both_full() {
        let (a, b) = crossfader_sides(0.0, CrossfaderCurve::Linear);
        assert!(approx(a, 1.0) && approx(b, 1.0));
    }

    #[test]
    fn linear_full_a_silences_b() {
        let (a, b) = crossfader_sides(-1.0, CrossfaderCurve::Linear);
        assert!(approx(a, 1.0) && approx(b, 0.0));
    }

    #[test]
    fn linear_full_b_silences_a() {
        let (a, b) = crossfader_sides(1.0, CrossfaderCurve::Linear);
        assert!(approx(a, 0.0) && approx(b, 1.0));
    }

    #[test]
    fn linear_midway() {
        let (a, b) = crossfader_sides(0.5, CrossfaderCurve::Linear);
        assert!(approx(a, 0.5) && approx(b, 1.0));
    }

    #[test]
    fn out_of_range_clamps() {
        let (a, b) = crossfader_sides(5.0, CrossfaderCurve::Linear);
        assert!(approx(a, 0.0) && approx(b, 1.0));
        let (a, b) = crossfader_sides(-5.0, CrossfaderCurve::Linear);
        assert!(approx(a, 1.0) && approx(b, 0.0));
    }
}
