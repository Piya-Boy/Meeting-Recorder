//! Sample-rate conversion to the recognizer's 16 kHz.
//!
//! Windowed-sinc (Hann window, 16 zero-crossings), the kernel of PoodPim's `audio::resample`
//! (MIT, https://github.com/thirawat/PoodPim, see THIRD_PARTY_NOTICES.md), made stateful so a
//! file can be converted block by block. Output positions are computed from the absolute output
//! index, so the result does not depend on how the input is split into blocks.

use std::f64::consts::PI;

pub struct Resampler {
    from: u32,
    to: u32,
    /// Low-pass cutoff relative to the input Nyquist (below 1 when downsampling).
    cutoff: f64,
    /// Kernel half-width in input samples.
    half: i64,
    /// Input kept for the next outputs; `buf[0]` is input sample `base`.
    buf: Vec<f32>,
    base: u64,
    /// Total input samples received.
    total_in: u64,
    /// Index of the next output sample.
    next_out: u64,
}

impl Resampler {
    pub fn new(from: u32, to: u32) -> Resampler {
        let ratio = f64::from(to) / f64::from(from);
        let cutoff = ratio.min(1.0) * 0.95;
        Resampler {
            from,
            to,
            cutoff,
            half: (16.0 / cutoff).ceil() as i64,
            buf: Vec::new(),
            base: 0,
            total_in: 0,
            next_out: 0,
        }
    }

    fn identity(&self) -> bool {
        self.from == self.to
    }

    /// Input position (in samples) of output sample `n`.
    fn position(&self, n: u64) -> f64 {
        n as f64 * f64::from(self.from) / f64::from(self.to)
    }

    fn sample_at(&self, k: i64) -> Option<f32> {
        if k < 0 || k as u64 >= self.total_in {
            return None;
        }
        let k = k as u64;
        debug_assert!(k >= self.base);
        self.buf.get((k - self.base) as usize).copied()
    }

    fn output(&self, n: u64) -> f32 {
        let t = self.position(n);
        let center = t.floor() as i64;
        let mut acc = 0.0f64;
        for k in (center - self.half + 1)..=(center + self.half) {
            let Some(x) = self.sample_at(k) else { continue };
            let d = t - k as f64;
            let arg = self.cutoff * d;
            let sinc = if arg.abs() < 1e-9 {
                1.0
            } else {
                (PI * arg).sin() / (PI * arg)
            };
            let window = 0.5 + 0.5 * (PI * d / self.half as f64).cos();
            acc += f64::from(x) * self.cutoff * sinc * window;
        }
        acc as f32
    }

    fn total_out(&self) -> u64 {
        (self.total_in as f64 * f64::from(self.to) / f64::from(self.from)).floor() as u64
    }

    fn drop_consumed(&mut self) {
        let keep_from = (self.position(self.next_out).floor() as i64 - self.half).max(0) as u64;
        if keep_from > self.base {
            let n = ((keep_from - self.base) as usize).min(self.buf.len());
            self.buf.drain(..n);
            self.base += n as u64;
        }
    }

    /// Converts a block; outputs whose kernel still needs later input wait for the next call.
    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if self.identity() {
            return input.to_vec();
        }
        self.buf.extend_from_slice(input);
        self.total_in += input.len() as u64;
        let mut out = Vec::new();
        while self.next_out < self.total_out() {
            let t = self.position(self.next_out);
            if t.floor() as i64 + self.half >= self.total_in as i64 {
                break; // needs input that has not arrived yet
            }
            out.push(self.output(self.next_out));
            self.next_out += 1;
        }
        self.drop_consumed();
        out
    }

    /// The outputs that were waiting for input that never came (end of the file).
    pub fn finish(&mut self) -> Vec<f32> {
        if self.identity() {
            return Vec::new();
        }
        let mut out = Vec::new();
        while self.next_out < self.total_out() {
            out.push(self.output(self.next_out));
            self.next_out += 1;
        }
        self.drop_consumed();
        out
    }
}

/// The whole of `input` converted at once (the tests' shortcut).
#[cfg(test)]
pub fn resample_all(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    let mut r = Resampler::new(from, to);
    let mut out = r.process(input);
    out.extend(r.finish());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f64, rate: u32, secs: f64) -> Vec<f32> {
        let n = (f64::from(rate) * secs) as usize;
        (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / f64::from(rate)).sin() as f32)
            .collect()
    }

    fn crossings(x: &[f32]) -> usize {
        x.windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0 || w[0] >= 0.0 && w[1] < 0.0)
            .count()
    }

    fn peak(x: &[f32]) -> f32 {
        x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
    }

    #[test]
    fn length_follows_the_ratio() {
        let out = resample_all(&sine(1000.0, 48_000, 1.0), 48_000, 16_000);
        assert!((out.len() as i64 - 16_000).abs() <= 1, "{}", out.len());
        let out = resample_all(&vec![0.1; 44_100], 44_100, 16_000);
        assert!((out.len() as i64 - 16_000).abs() <= 1, "{}", out.len());
        let n = 12_345;
        let out = resample_all(&vec![0.1; n], 44_100, 16_000);
        let expect = (n as f64 * 16_000.0 / 44_100.0).round() as i64;
        assert!(
            (out.len() as i64 - expect).abs() <= 1,
            "{} vs {expect}",
            out.len()
        );
    }

    #[test]
    fn tone_keeps_its_frequency() {
        let out = resample_all(&sine(1000.0, 48_000, 1.0), 48_000, 16_000);
        let c = crossings(&out) as f64;
        assert!((c - 2000.0).abs() / 2000.0 < 0.01, "{c}");
        assert!(peak(&out[200..out.len() - 200]) > 0.9);
    }

    #[test]
    fn chunked_equals_whole() {
        let src = sine(700.0, 48_000, 1.0);
        let whole = resample_all(&src, 48_000, 16_000);
        let mut r = Resampler::new(48_000, 16_000);
        let mut out = Vec::new();
        for block in src.chunks(7_001) {
            out.extend(r.process(block));
        }
        out.extend(r.finish());
        assert_eq!(out.len(), whole.len());
        for (a, b) in out.iter().zip(&whole) {
            assert!((a - b).abs() < 1e-4);
        }
    }

    #[test]
    fn same_rate_is_identity() {
        let src = sine(500.0, 16_000, 0.2);
        assert_eq!(resample_all(&src, 16_000, 16_000), src);
        let mut r = Resampler::new(16_000, 16_000);
        let mut out = r.process(&src);
        out.extend(r.finish());
        assert_eq!(out, src);
    }

    #[test]
    fn tone_above_the_new_nyquist_is_removed() {
        let out = resample_all(&sine(12_000.0, 48_000, 1.0), 48_000, 16_000);
        assert!(
            peak(&out[200..out.len() - 200]) < 0.1,
            "{}",
            peak(&out[200..out.len() - 200])
        );
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert!(resample_all(&[], 48_000, 16_000).is_empty());
        let mut r = Resampler::new(48_000, 16_000);
        assert!(r.process(&[]).is_empty());
        assert!(r.finish().is_empty());
    }
}
