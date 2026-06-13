// Level meter: tính RMS + peak từ buffer i16, normalize 0..1 cho UI.
#[derive(serde::Serialize, Clone, Copy)]
pub struct Level {
    pub rms: f32,
    pub peak: f32,
}

pub fn compute(samples: &[i16]) -> Level {
    if samples.is_empty() {
        return Level { rms: 0.0, peak: 0.0 };
    }
    let mut peak = 0i32;
    let mut sum_sq = 0f64;
    for &s in samples {
        let a = (s as i32).abs();
        if a > peak {
            peak = a;
        }
        let f = s as f64;
        sum_sq += f * f;
    }
    let rms = (sum_sq / samples.len() as f64).sqrt() / 32768.0;
    Level {
        rms: rms as f32,
        peak: peak as f32 / 32768.0,
    }
}
