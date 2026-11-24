//! Log-mel features matching NeMo's AudioToMelSpectrogramPreprocessor, as used by
//! OpenThai2.0-ASR-Streaming: 16 kHz, 128 mels, 25 ms Hann window, 10 ms hop,
//! n_fft 512, pre-emphasis 0.97, Slaney mel filters, log(x + 2^-24), no normalisation.
//!
//! Ported from PoodPim `src/features.rs` (commit 15e07cc, MIT, Copyright (c) 2026 Thirawat
//! Sutalungka, https://github.com/thirawat/PoodPim). See THIRD_PARTY_NOTICES.md.

use std::sync::OnceLock;

use realfft::RealFftPlanner;

pub const SAMPLE_RATE: u32 = 16_000;
const N_FFT: usize = 512;
const WIN_LENGTH: usize = 400;
const HOP_LENGTH: usize = 160;
pub const N_MELS: usize = 128;
const N_BINS: usize = N_FFT / 2 + 1;
const PREEMPH: f64 = 0.97;
const LOG_GUARD: f64 = 5.960_464_477_539_063e-8; // 2^-24

fn hz_to_mel(f: f64) -> f64 {
    let f_sp = 200.0 / 3.0;
    let min_log_hz = 1000.0;
    let min_log_mel = min_log_hz / f_sp;
    let logstep = 6.4f64.ln() / 27.0;
    if f >= min_log_hz {
        min_log_mel + (f / min_log_hz).ln() / logstep
    } else {
        f / f_sp
    }
}

fn mel_to_hz(m: f64) -> f64 {
    let f_sp = 200.0 / 3.0;
    let min_log_hz = 1000.0;
    let min_log_mel = min_log_hz / f_sp;
    let logstep = 6.4f64.ln() / 27.0;
    if m >= min_log_mel {
        min_log_hz * (logstep * (m - min_log_mel)).exp()
    } else {
        f_sp * m
    }
}

/// librosa.filters.mel(sr=16000, n_fft=512, n_mels=128, htk=False, norm="slaney"), row-major [N_MELS][N_BINS].
fn mel_filterbank() -> &'static [f64] {
    static FB: OnceLock<Vec<f64>> = OnceLock::new();
    FB.get_or_init(|| {
        let sr = SAMPLE_RATE as f64;
        let fftfreqs: Vec<f64> = (0..N_BINS)
            .map(|i| i as f64 * (sr / 2.0) / (N_BINS - 1) as f64)
            .collect();
        let (lo, hi) = (hz_to_mel(0.0), hz_to_mel(sr / 2.0));
        let mel_f: Vec<f64> = (0..N_MELS + 2)
            .map(|i| mel_to_hz(lo + (hi - lo) * i as f64 / (N_MELS + 1) as f64))
            .collect();
        let mut w = vec![0.0; N_MELS * N_BINS];
        for m in 0..N_MELS {
            let enorm = 2.0 / (mel_f[m + 2] - mel_f[m]);
            for (k, &f) in fftfreqs.iter().enumerate() {
                let lower = (f - mel_f[m]) / (mel_f[m + 1] - mel_f[m]);
                let upper = (mel_f[m + 2] - f) / (mel_f[m + 2] - mel_f[m + 1]);
                w[m * N_BINS + k] = lower.min(upper).max(0.0) * enorm;
            }
        }
        w
    })
}

/// torch.hann_window(400, periodic=False) zero-padded and centred in n_fft, as torch.stft does.
fn window() -> &'static [f64] {
    static W: OnceLock<Vec<f64>> = OnceLock::new();
    W.get_or_init(|| {
        let left = (N_FFT - WIN_LENGTH) / 2;
        let mut w = vec![0.0; N_FFT];
        for i in 0..WIN_LENGTH {
            w[left + i] =
                0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / (WIN_LENGTH - 1) as f64).cos();
        }
        w
    })
}

/// Returns (features in [N_MELS][T] row-major order, T).
pub fn log_mel(audio: &[f32]) -> (Vec<f32>, usize) {
    if audio.is_empty() {
        return (Vec::new(), 0);
    }
    let pad = N_FFT / 2;
    let mut x = vec![0.0f64; audio.len() + 2 * pad];
    x[pad] = audio[0] as f64;
    for i in 1..audio.len() {
        x[pad + i] = audio[i] as f64 - PREEMPH * audio[i - 1] as f64;
    }
    let n_frames = 1 + (x.len() - N_FFT) / HOP_LENGTH;

    let mut planner = RealFftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(N_FFT);
    let mut input = fft.make_input_vec();
    let mut spectrum = fft.make_output_vec();
    let win = window();
    let fb = mel_filterbank();
    let mut power = vec![0.0f64; N_BINS];
    let mut out = vec![0.0f32; N_MELS * n_frames];

    for t in 0..n_frames {
        let start = t * HOP_LENGTH;
        for i in 0..N_FFT {
            input[i] = x[start + i] * win[i];
        }
        fft.process(&mut input, &mut spectrum).expect("fft");
        for (p, c) in power.iter_mut().zip(spectrum.iter()) {
            *p = c.re * c.re + c.im * c.im;
        }
        for m in 0..N_MELS {
            let row = &fb[m * N_BINS..(m + 1) * N_BINS];
            let e: f64 = row.iter().zip(power.iter()).map(|(a, b)| a * b).sum();
            out[m * n_frames + t] = (e + LOG_GUARD).ln() as f32;
        }
    }
    (out, n_frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::features_reference::{FRAMES, VALUES};

    fn test_signal() -> Vec<f32> {
        (0..16_000)
            .map(|n| {
                let t = f64::from(n) / 16_000.0;
                (0.5 * (2.0 * std::f64::consts::PI * 440.0 * t).sin()
                    + 0.25 * (2.0 * std::f64::consts::PI * 3000.0 * t).sin()) as f32
            })
            .collect()
    }

    #[test]
    fn shape_matches_reference() {
        let (f, t) = log_mel(&test_signal());
        assert_eq!(t, FRAMES);
        assert_eq!(f.len(), N_MELS * FRAMES);
    }

    #[test]
    fn values_match_reference() {
        let (f, t) = log_mel(&test_signal());
        for (band, frame, want) in VALUES {
            let got = f[band * t + frame];
            assert!(
                (got - want).abs() < 1e-3,
                "({band},{frame}): {got} vs {want}"
            );
        }
    }

    #[test]
    fn silence_is_constant_and_finite() {
        let (f, t) = log_mel(&vec![0.0; 8_000]);
        assert!(t > 0);
        assert!(f.iter().all(|v| v.is_finite()));
        assert!(
            f.iter().all(|v| (*v - f[0]).abs() < 1e-6),
            "silence must give one value"
        );
    }

    #[test]
    fn empty_input_does_not_panic() {
        let (f, t) = log_mel(&[]);
        assert!(f.is_empty());
        assert_eq!(t, 0);
    }

    #[test]
    fn constants_match_the_model_contract() {
        assert_eq!(SAMPLE_RATE, 16_000);
        assert_eq!(N_MELS, 128);
    }
}
