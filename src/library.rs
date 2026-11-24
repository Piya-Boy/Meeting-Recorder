//! The recordings on disk: what the Recordings and Transcripts screens list.
//!
//! A recording is a session folder `<YYYY-MM-DD_HHMM>_<name>` directly under the destination
//! folder, holding `mic.wav` and/or `system.wav` and, once transcribed, `transcript.json`.
//! The folders are the data; nothing is cached, so the list always agrees with the disk.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

use crate::asr::transcript::{Line, Track};
use crate::asr::wavread::WavReader;

const MIC: &str = "mic.wav";
const SYSTEM: &str = "system.wav";
const TRANSCRIPT_JSON: &str = "transcript.json";
/// Files the app writes into a session folder; `delete_session` removes only these.
const OWN_FILES: [&str; 6] = [
    MIC,
    SYSTEM,
    TRANSCRIPT_JSON,
    "transcript.txt",
    "transcript.json.tmp",
    "transcript.txt.tmp",
];
const PREVIEW_CHARS: usize = 80;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranscriptInfo {
    pub lines: usize,
    /// The first line, cut to a short length, for the list.
    pub preview: String,
    /// Every line, for searching.
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub dir: PathBuf,
    pub folder: String,
    pub name: String,
    /// `YYYY-MM-DD HH:MM`, empty when the folder name has no stamp.
    pub stamp: String,
    pub duration_s: u64,
    pub bytes: u64,
    pub transcript: Option<TranscriptInfo>,
}

/// `2026-11-24_2248_standup` becomes (`2026-11-24 22:48`, `standup`).
fn split_folder_name(folder: &str) -> Option<(String, String)> {
    let b = folder.as_bytes();
    let digits =
        |r: std::ops::Range<usize>| b.get(r).is_some_and(|s| s.iter().all(u8::is_ascii_digit));
    let ok = b.len() >= 16
        && digits(0..4)
        && b[4] == b'-'
        && digits(5..7)
        && b[7] == b'-'
        && digits(8..10)
        && b[10] == b'_'
        && digits(11..15)
        && b[15] == b'_';
    if !ok {
        return None;
    }
    let stamp = format!("{} {}:{}", &folder[0..10], &folder[11..13], &folder[13..15]);
    Some((stamp, folder[16..].to_owned()))
}

fn track_seconds(path: &Path) -> u64 {
    match WavReader::open(path) {
        Ok(r) if r.rate() > 0 => r.total_frames() / u64::from(r.rate()),
        _ => 0,
    }
}

fn file_len(path: &Path) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// Lines of a version 1 `transcript.json`; `None` when the text is not one.
fn parse_transcript(text: &str) -> Option<Vec<Line>> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    if v.get("version")?.as_u64()? != 1 {
        return None;
    }
    let mut out = Vec::new();
    for l in v.get("lines")?.as_array()? {
        let track = match l.get("track")?.as_str()? {
            "mic" => Track::Mic,
            "system" => Track::System,
            _ => continue,
        };
        let start_ms = l.get("start_ms")?.as_u64()?;
        out.push(Line {
            track,
            start_ms,
            end_ms: l.get("end_ms").and_then(|e| e.as_u64()).unwrap_or(start_ms),
            text: l.get("text")?.as_str()?.to_owned(),
        });
    }
    Some(out)
}

fn read_transcript(dir: &Path) -> Option<Vec<Line>> {
    parse_transcript(&fs::read_to_string(dir.join(TRANSCRIPT_JSON)).ok()?)
}

fn transcript_info(lines: &[Line]) -> TranscriptInfo {
    let preview = lines
        .first()
        .map(|l| l.text.trim().chars().take(PREVIEW_CHARS).collect())
        .unwrap_or_default();
    let text = lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    TranscriptInfo {
        lines: lines.len(),
        preview,
        text,
    }
}

fn entry(dir: PathBuf, folder: String) -> Option<Entry> {
    let mic = dir.join(MIC);
    let system = dir.join(SYSTEM);
    if !mic.is_file() && !system.is_file() {
        return None;
    }
    let (stamp, name) = split_folder_name(&folder).unwrap_or((String::new(), folder.clone()));
    let duration_s = track_seconds(&mic).max(track_seconds(&system));
    let bytes = file_len(&mic) + file_len(&system);
    let transcript = read_transcript(&dir).map(|l| transcript_info(&l));
    Some(Entry {
        dir,
        folder,
        name,
        stamp,
        duration_s,
        bytes,
        transcript,
    })
}

/// Every recording directly under `base`, newest first. A missing `base` gives an empty list.
pub fn scan(base: &Path) -> Vec<Entry> {
    let Ok(read) = fs::read_dir(base) else {
        return Vec::new();
    };
    let mut out: Vec<Entry> = read
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| entry(e.path(), e.file_name().to_string_lossy().into_owned()))
        .collect();
    out.sort_by(|a, b| b.stamp.cmp(&a.stamp).then_with(|| b.folder.cmp(&a.folder)));
    out
}

/// The lines of the transcript in `dir`, for the detail page.
pub fn load_transcript(dir: &Path) -> io::Result<Vec<Line>> {
    let text = fs::read_to_string(dir.join(TRANSCRIPT_JSON))?;
    parse_transcript(&text).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "transcript.json is not a version 1 transcript",
        )
    })
}

/// Deletes a session: the files the app wrote, then the folder if nothing else is in it.
/// Files the app did not write stay (and so does the folder). `dir` must be a direct child of
/// `base`.
pub fn delete_session(base: &Path, dir: &Path) -> io::Result<()> {
    let base = fs::canonicalize(base)?;
    let dir = fs::canonicalize(dir)?;
    if dir.parent() != Some(base.as_path()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a recording folder",
        ));
    }
    for name in OWN_FILES {
        match fs::remove_file(dir.join(name)) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    match fs::remove_dir(&dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::DirectoryNotEmpty => Ok(()),
        Err(e) => Err(e),
    }
}

/// Days since 1970-01-01 of a `YYYY-MM-DD...` text, for "last 7 days" style filters.
pub fn day_number(date: &str) -> Option<i64> {
    let y: i64 = date.get(0..4)?.parse().ok()?;
    let m: i64 = date.get(5..7)?.parse().ok()?;
    let d: i64 = date.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Days from civil (Howard Hinnant).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// Runs scans on a short thread so the window never waits for the disk. Only the newest
/// request's result is ever handed out: a slow old scan cannot overwrite a newer one.
pub struct LibraryHandle {
    tx: Sender<(u64, Vec<Entry>)>,
    rx: Receiver<(u64, Vec<Entry>)>,
    requested: u64,
    delivered: u64,
}

impl Default for LibraryHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryHandle {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            tx,
            rx,
            requested: 0,
            delivered: 0,
        }
    }

    /// Starts a scan of `base`.
    pub fn refresh(&mut self, base: PathBuf) {
        self.requested += 1;
        let seq = self.requested;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send((seq, scan(&base)));
        });
    }

    /// The newest finished scan that was not handed out yet.
    pub fn poll(&mut self) -> Option<Vec<Entry>> {
        let mut newest: Option<(u64, Vec<Entry>)> = None;
        while let Ok((seq, list)) = self.rx.try_recv() {
            if seq > self.delivered && newest.as_ref().is_none_or(|(n, _)| seq > *n) {
                newest = Some((seq, list));
            }
        }
        let (seq, list) = newest?;
        self.delivered = seq;
        Some(list)
    }

    /// A scan was started and its result has not arrived.
    pub fn is_pending(&self) -> bool {
        self.delivered < self.requested
    }

    /// How many scans were started (tests).
    #[cfg(test)]
    pub fn requested(&self) -> u64 {
        self.requested
    }
}

/// `HH:MM:SS`.
pub fn format_duration(secs: u64) -> String {
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

/// `184.6 MB`, `2.1 GB`, `12 KB`, `300 B`.
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::transcript::{ModelInfo, Transcript};
    use crate::audio::wav::WavWriter;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct Tmp(PathBuf);
    impl Tmp {
        fn new() -> Tmp {
            static N: AtomicU32 = AtomicU32::new(0);
            let p = std::env::temp_dir().join(format!(
                "meeting-recorder-library-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
        fn dir(&self, name: &str) -> PathBuf {
            let d = self.0.join(name);
            fs::create_dir_all(&d).unwrap();
            d
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn wav(path: &Path, seconds: u32) {
        let mut w = WavWriter::new(fs::File::create(path).unwrap(), 8000).unwrap();
        w.write_samples(&vec![0i16; 8000 * seconds as usize])
            .unwrap();
        w.finish().unwrap();
    }

    fn line(track: Track, start_ms: u64, text: &str) -> Line {
        Line {
            track,
            start_ms,
            end_ms: start_ms + 500,
            text: text.to_owned(),
        }
    }

    fn write_transcript(dir: &Path, lines: Vec<Line>) {
        let t = Transcript {
            model: ModelInfo {
                name: "m".into(),
                revision: "r".into(),
                precision: "int8".into(),
            },
            created: "2026-11-24T22:48:00".into(),
            lines,
        };
        fs::write(
            dir.join("transcript.json"),
            crate::asr::transcript::to_json(&t),
        )
        .unwrap();
        fs::write(dir.join("transcript.txt"), "x").unwrap();
    }

    #[test]
    fn scan_lists_a_session_with_both_tracks_and_its_transcript() {
        let t = Tmp::new();
        let d = t.dir("2026-11-24_2248_standup");
        wav(&d.join("mic.wav"), 1);
        wav(&d.join("system.wav"), 2);
        write_transcript(
            &d,
            vec![line(Track::Mic, 0, "สวัสดี"), line(Track::System, 900, "ครับ")],
        );
        let list = scan(&t.0);
        assert_eq!(list.len(), 1);
        let e = &list[0];
        assert_eq!(e.name, "standup");
        assert_eq!(e.stamp, "2026-11-24 22:48");
        assert_eq!(e.folder, "2026-11-24_2248_standup");
        assert_eq!(e.duration_s, 2, "the longer track");
        assert!(e.bytes > 2 * 8000 * 2);
        let tr = e.transcript.as_ref().unwrap();
        assert_eq!(tr.lines, 2);
        assert_eq!(tr.preview, "สวัสดี");
        assert!(tr.text.contains("ครับ"));
    }

    #[test]
    fn scan_lists_a_mic_only_folder() {
        let t = Tmp::new();
        wav(&t.dir("2026-11-24_1000_solo").join("mic.wav"), 3);
        let list = scan(&t.0);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].duration_s, 3);
        assert!(list[0].transcript.is_none());
    }

    #[test]
    fn scan_ignores_folders_without_wavs_and_loose_files() {
        let t = Tmp::new();
        fs::write(t.dir("2026-11-24_1000_notes").join("notes.txt"), "x").unwrap();
        fs::write(t.0.join("loose.wav"), "x").unwrap();
        assert!(scan(&t.0).is_empty());
        assert!(scan(&t.0.join("missing")).is_empty());
    }

    #[test]
    fn a_truncated_wav_lists_with_zero_duration() {
        let t = Tmp::new();
        let d = t.dir("2026-11-24_1000_broken");
        fs::write(d.join("mic.wav"), b"RIFF").unwrap();
        fs::write(d.join("system.wav"), b"").unwrap();
        let list = scan(&t.0);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].duration_s, 0);
        assert_eq!(list[0].bytes, 4);
    }

    #[test]
    fn a_damaged_transcript_json_is_audio_only() {
        let t = Tmp::new();
        let d = t.dir("2026-11-24_1000_a");
        wav(&d.join("mic.wav"), 1);
        fs::write(d.join("transcript.json"), "{ not json").unwrap();
        let d2 = t.dir("2026-11-24_1001_b");
        wav(&d2.join("mic.wav"), 1);
        fs::write(
            d2.join("transcript.json"),
            "{\"version\": 2, \"lines\": []}",
        )
        .unwrap();
        for e in scan(&t.0) {
            assert!(e.transcript.is_none(), "{}", e.folder);
        }
        assert!(load_transcript(&d).is_err());
    }

    #[test]
    fn a_transcript_without_speech_is_still_a_transcript() {
        let t = Tmp::new();
        let d = t.dir("2026-11-24_1000_quiet");
        wav(&d.join("mic.wav"), 1);
        write_transcript(&d, vec![]);
        let tr = scan(&t.0)[0].transcript.clone().unwrap();
        assert_eq!((tr.lines, tr.preview.as_str()), (0, ""));
    }

    #[test]
    fn scan_sorts_newest_first_and_keeps_unstamped_folders_last() {
        let t = Tmp::new();
        wav(&t.dir("2026-11-23_0900_old").join("mic.wav"), 1);
        wav(&t.dir("2026-11-24_0900_new").join("mic.wav"), 1);
        wav(&t.dir("my own folder").join("mic.wav"), 1);
        let names: Vec<_> = scan(&t.0).into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["new", "old", "my own folder"]);
    }

    #[test]
    fn load_transcript_returns_lines_with_tracks() {
        let t = Tmp::new();
        let d = t.dir("2026-11-24_1000_x");
        write_transcript(
            &d,
            vec![line(Track::System, 1500, "ก"), line(Track::Mic, 2000, "ข")],
        );
        let lines = load_transcript(&d).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!((lines[0].track, lines[0].start_ms), (Track::System, 1500));
        assert_eq!(lines[1].text, "ข");
    }

    #[test]
    fn delete_removes_known_files_and_the_folder() {
        let t = Tmp::new();
        let d = t.dir("2026-11-24_1000_x");
        wav(&d.join("mic.wav"), 1);
        wav(&d.join("system.wav"), 1);
        write_transcript(&d, vec![]);
        delete_session(&t.0, &d).unwrap();
        assert!(!d.exists());
    }

    #[test]
    fn delete_keeps_unknown_files_and_the_folder() {
        let t = Tmp::new();
        let d = t.dir("2026-11-24_1000_x");
        wav(&d.join("mic.wav"), 1);
        fs::write(d.join("my-notes.txt"), "keep").unwrap();
        delete_session(&t.0, &d).unwrap();
        assert!(d.join("my-notes.txt").exists());
        assert!(!d.join("mic.wav").exists());
        assert!(scan(&t.0).is_empty(), "no WAV left, so it no longer lists");
    }

    #[test]
    fn delete_refuses_a_path_outside_base() {
        let t = Tmp::new();
        let base = t.dir("base");
        let d = t.dir("other");
        wav(&d.join("mic.wav"), 1);
        assert!(delete_session(&base, &d).is_err());
        assert!(delete_session(&base, &base).is_err());
        let nested = t.dir("base/2026-11-24_1000_x/inner");
        wav(&nested.join("mic.wav"), 1);
        assert!(delete_session(&base, &nested).is_err());
        assert!(delete_session(&base, &base.join("2026-11-24_1000_x/../..")).is_err());
        assert!(d.join("mic.wav").exists() && nested.join("mic.wav").exists());
    }

    #[test]
    fn handle_hands_out_only_the_newest_scan() {
        let t = Tmp::new();
        let a = t.dir("a");
        wav(&t.dir("a/2026-11-24_1000_first").join("mic.wav"), 1);
        let b = t.dir("b");
        wav(&t.dir("b/2026-11-24_1001_second").join("mic.wav"), 1);
        let mut h = LibraryHandle::new();
        h.refresh(a);
        h.refresh(b);
        assert!(h.is_pending());
        let mut got: Vec<Vec<Entry>> = Vec::new();
        let start = std::time::Instant::now();
        while h.is_pending() && start.elapsed().as_secs() < 10 {
            if let Some(list) = h.poll() {
                got.push(list);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Some(list) = h.poll() {
            got.push(list);
        }
        assert!(!h.is_pending());
        let last = got.last().expect("a result");
        assert_eq!(last[0].name, "second", "the newest request wins");
        assert!(h.poll().is_none(), "nothing is handed out twice");
        assert_eq!(h.requested(), 2);
    }

    #[test]
    fn day_number_counts_days() {
        assert_eq!(day_number("1970-01-01"), Some(0));
        assert_eq!(day_number("2026-11-24 22:48"), Some(20_781));
        assert_eq!(
            day_number("2026-11-24").unwrap() - day_number("2026-11-17").unwrap(),
            7
        );
        assert_eq!(
            day_number("2026-03-01").unwrap() - day_number("2026-02-28").unwrap(),
            1
        );
        assert_eq!(day_number("my own folder"), None);
        assert_eq!(day_number("2026-13-01"), None);
    }

    #[test]
    fn scanning_200_sessions_is_quick() {
        let t = Tmp::new();
        for i in 0..200 {
            let d = t.dir(&format!("2026-11-24_{:04}_session-{i}", i));
            wav(&d.join("mic.wav"), 1);
            wav(&d.join("system.wav"), 1);
            if i % 2 == 0 {
                write_transcript(&d, vec![line(Track::Mic, 0, "สวัสดีครับ")]);
            }
        }
        let start = std::time::Instant::now();
        let list = scan(&t.0);
        let took = start.elapsed();
        eprintln!("scan of 200 sessions took {took:?}");
        assert_eq!(list.len(), 200);
        assert_eq!(list.iter().filter(|e| e.transcript.is_some()).count(), 100);
        assert!(took.as_secs() < 5, "took {took:?}");
    }

    #[test]
    fn format_duration_and_size() {
        assert_eq!(format_duration(0), "00:00:00");
        assert_eq!(format_duration(3661), "01:01:01");
        assert_eq!(format_size(300), "300 B");
        assert_eq!(format_size(12 * 1024), "12 KB");
        assert_eq!(format_size(184 * 1024 * 1024 + 600 * 1024), "184.6 MB");
        assert_eq!(format_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }
}
