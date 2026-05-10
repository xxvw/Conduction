//! テンプレートのデータモデル + 補間ロジック (要件 §6.6)。
//!
//! Phase 4 の最小スコープ:
//! - 遷移テンプレート (Transition) のみ。Setlist は別 step。
//! - 内蔵パラメータターゲット (BuiltInTarget) のみ。Custom は別 step。
//! - 編集 UI は別 step (今は preset を Rust で組み立てる)。
//! - Override / Glide Back は別 step。

use serde::{Deserialize, Serialize};

/// オートメーションの曲線タイプ (要件 §6.6 の 6 種)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CurveType {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// 次のキーフレームまでは「現在値を保持」、瞬間的にジャンプ。
    Step,
    /// `Step` と同じ意味だが、UI 上で「無視」を表すマーカー。
    Hold,
}

/// テンプレート時間軸の単位。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TimePosition {
    /// 0 拍目から N 拍目。
    Beats(f64),
    /// 絶対時間 (秒)。テンプレート開始時点の BPM で拍に変換する。
    Seconds(f64),
    /// 終端から逆算した拍位置。
    BeatsFromEnd(f64),
}

impl TimePosition {
    /// テンプレート全長 (`total_beats`) と開始時 BPM (`bpm`) を使って拍位置に正規化する。
    pub fn to_beats(self, total_beats: f64, bpm: f32) -> f64 {
        match self {
            Self::Beats(b) => b,
            Self::Seconds(s) => s * (bpm as f64) / 60.0,
            Self::BeatsFromEnd(b) => total_beats - b,
        }
    }
}

/// オートメーションが書き込む内蔵パラメータ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BuiltInTarget {
    Crossfader,
    MasterVolume,
    DeckVolume { deck: DeckSlot },
    DeckEqLow { deck: DeckSlot },
    DeckEqMid { deck: DeckSlot },
    DeckEqHigh { deck: DeckSlot },
    DeckFilter { deck: DeckSlot },
    DeckEchoWet { deck: DeckSlot },
    DeckReverbWet { deck: DeckSlot },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum DeckSlot {
    A,
    B,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Keyframe {
    pub position: TimePosition,
    pub value: f32,
    pub curve: CurveType,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AutomationTrack {
    pub target: BuiltInTarget,
    pub keyframes: Vec<Keyframe>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Template {
    pub id: String,
    pub name: String,
    /// テンプレート全長 (拍数)。
    pub duration_beats: f64,
    pub tracks: Vec<AutomationTrack>,
}

impl Template {
    /// 内蔵プリセット: Long EQ Mix (32 bars = 128 beats)。
    /// クロスフェーダーで全体を A → B、Deck A の low EQ を後半でカット。
    pub fn long_eq_mix() -> Self {
        Self {
            id: "preset.long_eq_mix".into(),
            name: "Long EQ Mix".into(),
            duration_beats: 128.0,
            tracks: vec![
                AutomationTrack {
                    target: BuiltInTarget::Crossfader,
                    keyframes: vec![
                        Keyframe {
                            position: TimePosition::Beats(0.0),
                            value: -1.0,
                            curve: CurveType::EaseInOut,
                        },
                        Keyframe {
                            position: TimePosition::Beats(128.0),
                            value: 1.0,
                            curve: CurveType::EaseInOut,
                        },
                    ],
                },
                AutomationTrack {
                    target: BuiltInTarget::DeckEqLow { deck: DeckSlot::A },
                    keyframes: vec![
                        Keyframe {
                            position: TimePosition::Beats(0.0),
                            value: 0.0,
                            curve: CurveType::Linear,
                        },
                        Keyframe {
                            position: TimePosition::Beats(64.0),
                            value: 0.0,
                            curve: CurveType::EaseIn,
                        },
                        Keyframe {
                            position: TimePosition::Beats(96.0),
                            value: -26.0,
                            curve: CurveType::Linear,
                        },
                    ],
                },
                AutomationTrack {
                    target: BuiltInTarget::DeckEqLow { deck: DeckSlot::B },
                    keyframes: vec![
                        Keyframe {
                            position: TimePosition::Beats(0.0),
                            value: -26.0,
                            curve: CurveType::Linear,
                        },
                        Keyframe {
                            position: TimePosition::Beats(32.0),
                            value: -26.0,
                            curve: CurveType::EaseOut,
                        },
                        Keyframe {
                            position: TimePosition::Beats(64.0),
                            value: 0.0,
                            curve: CurveType::Linear,
                        },
                    ],
                },
            ],
        }
    }

    /// Quick Cut (4 bars = 16 beats) — 短い切り替え。HipHop・ドロップ合わせ向け。
    /// クロスフェーダーを EaseIn で 16 拍かけて A → B、低音は触らない。
    pub fn quick_cut() -> Self {
        Self {
            id: "preset.quick_cut".into(),
            name: "Quick Cut".into(),
            duration_beats: 16.0,
            tracks: vec![AutomationTrack {
                target: BuiltInTarget::Crossfader,
                keyframes: vec![
                    Keyframe {
                        position: TimePosition::Beats(0.0),
                        value: -1.0,
                        curve: CurveType::EaseIn,
                    },
                    Keyframe {
                        position: TimePosition::Beats(16.0),
                        value: 1.0,
                        curve: CurveType::Linear,
                    },
                ],
            }],
        }
    }

    /// Breakdown Swap (16 bars = 64 beats) — ブレイクダウン区間で入れ替え。EDM・フェス向け。
    /// 前半 16 拍は A 主体、中間 32 拍で「low EQ swap」、後半 16 拍は B 主体。
    pub fn breakdown_swap() -> Self {
        Self {
            id: "preset.breakdown_swap".into(),
            name: "Breakdown Swap".into(),
            duration_beats: 64.0,
            tracks: vec![
                AutomationTrack {
                    target: BuiltInTarget::Crossfader,
                    keyframes: vec![
                        Keyframe {
                            position: TimePosition::Beats(0.0),
                            value: -1.0,
                            curve: CurveType::Hold,
                        },
                        Keyframe {
                            position: TimePosition::Beats(16.0),
                            value: -1.0,
                            curve: CurveType::EaseInOut,
                        },
                        Keyframe {
                            position: TimePosition::Beats(48.0),
                            value: 1.0,
                            curve: CurveType::Linear,
                        },
                        Keyframe {
                            position: TimePosition::Beats(64.0),
                            value: 1.0,
                            curve: CurveType::Linear,
                        },
                    ],
                },
                AutomationTrack {
                    target: BuiltInTarget::DeckEqLow { deck: DeckSlot::A },
                    keyframes: vec![
                        Keyframe {
                            position: TimePosition::Beats(0.0),
                            value: 0.0,
                            curve: CurveType::Hold,
                        },
                        Keyframe {
                            position: TimePosition::Beats(16.0),
                            value: 0.0,
                            curve: CurveType::EaseIn,
                        },
                        Keyframe {
                            position: TimePosition::Beats(32.0),
                            value: -26.0,
                            curve: CurveType::Hold,
                        },
                    ],
                },
                AutomationTrack {
                    target: BuiltInTarget::DeckEqLow { deck: DeckSlot::B },
                    keyframes: vec![
                        Keyframe {
                            position: TimePosition::Beats(0.0),
                            value: -26.0,
                            curve: CurveType::Hold,
                        },
                        Keyframe {
                            position: TimePosition::Beats(32.0),
                            value: -26.0,
                            curve: CurveType::EaseOut,
                        },
                        Keyframe {
                            position: TimePosition::Beats(48.0),
                            value: 0.0,
                            curve: CurveType::Linear,
                        },
                    ],
                },
            ],
        }
    }

    /// Echo Out (8 bars = 32 beats) — エコーをかけてフェードアウト。アンビエント・セット区切り向け。
    /// Deck A に echo を立ち上げつつ、後半でチャンネルボリュームを 0 まで落とす。
    pub fn echo_out() -> Self {
        Self {
            id: "preset.echo_out".into(),
            name: "Echo Out".into(),
            duration_beats: 32.0,
            tracks: vec![
                AutomationTrack {
                    target: BuiltInTarget::DeckEchoWet { deck: DeckSlot::A },
                    keyframes: vec![
                        Keyframe {
                            position: TimePosition::Beats(0.0),
                            value: 0.0,
                            curve: CurveType::Linear,
                        },
                        Keyframe {
                            position: TimePosition::Beats(16.0),
                            value: 0.55,
                            curve: CurveType::EaseInOut,
                        },
                        Keyframe {
                            position: TimePosition::Beats(32.0),
                            value: 0.85,
                            curve: CurveType::Linear,
                        },
                    ],
                },
                AutomationTrack {
                    target: BuiltInTarget::DeckVolume { deck: DeckSlot::A },
                    keyframes: vec![
                        Keyframe {
                            position: TimePosition::Beats(0.0),
                            value: 1.0,
                            curve: CurveType::Hold,
                        },
                        Keyframe {
                            position: TimePosition::Beats(16.0),
                            value: 1.0,
                            curve: CurveType::EaseInOut,
                        },
                        Keyframe {
                            position: TimePosition::Beats(32.0),
                            value: 0.0,
                            curve: CurveType::Linear,
                        },
                    ],
                },
            ],
        }
    }

    /// Instant Swap (1 bar = 4 beats) — 緊急用の即時切替。失敗リカバリー向け。
    /// 3 拍目までは A を保持、最後の 1 拍で一気に B にスナップ。
    pub fn instant_swap() -> Self {
        Self {
            id: "preset.instant_swap".into(),
            name: "Instant Swap".into(),
            duration_beats: 4.0,
            tracks: vec![AutomationTrack {
                target: BuiltInTarget::Crossfader,
                keyframes: vec![
                    Keyframe {
                        position: TimePosition::Beats(0.0),
                        value: -1.0,
                        curve: CurveType::Step,
                    },
                    Keyframe {
                        position: TimePosition::Beats(3.0),
                        value: -1.0,
                        curve: CurveType::Linear,
                    },
                    Keyframe {
                        position: TimePosition::Beats(4.0),
                        value: 1.0,
                        curve: CurveType::Linear,
                    },
                ],
            }],
        }
    }

    /// 内蔵プリセット一覧 (要件 §6.6 のプリセット 5 種)。
    pub fn all_presets() -> Vec<Template> {
        vec![
            Self::long_eq_mix(),
            Self::quick_cut(),
            Self::breakdown_swap(),
            Self::echo_out(),
            Self::instant_swap(),
        ]
    }
}

/// 単一トラックを `beat` 拍時点で評価する。
/// キーフレームが空 / 範囲外なら `None`。
pub fn evaluate_track(
    track: &AutomationTrack,
    beat: f64,
    total_beats: f64,
    bpm: f32,
) -> Option<f32> {
    if track.keyframes.is_empty() {
        return None;
    }

    // 直前 (<= beat) と直後 (>= beat) のキーフレームを線形走査で探す。
    let mut prev: Option<(f64, &Keyframe)> = None;
    let mut next: Option<(f64, &Keyframe)> = None;
    for kf in &track.keyframes {
        let pos = kf.position.to_beats(total_beats, bpm);
        if pos <= beat {
            prev = Some((pos, kf));
        }
        if pos >= beat && next.is_none() {
            next = Some((pos, kf));
        }
    }

    match (prev, next) {
        // 開始前: 最初のキーフレームの値で hold
        (None, Some((_, n))) => Some(n.value),
        // 終端後: 最後のキーフレームの値で hold
        (Some((_, p)), None) => Some(p.value),
        (Some((pp, p)), Some((np, n))) => {
            if (np - pp).abs() < 1e-9 {
                return Some(p.value);
            }
            let t = ((beat - pp) / (np - pp)) as f32;
            // Step / Hold は次のキーフレーム到達まで現在値固定
            if matches!(p.curve, CurveType::Step | CurveType::Hold) {
                return Some(p.value);
            }
            let eased = ease(t, p.curve);
            Some(p.value + (n.value - p.value) * eased)
        }
        (None, None) => None,
    }
}

fn ease(t: f32, curve: CurveType) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match curve {
        CurveType::Linear => t,
        CurveType::EaseIn => t * t,
        CurveType::EaseOut => 1.0 - (1.0 - t).powi(2),
        CurveType::EaseInOut => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
            }
        }
        CurveType::Step | CurveType::Hold => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear_track() -> AutomationTrack {
        AutomationTrack {
            target: BuiltInTarget::Crossfader,
            keyframes: vec![
                Keyframe {
                    position: TimePosition::Beats(0.0),
                    value: 0.0,
                    curve: CurveType::Linear,
                },
                Keyframe {
                    position: TimePosition::Beats(8.0),
                    value: 1.0,
                    curve: CurveType::Linear,
                },
            ],
        }
    }

    #[test]
    fn linear_midpoint() {
        let v = evaluate_track(&linear_track(), 4.0, 8.0, 128.0).unwrap();
        assert!((v - 0.5).abs() < 1e-6);
    }

    #[test]
    fn before_start_holds_first() {
        let v = evaluate_track(&linear_track(), -1.0, 8.0, 128.0).unwrap();
        assert!((v - 0.0).abs() < 1e-6);
    }

    #[test]
    fn after_end_holds_last() {
        let v = evaluate_track(&linear_track(), 99.0, 8.0, 128.0).unwrap();
        assert!((v - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ease_in_out_symmetric() {
        let a = ease(0.25, CurveType::EaseInOut);
        let b = 1.0 - ease(0.75, CurveType::EaseInOut);
        assert!((a - b).abs() < 1e-6);
    }

    #[test]
    fn step_curve_holds_until_next_kf() {
        let track = AutomationTrack {
            target: BuiltInTarget::Crossfader,
            keyframes: vec![
                Keyframe {
                    position: TimePosition::Beats(0.0),
                    value: 0.0,
                    curve: CurveType::Step,
                },
                Keyframe {
                    position: TimePosition::Beats(4.0),
                    value: 1.0,
                    curve: CurveType::Linear,
                },
            ],
        };
        let v = evaluate_track(&track, 3.99, 4.0, 128.0).unwrap();
        assert!((v - 0.0).abs() < 1e-6);
        let v_at_end = evaluate_track(&track, 4.0, 4.0, 128.0).unwrap();
        assert!((v_at_end - 1.0).abs() < 1e-6);
    }

    #[test]
    fn long_eq_mix_preset_is_well_formed() {
        let t = Template::long_eq_mix();
        assert!(t.duration_beats > 0.0);
        assert!(!t.tracks.is_empty());
        // 開始 (-1) → 終端 (+1) のクロスフェーダー
        let xfader = &t.tracks[0];
        let v0 = evaluate_track(xfader, 0.0, t.duration_beats, 128.0).unwrap();
        let v_end = evaluate_track(xfader, t.duration_beats, t.duration_beats, 128.0).unwrap();
        assert!((v0 + 1.0).abs() < 1e-6);
        assert!((v_end - 1.0).abs() < 1e-6);
    }
}
