use cpal::traits::{DeviceTrait, HostTrait};
use rodio::{OutputStream, OutputStreamHandle};
use tracing::info;

use crate::error::{AudioError, AudioResult};

/// 出力オーディオデバイスのハンドル保持者。
///
/// `OutputStream` は drop されるとデバイスを閉じるため、プレイヤー/ミキサーのライフタイムを
/// 通じて保持される必要がある。複数の `Deck` が同一 `OutputDevice` の
/// `OutputStreamHandle` を共有することで、rodio が自動的にサムミキシングする。
pub struct OutputDevice {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    name: String,
}

impl OutputDevice {
    /// OS のデフォルト出力デバイスを開く。
    pub fn open_default() -> AudioResult<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AudioError::Stream("no default output device".into()))?;
        let name = device.name().unwrap_or_else(|_| "default".into());
        let (stream, handle) = OutputStream::try_from_device(&device)
            .map_err(|e| AudioError::Stream(e.to_string()))?;
        info!(device = %name, "audio output device opened (default)");
        Ok(Self {
            _stream: stream,
            handle,
            name,
        })
    }

    /// 名前で出力デバイスを指定して開く。完全一致の最初のデバイスを選ぶ。
    pub fn open_by_name(name: &str) -> AudioResult<Self> {
        let host = cpal::default_host();
        let device = host
            .output_devices()
            .map_err(|e| AudioError::Stream(format!("enumerate devices: {e}")))?
            .find(|d| d.name().ok().as_deref() == Some(name))
            .ok_or_else(|| AudioError::Stream(format!("output device not found: {name}")))?;
        let resolved_name = device.name().unwrap_or_else(|_| name.into());
        let (stream, handle) = OutputStream::try_from_device(&device)
            .map_err(|e| AudioError::Stream(e.to_string()))?;
        info!(device = %resolved_name, "audio output device opened (by name)");
        Ok(Self {
            _stream: stream,
            handle,
            name: resolved_name,
        })
    }

    /// 利用可能な出力デバイス名の一覧。デフォルトデバイスが先頭。
    pub fn list_available() -> Vec<String> {
        let host = cpal::default_host();
        let mut names: Vec<String> = host
            .output_devices()
            .ok()
            .map(|iter| iter.filter_map(|d| d.name().ok()).collect())
            .unwrap_or_default();
        // デフォルトデバイスを先頭に揃える
        if let Some(default_name) = host
            .default_output_device()
            .and_then(|d| d.name().ok())
        {
            if let Some(pos) = names.iter().position(|n| n == &default_name) {
                names.swap(0, pos);
            }
        }
        names
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn handle(&self) -> &OutputStreamHandle {
        &self.handle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 実機依存。ローカルで `cargo test -p conduction-audio -- --ignored list_outputs`
    /// として手動実行する。
    #[test]
    #[ignore = "depends on host audio devices"]
    fn list_outputs() {
        let devices = OutputDevice::list_available();
        eprintln!("Available output devices ({}):", devices.len());
        for d in &devices {
            eprintln!("  - {}", d);
        }
        assert!(!devices.is_empty(), "should have at least one output device");
    }

    #[test]
    #[ignore = "depends on host audio devices"]
    fn open_default_then_by_name() {
        let default = OutputDevice::open_default().expect("open default");
        eprintln!("default device name: {}", default.name());
        let by_name = OutputDevice::open_by_name(default.name()).expect("open by name");
        assert_eq!(default.name(), by_name.name());
    }
}
