//! テンプレート実行エンジン (要件 §6.7 の "通常実行" 部分)。
//!
//! Phase 4 最小: wall-clock ベースで進行。1 度に 1 つだけ実行可。
//! Override (4 状態) / Glide Back / Abort 確認ダイアログは別 step で。

use std::time::Instant;

use crate::template::{evaluate_track, BuiltInTarget, Template};

#[derive(Debug)]
pub struct TemplateRunner {
    template: Template,
    started_at: Instant,
    /// 開始時点の BPM。テンプレート内の `Seconds(_)` を beats に正規化するのに使う。
    /// 進行は wall-clock ベース (BPM 変動はこの runner では追跡しない)。
    bpm: f32,
}

impl TemplateRunner {
    pub fn new(template: Template, bpm: f32) -> Self {
        Self {
            template,
            started_at: Instant::now(),
            bpm,
        }
    }

    pub fn template(&self) -> &Template {
        &self.template
    }

    pub fn elapsed_beats(&self) -> f64 {
        let elapsed = self.started_at.elapsed().as_secs_f64();
        elapsed * (self.bpm as f64) / 60.0
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
    fn progress_grows_monotonically() {
        let runner = TemplateRunner::new(Template::long_eq_mix(), 128.0);
        let p1 = runner.progress();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let p2 = runner.progress();
        assert!(p2 >= p1);
    }
}
