//! Crash-safe 16-bit mono PCM WAV writer.
//!
//! The header's size fields are rewritten by `flush_header` (the worker calls it every 5 s) so
//! a crash leaves a file that players open; `recover_header` repairs one whose header was
//! never updated, trusting the file length.

use std::fs::OpenOptions;
use std::io::{self, Seek, SeekFrom, Write};
use std::path::Path;

const HEADER_LEN: u64 = 44;
/// Largest data chunk a WAV (u32 sizes) can describe.
pub const MAX_DATA_BYTES: u64 = u32::MAX as u64 - 36;

pub struct WavWriter<W: Write + Seek> {
    out: W,
    data_bytes: u64,
    max_data_bytes: u64,
}

impl<W: Write + Seek> WavWriter<W> {
    pub fn new(out: W, sample_rate: u32) -> io::Result<Self> {
        Self::with_limit(out, sample_rate, MAX_DATA_BYTES)
    }

    /// `max_data_bytes` lets tests use a tiny limit.
    pub fn with_limit(mut out: W, sample_rate: u32, max_data_bytes: u64) -> io::Result<Self> {
        let mut h = [0u8; HEADER_LEN as usize];
        h[0..4].copy_from_slice(b"RIFF");
        h[4..8].copy_from_slice(&36u32.to_le_bytes());
        h[8..12].copy_from_slice(b"WAVE");
        h[12..16].copy_from_slice(b"fmt ");
        h[16..20].copy_from_slice(&16u32.to_le_bytes());
        h[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM
        h[22..24].copy_from_slice(&1u16.to_le_bytes()); // mono
        h[24..28].copy_from_slice(&sample_rate.to_le_bytes());
        h[28..32].copy_from_slice(&(sample_rate * 2).to_le_bytes());
        h[32..34].copy_from_slice(&2u16.to_le_bytes()); // block align
        h[34..36].copy_from_slice(&16u16.to_le_bytes()); // bits
        h[36..40].copy_from_slice(b"data");
        out.write_all(&h)?;
        Ok(Self {
            out,
            data_bytes: 0,
            max_data_bytes,
        })
    }

    /// Appends samples. Errors (never panics) on I/O failure or when the size limit would be
    /// exceeded; nothing is written in the second case.
    pub fn write_samples(&mut self, s: &[i16]) -> io::Result<()> {
        let bytes = s.len() as u64 * 2;
        if self.data_bytes + bytes > self.max_data_bytes {
            return Err(io::Error::other("WAV size limit reached"));
        }
        let mut buf = Vec::with_capacity(s.len() * 2);
        for v in s {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        self.out.write_all(&buf)?;
        self.data_bytes += bytes;
        Ok(())
    }

    /// True when not even one more sample fits.
    pub fn is_full(&self) -> bool {
        self.data_bytes + 2 > self.max_data_bytes
    }

    pub fn frames_written(&self) -> u64 {
        self.data_bytes / 2
    }

    /// Rewrites the RIFF and data sizes, flushes, and returns to the end for further writes.
    pub fn flush_header(&mut self) -> io::Result<()> {
        self.out.seek(SeekFrom::Start(4))?;
        self.out
            .write_all(&((self.data_bytes + 36) as u32).to_le_bytes())?;
        self.out.seek(SeekFrom::Start(40))?;
        self.out
            .write_all(&(self.data_bytes as u32).to_le_bytes())?;
        self.out.flush()?;
        self.out
            .seek(SeekFrom::Start(HEADER_LEN + self.data_bytes))?;
        Ok(())
    }

    /// Writes the final header and returns the number of frames.
    pub fn finish(mut self) -> io::Result<u64> {
        self.flush_header()?;
        Ok(self.frames_written())
    }
}

/// Fixes both size fields of a WAV written by `WavWriter` from the file length (for a file
/// left by a crash) and returns the frame count.
pub fn recover_header(path: &Path) -> io::Result<u64> {
    let mut f = OpenOptions::new().read(true).write(true).open(path)?;
    let len = f.metadata()?.len();
    let data = (len.saturating_sub(HEADER_LEN) / 2 * 2).min(MAX_DATA_BYTES);
    f.seek(SeekFrom::Start(4))?;
    f.write_all(&((data + 36) as u32).to_le_bytes())?;
    f.seek(SeekFrom::Start(40))?;
    f.write_all(&(data as u32).to_le_bytes())?;
    f.flush()?;
    Ok(data / 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor, Seek, SeekFrom, Write};
    use std::path::Path;

    fn u32_at(b: &[u8], i: usize) -> u32 {
        u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
    }
    fn u16_at(b: &[u8], i: usize) -> u16 {
        u16::from_le_bytes(b[i..i + 2].try_into().unwrap())
    }
    fn read_sizes(path: &Path) -> (u32, u32) {
        let b = std::fs::read(path).unwrap();
        (u32_at(&b, 4), u32_at(&b, 40))
    }
    fn open_file(path: &Path) -> std::fs::File {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .unwrap()
    }
    /// Seekable writer that fails once more than `limit` bytes are written.
    struct FailingWriter {
        inner: Cursor<Vec<u8>>,
        limit: usize,
    }
    impl Write for FailingWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.inner.get_ref().len() + buf.len() > self.limit {
                return Err(io::Error::other("disk full"));
            }
            self.inner.write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.inner.flush()
        }
    }
    impl Seek for FailingWriter {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    #[test]
    fn header_fields_after_finish() {
        let dir = crate::audio::temp_dir("wav-header");
        let path = dir.join("a.wav");
        let mut w = WavWriter::new(open_file(&path), 48_000).unwrap();
        w.write_samples(&vec![0i16; 48_000]).unwrap();
        assert_eq!(w.finish().unwrap(), 48_000);
        let b = std::fs::read(&path).unwrap();
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(&b[8..12], b"WAVE");
        assert_eq!(&b[12..16], b"fmt ");
        assert_eq!(u32_at(&b, 16), 16);
        assert_eq!(u16_at(&b, 20), 1);
        assert_eq!(u16_at(&b, 22), 1);
        assert_eq!(u32_at(&b, 24), 48_000);
        assert_eq!(u32_at(&b, 28), 96_000);
        assert_eq!(u16_at(&b, 32), 2);
        assert_eq!(u16_at(&b, 34), 16);
        assert_eq!(&b[36..40], b"data");
        assert_eq!(u32_at(&b, 40), 96_000);
        assert_eq!(u32_at(&b, 4), 96_036);
        assert_eq!(b.len(), 44 + 96_000);
    }

    #[test]
    fn flush_header_midway_is_readable() {
        let dir = crate::audio::temp_dir("wav-flush");
        let path = dir.join("a.wav");
        let mut w = WavWriter::new(open_file(&path), 48_000).unwrap();
        w.write_samples(&vec![1i16; 24_000]).unwrap();
        w.flush_header().unwrap();
        assert_eq!(read_sizes(&path).1, 48_000);
        w.write_samples(&vec![1i16; 24_000]).unwrap();
        w.finish().unwrap();
        assert_eq!(read_sizes(&path).1, 96_000);
    }

    #[test]
    fn recover_header_fixes_crash_file() {
        let dir = crate::audio::temp_dir("wav-recover");
        let path = dir.join("a.wav");
        let mut w = WavWriter::new(open_file(&path), 48_000).unwrap();
        w.write_samples(&vec![1i16; 48_000]).unwrap();
        drop(w); // crash: no flush_header, no finish
        assert_eq!(read_sizes(&path), (36, 0)); // fresh header: riff 36, data 0
        assert_eq!(recover_header(&path).unwrap(), 48_000);
        assert_eq!(read_sizes(&path), (96_036, 96_000));
    }

    #[test]
    fn write_error_is_returned_not_panicked() {
        let fw = FailingWriter {
            inner: Cursor::new(Vec::new()),
            limit: 60,
        };
        let mut w = WavWriter::new(fw, 48_000).unwrap();
        assert!(w.write_samples(&[0i16; 100]).is_err());
    }

    #[test]
    fn limit_is_enforced() {
        let mut w = WavWriter::with_limit(Cursor::new(Vec::new()), 48_000, 100).unwrap();
        w.write_samples(&[0i16; 50]).unwrap();
        assert!(w.is_full());
        assert!(w.write_samples(&[0i16; 1]).is_err());
        assert_eq!(w.frames_written(), 50);
    }
}
