//! Cutting a recording into segments the recognizer can take, at pauses.
//!
//! The model decodes a whole segment at once and drops words that follow long near-silent
//! pauses, so audio is cut where people stop talking. The cut rules are PoodPim's
//! `src/segment.rs` (commit 15e07cc, MIT, Copyright (c) 2026 Thirawat Sutalungka,
//! https://github.com/thirawat/PoodPim; see THIRD_PARTY_NOTICES.md):
//! - a pause of at least 0.5 s once the segment is at least 8 s long cuts in the middle of
//!   the pause;
//! - a pause of at least 1.2 s once the segment is at least 2 s long cuts;
//! - no pause by 25 s cuts at the quietest 0.2 s in the last 6 s.
//!
//! Differences from PoodPim: the rules look at a fixed 30 s window of the pending audio, so
//! the cuts do not depend on how the audio arrives in blocks and a file of any length is
//! handled in a bounded amount of memory; each segment is trimmed of leading and trailing
//! silence; segments shorter than 0.3 s or without any sound are dropped.

use std::io;
use std::path::Path;

use crate::asr::resample::Resampler;
use crate::asr::wavread::WavReader;

const FRAME_MS: usize = 20;
const MIN_SEG_S: f32 = 8.0;
const SHORT_MIN_SEG_S: f32 = 2.0;
const PAUSE_S: f32 = 0.5;
const LONG_PAUSE_S: f32 = 1.2;
const MAX_SEG_S: f32 = 25.0;
/// Pending audio the cut rules look at; longer than `MAX_SEG_S` so a cut always exists.
const WINDOW_S: f32 = 30.0;
/// Segments shorter than this are dropped (seconds).
const MIN_SEGMENT_S: f32 = 0.3;
/// Frames quieter than this count as silence when trimming (dBFS, rms).
const SOUND_DB: f32 = -50.0;
/// Kept around the sound when trimming (seconds).
const TRIM_PAD_S: f32 = 0.25;

fn frame_db(audio: &[f32], frame: usize) -> Vec<f32> {
    audio
        .chunks_exact(frame)
        .map(|f| {
            let ms = f.iter().map(|v| v * v).sum::<f32>() / frame as f32;
            10.0 * (ms + 1e-12).log10()
        })
        .collect()
}

fn percentile(v: &[f32], p: f32) -> f32 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    s[((s.len() - 1) as f32 * p) as usize]
}

/// Length of one frame of `quiet_frames`, in ms.
pub const QUIET_FRAME_MS: usize = FRAME_MS;

/// Which 20 ms frames of `audio` are silent, by the same measure the cuts use. Empty when the
/// audio is shorter than one frame. (PoodPim `quiet_frames`.)
pub fn quiet_frames(audio: &[f32], rate: usize) -> Vec<bool> {
    let frame = rate * FRAME_MS / 1000;
    let db = frame_db(audio, frame);
    if db.is_empty() {
        return Vec::new();
    }
    let threshold = (percentile(&db, 0.1) + 8.0)
        .min(percentile(&db, 0.9) - 12.0)
        .max(-60.0);
    db.iter().map(|&d| d < threshold).collect()
}

/// Cut position (samples from the start of `pending`) if a segment is ready.
fn find_cut(pending: &[f32], rate: usize) -> Option<usize> {
    let frame = rate * FRAME_MS / 1000;
    let db = frame_db(pending, frame);
    let secs = |frames: usize| frames as f32 * FRAME_MS as f32 / 1000.0;
    if secs(db.len()) < SHORT_MIN_SEG_S {
        return None;
    }
    // Silence = well below this stretch's speech level and near its noise floor.
    let floor = percentile(&db, 0.1);
    let speech = percentile(&db, 0.9);
    let threshold = (floor + 8.0).min(speech - 12.0).max(-60.0);
    let wanted = |at: f32, run: f32| {
        (at >= MIN_SEG_S && run >= PAUSE_S) || (at >= SHORT_MIN_SEG_S && run >= LONG_PAUSE_S)
    };

    let mut run_start = None;
    for (i, &d) in db.iter().enumerate() {
        if d < threshold {
            run_start.get_or_insert(i);
            continue;
        }
        if let Some(rs) = run_start.take() {
            if wanted(secs(rs), secs(i - rs)) {
                return Some((rs + i) / 2 * frame);
            }
        }
    }
    // A pause still going on at the end of the audio.
    if let Some(rs) = run_start {
        if wanted(secs(rs), secs(db.len() - rs)) {
            return Some((rs + db.len()) / 2 * frame);
        }
    }
    // No pause: force a cut at the quietest 0.2 s window in the last 6 s before the limit.
    let max_frames = (MAX_SEG_S * 1000.0) as usize / FRAME_MS;
    if db.len() >= max_frames {
        let win = 200 / FRAME_MS;
        let from = max_frames - 6000 / FRAME_MS;
        let best = (from..max_frames - win)
            .min_by(|&a, &b| {
                db[a..a + win]
                    .iter()
                    .sum::<f32>()
                    .total_cmp(&db[b..b + win].iter().sum::<f32>())
            })
            .unwrap_or(max_frames - win);
        return Some((best + win / 2) * frame);
    }
    None
}

/// Range of `seg` from the first to the last loud 20 ms frame, with padding; `None` when
/// nothing in it is loud.
fn trim(seg: &[f32], rate: usize) -> Option<(usize, usize)> {
    let frame = (rate / 50).max(1);
    let loud: Vec<usize> = frame_db(seg, frame)
        .iter()
        .enumerate()
        .filter(|(_, &d)| d > SOUND_DB) // 10 log10(mean square) is the rms level in dBFS
        .map(|(i, _)| i)
        .collect();
    let (first, last) = (*loud.first()?, *loud.last()?);
    let pad = (TRIM_PAD_S * rate as f32) as usize;
    Some((
        (first * frame).saturating_sub(pad),
        ((last + 1) * frame + pad).min(seg.len()),
    ))
}

/// Cuts audio that arrives in blocks into segments, holding only the unfinished tail.
pub struct SegmentStream {
    rate: usize,
    /// Audio after the last cut; `buf[0]` is sample `offset` of the whole recording.
    buf: Vec<f32>,
    offset: usize,
}

impl SegmentStream {
    pub fn new(rate: u32) -> SegmentStream {
        SegmentStream {
            rate: rate as usize,
            buf: Vec::new(),
            offset: 0,
        }
    }

    /// Samples held back for the next cut (checked by the tests).
    #[cfg(test)]
    pub fn buffered_samples(&self) -> usize {
        self.buf.len()
    }

    fn emit(&mut self, cut: usize, on_segment: &mut dyn FnMut(usize, &[f32])) {
        let seg = &self.buf[..cut];
        if let Some((a, b)) = trim(seg, self.rate) {
            if b - a >= (MIN_SEGMENT_S * self.rate as f32) as usize {
                on_segment(self.offset + a, &seg[a..b]);
            }
        }
        self.buf.drain(..cut);
        self.offset += cut;
    }

    /// Adds a block; calls `on_segment(start_sample, samples)` for each segment it completes.
    pub fn push(&mut self, block: &[f32], on_segment: &mut dyn FnMut(usize, &[f32])) {
        self.buf.extend_from_slice(block);
        let window = (WINDOW_S * self.rate as f32) as usize;
        while self.buf.len() >= window {
            let cut = match find_cut(&self.buf[..window], self.rate) {
                Some(c) if c > 0 => c,
                _ => window,
            };
            self.emit(cut, on_segment);
        }
    }

    /// The end of the recording: the rest is cut and emitted.
    pub fn finish(&mut self, on_segment: &mut dyn FnMut(usize, &[f32])) {
        while let Some(cut) = find_cut(&self.buf, self.rate) {
            if cut == 0 {
                break;
            }
            self.emit(cut, on_segment);
        }
        let rest = self.buf.len();
        if rest > 0 {
            self.emit(rest, on_segment);
        }
    }
}

/// The segments of a whole recording as `(start, end)` sample ranges (the tests' shortcut).
#[cfg(test)]
pub fn split(audio: &[f32], rate: u32) -> Vec<(usize, usize)> {
    let mut stream = SegmentStream::new(rate);
    let mut out = Vec::new();
    stream.push(audio, &mut |start, seg| {
        out.push((start, start + seg.len()))
    });
    stream.finish(&mut |start, seg| out.push((start, start + seg.len())));
    out
}

/// Seconds of audio read from a file per block.
const BLOCK_S: usize = 30;

/// Reads a WAV file in blocks, converts it to 16 kHz and calls `on_segment(start_sample,
/// samples)` (16 kHz sample numbers) for each segment. `progress(done_secs, total_secs)` is
/// called after every block; when it returns `false` the file is abandoned and `Ok` returned.
pub fn segment_file(
    path: &Path,
    on_segment: &mut dyn FnMut(usize, &[f32]),
    progress: &mut dyn FnMut(f32, f32) -> bool,
) -> io::Result<()> {
    let mut reader = WavReader::open(path)?;
    let rate = reader.rate();
    let total_secs = reader.total_frames() as f32 / rate as f32;
    let mut resampler = Resampler::new(rate, crate::asr::features::SAMPLE_RATE);
    let mut stream = SegmentStream::new(crate::asr::features::SAMPLE_RATE);
    let mut done_frames = 0u64;
    loop {
        let block = reader.read_block(BLOCK_S * rate as usize)?;
        if block.is_empty() {
            break;
        }
        done_frames += block.len() as u64;
        stream.push(&resampler.process(&block), on_segment);
        if !progress(done_frames as f32 / rate as f32, total_secs) {
            return Ok(());
        }
    }
    stream.push(&resampler.finish(), on_segment);
    stream.finish(on_segment);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::wav::WavWriter;

    const RATE: usize = 16_000;

    fn tone(secs: f32) -> Vec<f32> {
        (0..(secs * RATE as f32) as usize)
            .map(|i| 0.3 * (i as f32 * 0.07).sin())
            .collect()
    }

    fn silence(secs: f32) -> Vec<f32> {
        vec![0.0; (secs * RATE as f32) as usize]
    }

    fn cat(parts: &[Vec<f32>]) -> Vec<f32> {
        parts.iter().flatten().copied().collect()
    }

    fn secs(samples: usize) -> f32 {
        samples as f32 / RATE as f32
    }

    #[test]
    fn two_utterances_with_a_pause_are_two_segments() {
        let a = cat(&[tone(3.0), silence(2.0), tone(3.0)]);
        let segs = split(&a, RATE as u32);
        assert_eq!(segs.len(), 2, "{segs:?}");
        assert!(
            (3.0..3.5).contains(&secs(segs[0].1)),
            "end {}",
            secs(segs[0].1)
        );
        assert!(
            (4.5..5.2).contains(&secs(segs[1].0)),
            "start {}",
            secs(segs[1].0)
        );
        assert_eq!(segs[1].1, a.len());
    }

    #[test]
    fn pause_in_a_long_stretch_cuts() {
        // 10 s stretch (above 8 s) with a 0.6 s pause (at least 0.5 s)
        let a = cat(&[tone(10.0), silence(0.6), tone(5.0)]);
        assert_eq!(split(&a, RATE as u32).len(), 2);
    }

    #[test]
    fn pause_in_a_short_stretch_does_not_cut() {
        // 5 s stretch (below 8 s) with a 0.6 s pause (below 1.2 s)
        let a = cat(&[tone(5.0), silence(0.6), tone(5.0)]);
        assert_eq!(split(&a, RATE as u32).len(), 1);
    }

    #[test]
    fn long_pause_in_a_short_stretch_cuts() {
        // 5 s stretch (above 2 s) with a 1.3 s pause (at least 1.2 s)
        let a = cat(&[tone(5.0), silence(1.3), tone(5.0)]);
        assert_eq!(split(&a, RATE as u32).len(), 2);
    }

    #[test]
    fn continuous_speech_is_cut_at_25_seconds() {
        let a = tone(40.0);
        let segs = split(&a, RATE as u32);
        assert!(segs.len() >= 2, "{segs:?}");
        for (s, e) in &segs {
            assert!(secs(e - s) <= 25.1, "segment of {} s", secs(e - s));
        }
        // nothing is lost: the segments cover the whole signal
        let covered: usize = segs.iter().map(|(s, e)| e - s).sum();
        assert!(
            covered + RATE / 2 >= a.len(),
            "covered {covered} of {}",
            a.len()
        );
    }

    #[test]
    fn a_30_second_gap_is_not_inside_any_segment() {
        let a = cat(&[tone(3.0), silence(30.0), tone(3.0)]);
        let segs = split(&a, RATE as u32);
        assert_eq!(segs.len(), 2, "{segs:?}");
        for (s, e) in &segs {
            assert!(
                secs(e - s) < 4.5,
                "segment of {} s holds silence",
                secs(e - s)
            );
        }
        assert!(
            secs(segs[1].0) > 32.0,
            "second segment starts at {}",
            secs(segs[1].0)
        );
    }

    #[test]
    fn quiet_frames_flag_the_silence() {
        let a = cat(&[tone(1.0), silence(1.0), tone(1.0)]);
        let q = quiet_frames(&a, RATE);
        assert_eq!(q.len(), 150);
        assert!(!q[10] && q[75] && !q[140]);
        assert!(quiet_frames(&[0.0; 10], RATE).is_empty());
    }

    #[test]
    fn very_short_utterances_are_dropped() {
        assert!(split(&tone(0.2), RATE as u32).is_empty());
        assert_eq!(split(&tone(0.5), RATE as u32).len(), 1);
    }

    #[test]
    fn silence_and_faint_noise_give_no_segments() {
        assert!(split(&silence(20.0), RATE as u32).is_empty());
        let faint: Vec<f32> = (0..RATE * 10)
            .map(|i| 0.0005 * (i as f32 * 0.3).sin())
            .collect();
        assert!(split(&faint, RATE as u32).is_empty());
        assert!(split(&[], RATE as u32).is_empty());
    }

    fn collect(blocks: &[f32], block: usize) -> Vec<(usize, usize)> {
        let mut stream = SegmentStream::new(RATE as u32);
        let mut out = Vec::new();
        for b in blocks.chunks(block) {
            stream.push(b, &mut |start, seg| out.push((start, start + seg.len())));
        }
        stream.finish(&mut |start, seg| out.push((start, start + seg.len())));
        out
    }

    fn long_mixed_signal() -> Vec<f32> {
        cat(&[
            tone(3.0),
            silence(2.0),
            tone(12.0),
            silence(1.0),
            tone(40.0),
            silence(3.0),
            tone(5.0),
            silence(30.0),
            tone(2.0),
        ])
    }

    #[test]
    fn block_size_does_not_change_the_segments() {
        let a = long_mixed_signal();
        let whole = split(&a, RATE as u32);
        assert!(whole.len() >= 5, "{whole:?}");
        for block in [1_000, 16_000, 100_000] {
            assert_eq!(collect(&a, block), whole, "block {block}");
        }
    }

    #[test]
    fn segment_samples_match_the_reported_range() {
        let a = long_mixed_signal();
        let mut stream = SegmentStream::new(RATE as u32);
        let mut checked = 0;
        for b in a.chunks(16_000) {
            stream.push(b, &mut |start, seg| {
                assert_eq!(seg, &a[start..start + seg.len()]);
                checked += 1;
            });
        }
        stream.finish(&mut |start, seg| {
            assert_eq!(seg, &a[start..start + seg.len()]);
            checked += 1;
        });
        assert!(checked >= 5);
    }

    #[test]
    fn memory_stays_bounded_over_ten_minutes() {
        let unit = cat(&[tone(3.0), silence(2.0)]);
        let mut stream = SegmentStream::new(RATE as u32);
        let mut max = 0;
        for _ in 0..120 {
            for b in unit.chunks(16_000) {
                stream.push(b, &mut |_, _| {});
                max = max.max(stream.buffered_samples());
            }
        }
        assert!(secs(max) < 45.0, "held {} s", secs(max));
    }

    fn write_wav(path: &std::path::Path, rate: u32, samples: &[i16]) {
        let file = std::fs::File::create(path).unwrap();
        let mut w = WavWriter::new(file, rate).unwrap();
        w.write_samples(samples).unwrap();
        w.finish().unwrap();
    }

    fn tone_i16(secs: f32, rate: u32, amp: f32) -> Vec<i16> {
        (0..(secs * rate as f32) as usize)
            .map(|i| (amp * 32_767.0 * (i as f32 * 0.07 * 16_000.0 / rate as f32).sin()) as i16)
            .collect()
    }

    #[test]
    fn segment_file_matches_split_of_the_resampled_signal() {
        let dir = crate::audio::temp_dir("segment-file");
        let path = dir.join("a.wav");
        let mut s = tone_i16(2.0, 48_000, 0.3);
        s.extend(vec![0i16; 96_000]);
        s.extend(tone_i16(2.0, 48_000, 0.3));
        write_wav(&path, 48_000, &s);

        let float: Vec<f32> = s.iter().map(|v| f32::from(*v) / 32_768.0).collect();
        let want = split(
            &crate::asr::resample::resample_all(&float, 48_000, 16_000),
            16_000,
        );

        let mut got = Vec::new();
        let mut last = (0.0f32, 0.0f32);
        segment_file(
            &path,
            &mut |start, seg| got.push((start, start + seg.len())),
            &mut |done, total| {
                last = (done, total);
                true
            },
        )
        .unwrap();
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(&want) {
            assert!(
                g.0.abs_diff(w.0) <= 160 && g.1.abs_diff(w.1) <= 160,
                "{g:?} vs {w:?}"
            );
        }
        assert!((last.1 - 6.0).abs() < 0.01, "total {}", last.1);
        assert!((last.0 - 6.0).abs() < 0.01, "done {}", last.0);
    }

    #[test]
    fn segment_file_stops_when_progress_says_so() {
        let dir = crate::audio::temp_dir("segment-cancel");
        let path = dir.join("a.wav");
        // 100 s of speech with pauses: many blocks, many segments
        let mut s = Vec::new();
        for _ in 0..20 {
            s.extend(tone_i16(3.0, 16_000, 0.3));
            s.extend(vec![0i16; 32_000]);
        }
        write_wav(&path, 16_000, &s);
        let mut calls = 0;
        let mut segments = 0;
        segment_file(&path, &mut |_, _| segments += 1, &mut |_, _| {
            calls += 1;
            false
        })
        .unwrap();
        assert_eq!(
            calls, 1,
            "progress is asked once, then the file is abandoned"
        );
        assert!(segments < 20, "{segments} segments after cancel");
    }

    #[test]
    fn segment_file_rejects_a_bad_file() {
        let dir = crate::audio::temp_dir("segment-bad");
        let path = dir.join("a.wav");
        std::fs::write(&path, b"nope").unwrap();
        assert!(segment_file(&path, &mut |_, _| {}, &mut |_, _| true).is_err());
    }
}
