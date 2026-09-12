//! Serializable routing contract. Channel pairs use zero-based first-channel indices.
use crate::{AudioError, AudioResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MixingMode {
    #[default]
    Internal,
    External,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioOutputConfig {
    pub device_name: Option<String>,
    pub cue_device_name: Option<String>,
    pub sample_rate: Option<u32>,
    pub buffer_frames: Option<u32>,
    pub mode: MixingMode,
    pub main_pair: u16,
    pub cue_pair: Option<u16>,
    pub deck_a_pair: u16,
    pub deck_b_pair: u16,
    /// 0 = pre-fader CUE, 1 = MASTER.
    pub headphone_mix: f32,
    pub headphone_volume: f32,
}
impl Default for AudioOutputConfig {
    fn default() -> Self {
        Self {
            device_name: None,
            cue_device_name: None,
            sample_rate: None,
            buffer_frames: None,
            mode: MixingMode::Internal,
            main_pair: 0,
            cue_pair: None,
            deck_a_pair: 0,
            deck_b_pair: 2,
            headphone_mix: 0.0,
            headphone_volume: 1.0,
        }
    }
}
impl AudioOutputConfig {
    pub fn validate(&self, channels: u16, cue_channels: Option<u16>) -> AudioResult<()> {
        if !self.headphone_mix.is_finite()
            || !(0.0..=1.0).contains(&self.headphone_mix)
            || !self.headphone_volume.is_finite()
            || !(0.0..=2.0).contains(&self.headphone_volume)
        {
            return Err(AudioError::Playback(
                "invalid headphone mix or level".into(),
            ));
        }
        if self
            .sample_rate
            .is_some_and(|r| !(8000..=384000).contains(&r))
        {
            return Err(AudioError::Playback("invalid sample rate".into()));
        }
        if self
            .buffer_frames
            .is_some_and(|n| !(32..=8192).contains(&n))
        {
            return Err(AudioError::Playback(
                "buffer frames must be 32 through 8192".into(),
            ));
        }
        let mut occupied = Vec::new();
        let pairs: Vec<u16> = match self.mode {
            MixingMode::Internal => {
                let mut p = vec![self.main_pair];
                if cue_channels.is_none() {
                    if let Some(cue) = self.cue_pair {
                        p.push(cue);
                    }
                }
                p
            }
            MixingMode::External => vec![self.deck_a_pair, self.deck_b_pair],
        };
        for pair in pairs {
            if pair.checked_add(1).is_none_or(|r| r >= channels) {
                return Err(AudioError::Playback(format!(
                    "stereo pair {} is outside {channels} output channels",
                    pair
                )));
            }
            if occupied.contains(&pair) || occupied.contains(&(pair + 1)) {
                return Err(AudioError::Playback("output stereo pairs overlap".into()));
            }
            occupied.extend([pair, pair + 1]);
        }
        if let Some(n) = cue_channels {
            let pair = self.cue_pair.unwrap_or(0);
            if pair.checked_add(1).is_none_or(|r| r >= n) {
                return Err(AudioError::Playback(
                    "CUE pair is outside the cue device channels".into(),
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn required_channels(&self) -> u16 {
        match self.mode {
            MixingMode::Internal => self
                .main_pair
                .max(if self.cue_device_name.is_none() {
                    self.cue_pair.unwrap_or(0)
                } else {
                    0
                })
                .saturating_add(2),
            MixingMode::External => self.deck_a_pair.max(self.deck_b_pair).saturating_add(2),
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioOutputStatus {
    pub sample_rate: u32,
    pub output_channels: u16,
    pub elapsed_frames: u64,
    pub underruns: u64,
    pub device_lost: bool,
    pub error: Option<String>,
    pub estimated_latency_ms: f64,
    pub peak_main: [f32; 2],
    pub peak_cue: [f32; 2],
    pub peak_decks: [[f32; 2]; 2],
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_routing() {
        let mut c = AudioOutputConfig::default();
        assert!(c.validate(2, None).is_ok());
        c.cue_pair = Some(1);
        assert!(c.validate(4, None).is_err());
        c.cue_pair = Some(2);
        assert!(c.validate(4, None).is_ok());
        assert!(c.validate(2, None).is_err());
        c.mode = MixingMode::External;
        assert!(c.validate(4, None).is_ok());
        c.deck_b_pair = 1;
        assert!(c.validate(4, None).is_err());
    }
    #[test]
    fn separate_cue_may_use_same_pair() {
        let c = AudioOutputConfig {
            cue_pair: Some(0),
            ..Default::default()
        };
        assert!(c.validate(2, Some(2)).is_ok());
        assert!(c.validate(2, None).is_err());
    }
}
