//! テンプレート実行中の各パラメータ Override 状態 (要件 §6.7)。
//!
//! 1 ターゲット = 1 `AutomationMode`。tick 毎に「テンプレート評価値を書くか / 書かないか / 補間値を書くか」を判定する。

use serde::Serialize;

/// UI に流す state 種別。`AutomationMode` の variant 名と 1:1。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AutomationModeKind {
    /// テンプレート未実行 (or 終了済)。書かない。
    Idle,
    /// テンプレートが制御中。評価値をそのまま書く。
    Automated,
    /// 手動操作中。テンプレートの書き込みを skip する。
    Overridden,
    /// Resume 押下後の Glide Back 中。`from_value → template_value` を補間。
    Resuming,
    /// Commit 後。テンプレートの書き込みを skip し、ユーザーが確定した値が残る。
    Committed,
}

#[derive(Debug, Clone)]
pub enum AutomationMode {
    Idle,
    Automated,
    Overridden,
    Resuming {
        from_value: f32,
        started_at_beats: f64,
        duration_beats: f64,
    },
    Committed {
        fixed_value: f32,
    },
}

impl AutomationMode {
    pub fn kind(&self) -> AutomationModeKind {
        match self {
            Self::Idle => AutomationModeKind::Idle,
            Self::Automated => AutomationModeKind::Automated,
            Self::Overridden => AutomationModeKind::Overridden,
            Self::Resuming { .. } => AutomationModeKind::Resuming,
            Self::Committed { .. } => AutomationModeKind::Committed,
        }
    }
}

/// `mode` を見て tick 毎に mixer に書くべき値を返す。`None` なら書かない (= 既存値を保持)。
/// `mode` を `Resuming` から `Automated` に遷移させるなど、状態の自然進行はここで起こす。
pub fn effective_value(
    mode: &mut AutomationMode,
    template_value: f32,
    current_beats: f64,
) -> Option<f32> {
    match mode {
        AutomationMode::Idle => None,
        AutomationMode::Automated => Some(template_value),
        AutomationMode::Overridden => None,
        AutomationMode::Resuming {
            from_value,
            started_at_beats,
            duration_beats,
        } => {
            let dur = duration_beats.max(1e-3);
            let t = ((current_beats - *started_at_beats) / dur).clamp(0.0, 1.0);
            let mixed = (*from_value as f64) * (1.0 - t) + (template_value as f64) * t;
            if t >= 1.0 {
                *mode = AutomationMode::Automated;
            }
            Some(mixed as f32)
        }
        AutomationMode::Committed { fixed_value: _ } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automated_returns_template_value() {
        let mut m = AutomationMode::Automated;
        assert_eq!(effective_value(&mut m, 0.5, 10.0), Some(0.5));
    }

    #[test]
    fn overridden_blocks_template() {
        let mut m = AutomationMode::Overridden;
        assert_eq!(effective_value(&mut m, 0.5, 10.0), None);
    }

    #[test]
    fn resuming_interpolates_then_promotes() {
        let mut m = AutomationMode::Resuming {
            from_value: 0.0,
            started_at_beats: 0.0,
            duration_beats: 4.0,
        };
        // 半分時点 → 0.5 * 1.0 = 0.5 に近い (template_value = 1.0)
        let v = effective_value(&mut m, 1.0, 2.0).unwrap();
        assert!((v - 0.5).abs() < 1e-3);
        assert!(matches!(m, AutomationMode::Resuming { .. }));
        // 完了時点 → 1.0 に到達 + Automated に遷移
        let v = effective_value(&mut m, 1.0, 4.5).unwrap();
        assert!((v - 1.0).abs() < 1e-3);
        assert!(matches!(m, AutomationMode::Automated));
    }
}
