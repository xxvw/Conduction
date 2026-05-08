//! Schroeder reverb (4 comb + 2 allpass)。チャンネル毎に独立したインスタンスを使う。
//!
//! 軽量実装のため、超大型のホール感は出ないが DJ ミキサー上のテクスチャエフェクト
//! として実用的。`set_room` でフィードバック量を 0.5..=0.95 にスケール。

struct CombFilter {
    buffer: Vec<f32>,
    write_pos: usize,
    feedback: f32,
}

impl CombFilter {
    fn new(delay_samples: usize) -> Self {
        Self {
            buffer: vec![0.0; delay_samples.max(1)],
            write_pos: 0,
            feedback: 0.7,
        }
    }

    fn set_feedback(&mut self, fb: f32) {
        self.feedback = fb.clamp(0.0, 0.97);
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.write_pos];
        self.buffer[self.write_pos] = input + output * self.feedback;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
        output
    }
}

struct AllPassFilter {
    buffer: Vec<f32>,
    write_pos: usize,
}

impl AllPassFilter {
    fn new(delay_samples: usize) -> Self {
        Self {
            buffer: vec![0.0; delay_samples.max(1)],
            write_pos: 0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let buf_out = self.buffer[self.write_pos];
        let output = -input + buf_out;
        self.buffer[self.write_pos] = input + buf_out * 0.5;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
        output
    }
}

pub struct SchroederReverb {
    combs: Vec<CombFilter>,
    allpasses: Vec<AllPassFilter>,
}

impl SchroederReverb {
    pub fn new(sample_rate: f32) -> Self {
        // Schroeder 1962 標準遅延長を sample rate にスケール。
        let comb_lens = [
            (0.0297 * sample_rate) as usize,
            (0.0371 * sample_rate) as usize,
            (0.0411 * sample_rate) as usize,
            (0.0437 * sample_rate) as usize,
        ];
        let allpass_lens = [
            (0.0050 * sample_rate) as usize,
            (0.0017 * sample_rate) as usize,
        ];
        Self {
            combs: comb_lens.iter().map(|&l| CombFilter::new(l)).collect(),
            allpasses: allpass_lens.iter().map(|&l| AllPassFilter::new(l)).collect(),
        }
    }

    /// `room` は 0..1。フィードバックを 0.5 〜 0.95 にマップ。
    pub fn set_room(&mut self, room: f32) {
        let r = room.clamp(0.0, 1.0);
        let fb = 0.5 + r * 0.45;
        for c in &mut self.combs {
            c.set_feedback(fb);
        }
    }

    /// 入力サンプルを反響させて返す。出力は `wet` 信号のみ（呼び出し側で dry/wet ミックス）。
    pub fn process(&mut self, input: f32) -> f32 {
        let mut out: f32 = self.combs.iter_mut().map(|c| c.process(input)).sum();
        out *= 0.25; // 4 comb 合算の正規化
        for ap in &mut self.allpasses {
            out = ap.process(out);
        }
        out
    }
}
