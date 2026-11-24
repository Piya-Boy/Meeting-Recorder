//! Per-track sample clock: keeps a track's length equal to recorded time.
//!
//! Buffers carry the frame at which they start on a common timeline (frames since the shared
//! session start, at the track's rate). A buffer that starts later than the samples written so
//! far gets zeros in front; one that overlaps gets its head skipped. Differences smaller than
//! the tolerance (10 ms) are ignored, so the error stays below the tolerance and drift between
//! device clocks cannot accumulate.

/// What the caller must do with a buffer: write `silence_frames` zeros, then the samples from
/// `skip_frames` on.
pub struct Placement {
    pub silence_frames: u64,
    pub skip_frames: usize,
}

pub struct TrackClock {
    tolerance: i64,
    written: u64,
    /// File position minus timeline position; changed only by `resume`.
    offset: i64,
}

impl TrackClock {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            tolerance: i64::from((sample_rate / 100).max(1)),
            written: 0,
            offset: 0,
        }
    }

    /// Accounts for a buffer of `frames` frames starting at `source_frame` on the timeline.
    pub fn place(&mut self, source_frame: u64, frames: usize) -> Placement {
        let expected = source_frame as i64 + self.offset;
        let gap = expected - self.written as i64;
        let mut placement = Placement {
            silence_frames: 0,
            skip_frames: 0,
        };
        if gap >= self.tolerance {
            placement.silence_frames = gap as u64;
        } else if gap <= -self.tolerance {
            placement.skip_frames = ((-gap) as usize).min(frames);
        }
        self.written += placement.silence_frames + (frames - placement.skip_frames) as u64;
        placement
    }

    /// Maps `source_frame` to the current end of the track (after a pause).
    pub fn resume(&mut self, source_frame: u64) {
        self.offset = self.written as i64 - source_frame as i64;
    }

    pub fn written(&self) -> u64 {
        self.written
    }

    /// Zeros needed to reach `target_frames`; 0 if the track is already longer.
    pub fn tail_silence(&self, target_frames: u64) -> u64 {
        target_frames.saturating_sub(self.written)
    }

    /// Records frames the caller wrote as tail or pause padding.
    pub fn pad(&mut self, frames: u64) {
        self.written += frames;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn clock() -> TrackClock {
        TrackClock::new(RATE)
    }
    fn p(c: &mut TrackClock, start: u64, frames: usize) -> (u64, usize) {
        let pl = c.place(start, frames);
        (pl.silence_frames, pl.skip_frames)
    }

    #[test]
    fn contiguous_buffers_need_nothing() {
        let mut c = clock();
        assert_eq!(p(&mut c, 0, 480), (0, 0));
        assert_eq!(p(&mut c, 480, 480), (0, 0));
        assert_eq!(c.written(), 960);
    }

    #[test]
    fn late_first_buffer_is_front_filled() {
        let mut c = clock();
        assert_eq!(p(&mut c, 4_800, 480).0, 4_800);
        assert_eq!(c.written(), 5_280);
    }

    #[test]
    fn one_second_gap_is_filled() {
        let mut c = clock();
        p(&mut c, 0, 480);
        assert_eq!(p(&mut c, 48_480, 480).0, 48_000);
    }

    #[test]
    fn jitter_inside_tolerance_is_ignored() {
        let mut c = clock();
        p(&mut c, 0, 480);
        assert_eq!(p(&mut c, 680, 480), (0, 0)); // 200 frames late
    }

    #[test]
    fn drift_is_bounded() {
        let mut c = clock();
        let mut start = 0u64;
        for _ in 0..100 {
            c.place(start, 480);
            start += 480 + 20; // source clock runs 20 frames slow per buffer
        }
        let source_end = start - 20;
        let diff = (c.written() as i64 - source_end as i64).unsigned_abs();
        assert!(
            diff < 480,
            "written {} vs source end {}",
            c.written(),
            source_end
        );
    }

    #[test]
    fn overlap_is_skipped() {
        let mut c = clock();
        c.place(0, 4_800);
        assert_eq!(p(&mut c, 3_800, 480), (0, 480));
        assert_eq!(c.written(), 4_800);
        assert_eq!(p(&mut c, 4_800, 480), (0, 0));
    }

    #[test]
    fn resume_rebases() {
        let mut c = clock();
        c.place(0, 48_000);
        c.resume(200_000);
        assert_eq!(p(&mut c, 200_000, 480), (0, 0));
        assert_eq!(c.written(), 48_480);
    }

    #[test]
    fn tail_silence_pads_up() {
        let mut c = clock();
        c.place(0, 47_000);
        assert_eq!(c.tail_silence(48_000), 1_000);
        c.pad(1_000);
        assert_eq!(c.written(), 48_000);
        let mut long = clock();
        long.place(0, 49_000);
        assert_eq!(long.tail_silence(48_000), 0);
    }
}
