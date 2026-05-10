//! デッキ内蔵 DSP（3 バンド EQ + Filter + Echo + Reverb）。
//!
//! `DspParams` を `Arc` で UI スレッドと共有し、`DjEffectSource` が rodio の
//! `Source<Item=f32>` を decorate して chain 内で適用する。

mod coefficients;
mod echo;
mod params;
mod reverb;
mod source;
mod timestretch;

pub use params::DspParams;
pub use source::DjEffectSource;
pub use timestretch::{TimeStretchParams, TimeStretchSource};
