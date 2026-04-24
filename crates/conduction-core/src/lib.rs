//! conduction-core — Conduction のデータモデル骨格。
//!
//! Track / Cue / Template / Deck などプロダクト全体で共有されるドメイン型をここに集約する。
//! Phase 1 では Track / Cue を中心に、以降のフェーズで Template / Deck / Automation を追加する。

#![forbid(unsafe_code)]

pub mod beat;
pub mod cue;
pub mod error;
pub mod ids;
pub mod key;
pub mod time;
pub mod track;

pub use beat::Beat;
pub use cue::{Cue, CueType, MixRole};
pub use error::{CoreError, CoreResult};
pub use ids::{CueId, TrackId};
pub use key::{Key, KeyMode};
pub use time::TimePosition;
pub use track::Track;
