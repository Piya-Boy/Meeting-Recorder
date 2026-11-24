//! Block-wise reader for the 16-bit mono PCM WAV files the recorder writes.
//!
//! The length of the audio comes from the file length, not from the header: the header is only
//! rewritten every few seconds while recording, so a file left by a crash has stale sizes. The
//! recorder's files hold the data chunk last, so everything after the data header is audio.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

pub struct WavReader {
    file: File,
    rate: u32,
    data_start: u64,
    total_frames: u64,
    read_frames: u64,
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_owned())
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

impl WavReader {
    pub fn open(path: &Path) -> io::Result<WavReader> {
        let mut file = File::open(path)?;
        let len = file.metadata()?.len();
        let mut head = [0u8; 12];
        file.read_exact(&mut head)
            .map_err(|_| invalid("not a WAV file"))?;
        if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
            return Err(invalid("not a WAV file"));
        }
        let mut pos = 12u64;
        let mut rate = None;
        loop {
            let mut ch = [0u8; 8];
            file.read_exact(&mut ch)
                .map_err(|_| invalid("WAV has no data chunk"))?;
            pos += 8;
            let size = u64::from(u32_at(&ch, 4));
            match &ch[0..4] {
                b"fmt " => {
                    if size < 16 {
                        return Err(invalid("bad fmt chunk"));
                    }
                    let mut fmt = vec![0u8; size as usize];
                    file.read_exact(&mut fmt)
                        .map_err(|_| invalid("bad fmt chunk"))?;
                    let (tag, channels, bits) =
                        (u16_at(&fmt, 0), u16_at(&fmt, 2), u16_at(&fmt, 14));
                    if tag != 1 || channels != 1 || bits != 16 {
                        return Err(invalid("only 16-bit mono PCM WAV is supported"));
                    }
                    rate = Some(u32_at(&fmt, 4));
                    pos += size;
                    if size % 2 == 1 {
                        file.seek(SeekFrom::Current(1))?;
                        pos += 1;
                    }
                }
                b"data" => {
                    let rate = rate.ok_or_else(|| invalid("data before fmt"))?;
                    let total_frames = len.saturating_sub(pos) / 2;
                    return Ok(WavReader {
                        file,
                        rate,
                        data_start: pos,
                        total_frames,
                        read_frames: 0,
                    });
                }
                _ => {
                    let skip = size + size % 2;
                    file.seek(SeekFrom::Current(skip as i64))?;
                    pos += skip;
                }
            }
        }
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }

    /// Moves the read position to `frame` (clamped to the end of the audio).
    pub fn seek_frame(&mut self, frame: u64) -> io::Result<()> {
        let frame = frame.min(self.total_frames);
        self.file
            .seek(SeekFrom::Start(self.data_start + frame * 2))?;
        self.read_frames = frame;
        Ok(())
    }

    /// Up to `max_frames` samples scaled to [-1, 1]; empty at the end of the file.
    pub fn read_block(&mut self, max_frames: usize) -> io::Result<Vec<f32>> {
        let want = (self.total_frames - self.read_frames).min(max_frames as u64) as usize;
        let mut bytes = vec![0u8; want * 2];
        self.file.read_exact(&mut bytes)?;
        self.read_frames += want as u64;
        Ok(bytes
            .chunks_exact(2)
            .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32_768.0)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::wav::WavWriter;
    use std::io::Write as _;

    fn write_wav(path: &std::path::Path, rate: u32, samples: &[i16], finish: bool) {
        let file = std::fs::File::create(path).unwrap();
        let mut w = WavWriter::new(file, rate).unwrap();
        w.write_samples(samples).unwrap();
        if finish {
            w.finish().unwrap();
        } // else: simulated crash, the header still says 0 bytes
    }

    fn ramp(n: usize) -> Vec<i16> {
        (0..n)
            .map(|i| ((i % 20_000) as i32 - 10_000) as i16)
            .collect()
    }

    #[test]
    fn seek_frame_moves_the_read_position() {
        let dir = crate::audio::temp_dir("wavread-seek");
        let path = dir.join("a.wav");
        let src = ramp(1_000);
        write_wav(&path, 16_000, &src, true);
        let mut r = WavReader::open(&path).unwrap();
        let _ = r.read_block(300).unwrap();
        r.seek_frame(100).unwrap();
        let got = r.read_block(5).unwrap();
        let want: Vec<f32> = src[100..105]
            .iter()
            .map(|&v| f32::from(v) / 32_768.0)
            .collect();
        assert_eq!(got, want);
        // Back to the start, and past the end (clamped: nothing left to read).
        r.seek_frame(0).unwrap();
        assert_eq!(r.read_block(1).unwrap().len(), 1);
        r.seek_frame(5_000).unwrap();
        assert!(r.read_block(10).unwrap().is_empty());
        // Seeking works after the end was reached, too.
        r.seek_frame(999).unwrap();
        assert_eq!(r.read_block(10).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_back_in_blocks() {
        let dir = crate::audio::temp_dir("wavread-blocks");
        let path = dir.join("a.wav");
        let src = ramp(48_000);
        write_wav(&path, 48_000, &src, true);
        let mut r = WavReader::open(&path).unwrap();
        assert_eq!(r.rate(), 48_000);
        assert_eq!(r.total_frames(), 48_000);
        let mut got = Vec::new();
        let mut blocks = 0;
        loop {
            let b = r.read_block(20_000).unwrap();
            if b.is_empty() {
                break;
            }
            blocks += 1;
            got.extend(b);
        }
        assert_eq!(blocks, 3);
        assert_eq!(got.len(), src.len());
        for (g, s) in got.iter().zip(&src) {
            assert!((g - f32::from(*s) / 32_768.0).abs() < 1.0 / 32_768.0);
        }
    }

    #[test]
    fn stereo_and_8_bit_are_rejected() {
        let dir = crate::audio::temp_dir("wavread-format");
        let path = dir.join("a.wav");
        write_wav(&path, 48_000, &ramp(100), true);
        // channels field is at byte 22, bits per sample at byte 34
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[22] = 2;
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            WavReader::open(&path).err().unwrap().kind(),
            std::io::ErrorKind::InvalidData
        );
        bytes[22] = 1;
        bytes[34] = 8;
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            WavReader::open(&path).err().unwrap().kind(),
            std::io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn not_a_wav_is_invalid_data() {
        let dir = crate::audio::temp_dir("wavread-garbage");
        let path = dir.join("a.wav");
        std::fs::write(&path, b"hello world, not a wav file at all").unwrap();
        assert_eq!(
            WavReader::open(&path).err().unwrap().kind(),
            std::io::ErrorKind::InvalidData
        );
        std::fs::write(&path, b"RIFF").unwrap();
        assert!(WavReader::open(&path).is_err());
    }

    #[test]
    fn crash_file_with_zero_header_is_read_by_length() {
        let dir = crate::audio::temp_dir("wavread-crash");
        let path = dir.join("a.wav");
        write_wav(&path, 48_000, &ramp(48_000), false);
        let mut r = WavReader::open(&path).unwrap();
        assert_eq!(r.total_frames(), 48_000);
        assert_eq!(r.read_block(100_000).unwrap().len(), 48_000);
    }

    #[test]
    fn stale_header_shorter_than_data_is_read_by_length() {
        let dir = crate::audio::temp_dir("wavread-stale");
        let path = dir.join("a.wav");
        let file = std::fs::File::create(&path).unwrap();
        let mut w = WavWriter::new(file, 48_000).unwrap();
        w.write_samples(&ramp(24_000)).unwrap();
        w.flush_header().unwrap();
        w.write_samples(&ramp(24_000)).unwrap(); // after the last flush, then "crash"
        drop(w);
        let r = WavReader::open(&path).unwrap();
        assert_eq!(r.total_frames(), 48_000);
    }

    #[test]
    fn half_sample_at_the_end_is_dropped() {
        let dir = crate::audio::temp_dir("wavread-half");
        let path = dir.join("a.wav");
        write_wav(&path, 16_000, &ramp(1000), true);
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        f.write_all(&[0x7F]).unwrap();
        drop(f);
        let mut r = WavReader::open(&path).unwrap();
        assert_eq!(r.total_frames(), 1000);
        assert_eq!(r.read_block(5000).unwrap().len(), 1000);
    }

    #[test]
    fn empty_data_is_zero_frames() {
        let dir = crate::audio::temp_dir("wavread-empty");
        let path = dir.join("a.wav");
        write_wav(&path, 48_000, &[], true);
        let mut r = WavReader::open(&path).unwrap();
        assert_eq!(r.total_frames(), 0);
        assert!(r.read_block(100).unwrap().is_empty());
    }
}
