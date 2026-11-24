//! Follows a recording that is still being written and turns what is new into transcript lines.
//!
//! One `LiveTrack` per file. The recorder rewrites the WAV header and flushes every few seconds,
//! so the audio lags the clock by about that long; the segmenter then waits for a pause, so a
//! line appears a few seconds after the speaker stops. Nothing is written to disk.

use std::io;
use std::path::Path;

use crate::asr::engine::Recognizer;
use crate::asr::features::SAMPLE_RATE;
use crate::asr::resample::Resampler;
use crate::asr::segment::{trim, SegmentStream, MIN_SEGMENT_S};
use crate::asr::transcript::{Line, Track};
use crate::asr::wavread::WavReader;

/// Seconds of audio read from the file per step.
const BLOCK_S: usize = 30;

/// New audio, in seconds, that has to arrive before the unfinished tail is read again.
const PARTIAL_EVERY_S: usize = 2;

pub struct LiveTrack {
    track: Track,
    reader: WavReader,
    resampler: Resampler,
    stream: SegmentStream,
    /// Whether the unfinished tail is also read, for a preview line.
    partial_on: bool,
    partial: Option<Line>,
    partial_changed: bool,
    /// End (sample of the whole recording) of the audio the preview was last made from.
    decoded_end: usize,
}

impl LiveTrack {
    pub fn open(path: &Path, track: Track) -> io::Result<LiveTrack> {
        let reader = WavReader::open(path)?;
        let rate = reader.rate();
        Ok(LiveTrack {
            track,
            reader,
            resampler: Resampler::new(rate, SAMPLE_RATE),
            stream: SegmentStream::new(SAMPLE_RATE),
            partial_on: false,
            partial: None,
            partial_changed: false,
            decoded_end: 0,
        })
    }

    /// Also reads the segment that is still being spoken, every couple of seconds, as a
    /// preview line that the final line of that segment replaces.
    pub fn with_partial(mut self) -> LiveTrack {
        self.partial_on = true;
        self
    }

    pub fn track(&self) -> Track {
        self.track
    }

    /// The preview of the segment being spoken, if there is one.
    #[cfg(test)]
    pub fn partial(&self) -> Option<&Line> {
        self.partial.as_ref()
    }

    /// `Some(preview)` when the preview changed since the last call (`Some(None)`: it was
    /// withdrawn), `None` when it did not.
    pub fn take_partial_change(&mut self) -> Option<Option<Line>> {
        if std::mem::take(&mut self.partial_changed) {
            Some(self.partial.clone())
        } else {
            None
        }
    }

    fn set_partial(&mut self, line: Option<Line>) {
        if self.partial.as_ref().map(|l| &l.text) != line.as_ref().map(|l| &l.text) {
            self.partial_changed = true;
        }
        self.partial = line;
    }

    fn refresh_partial(&mut self, rec: &mut dyn Recognizer, cut: bool) -> Result<(), String> {
        if !self.partial_on {
            return Ok(());
        }
        let rate = SAMPLE_RATE as usize;
        let offset = self.stream.tail().0;
        if cut {
            // The preview belonged to a segment that is now final.
            self.set_partial(None);
            self.decoded_end = offset;
        }
        let tail = self.stream.tail().1;
        let end = offset + tail.len();
        if end.saturating_sub(self.decoded_end) < PARTIAL_EVERY_S * rate {
            return Ok(());
        }
        let span =
            trim(tail, rate).filter(|(a, b)| b - a >= (MIN_SEGMENT_S * rate as f32) as usize);
        let line = match span {
            None => None,
            Some((a, b)) => {
                let text = rec
                    .recognize(&tail[a..b])
                    .map_err(|e| format!("Recognition failed: {e}"))?;
                (!text.is_empty()).then(|| Line {
                    track: self.track,
                    start_ms: (offset + a) as u64 * 1000 / u64::from(SAMPLE_RATE),
                    end_ms: (offset + b) as u64 * 1000 / u64::from(SAMPLE_RATE),
                    text,
                })
            }
        };
        self.decoded_end = end;
        self.set_partial(line);
        Ok(())
    }

    /// Reads what was written since the last call, recognizes every segment that is ready and
    /// appends its non-empty text to `out`. `Err` is a file or recognizer failure.
    pub fn poll(&mut self, rec: &mut dyn Recognizer, out: &mut Vec<Line>) -> Result<(), String> {
        self.reader
            .refresh()
            .map_err(|e| format!("Cannot read the recording: {e}"))?;
        let rate = self.reader.rate() as usize;
        let tail_start = self.stream.tail().0;
        loop {
            let block = self
                .reader
                .read_block(BLOCK_S * rate)
                .map_err(|e| format!("Cannot read the recording: {e}"))?;
            if block.is_empty() {
                let cut = self.stream.tail().0 != tail_start;
                return self.refresh_partial(rec, cut);
            }
            let mut ready: Vec<(usize, Vec<f32>)> = Vec::new();
            let mut take = |start: usize, seg: &[f32]| ready.push((start, seg.to_vec()));
            self.stream.push(&self.resampler.process(&block), &mut take);
            self.stream.poll(&mut take);
            for (start, seg) in ready {
                let text = rec
                    .recognize(&seg)
                    .map_err(|e| format!("Recognition failed: {e}"))?;
                if text.is_empty() {
                    continue;
                }
                out.push(Line {
                    track: self.track,
                    start_ms: start as u64 * 1000 / u64::from(SAMPLE_RATE),
                    end_ms: (start + seg.len()) as u64 * 1000 / u64::from(SAMPLE_RATE),
                    text,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::engine::FakeRecognizer;
    use crate::audio::wav::WavWriter;
    use std::io::Write as _;

    const RATE: u32 = 16_000;

    fn tone(secs: f32) -> Vec<i16> {
        (0..(secs * RATE as f32) as usize)
            .map(|i| (0.3 * 32_767.0 * (i as f32 * 0.07).sin()) as i16)
            .collect()
    }

    fn silence(secs: f32) -> Vec<i16> {
        vec![0; (secs * RATE as f32) as usize]
    }

    /// A WAV file as the recorder leaves it mid-recording: the header still says 0 bytes.
    fn start_file(path: &Path, samples: &[i16]) {
        let f = std::fs::File::create(path).unwrap();
        let mut w = WavWriter::new(f, RATE).unwrap();
        w.write_samples(samples).unwrap();
        w.flush_header().unwrap();
    }

    fn append(path: &Path, samples: &[i16]) {
        let mut f = std::fs::OpenOptions::new().append(true).open(path).unwrap();
        for v in samples {
            f.write_all(&v.to_le_bytes()).unwrap();
        }
        f.flush().unwrap();
    }

    #[test]
    fn a_growing_file_gives_each_segment_once_in_time_order() {
        let dir = crate::audio::temp_dir("live-grow");
        let path = dir.join("system.wav");
        start_file(&path, &tone(3.0));
        let mut live = LiveTrack::open(&path, Track::System).unwrap();
        let mut rec = FakeRecognizer::new(&["หนึ่ง", "สอง"]);
        let calls = rec.lengths.clone();
        let mut lines = Vec::new();

        live.poll(&mut rec, &mut lines).unwrap();
        assert!(lines.is_empty(), "no pause yet, nothing to cut");

        append(&path, &silence(1.5));
        live.poll(&mut rec, &mut lines).unwrap();
        assert_eq!(lines.len(), 1, "the first utterance is complete");

        append(&path, &tone(2.5));
        live.poll(&mut rec, &mut lines).unwrap();
        assert_eq!(lines.len(), 1, "the second one is still going");

        append(&path, &silence(1.5));
        live.poll(&mut rec, &mut lines).unwrap();
        live.poll(&mut rec, &mut lines).unwrap(); // nothing new: nothing again

        assert_eq!(
            calls.lock().unwrap().len(),
            2,
            "each segment is recognized once"
        );
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
            ["หนึ่ง", "สอง"]
        );
        assert!(lines.iter().all(|l| l.track == Track::System));
        assert!(lines[0].start_ms < 1_000, "{:?}", lines[0]);
        assert!(
            (4_000..5_500).contains(&lines[1].start_ms),
            "{:?}",
            lines[1]
        );
        assert!(lines[1].start_ms > lines[0].end_ms);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_with_no_new_audio_gives_nothing() {
        let dir = crate::audio::temp_dir("live-idle");
        let path = dir.join("mic.wav");
        start_file(&path, &[]);
        let mut live = LiveTrack::open(&path, Track::Mic).unwrap();
        let mut rec = FakeRecognizer::new(&["x"]);
        let mut lines = Vec::new();
        for _ in 0..3 {
            live.poll(&mut rec, &mut lines).unwrap();
        }
        assert!(lines.is_empty());
        assert!(rec.lengths.lock().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn silent_segments_with_no_text_make_no_lines() {
        let dir = crate::audio::temp_dir("live-empty-text");
        let path = dir.join("mic.wav");
        start_file(&path, &[tone(3.0), silence(1.5)].concat());
        let mut live = LiveTrack::open(&path, Track::Mic).unwrap();
        let mut rec = FakeRecognizer::new(&[]); // answers "" for everything
        let mut lines = Vec::new();
        live.poll(&mut rec, &mut lines).unwrap();
        assert_eq!(rec.lengths.lock().unwrap().len(), 1, "it was recognized");
        assert!(lines.is_empty(), "but empty text is not a line");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_recognizer_error_is_returned() {
        let dir = crate::audio::temp_dir("live-error");
        let path = dir.join("mic.wav");
        start_file(&path, &[tone(3.0), silence(1.5)].concat());
        let mut live = LiveTrack::open(&path, Track::Mic).unwrap();
        let mut rec = FakeRecognizer::failing("boom");
        let err = live.poll(&mut rec, &mut Vec::new()).unwrap_err();
        assert!(err.contains("boom"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn partial_texts(live: &mut LiveTrack) -> Option<Option<String>> {
        live.take_partial_change().map(|l| l.map(|l| l.text))
    }

    #[test]
    fn an_unfinished_utterance_gives_a_partial_that_the_final_line_replaces() {
        let dir = crate::audio::temp_dir("live-partial");
        let path = dir.join("mic.wav");
        start_file(&path, &tone(3.0));
        let mut live = LiveTrack::open(&path, Track::Mic).unwrap().with_partial();
        let mut rec = FakeRecognizer::new(&["ร่าง", "ฉบับเต็ม"]);
        let mut lines = Vec::new();

        live.poll(&mut rec, &mut lines).unwrap();
        assert!(lines.is_empty());
        assert_eq!(partial_texts(&mut live), Some(Some("ร่าง".to_owned())));
        let partial = live.partial().cloned().unwrap();
        assert_eq!((partial.track, partial.start_ms < 500), (Track::Mic, true));

        append(&path, &silence(1.5));
        live.poll(&mut rec, &mut lines).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "ฉบับเต็ม");
        assert_eq!(
            partial_texts(&mut live),
            Some(None),
            "the partial is withdrawn"
        );
        assert!(live.partial().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_partial_is_decoded_again_only_after_two_more_seconds() {
        let dir = crate::audio::temp_dir("live-partial-pace");
        let path = dir.join("mic.wav");
        start_file(&path, &tone(3.0));
        let mut live = LiveTrack::open(&path, Track::Mic).unwrap().with_partial();
        let mut rec = FakeRecognizer::new(&["a", "b", "c"]);
        let calls = rec.lengths.clone();
        let mut lines = Vec::new();

        live.poll(&mut rec, &mut lines).unwrap();
        assert_eq!(partial_texts(&mut live), Some(Some("a".to_owned())));
        live.poll(&mut rec, &mut lines).unwrap(); // nothing new
        assert_eq!(calls.lock().unwrap().len(), 1);
        append(&path, &tone(1.0));
        live.poll(&mut rec, &mut lines).unwrap();
        assert_eq!(calls.lock().unwrap().len(), 1, "only 1 s is new");
        append(&path, &tone(1.5));
        live.poll(&mut rec, &mut lines).unwrap();
        assert_eq!(calls.lock().unwrap().len(), 2, "2.5 s is new");
        assert_eq!(partial_texts(&mut live), Some(Some("b".to_owned())));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn silence_gives_no_partial_and_no_decoding() {
        let dir = crate::audio::temp_dir("live-partial-quiet");
        let path = dir.join("mic.wav");
        start_file(&path, &silence(6.0));
        let mut live = LiveTrack::open(&path, Track::Mic).unwrap().with_partial();
        let mut rec = FakeRecognizer::new(&["x"]);
        live.poll(&mut rec, &mut Vec::new()).unwrap();
        assert!(rec.lengths.lock().unwrap().is_empty());
        assert_eq!(partial_texts(&mut live), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_partial_with_the_same_text_is_not_reported_twice() {
        let dir = crate::audio::temp_dir("live-partial-same");
        let path = dir.join("mic.wav");
        start_file(&path, &tone(3.0));
        let mut live = LiveTrack::open(&path, Track::Mic).unwrap().with_partial();
        let mut rec = FakeRecognizer::new(&["same", "same"]);
        let mut lines = Vec::new();
        live.poll(&mut rec, &mut lines).unwrap();
        assert!(live.take_partial_change().is_some());
        append(&path, &tone(2.5));
        live.poll(&mut rec, &mut lines).unwrap();
        assert_eq!(rec.lengths.lock().unwrap().len(), 2, "it was decoded again");
        assert_eq!(partial_texts(&mut live), None, "but nothing changed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_with_partial_nothing_is_decoded_for_an_unfinished_tail() {
        let dir = crate::audio::temp_dir("live-partial-off");
        let path = dir.join("mic.wav");
        start_file(&path, &tone(3.0));
        let mut live = LiveTrack::open(&path, Track::Mic).unwrap();
        let mut rec = FakeRecognizer::new(&["x"]);
        live.poll(&mut rec, &mut Vec::new()).unwrap();
        assert!(rec.lengths.lock().unwrap().is_empty());
        assert_eq!(partial_texts(&mut live), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
