//! シンプルなフィードバック付きディレイ（モノラル単位、チャンネル毎にインスタンス化）。

pub struct EchoEffect {
    buffer: Vec<f32>,
    write_pos: usize,
    sample_rate: f32,
}

impl EchoEffect {
    pub fn new(sample_rate: f32) -> Self {
        let max_samples = (2.5 * sample_rate) as usize; // up to 2.5 sec
        Self {
            buffer: vec![0.0; max_samples.max(1)],
            write_pos: 0,
            sample_rate,
        }
    }

    pub fn process(&mut self, input: f32, time_ms: f32, feedback: f32, wet: f32) -> f32 {
        if wet < 0.001 {
            // ドライそのまま。バッファ更新は止めて状態をリセット気味に。
            self.buffer[self.write_pos] = input;
            self.write_pos = (self.write_pos + 1) % self.buffer.len();
            return input;
        }
        let time_ms = time_ms.clamp(10.0, 2400.0);
        let delay_samples =
            ((time_ms / 1000.0) * self.sample_rate) as usize;
        let delay_samples = delay_samples.clamp(1, self.buffer.len() - 1);
        let read_pos =
            (self.write_pos + self.buffer.len() - delay_samples) % self.buffer.len();
        let delayed = self.buffer[read_pos];
        let fb = feedback.clamp(0.0, 0.92);
        // フィードバックループ: 入力 + 過去の出力 * fb
        self.buffer[self.write_pos] = input + delayed * fb;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
        let wet = wet.clamp(0.0, 1.0);
        input * (1.0 - wet) + delayed * wet
    }
}
