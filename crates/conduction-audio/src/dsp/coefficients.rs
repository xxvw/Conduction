//! biquad 0.5 がサポートしない peaking / shelf を含む係数生成。
//! RBJ Audio EQ Cookbook 準拠。

use biquad::Coefficients;

const TWO_PI: f32 = std::f32::consts::TAU;

fn normalize(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Coefficients<f32> {
    Coefficients {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

/// Peaking EQ。`db_gain` は中心周波数におけるゲイン (dB)。
pub fn peaking_eq(fs: f32, f0: f32, q: f32, db_gain: f32) -> Coefficients<f32> {
    let a = 10.0_f32.powf(db_gain / 40.0);
    let omega = TWO_PI * f0 / fs;
    let cos_omega = omega.cos();
    let alpha = omega.sin() / (2.0 * q.max(1e-3));

    let b0 = 1.0 + alpha * a;
    let b1 = -2.0 * cos_omega;
    let b2 = 1.0 - alpha * a;
    let a0 = 1.0 + alpha / a;
    let a1 = -2.0 * cos_omega;
    let a2 = 1.0 - alpha / a;
    normalize(b0, b1, b2, a0, a1, a2)
}

/// Low Shelf。`db_gain` は低域の最終的なゲイン (dB)。
pub fn low_shelf(fs: f32, f0: f32, q: f32, db_gain: f32) -> Coefficients<f32> {
    let a = 10.0_f32.powf(db_gain / 40.0);
    let omega = TWO_PI * f0 / fs;
    let cos_omega = omega.cos();
    let sin_omega = omega.sin();
    let alpha = sin_omega / (2.0 * q.max(1e-3));
    let beta = (a / q.max(1e-3)).sqrt() * 2.0 * alpha;

    let b0 = a * ((a + 1.0) - (a - 1.0) * cos_omega + beta * sin_omega);
    let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_omega);
    let b2 = a * ((a + 1.0) - (a - 1.0) * cos_omega - beta * sin_omega);
    let a0 = (a + 1.0) + (a - 1.0) * cos_omega + beta * sin_omega;
    let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_omega);
    let a2 = (a + 1.0) + (a - 1.0) * cos_omega - beta * sin_omega;
    normalize(b0, b1, b2, a0, a1, a2)
}

/// High Shelf。`db_gain` は高域の最終的なゲイン (dB)。
pub fn high_shelf(fs: f32, f0: f32, q: f32, db_gain: f32) -> Coefficients<f32> {
    let a = 10.0_f32.powf(db_gain / 40.0);
    let omega = TWO_PI * f0 / fs;
    let cos_omega = omega.cos();
    let sin_omega = omega.sin();
    let alpha = sin_omega / (2.0 * q.max(1e-3));
    let beta = (a / q.max(1e-3)).sqrt() * 2.0 * alpha;

    let b0 = a * ((a + 1.0) + (a - 1.0) * cos_omega + beta * sin_omega);
    let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_omega);
    let b2 = a * ((a + 1.0) + (a - 1.0) * cos_omega - beta * sin_omega);
    let a0 = (a + 1.0) - (a - 1.0) * cos_omega + beta * sin_omega;
    let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_omega);
    let a2 = (a + 1.0) - (a - 1.0) * cos_omega - beta * sin_omega;
    normalize(b0, b1, b2, a0, a1, a2)
}
