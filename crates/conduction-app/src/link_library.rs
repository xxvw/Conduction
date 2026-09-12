//! Prepare an immutable, explicitly permitted Pro DJ Link library off the audio thread.
//!
//! Database access ends before probing/hashing/decoding any files. The network service only
//! receives canonical paths of registered tracks or completed conversions, never a directory
//! from which a client may request arbitrary files. Cache filenames contain stable database
//! IDs and a content/settings digest; partially written files are never published.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use conduction_analysis::WaveformPreview;
use conduction_core::{Beat, Track, TrackId};
use conduction_library::Library;
use conduction_link::{
    encode_three_band_waveform, LibrarySnapshot, LinkBeat, LinkCue, LinkPlaylist, LinkTrack,
};
use rubato::{FftFixedInOut, Resampler};
use serde::Serialize;
use sha2::{Digest, Sha256};
use symphonia::core::audio::{Channels, SampleBuffer};
use symphonia::core::codecs::{
    CodecParameters, DecoderOptions, CODEC_TYPE_AAC, CODEC_TYPE_ALAC, CODEC_TYPE_FLAC,
    CODEC_TYPE_MP3, CODEC_TYPE_NULL, CODEC_TYPE_PCM_S16BE, CODEC_TYPE_PCM_S16LE,
    CODEC_TYPE_PCM_S24BE, CODEC_TYPE_PCM_S24LE,
};
use symphonia::core::errors::Error as DecodeError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::library_state::LibraryHandle;

const CACHE_SETTINGS: &[u8] =
    b"conduction-link-pcm-v1;44100Hz;stereo;s16;fft-resample;trim-resampler-delay;codec-gapless=false";
const CACHE_RATE: u32 = 44_100;
const MAX_WAVEFORM_POINTS: usize = 150 * 60 * 60 * 24;

/// A track omitted from the published snapshot. Surface these messages next to Link status.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogIssue {
    pub track_id: String,
    pub title: String,
    pub message: String,
}

#[derive(Debug)]
pub struct PreparedCatalog {
    pub snapshot: LibrarySnapshot,
    pub issues: Vec<CatalogIssue>,
}

/// Synchronous preparation; invoke from a blocking worker, never an audio callback.
pub fn prepare_catalog(
    library: &LibraryHandle,
    cache_dir: impl AsRef<Path>,
) -> Result<LibrarySnapshot> {
    Ok(prepare_catalog_report(library, cache_dir)?.snapshot)
}

/// Like `prepare_catalog`, with per-track failures for a useful connection-status display.
pub fn prepare_catalog_report(
    library: &LibraryHandle,
    cache_dir: impl AsRef<Path>,
) -> Result<PreparedCatalog> {
    let rows = library.with_library(|library| read_rows(library))?;
    materialize(rows, cache_dir.as_ref())
}

struct CatalogRows {
    tracks: Vec<TrackRows>,
    playlists: Vec<LinkPlaylist>,
}

struct TrackRows {
    id: u32,
    track: Track,
    beats: Vec<Beat>,
    hot_cues: Vec<(u8, f64)>,
    waveform: Option<WaveformPreview>,
}

fn read_rows(library: &Library) -> Result<CatalogRows> {
    let mut ids = HashMap::<TrackId, u32>::new();
    let mut tracks = Vec::new();
    for track in library.list_tracks()? {
        let id = library
            .link_track_id(track.id)?
            .ok_or_else(|| anyhow!("missing stable Link ID for {}", track.id))?;
        ids.insert(track.id, id);
        tracks.push(TrackRows {
            id,
            beats: library.load_beatgrid(track.id)?,
            hot_cues: library.list_hot_cues(track.id)?,
            waveform: library.load_waveform(track.id)?,
            track,
        });
    }
    let mut playlists = Vec::new();
    for setlist in library.list_setlists()? {
        playlists.push(LinkPlaylist {
            id: library
                .link_setlist_id(setlist.id)?
                .ok_or_else(|| anyhow!("missing stable Link ID for {}", setlist.id))?,
            name: setlist.name,
            track_ids: setlist
                .entries
                .iter()
                .filter_map(|entry| ids.get(&entry.track_id).copied())
                .collect(),
        });
    }
    Ok(CatalogRows { tracks, playlists })
}

fn materialize(rows: CatalogRows, cache_dir: &Path) -> Result<PreparedCatalog> {
    fs::create_dir_all(cache_dir)
        .with_context(|| format!("create Link cache {}", cache_dir.display()))?;
    let cache_dir = cache_dir.canonicalize()?;
    let mut tracks = Vec::new();
    let mut issues = Vec::new();
    for row in rows.tracks {
        match prepare_track(&row, &cache_dir) {
            Ok(track) => tracks.push(track),
            Err(error) => {
                let message = format!("{error:#}");
                tracing::warn!(track_id = %row.track.id, %message, "track unavailable on Pro DJ Link");
                issues.push(CatalogIssue {
                    track_id: row.track.id.to_string(),
                    title: row.track.title,
                    message,
                });
            }
        }
    }
    let available: HashSet<u32> = tracks.iter().map(|track| track.id).collect();
    let mut playlists = rows.playlists;
    for playlist in &mut playlists {
        playlist.track_ids.retain(|id| available.contains(id));
    }
    Ok(PreparedCatalog {
        snapshot: LibrarySnapshot { tracks, playlists },
        issues,
    })
}

fn prepare_track(row: &TrackRows, cache_dir: &Path) -> Result<LinkTrack> {
    let source = permitted_original(&row.track.path)?;
    let probed = probe(&source)?;
    let (path, duration_sec) = if compatible_original(&source, &probed.params) {
        let duration = probed_duration(&probed.params).unwrap_or(row.track.duration.as_secs_f64());
        (source, duration)
    } else {
        cached_pcm(row.id, &source, cache_dir)?
    };
    let duration_ms = milliseconds(duration_sec)?;
    let (waveform_preview, waveform_detail) = match &row.waveform {
        Some(waveform) => encode_waveform(waveform, duration_ms)?,
        None => (Vec::new(), Vec::new()),
    };
    let cues = row
        .hot_cues
        .iter()
        .map(|&(slot, position)| {
            Ok(LinkCue {
                slot,
                time_ms: milliseconds(position)?,
                end_ms: None,
                // Conduction currently stores positions, not labels, for its eight Hot Cues.
                name: String::new(),
            })
        })
        .collect::<Result<_>>()?;
    let byte_size = fs::metadata(&path)?.len();
    Ok(LinkTrack {
        id: row.id,
        title: row.track.title.clone(),
        artist: row.track.artist.clone(),
        album: row.track.album.clone(),
        genre: row.track.genre.clone(),
        duration_ms,
        bpm: row.track.bpm as f64,
        path,
        byte_size,
        beats: link_beats(&row.beats, row.track.bpm as f64)?,
        cues,
        waveform_preview,
        waveform_detail,
    })
}

fn milliseconds(seconds: f64) -> Result<u32> {
    let ms = (seconds * 1000.0).round();
    if !ms.is_finite() || ms < 0.0 || ms > u32::MAX as f64 {
        bail!("audio timestamp is outside the Link protocol range");
    }
    Ok(ms as u32)
}

fn link_beats(beats: &[Beat], track_bpm: f64) -> Result<Vec<LinkBeat>> {
    // CDJ's wire representation requires 1..=4. If analysis has no bar origin, anchor the
    // first stored beat to bar position one. This is a display convention, not a new beat.
    let first_downbeat = beats.iter().position(|beat| beat.is_downbeat).unwrap_or(0);
    let mut bar_position = (4 - first_downbeat % 4) % 4;
    beats
        .iter()
        .enumerate()
        .map(|(index, beat)| {
            let interval_bpm = beats
                .get(index + 1)
                .map(|next| 60.0 / (next.position_sec - beat.position_sec));
            let bpm = beat
                .instantaneous_bpm
                .map(f64::from)
                .filter(|bpm| bpm.is_finite() && *bpm > 0.0)
                .or_else(|| (track_bpm.is_finite() && track_bpm > 0.0).then_some(track_bpm))
                .or_else(|| interval_bpm.filter(|bpm| bpm.is_finite() && *bpm > 0.0))
                .unwrap_or(0.0);
            if beat.is_downbeat {
                bar_position = 0;
            }
            let beat_in_bar = (bar_position + 1) as u8;
            bar_position = (bar_position + 1) % 4;
            Ok(LinkBeat {
                time_ms: milliseconds(beat.position_sec)?,
                beat: beat_in_bar,
                bpm,
            })
        })
        .collect()
}

fn encode_waveform(waveform: &WaveformPreview, duration_ms: u32) -> Result<(Vec<u8>, Vec<u8>)> {
    let source_len = waveform.sample_count as usize;
    if source_len == 0 || duration_ms == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    if [waveform.low.len(), waveform.mid.len(), waveform.high.len()]
        .iter()
        .any(|&len| len != source_len)
    {
        bail!("stored waveform dimensions are inconsistent");
    }
    // Legacy CDJs consume 150 columns/second. Interpolate measured RMS bins to that clock;
    // this preserves their real timing but does not claim native enhanced three-band detail.
    let points = ((duration_ms as u64 * 150).div_ceil(1000)) as usize;
    if points > MAX_WAVEFORM_POINTS {
        bail!("track exceeds 24-hour waveform limit");
    }
    let sample_band = |band: &[f32]| {
        (0..points)
            .map(|index| {
                let position = index as f64 * source_len as f64 / points as f64;
                let left = (position as usize).min(source_len - 1);
                let right = (left + 1).min(source_len - 1);
                let fraction = (position - left as f64) as f32;
                let value = band[left] + (band[right] - band[left]) * fraction;
                if value.is_finite() {
                    value.clamp(0.0, 1.0)
                } else {
                    0.0
                }
            })
            .collect::<Vec<_>>()
    };
    Ok(encode_three_band_waveform(
        &sample_band(&waveform.low),
        &sample_band(&waveform.mid),
        &sample_band(&waveform.high),
    ))
}

/// Only called with an exact path obtained from the local registered-track table.
fn permitted_original(path: &Path) -> Result<PathBuf> {
    let path = path
        .canonicalize()
        .with_context(|| format!("registered audio file is unavailable: {}", path.display()))?;
    let metadata = fs::metadata(&path)?;
    if !metadata.is_file() || metadata.len() == 0 {
        bail!("registered audio path is not a nonempty regular file");
    }
    Ok(path)
}

struct ProbedAudio {
    format: Box<dyn FormatReader>,
    track_id: u32,
    params: CodecParameters,
}

fn probed_duration(params: &CodecParameters) -> Option<f64> {
    let frames = params.n_frames?;
    if let Some(time_base) = params.time_base {
        let time = time_base.calc_time(frames);
        Some(time.seconds as f64 + time.frac)
    } else {
        params
            .sample_rate
            .filter(|rate| *rate != 0)
            .map(|rate| frames as f64 / rate as f64)
    }
}

fn probe(path: &Path) -> Result<ProbedAudio> {
    let stream = MediaSourceStream::new(Box::new(File::open(path)?), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        hint.with_extension(extension);
    }
    let result = symphonia::default::get_probe().format(
        &hint,
        stream,
        // Existing analyzed beat grids and playback use the decoder's untrimmed timeline.
        // Change this only with a matching analysis migration and audio-engine convention.
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let track = result
        .format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow!("no supported audio stream"))?;
    let track_id = track.id;
    let params = track.codec_params.clone();
    Ok(ProbedAudio {
        format: result.format,
        track_id,
        params,
    })
}

fn compatible_original(path: &Path, params: &CodecParameters) -> bool {
    // Common denominator from Pioneer DJ's CDJ-2000NXS2 published audio specifications:
    // https://www.pioneerdj.com/en/news/2016/meet-the-new-cdj-2000nxs2-and-djm-900nxs2/
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(params.channels.map(Channels::count), Some(1 | 2)) {
        return false;
    }
    let rate = params.sample_rate.unwrap_or(0);
    let linear_rate = matches!(rate, 44_100 | 48_000 | 88_200 | 96_000);
    let depth = matches!(params.bits_per_sample, Some(16 | 24));
    match params.codec {
        CODEC_TYPE_PCM_S16LE | CODEC_TYPE_PCM_S24LE => extension == "wav" && linear_rate && depth,
        CODEC_TYPE_PCM_S16BE | CODEC_TYPE_PCM_S24BE => {
            matches!(extension.as_str(), "aif" | "aiff") && linear_rate && depth
        }
        CODEC_TYPE_FLAC => extension == "flac" && linear_rate && depth,
        // dbserver currently infers the file-type field from the published extension. Its
        // M4A value denotes AAC, so ALAC must use PCM until that metadata carries a codec.
        CODEC_TYPE_ALAC => false,
        CODEC_TYPE_MP3 => extension == "mp3" && matches!(rate, 32_000 | 44_100 | 48_000),
        CODEC_TYPE_AAC => {
            matches!(extension.as_str(), "m4a" | "mp4" | "aac")
                && matches!(rate, 16_000 | 22_050 | 24_000 | 32_000 | 44_100 | 48_000)
        }
        _ => false,
    }
}

fn fingerprint(source: &Path, settings: &[u8]) -> Result<String> {
    let mut digest = Sha256::new();
    digest.update(settings);
    let mut reader = BufReader::new(File::open(source)?);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn permitted_cache(path: &Path, cache_dir: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        bail!("Link cache entry must be a regular file, not a symbolic link");
    }
    let canonical = path.canonicalize()?;
    if canonical.parent() != Some(cache_dir) {
        bail!("Link cache entry escaped its configured directory");
    }
    Ok(canonical)
}

fn cached_pcm(id: u32, source: &Path, cache_dir: &Path) -> Result<(PathBuf, f64)> {
    let digest = fingerprint(source, CACHE_SETTINGS)?;
    let target = cache_dir.join(format!("{id}-{digest}.wav"));
    if fs::symlink_metadata(&target).is_ok() {
        let path = permitted_cache(&target, cache_dir)?;
        if let Ok(duration) = cache_duration(&path) {
            return Ok((path, duration));
        }
        // Only remove our exact, canonical generated entry; never recursively clean a folder.
        fs::remove_file(path)?;
    }
    let temporary = cache_dir.join(format!(".{id}-{}.partial", uuid::Uuid::new_v4()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let conversion = (|| -> Result<f64> {
        let duration = transcode_pcm(source, file)?;
        if fingerprint(source, CACHE_SETTINGS)? != digest {
            bail!("source audio changed during Link cache preparation; retry the refresh");
        }
        fs::rename(&temporary, &target)?;
        Ok(duration)
    })();
    if conversion.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    let duration = conversion?;
    Ok((permitted_cache(&target, cache_dir)?, duration))
}

fn cache_duration(path: &Path) -> Result<f64> {
    let reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.channels != 2
        || spec.sample_rate != CACHE_RATE
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
        || reader.duration() == 0
        || fs::metadata(path)?.len() < 44 + reader.duration() as u64 * 4
    {
        bail!("invalid or incomplete Link audio cache");
    }
    Ok(reader.duration() as f64 / CACHE_RATE as f64)
}

fn transcode_pcm(source: &Path, target: File) -> Result<f64> {
    let mut probed = probe(source)?;
    let rate = probed
        .params
        .sample_rate
        .ok_or_else(|| anyhow!("unknown sample rate"))?;
    if rate == 0 {
        bail!("invalid sample rate");
    }
    let mut decoder =
        symphonia::default::get_codecs().make(&probed.params, &DecoderOptions::default())?;
    let mut output = StereoWav::new(target, rate)?;
    loop {
        let packet = match probed.format.next_packet() {
            Ok(packet) => packet,
            Err(DecodeError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(error) => return Err(error.into()),
        };
        if packet.track_id() != probed.track_id {
            continue;
        }
        // Do not skip corrupt packets: shortening the file would invalidate its beatgrid and cues.
        let decoded = decoder.decode(&packet)?;
        let spec = *decoded.spec();
        if spec.rate != rate || spec.channels.count() == 0 {
            bail!("audio stream changes sample rate or has no channels");
        }
        let mut samples = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        samples.copy_interleaved_ref(decoded);
        for frame in samples.samples().chunks_exact(spec.channels.count()) {
            output.push(downmix(frame, spec.channels))?;
        }
    }
    output.finish()
}

fn downmix(frame: &[f32], channels: Channels) -> [f32; 2] {
    if frame.len() == 1 {
        return [frame[0], frame[0]];
    }
    if frame.len() == 2 {
        return [frame[0], frame[1]];
    }
    let mut stereo = [0.0_f32; 2];
    let mut weight = [0.0_f32; 2];
    for (&value, channel) in frame.iter().zip(channels.iter()) {
        let weights = if channel == Channels::FRONT_LEFT {
            [1.0, 0.0]
        } else if channel == Channels::FRONT_RIGHT {
            [0.0, 1.0]
        } else if channel
            .intersects(Channels::REAR_LEFT | Channels::SIDE_LEFT | Channels::FRONT_LEFT_CENTRE)
        {
            [std::f32::consts::FRAC_1_SQRT_2, 0.0]
        } else if channel
            .intersects(Channels::REAR_RIGHT | Channels::SIDE_RIGHT | Channels::FRONT_RIGHT_CENTRE)
        {
            [0.0, std::f32::consts::FRAC_1_SQRT_2]
        } else if channel.intersects(Channels::LFE1 | Channels::LFE2) {
            [0.0, 0.0]
        } else {
            [std::f32::consts::FRAC_1_SQRT_2; 2]
        };
        for side in 0..2 {
            stereo[side] += value * weights[side];
            weight[side] += weights[side];
        }
    }
    for side in 0..2 {
        stereo[side] /= weight[side].max(1.0);
    }
    stereo
}

struct StereoWav {
    writer: hound::WavWriter<BufWriter<File>>,
    resampler: Option<FftFixedInOut<f32>>,
    pending: [Vec<f32>; 2],
    input_frames: u64,
    output_frames: u64,
    discard_delay: usize,
    source_rate: u32,
}

impl StereoWav {
    fn new(file: File, source_rate: u32) -> Result<Self> {
        let resampler = if source_rate == CACHE_RATE {
            None
        } else {
            Some(FftFixedInOut::<f32>::new(
                source_rate as usize,
                CACHE_RATE as usize,
                1024,
                2,
            )?)
        };
        let discard_delay = resampler.as_ref().map_or(0, Resampler::output_delay);
        Ok(Self {
            writer: hound::WavWriter::new(
                BufWriter::new(file),
                hound::WavSpec {
                    channels: 2,
                    sample_rate: CACHE_RATE,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )?,
            resampler,
            pending: [Vec::new(), Vec::new()],
            input_frames: 0,
            output_frames: 0,
            discard_delay,
            source_rate,
        })
    }

    fn push(&mut self, stereo: [f32; 2]) -> Result<()> {
        self.input_frames += 1;
        if self.resampler.is_none() {
            return self.write(stereo);
        }
        self.pending[0].push(stereo[0]);
        self.pending[1].push(stereo[1]);
        let resampler = self.resampler.as_mut().expect("checked resampler");
        if self.pending[0].len() == resampler.input_frames_next() {
            let output = resampler.process(&self.pending, None)?;
            self.pending.iter_mut().for_each(Vec::clear);
            self.write_resampled(output, u64::MAX)?;
        }
        Ok(())
    }

    fn write(&mut self, stereo: [f32; 2]) -> Result<()> {
        if self.output_frames >= (u32::MAX as u64 - 44) / 4 {
            bail!("converted audio exceeds the 4 GiB WAV file limit");
        }
        for sample in stereo {
            if !sample.is_finite() {
                bail!("decoded audio contains a nonfinite sample");
            }
            self.writer
                .write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16)?;
        }
        self.output_frames += 1;
        Ok(())
    }

    fn write_resampled(&mut self, output: Vec<Vec<f32>>, limit: u64) -> Result<()> {
        for (&left, &right) in output[0].iter().zip(&output[1]) {
            if self.discard_delay > 0 {
                self.discard_delay -= 1;
                continue;
            }
            if self.output_frames == limit {
                break;
            }
            self.write([left, right])?;
        }
        Ok(())
    }

    fn finish(mut self) -> Result<f64> {
        if self.input_frames == 0 {
            bail!("audio stream contains no decoded frames");
        }
        let expected = (self.input_frames * CACHE_RATE as u64 + self.source_rate as u64 / 2)
            / self.source_rate as u64;
        if let Some(resampler) = self.resampler.as_mut() {
            if !self.pending[0].is_empty() {
                let output = resampler.process_partial(Some(&self.pending), None)?;
                self.write_resampled(output, expected)?;
            }
            while self.output_frames < expected {
                let output = self
                    .resampler
                    .as_mut()
                    .expect("resampler exists")
                    .process_partial::<Vec<f32>>(None, None)?;
                self.write_resampled(output, expected)?;
            }
        }
        let duration = self.output_frames as f64 / CACHE_RATE as f64;
        self.writer.finalize()?;
        Ok(duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduction_core::{Key, KeyMode};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("conduction-link-library-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_wave(path: &Path, rate: u32, float: bool) {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: rate,
            bits_per_sample: if float { 32 } else { 16 },
            sample_format: if float {
                hound::SampleFormat::Float
            } else {
                hound::SampleFormat::Int
            },
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for index in 0..rate {
            let left = (index as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.4;
            if float {
                writer.write_sample(left).unwrap();
                writer.write_sample(0.0_f32).unwrap();
            } else {
                writer
                    .write_sample((left * i16::MAX as f32) as i16)
                    .unwrap();
                writer.write_sample(0_i16).unwrap();
            }
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn compatible_pcm_original_is_served_unchanged_and_japanese_metadata_survives() {
        let directory = TempDir::new();
        let source = directory.0.join("夜明け.wav");
        write_wave(&source, CACHE_RATE, false);
        let mut library = Library::in_memory().unwrap();
        let mut track = Track::placeholder(source.clone(), Key::new(8, KeyMode::Minor).unwrap());
        track.title = "夜明け".into();
        track.artist = "東京".into();
        track.bpm = 120.0;
        library.insert_track(&track).unwrap();
        library
            .replace_beatgrid(track.id, &[Beat::new(0.0, true), Beat::new(0.5, false)])
            .unwrap();
        library.set_hot_cue(track.id, 8, 0.5).unwrap();
        let setlist = library.create_setlist("夜のセット".into()).unwrap();
        library.add_setlist_entry(setlist.id, track.id).unwrap();
        let result = materialize(read_rows(&library).unwrap(), &directory.0.join("cache")).unwrap();
        assert!(result.issues.is_empty());
        let published = &result.snapshot.tracks[0];
        assert_eq!(published.path, source.canonicalize().unwrap());
        assert_eq!(published.title, "夜明け");
        assert_eq!(published.artist, "東京");
        assert_eq!(published.duration_ms, 1000);
        assert_eq!(published.beats[1].time_ms, 500);
        assert_eq!(published.cues[0].slot, 8);
        assert_eq!(result.snapshot.playlists[0].name, "夜のセット");
        assert_eq!(result.snapshot.playlists[0].track_ids, [published.id]);
        assert!(published.waveform_preview.is_empty());
    }

    #[test]
    fn unsupported_pcm_transcodes_preserving_stereo_duration_and_resampler_delay() {
        let directory = TempDir::new();
        let source = directory.0.join("float.wav");
        write_wave(&source, 48_000, true);
        let params = probe(&source).unwrap().params;
        assert!(!compatible_original(&source, &params));
        let (path, duration) =
            cached_pcm(1, &source, &directory.0.canonicalize().unwrap()).unwrap();
        assert!((duration - 1.0).abs() < 1.0 / CACHE_RATE as f64);
        let mut reader = hound::WavReader::open(path).unwrap();
        assert_eq!(reader.spec().channels, 2);
        assert_eq!(reader.spec().sample_rate, CACHE_RATE);
        let samples: Vec<i16> = reader.samples::<i16>().map(Result::unwrap).collect();
        assert_eq!(samples.len(), CACHE_RATE as usize * 2);
        assert!(samples.iter().skip(1).step_by(2).all(|sample| *sample == 0));
        for frame in [50, 500, 5_000, 40_000] {
            let expected =
                (frame as f32 * 440.0 * std::f32::consts::TAU / CACHE_RATE as f32).sin() * 0.4;
            assert!((samples[frame * 2] as f32 / i16::MAX as f32 - expected).abs() < 0.015);
        }
    }

    #[test]
    fn ogg_vorbis_is_converted_to_pcm_with_independent_stereo_channels() {
        let directory = TempDir::new();
        let source = directory.0.join("stereo.ogg");
        // Generated test signal: 48 kHz, 440 Hz sine on L, silence on R, 100 ms.
        // ffmpeg -f lavfi -i 'aevalsrc=0.4*sin(2*PI*440*t)|0:s=48000:d=0.1'
        //        -c:a vorbis -strict experimental -q:a 4 -fflags +bitexact -flags:a +bitexact
        fs::write(&source, include_bytes!("../tests/fixtures/link-stereo.ogg")).unwrap();
        assert!(!compatible_original(
            &source,
            &probe(&source).unwrap().params
        ));
        let (path, duration) =
            cached_pcm(7, &source, &directory.0.canonicalize().unwrap()).unwrap();
        let analysis_duration = conduction_analysis::decode_to_pcm(&source)
            .unwrap()
            .duration_sec();
        assert!((duration - analysis_duration).abs() < 1.0 / CACHE_RATE as f64);
        let mut reader = hound::WavReader::open(path).unwrap();
        let samples: Vec<i16> = reader.samples::<i16>().map(Result::unwrap).collect();
        assert!(samples.iter().step_by(2).any(|sample| sample.abs() > 1000));
        assert!(samples
            .iter()
            .skip(1)
            .step_by(2)
            .all(|sample| sample.abs() <= 1));
    }

    #[test]
    fn fingerprint_invalidates_for_content_even_at_equal_size_and_for_settings() {
        let directory = TempDir::new();
        let source = directory.0.join("input");
        fs::write(&source, b"first").unwrap();
        let first = fingerprint(&source, CACHE_SETTINGS).unwrap();
        fs::write(&source, b"other").unwrap();
        assert_ne!(first, fingerprint(&source, CACHE_SETTINGS).unwrap());
        assert_ne!(
            fingerprint(&source, CACHE_SETTINGS).unwrap(),
            fingerprint(&source, b"pcm-v2").unwrap()
        );
    }

    #[test]
    fn absent_registered_files_are_reported_and_removed_from_playlists() {
        let directory = TempDir::new();
        let mut library = Library::in_memory().unwrap();
        let track = Track::placeholder(
            directory.0.join("missing.wav"),
            Key::new(8, KeyMode::Minor).unwrap(),
        );
        library.insert_track(&track).unwrap();
        let setlist = library.create_setlist("Missing".into()).unwrap();
        library.add_setlist_entry(setlist.id, track.id).unwrap();
        let result = materialize(read_rows(&library).unwrap(), &directory.0.join("cache")).unwrap();
        assert!(result.snapshot.tracks.is_empty());
        assert_eq!(result.issues.len(), 1);
        assert!(result.snapshot.playlists[0].track_ids.is_empty());
    }

    #[test]
    fn cache_reuse_and_source_invalidation_produce_expected_files() {
        let directory = TempDir::new();
        let source = directory.0.join("float.wav");
        write_wave(&source, CACHE_RATE, true);
        let cache = directory.0.canonicalize().unwrap();
        let (first, _) = cached_pcm(19, &source, &cache).unwrap();
        let modified = fs::metadata(&first).unwrap().modified().unwrap();
        let (reused, _) = cached_pcm(19, &source, &cache).unwrap();
        assert_eq!(first, reused);
        assert_eq!(modified, fs::metadata(reused).unwrap().modified().unwrap());
        write_wave(&source, 48_000, true);
        let (changed, _) = cached_pcm(19, &source, &cache).unwrap();
        assert_ne!(first, changed);
        assert_eq!(cache_duration(&changed).unwrap(), 1.0);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_and_parent_traversal_cannot_authorize_unrelated_cache_files() {
        let directory = TempDir::new();
        let cache = directory.0.join("cache");
        fs::create_dir(&cache).unwrap();
        let outside = directory.0.join("private.wav");
        write_wave(&outside, CACHE_RATE, false);
        let symlink = cache.join("public.wav");
        std::os::unix::fs::symlink(&outside, &symlink).unwrap();
        let cache = cache.canonicalize().unwrap();
        assert!(permitted_cache(&symlink, &cache).is_err());
        assert!(permitted_cache(&cache.join("../private.wav"), &cache).is_err());
        assert!(permitted_original(&directory.0).is_err());
    }

    #[test]
    fn waveform_uses_real_timing_and_missing_bar_origin_counts_from_first_beat() {
        let waveform = WaveformPreview {
            sample_count: 2,
            low: vec![0.0, 1.0],
            mid: vec![0.0; 2],
            high: vec![0.0; 2],
        };
        let (_, detail) = encode_waveform(&waveform, 2000).unwrap();
        assert_eq!(detail.len(), 300);
        let beats = link_beats(&[Beat::new(0.25, false), Beat::new(0.75, false)], 120.0).unwrap();
        assert_eq!(beats[0].time_ms, 250);
        assert_eq!(beats[0].beat, 1);
        assert_eq!(beats[1].beat, 2);
    }

    #[test]
    fn container_time_base_controls_duration_instead_of_sample_rate() {
        let mut params = CodecParameters {
            sample_rate: Some(48_000),
            time_base: Some(symphonia::core::units::TimeBase::new(1, 1_000)),
            n_frames: Some(3_000),
            ..Default::default()
        };
        assert_eq!(probed_duration(&params), Some(3.0));
        params.time_base = None;
        assert_eq!(probed_duration(&params), Some(0.0625));
    }

    #[test]
    fn alac_uses_pcm_cache_instead_of_being_advertised_as_aac() {
        let mut params = CodecParameters {
            codec: CODEC_TYPE_ALAC,
            sample_rate: Some(44_100),
            bits_per_sample: Some(16),
            channels: Some(Channels::FRONT_LEFT | Channels::FRONT_RIGHT),
            ..Default::default()
        };
        assert!(!compatible_original(Path::new("lossless.m4a"), &params));
        assert!(!compatible_original(Path::new("lossless.mp4"), &params));
        params.codec = CODEC_TYPE_AAC;
        assert!(compatible_original(Path::new("compressed.m4a"), &params));
    }
}
