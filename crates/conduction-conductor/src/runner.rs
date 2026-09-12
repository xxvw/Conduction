//! テンプレート実行エンジン (要件 §6.7 の "通常実行" 部分)。
//!
//! 音声出力クロックとその時点の BPM を積分して進行する。
//! Override (4 状態) / Glide Back / Abort 確認ダイアログは別 step で。

use crate::template::{evaluate_track, BuiltInTarget, Template};

#[derive(Debug)]
pub struct TemplateRunner {
    template: Template,
    last_output_seconds: Option<f64>,
    elapsed_beats: f64,
    /// 開始時点の BPM。テンプレート内の `Seconds(_)` を beats に正規化するのに使う。
    bpm: f32,
}

impl TemplateRunner {
    pub fn new(template: Template, bpm: f32) -> Self {
        Self {
            template,
            last_output_seconds: None,
            elapsed_beats: 0.0,
            bpm,
        }
    }

    pub fn template(&self) -> &Template {
        &self.template
    }

    /// 開始時 BPM。音源の BPM が未確定のときのクロック更新にも使える。
    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    /// 出力済み音声の秒数と現在の BPM で進行を更新する。
    ///
    /// 最初の値は基準点として保存する。停止したクロックは進行せず、
    /// デバイス再作成などでクロックが戻った場合は進行を保持して再基準化する。
    /// 不正な値は基準点も含めて変更しない。
    pub fn advance_output_clock(&mut self, output_seconds: f64, current_bpm: f32) {
        if !output_seconds.is_finite()
            || output_seconds < 0.0
            || !current_bpm.is_finite()
            || current_bpm <= 0.0
        {
            return;
        }

        if let Some(previous) = self.last_output_seconds {
            let elapsed_seconds = (output_seconds - previous).max(0.0);
            let next = self.elapsed_beats + elapsed_seconds * f64::from(current_bpm) / 60.0;
            if !next.is_finite() {
                return;
            }
            self.elapsed_beats = next;
        }
        self.last_output_seconds = Some(output_seconds);
    }

    pub fn elapsed_beats(&self) -> f64 {
        self.elapsed_beats
    }

    pub fn progress(&self) -> f32 {
        if self.template.duration_beats <= 0.0 {
            return 1.0;
        }
        (self.elapsed_beats() / self.template.duration_beats).clamp(0.0, 1.0) as f32
    }

    pub fn beats_remaining(&self) -> f64 {
        (self.template.duration_beats - self.elapsed_beats()).max(0.0)
    }

    pub fn is_done(&self) -> bool {
        self.elapsed_beats() >= self.template.duration_beats
    }

    /// 各 AutomationTrack を現時点で評価し `(target, value)` を返す。
    pub fn evaluate_now(&self) -> Vec<(BuiltInTarget, f32)> {
        let beat = self.elapsed_beats().min(self.template.duration_beats);
        self.template
            .tracks
            .iter()
            .filter_map(|t| {
                evaluate_track(t, beat, self.template.duration_beats, self.bpm)
                    .map(|v| (t.target, v))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::Template;

    #[test]
    fn clock_is_anchored_on_first_update_and_integrates_tempo_changes() {
        let mut runner = TemplateRunner::new(Template::long_eq_mix(), 128.0);
        assert_eq!(runner.bpm(), 128.0);
        runner.advance_output_clock(50.0, 120.0);
        assert_eq!(runner.elapsed_beats(), 0.0);
        runner.advance_output_clock(51.0, 120.0);
        assert_eq!(runner.elapsed_beats(), 2.0);
        runner.advance_output_clock(53.0, 90.0);
        assert_eq!(runner.elapsed_beats(), 5.0);
        assert_eq!(runner.progress(), 5.0 / 128.0);
        assert_eq!(runner.beats_remaining(), 123.0);
    }

    #[test]
    fn frozen_output_clock_freezes_progress() {
        let mut runner = TemplateRunner::new(Template::quick_cut(), 120.0);
        runner.advance_output_clock(1.0, 120.0);
        runner.advance_output_clock(2.0, 120.0);
        let progress = runner.progress();
        let values = runner.evaluate_now();
        for bpm in [60.0, 120.0, 180.0] {
            runner.advance_output_clock(2.0, bpm);
            assert_eq!(runner.progress(), progress);
            assert_eq!(runner.evaluate_now(), values);
        }
    }

    #[test]
    fn backwards_clock_reset_preserves_progress_and_reanchors() {
        let mut runner = TemplateRunner::new(Template::quick_cut(), 120.0);
        runner.advance_output_clock(10.0, 120.0);
        runner.advance_output_clock(11.0, 120.0);
        runner.advance_output_clock(0.0, 120.0);
        assert_eq!(runner.elapsed_beats(), 2.0);
        runner.advance_output_clock(1.0, 120.0);
        assert_eq!(runner.elapsed_beats(), 4.0);
    }

    #[test]
    fn invalid_clock_samples_do_not_change_the_baseline() {
        let mut runner = TemplateRunner::new(Template::quick_cut(), 120.0);
        runner.advance_output_clock(10.0, 120.0);
        for (seconds, bpm) in [
            (f64::NAN, 120.0),
            (f64::INFINITY, 120.0),
            (-1.0, 120.0),
            (12.0, f32::NAN),
            (12.0, f32::INFINITY),
            (12.0, 0.0),
            (12.0, -120.0),
            (f64::MAX, f32::MAX),
        ] {
            runner.advance_output_clock(seconds, bpm);
            assert_eq!(runner.elapsed_beats(), 0.0);
        }
        runner.advance_output_clock(11.0, 120.0);
        assert_eq!(runner.elapsed_beats(), 2.0);
    }

    #[test]
    fn evaluation_follows_output_clock_and_holds_at_completion() {
        let mut runner = TemplateRunner::new(Template::quick_cut(), 120.0);
        runner.advance_output_clock(100.0, 120.0);
        assert_eq!(
            runner.evaluate_now(),
            vec![(BuiltInTarget::Crossfader, -1.0)]
        );
        runner.advance_output_clock(104.0, 120.0);
        assert_eq!(
            runner.evaluate_now(),
            vec![(BuiltInTarget::Crossfader, -0.5)]
        );
        assert!(!runner.is_done());
        runner.advance_output_clock(108.0, 120.0);
        assert_eq!(
            runner.evaluate_now(),
            vec![(BuiltInTarget::Crossfader, 1.0)]
        );
        assert!(runner.is_done());
        runner.advance_output_clock(110.0, 120.0);
        assert_eq!(runner.progress(), 1.0);
        assert_eq!(runner.beats_remaining(), 0.0);
        assert_eq!(
            runner.evaluate_now(),
            vec![(BuiltInTarget::Crossfader, 1.0)]
        );
    }
}
