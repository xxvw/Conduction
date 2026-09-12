//! Full-file decoding performed outside the real-time audio callback.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::error::{AudioError, AudioResult};

/// Immutable stereo frames at the source sample rate.
///
/// Mono sources are duplicated into left and right. Sources with more than two
/// channels are rejected rather than silently discarding channels. No device is
/// opened and no resampling or tempo processing is performed during decoding.
#[derive(Debug, Clone)]
pub struct PcmTrack {
    pub samples: Arc<[[f32; 2]]>,
    pub sample_rate: u32,
}

impl PcmTrack {
    pub fn decode(path: &Path) -> AudioResult<Self> {
        let file = File::open(path).map_err(|source| AudioError::FileOpen {
            path: path.to_path_buf(),
            source,
        })?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
            hint.with_extension(extension);
        }
        let options = FormatOptions {
            // Preserve the existing analysis/cue timeline (including codec delay).
            enable_gapless: false,
            ..Default::default()
        };
        let mut format = symphonia::default::get_probe()
            .format(&hint, stream, &options, &MetadataOptions::default())
            .map_err(|error| AudioError::Decode(format!("probe: {error}")))?
            .format;
        let track = format
            .default_track()
            .filter(|track| track.codec_params.codec != CODEC_TYPE_NULL)
            .or_else(|| {
                format
                    .tracks()
                    .iter()
                    .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
            })
            .ok_or_else(|| AudioError::Decode("no audio track".into()))?;
        let track_id = track.id;
        let mut sample_rate = track.codec_params.sample_rate;
        if sample_rate == Some(0) {
            return Err(AudioError::Decode("source sample rate is zero".into()));
        }
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|error| AudioError::Decode(format!("codec: {error}")))?;
        let mut samples = Vec::new();
        let mut sample_buffer: Option<SampleBuffer<f32>> = None;
        let mut buffer_capacity = 0;
        let mut channel_count = None;

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                // Symphonia reports a normal end of the packet stream this way.
                Err(SymphoniaError::IoError(error))
                    if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(error) => return Err(AudioError::Decode(format!("packet: {error}"))),
            };
            if packet.track_id() != track_id {
                continue;
            }
            // A damaged packet must fail the load, not create a shortened track
            // whose sample clock no longer matches the analyzed beat grid.
            let decoded = decoder
                .decode(&packet)
                .map_err(|error| AudioError::Decode(format!("decode: {error}")))?;
            let spec = *decoded.spec();
            if spec.rate == 0 || sample_rate.is_some_and(|rate| rate != spec.rate) {
                return Err(AudioError::Decode(
                    "source sample rate is zero or changes during decoding".into(),
                ));
            }
            sample_rate = Some(spec.rate);
            let channels = spec.channels.count();
            if !(1..=2).contains(&channels) {
                return Err(AudioError::Decode(format!(
                    "unsupported channel count: {channels}; expected mono or stereo"
                )));
            }
            if channel_count.is_some_and(|count| count != channels) {
                return Err(AudioError::Decode(
                    "source channel count changes during decoding".into(),
                ));
            }
            channel_count = Some(channels);
            let capacity = decoded.capacity();
            if sample_buffer.is_none() || capacity > buffer_capacity {
                sample_buffer = Some(SampleBuffer::<f32>::new(capacity as u64, spec));
                buffer_capacity = capacity;
            }
            let buffer = sample_buffer.as_mut().expect("sample buffer initialized");
            buffer.copy_interleaved_ref(decoded);
            for frame in buffer.samples().chunks_exact(channels) {
                let left = frame[0];
                let right = if channels == 1 { left } else { frame[1] };
                if !left.is_finite() || !right.is_finite() {
                    return Err(AudioError::Decode("source contains non-finite PCM".into()));
                }
                samples.push([left, right]);
            }
        }

        if samples.is_empty() {
            return Err(AudioError::Decode("source contains no audio frames".into()));
        }
        let sample_rate = sample_rate
            .filter(|rate| *rate > 0)
            .ok_or_else(|| AudioError::Decode("unknown source sample rate".into()))?;
        Ok(Self {
            samples: samples.into(),
            sample_rate,
        })
    }

    pub fn duration(&self) -> Duration {
        if self.sample_rate == 0 {
            return Duration::ZERO;
        }
        Duration::from_secs_f64(self.frames() as f64 / self.sample_rate as f64)
    }

    pub fn frames(&self) -> usize {
        self.samples.len()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

    struct TestWav(PathBuf);

    impl TestWav {
        fn new(channels: u16, sample_rate: u32, samples: &[i16]) -> Self {
            let path = std::env::temp_dir().join(format!(
                "conduction-pcm-{}-{}.wav",
                std::process::id(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed)
            ));
            let data_len = samples.len() as u32 * 2;
            let mut bytes = Vec::new();
            bytes.extend_from_slice(b"RIFF");
            bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
            bytes.extend_from_slice(b"WAVEfmt ");
            bytes.extend_from_slice(&16_u32.to_le_bytes());
            bytes.extend_from_slice(&1_u16.to_le_bytes());
            bytes.extend_from_slice(&channels.to_le_bytes());
            bytes.extend_from_slice(&sample_rate.to_le_bytes());
            bytes.extend_from_slice(&(sample_rate * u32::from(channels) * 2).to_le_bytes());
            bytes.extend_from_slice(&(channels * 2).to_le_bytes());
            bytes.extend_from_slice(&16_u16.to_le_bytes());
            bytes.extend_from_slice(b"data");
            bytes.extend_from_slice(&data_len.to_le_bytes());
            for sample in samples {
                bytes.extend_from_slice(&sample.to_le_bytes());
            }
            std::fs::write(&path, bytes).expect("write WAV fixture");
            Self(path)
        }
    }

    impl Drop for TestWav {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn stereo_wav_preserves_frame_interleaving() {
        let wav = TestWav::new(2, 48_000, &[16_384, -16_384, 8_192, -8_192, 0, 32_767]);
        let decoded = PcmTrack::decode(&wav.0).expect("decode stereo WAV");
        assert_eq!(decoded.sample_rate, 48_000);
        assert_eq!(decoded.frames(), 3);
        assert_eq!(decoded.samples[0], [0.5, -0.5]);
        assert_eq!(decoded.samples[1], [0.25, -0.25]);
        assert_eq!(decoded.samples[2], [0.0, 32_767.0 / 32_768.0]);
        assert_eq!(decoded.duration(), Duration::from_nanos(62_500));
    }

    #[test]
    fn mono_wav_duplicates_both_channels() {
        let wav = TestWav::new(1, 44_100, &[16_384, -32_768, 0, 8_192]);
        let decoded = PcmTrack::decode(&wav.0).expect("decode mono WAV");
        assert_eq!(decoded.sample_rate, 44_100);
        assert_eq!(decoded.frames(), 4);
        assert_eq!(
            decoded.samples.as_ref(),
            &[[0.5, 0.5], [-1.0, -1.0], [0.0, 0.0], [0.25, 0.25]]
        );
    }

    #[test]
    fn empty_wav_fails_loading() {
        let wav = TestWav::new(2, 48_000, &[]);
        assert!(matches!(
            PcmTrack::decode(&wav.0),
            Err(AudioError::Decode(_))
        ));
    }

    #[test]
    fn multichannel_wav_is_explicitly_rejected() {
        let wav = TestWav::new(4, 48_000, &[0, 1, 2, 3]);
        assert!(matches!(
            PcmTrack::decode(&wav.0),
            Err(AudioError::Decode(_))
        ));
    }

    #[test]
    fn invalid_file_fails_loading() {
        let wav = TestWav::new(1, 44_100, &[0]);
        std::fs::write(&wav.0, b"not an audio file").expect("write invalid fixture");
        assert!(matches!(
            PcmTrack::decode(&wav.0),
            Err(AudioError::Decode(_))
        ));
    }
}
