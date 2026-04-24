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
}

impl OutputDevice {
    /// OS のデフォルト出力デバイスを開く。
    pub fn open_default() -> AudioResult<Self> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|e| AudioError::Stream(e.to_string()))?;
        info!("audio output device opened (default)");
        Ok(Self {
            _stream: stream,
            handle,
        })
    }

    pub(crate) fn handle(&self) -> &OutputStreamHandle {
        &self.handle
    }
}
