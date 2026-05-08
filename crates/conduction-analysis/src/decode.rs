//! Symphonia ベースの PCM デコーダ。f32 モノラルに正規化して返す。

use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use tracing::debug;

use crate::error::{AnalysisError, AnalysisResult};

/// f32 モノラル PCM。サンプリングレートは元のファイルに準拠。
#[derive(Debug, Clone)]
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl DecodedAudio {
    pub fn duration_sec(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.samples.len() as f64 / self.sample_rate as f64
        }
    }
}

/// 音声ファイルをデコードして f32 モノ PCM に変換する。
///
/// マルチチャンネルのソースは平均化してモノに落とす。リサンプリングは行わない
/// （波形プレビュー用途では元レートで十分）。
pub fn decode_to_pcm(path: &Path) -> AnalysisResult<DecodedAudio> {
    let file = File::open(path).map_err(|source| AnalysisError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| AnalysisError::Decode(format!("probe: {e}")))?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| AnalysisError::Unsupported("no audio track".into()))?;
    let track_id = track.id;
    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| AnalysisError::Unsupported("unknown sample rate".into()))?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| AnalysisError::Decode(format!("codec: {e}")))?;

    let mut samples = Vec::<f32>::new();
    let mut sample_buf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => continue,
            Err(e) => return Err(AnalysisError::Decode(format!("packet: {e}"))),
        };
        if packet.track_id() != track_id {
            continue;
        }

        let buf_ref = match decoder.decode(&packet) {
            Ok(b) => b,
            Err(SymphoniaError::IoError(_)) | Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(AnalysisError::Decode(format!("decode: {e}"))),
        };

        let spec = *buf_ref.spec();
        let n_channels = spec.channels.count().max(1);

        if sample_buf.is_none() {
            let cap = buf_ref.capacity() as u64;
            sample_buf = Some(SampleBuffer::<f32>::new(cap, spec));
        }
        let sb = sample_buf.as_mut().expect("just initialized");
        sb.copy_interleaved_ref(buf_ref);

        let interleaved = sb.samples();
        if n_channels == 1 {
            samples.extend_from_slice(interleaved);
        } else {
            for frame in interleaved.chunks(n_channels) {
                let sum: f32 = frame.iter().sum();
                samples.push(sum / n_channels as f32);
            }
        }
    }

    debug!(
        path = %path.display(),
        sample_rate,
        sample_count = samples.len(),
        duration_sec = samples.len() as f64 / sample_rate as f64,
        "decoded to mono PCM"
    );

    Ok(DecodedAudio {
        samples,
        sample_rate,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    #[ignore = "requires ~/Downloads/wekapipo.mp3"]
    fn decode_real_mp3() {
        let home = std::env::var("HOME").unwrap();
        let path = PathBuf::from(format!("{home}/Downloads/wekapipo.mp3"));
        if !path.exists() {
            return;
        }
        let audio = decode_to_pcm(&path).expect("decode");
        assert!(audio.sample_rate > 0);
        assert!(audio.samples.len() > 0);
        assert!(audio.duration_sec() > 1.0);
    }

    #[test]
    fn missing_file_errors() {
        let err = decode_to_pcm(Path::new("/nope.mp3")).unwrap_err();
        assert!(matches!(err, AnalysisError::Io { .. }));
    }
}
