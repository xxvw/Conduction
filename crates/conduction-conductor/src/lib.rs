//! conduction-conductor — 司令塔。
//!
//! Phase 4 でテンプレート実行エンジン (2 層ティック構造) ・Cue 動的マッチングを実装する。
//!
//! 現状: Cue 動的マッチング (`matching` モジュール) のみ実装済み。

#![forbid(unsafe_code)]

pub mod automation;
pub mod matching;
pub mod runner;
pub mod template;

pub use automation::{effective_value, AutomationMode, AutomationModeKind};
pub use matching::{find_candidates, score, MatchQuery, MatchScore, ScoredCue};
pub use runner::TemplateRunner;
pub use template::{
    evaluate_track, AutomationTrack, BuiltInTarget, CurveType, DeckSlot, Keyframe, Template,
    TimePosition,
};
