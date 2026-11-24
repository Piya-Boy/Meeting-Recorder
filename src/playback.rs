//! Playback of a saved recording: the two tracks mixed and sent to the default output device.
//! See docs/superpowers/specs/2026-11-25-playback-design.md.

use std::io;
use std::path::Path;

use crate::asr::wavread::WavReader;

/// `mic.wav` and `system.wav` of one session read as a single mixed mono stream.
pub struct Mixer {
    tracks: Vec<WavReader>,
    rate: u32,
    total: u64,
    pos: u64,
}

impl Mixer {
    /// Opens whichever of `mic.wav` / `system.wav` exist. A folder with neither, a track that is
    /// not 16-bit mono PCM, or two tracks with different rates is an error.
    pub fn open(dir: &Path) -> io::Result<Mixer> {
        let mut tracks = Vec::new();
        for name in ["mic.wav", "system.wav"] {
            let path = dir.join(name);
            if !path.is_file() {
                continue;
            }
            tracks.push(WavReader::open(&path)?);
        }
        let Some(first) = tracks.first() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no mic.wav or system.wav in the recording folder",
            ));
        };
        let rate = first.rate();
        if tracks.iter().any(|t| t.rate() != rate) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the two tracks have different sample rates",
            ));
        }
        let total = tracks.iter().map(|t| t.total_frames()).max().unwrap_or(0);
        Ok(Mixer {
            tracks,
            rate,
            total,
            pos: 0,
        })
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn total_frames(&self) -> u64 {
        self.total
    }

    pub fn position(&self) -> u64 {
        self.pos
    }

    /// Moves to `frame` (clamped to the end).
    pub fn seek(&mut self, frame: u64) -> io::Result<()> {
        let frame = frame.min(self.total);
        for t in &mut self.tracks {
            t.seek_frame(frame)?;
        }
        self.pos = frame;
        Ok(())
    }

    /// Up to `max_frames` mixed samples in [-1, 1]; empty at the end.
    pub fn read(&mut self, max_frames: usize) -> io::Result<Vec<f32>> {
        let n = (self.total - self.pos).min(max_frames as u64) as usize;
        let mut out = vec![0.0f32; n];
        for t in &mut self.tracks {
            // A track that ends early returns fewer samples; the rest of the block stays silent.
            for (o, v) in out.iter_mut().zip(t.read_block(n)?) {
                *o += v;
            }
        }
        for o in &mut out {
            *o = o.clamp(-1.0, 1.0);
        }
        self.pos += n as u64;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::wav::WavWriter;
    use std::fs;
    use std::path::PathBuf;

    fn write_wav(path: &Path, rate: u32, samples: &[i16]) {
        let f = fs::File::create(path).unwrap();
        let mut w = WavWriter::new(f, rate).unwrap();
        w.write_samples(samples).unwrap();
        w.finish().unwrap();
    }

    fn dir(tag: &str) -> PathBuf {
        crate::audio::temp_dir(tag)
    }

    fn s(v: i16) -> f32 {
        f32::from(v) / 32_768.0
    }

    fn close(a: &[f32], b: &[f32]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
    }

    #[test]
    fn mixer_sums_the_two_tracks() {
        let d = dir("mix-sum");
        write_wav(&d.join("mic.wav"), 16_000, &[1000, 2000, 3000]);
        write_wav(&d.join("system.wav"), 16_000, &[10, 20, 30]);
        let mut m = Mixer::open(&d).unwrap();
        assert_eq!((m.rate(), m.total_frames(), m.position()), (16_000, 3, 0));
        let got = m.read(10).unwrap();
        assert!(close(&got, &[s(1010), s(2020), s(3030)]), "{got:?}");
        assert!(m.read(10).unwrap().is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn mixer_clamps_loud_sums_to_one() {
        let d = dir("mix-clamp");
        write_wav(&d.join("mic.wav"), 16_000, &[30_000, -30_000]);
        write_wav(&d.join("system.wav"), 16_000, &[30_000, -30_000]);
        let got = Mixer::open(&d).unwrap().read(10).unwrap();
        assert_eq!(got, vec![1.0, -1.0]);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_shorter_track_is_silent_after_its_end() {
        let d = dir("mix-short");
        write_wav(&d.join("mic.wav"), 16_000, &[100, 200, 300, 400]);
        write_wav(&d.join("system.wav"), 16_000, &[1, 2]);
        let mut m = Mixer::open(&d).unwrap();
        assert_eq!(m.total_frames(), 4);
        let got = m.read(10).unwrap();
        assert!(close(&got, &[s(101), s(202), s(300), s(400)]), "{got:?}");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn mixer_plays_a_folder_with_only_one_track() {
        for only in ["mic.wav", "system.wav"] {
            let d = dir("mix-one");
            write_wav(&d.join(only), 48_000, &[5, 6, 7]);
            let mut m = Mixer::open(&d).unwrap();
            assert_eq!((m.rate(), m.total_frames()), (48_000, 3));
            assert!(close(&m.read(10).unwrap(), &[s(5), s(6), s(7)]), "{only}");
            let _ = fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn mixer_rejects_no_wav_a_bad_wav_and_mismatched_rates() {
        let d = dir("mix-bad-none");
        assert!(Mixer::open(&d).is_err(), "empty folder");

        let d2 = dir("mix-bad-garbage");
        fs::write(d2.join("mic.wav"), b"not a wav").unwrap();
        assert!(Mixer::open(&d2).is_err(), "garbage file");

        // A real header, but 2 channels: the recorder never writes that.
        let d3 = dir("mix-bad-stereo");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&2u16.to_le_bytes()); // channels
        bytes.extend_from_slice(&16_000u32.to_le_bytes());
        bytes.extend_from_slice(&64_000u32.to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        fs::write(d3.join("mic.wav"), bytes).unwrap();
        assert!(Mixer::open(&d3).is_err(), "stereo file");

        let d4 = dir("mix-bad-rates");
        write_wav(&d4.join("mic.wav"), 16_000, &[1, 2]);
        write_wav(&d4.join("system.wav"), 48_000, &[1, 2]);
        assert!(Mixer::open(&d4).is_err(), "rates differ");

        // One good track and one broken: an error, not a silent half recording.
        let d5 = dir("mix-bad-one-of-two");
        write_wav(&d5.join("mic.wav"), 16_000, &[1, 2]);
        fs::write(d5.join("system.wav"), b"junk").unwrap();
        assert!(Mixer::open(&d5).is_err(), "one broken track");

        for d in [d, d2, d3, d4, d5] {
            let _ = fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn mixer_seek_and_position() {
        let d = dir("mix-seek");
        let ramp: Vec<i16> = (0..100).collect();
        write_wav(&d.join("mic.wav"), 16_000, &ramp);
        let mut m = Mixer::open(&d).unwrap();
        m.seek(50).unwrap();
        assert_eq!(m.position(), 50);
        let got = m.read(10).unwrap();
        let want: Vec<f32> = (50..60).map(s).collect();
        assert!(close(&got, &want), "{got:?}");
        assert_eq!(m.position(), 60);

        m.seek(1_000).unwrap();
        assert_eq!(m.position(), 100, "clamped to the end");
        assert!(m.read(10).unwrap().is_empty());

        m.seek(0).unwrap();
        let mut all = Vec::new();
        loop {
            let b = m.read(7).unwrap();
            if b.is_empty() {
                break;
            }
            all.extend(b);
        }
        assert_eq!(all.len(), 100);
        assert_eq!(m.position(), 100);
        let _ = fs::remove_dir_all(&d);
    }
}
