use crate::{
    config::{AudioOutputConfig, AudioOutputStatus},
    error::{AudioError, AudioResult},
    runtime::{start_worker, CueCallback, MainCallback, Shared},
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::{atomic::Ordering, Arc};
use std::thread::JoinHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioOutputDescriptor {
    pub name: String,
    pub is_default: bool,
    pub max_output_channels: u16,
    pub sample_rates: Vec<u32>,
    pub buffer_frames_min: Option<u32>,
    pub buffer_frames_max: Option<u32>,
}
/// Enumerate capabilities without starting a stream or playing any sound.
pub fn list_audio_outputs() -> Vec<AudioOutputDescriptor> {
    let host = cpal::default_host();
    let default = host.default_output_device().and_then(|d| d.name().ok());
    let mut result = Vec::new();
    if let Ok(devices) = host.output_devices() {
        for d in devices {
            let Ok(name) = d.name() else {
                continue;
            };
            let mut max_output_channels = 0;
            let mut sample_rates = Vec::new();
            let mut buffer_frames_min: Option<u32> = None;
            let mut buffer_frames_max: Option<u32> = None;
            if let Ok(ranges) = d.supported_output_configs() {
                for range in ranges {
                    if let cpal::SupportedBufferSize::Range { min, max } = range.buffer_size() {
                        buffer_frames_min = Some(buffer_frames_min.map_or(*min, |v| v.min(*min)));
                        buffer_frames_max = Some(buffer_frames_max.map_or(*max, |v| v.max(*max)));
                    }
                    max_output_channels = max_output_channels.max(range.channels());
                    for rate in [
                        8000, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000,
                    ] {
                        if range.min_sample_rate().0 <= rate
                            && rate <= range.max_sample_rate().0
                            && !sample_rates.contains(&rate)
                        {
                            sample_rates.push(rate);
                        }
                    }
                }
            }
            sample_rates.sort_unstable();
            result.push(AudioOutputDescriptor {
                is_default: default.as_deref() == Some(&name),
                name,
                max_output_channels,
                sample_rates,
                buffer_frames_min,
                buffer_frames_max,
            });
        }
    }
    result.sort_by_key(|d| !d.is_default);
    result
}

/// A selected CPAL device and shared stereo renderer. Streams start lazily when
/// a Deck/Mixer is attached; selecting an unavailable named device never falls
/// back to the system speakers.
pub struct OutputDevice {
    device: cpal::Device,
    selected: cpal::SupportedStreamConfig,
    name: String,
    pub(crate) shared: Arc<Shared>,
    streams: RefCell<Vec<cpal::Stream>>,
    worker: RefCell<Option<JoinHandle<()>>>,
}
impl OutputDevice {
    pub fn open_default() -> AudioResult<Self> {
        Self::open_with_config(&AudioOutputConfig::default())
    }
    pub fn open_by_name(name: &str) -> AudioResult<Self> {
        Self::open_with_config(&AudioOutputConfig {
            device_name: Some(name.into()),
            ..Default::default()
        })
    }
    pub fn open_with_config(config: &AudioOutputConfig) -> AudioResult<Self> {
        let device = select_device(config.device_name.as_deref())?;
        let name = device
            .name()
            .map_err(|e| AudioError::Stream(e.to_string()))?;
        let mut config = config.clone();
        config.device_name = Some(name.clone());
        // A single multi-channel device needs one stream; same-name CUE is a
        // channel route, not a second independently clocked device.
        if config.cue_device_name.as_deref() == Some(&name) {
            config.cue_device_name = None;
            if config.cue_pair.is_none() {
                config.cue_pair = Some(2);
            }
        }
        let selected = select_format(&device, config.required_channels(), config.sample_rate)?;
        validate_buffer(&selected, config.buffer_frames)?;
        let cue_channels = if let Some(cue) = config.cue_device_name.as_deref() {
            let d = select_device(Some(cue))?;
            Some(
                select_format(&d, config.cue_pair.unwrap_or(0).saturating_add(2), None)?.channels(),
            )
        } else {
            None
        };
        config.validate(selected.channels(), cue_channels)?;
        let shared = Shared::new(config, selected.sample_rate().0, selected.channels());
        Ok(Self {
            device,
            selected,
            name,
            shared,
            streams: RefCell::new(Vec::new()),
            worker: RefCell::new(None),
        })
    }
    pub fn list_available() -> Vec<String> {
        list_audio_outputs().into_iter().map(|d| d.name).collect()
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn config(&self) -> AudioOutputConfig {
        self.shared.controls.lock().config.clone()
    }
    pub fn status(&self) -> AudioOutputStatus {
        self.shared.status()
    }
    pub fn sample_rate(&self) -> u32 {
        self.shared.sample_rate
    }
    pub fn elapsed_frames(&self) -> u64 {
        self.shared.consumed.load(Ordering::Relaxed)
    }
    pub fn elapsed_seconds(&self) -> f64 {
        self.elapsed_frames() as f64 / self.sample_rate() as f64
    }
    pub(crate) fn start(&self, legacy_cue: Option<&OutputDevice>) -> AudioResult<()> {
        if self.shared.running.load(Ordering::Acquire) {
            return Ok(());
        }
        let mut config = self.config();
        if let Some(cue) = legacy_cue {
            if cue.name == self.name {
                return Err(AudioError::Playback(
                    "same-device CUE must use a distinct pair in AudioOutputConfig".into(),
                ));
            }
            config.cue_device_name = Some(cue.name.clone());
            config.cue_pair = Some(0);
        }
        let cue = if let Some(name) = config.cue_device_name.as_deref() {
            let dev = select_device(Some(name))?;
            let format = select_format(&dev, config.cue_pair.unwrap_or(0).saturating_add(2), None)?;
            Some((dev, format))
        } else {
            None
        };
        config.validate(
            self.selected.channels(),
            cue.as_ref().map(|(_, f)| f.channels()),
        )?;
        self.shared.controls.lock().config = config.clone();
        let mut streams = Vec::new();
        if let Some((dev, format)) = cue {
            streams.push(build_cue(
                &dev,
                &format,
                self.shared.clone(),
                config.cue_pair.unwrap_or(0),
            )?);
            self.shared.cue_attached.store(true, Ordering::Release);
        }
        streams.push(build_main(
            &self.device,
            &self.selected,
            self.shared.clone(),
            config,
        )?);
        *self.worker.borrow_mut() = Some(start_worker(self.shared.clone()));
        // Prime bounded render FIFO before the first hardware callback.
        for _ in 0..100 {
            if self.shared.frames.len() >= self.shared.render_ahead.load(Ordering::Relaxed) as usize
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        for stream in &streams {
            if let Err(e) = stream.play() {
                self.shared.quit.store(true, Ordering::Release);
                if let Some(worker) = self.worker.borrow_mut().take() {
                    let _ = worker.join();
                }
                return Err(AudioError::Stream(e.to_string()));
            }
        }
        *self.streams.borrow_mut() = streams;
        self.shared.running.store(true, Ordering::Release);
        Ok(())
    }
}
impl Drop for OutputDevice {
    fn drop(&mut self) {
        self.shared.quit.store(true, Ordering::Release);
        self.shared.lost.store(true, Ordering::Release);
        self.streams.get_mut().clear();
        if let Some(worker) = self.worker.get_mut().take() {
            let _ = worker.join();
        }
    }
}
fn select_device(name: Option<&str>) -> AudioResult<cpal::Device> {
    let host = cpal::default_host();
    match name {
        Some(name) => host
            .output_devices()
            .map_err(|e| AudioError::Stream(e.to_string()))?
            .find(|d| d.name().ok().as_deref() == Some(name))
            .ok_or_else(|| AudioError::Stream(format!("output device not found: {name}"))),
        None => host
            .default_output_device()
            .ok_or(AudioError::NoDefaultDevice),
    }
}
fn select_format(
    device: &cpal::Device,
    channels: u16,
    rate: Option<u32>,
) -> AudioResult<cpal::SupportedStreamConfig> {
    let default = device.default_output_config().ok();
    let preferred = rate
        .or_else(|| default.as_ref().map(|d| d.sample_rate().0))
        .unwrap_or(48000);
    let mut candidates = Vec::new();
    for range in device
        .supported_output_configs()
        .map_err(|e| AudioError::Stream(e.to_string()))?
    {
        if range.channels() < channels {
            continue;
        }
        let sample_rate =
            if preferred >= range.min_sample_rate().0 && preferred <= range.max_sample_rate().0 {
                preferred
            } else if rate.is_none() {
                range
                    .max_sample_rate()
                    .0
                    .min(48000)
                    .max(range.min_sample_rate().0)
            } else {
                continue;
            };
        candidates.push(range.with_sample_rate(cpal::SampleRate(sample_rate)));
    }
    candidates.sort_by_key(|c| {
        (
            c.channels(),
            c.sample_rate().0 != preferred,
            c.sample_format() != cpal::SampleFormat::F32,
        )
    });
    candidates.into_iter().next().ok_or_else(|| {
        AudioError::Stream(format!(
            "device does not support {channels} output channels at the requested sample rate"
        ))
    })
}
fn build_main(
    device: &cpal::Device,
    format: &cpal::SupportedStreamConfig,
    shared: Arc<Shared>,
    config: AudioOutputConfig,
) -> AudioResult<cpal::Stream> {
    macro_rules! build {
        ($t:ty) => {{
            let mut callback = MainCallback {
                shared: shared.clone(),
                config,
            };
            let error = shared.clone();
            device.build_output_stream(
                &stream_config(format, shared.controls.lock().config.buffer_frames),
                move |data: &mut [$t], _| callback.write(data),
                move |_| {
                    error.lost.store(true, Ordering::Release);
                },
                None,
            )
        }};
    }
    let result = match format.sample_format() {
        cpal::SampleFormat::F32 => build!(f32),
        cpal::SampleFormat::F64 => build!(f64),
        cpal::SampleFormat::I8 => build!(i8),
        cpal::SampleFormat::I16 => build!(i16),
        cpal::SampleFormat::I32 => build!(i32),
        cpal::SampleFormat::I64 => build!(i64),
        cpal::SampleFormat::U8 => build!(u8),
        cpal::SampleFormat::U16 => build!(u16),
        cpal::SampleFormat::U32 => build!(u32),
        cpal::SampleFormat::U64 => build!(u64),
        _ => {
            return Err(AudioError::Stream(
                "unsupported output sample format".into(),
            ))
        }
    };
    result.map_err(|e| AudioError::Stream(e.to_string()))
}
fn build_cue(
    device: &cpal::Device,
    format: &cpal::SupportedStreamConfig,
    shared: Arc<Shared>,
    pair: u16,
) -> AudioResult<cpal::Stream> {
    validate_buffer(format, shared.controls.lock().config.buffer_frames)?;
    macro_rules! build {
        ($t:ty) => {{
            let mut callback = CueCallback {
                shared: shared.clone(),
                channels: format.channels() as usize,
                pair,
                ratio: shared.sample_rate as f64 / format.sample_rate().0 as f64,
                phase: 0.0,
                previous: Default::default(),
                next: Default::default(),
                primed: false,
            };
            let error = shared.clone();
            device.build_output_stream(
                &stream_config(format, shared.controls.lock().config.buffer_frames),
                move |data: &mut [$t], _| callback.write(data),
                move |_| {
                    error.lost.store(true, Ordering::Release);
                },
                None,
            )
        }};
    }
    let result = match format.sample_format() {
        cpal::SampleFormat::F32 => build!(f32),
        cpal::SampleFormat::F64 => build!(f64),
        cpal::SampleFormat::I8 => build!(i8),
        cpal::SampleFormat::I16 => build!(i16),
        cpal::SampleFormat::I32 => build!(i32),
        cpal::SampleFormat::I64 => build!(i64),
        cpal::SampleFormat::U8 => build!(u8),
        cpal::SampleFormat::U16 => build!(u16),
        cpal::SampleFormat::U32 => build!(u32),
        cpal::SampleFormat::U64 => build!(u64),
        _ => return Err(AudioError::Stream("unsupported cue sample format".into())),
    };
    result.map_err(|e| AudioError::Stream(e.to_string()))
}

fn validate_buffer(format: &cpal::SupportedStreamConfig, frames: Option<u32>) -> AudioResult<()> {
    if let (Some(n), cpal::SupportedBufferSize::Range { min, max }) = (frames, format.buffer_size())
    {
        if n < *min || n > *max {
            return Err(AudioError::Stream(format!(
                "buffer frames {n} outside device range {min}..={max}"
            )));
        }
    }
    Ok(())
}
fn stream_config(format: &cpal::SupportedStreamConfig, frames: Option<u32>) -> cpal::StreamConfig {
    let mut config = format.config();
    if let Some(n) = frames {
        config.buffer_size = cpal::BufferSize::Fixed(n);
    }
    config
}
