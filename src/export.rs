//! Transcript exports (TXT, SRT, JSON): files written next to the recording.
//!
//! Pure file logic, no egui. `transcript.json` and `transcript.txt` come from the transcription
//! and are never overwritten; `transcript.srt` is derived data and is rewritten on each export.

use std::io;
use std::path::{Path, PathBuf};

use crate::asr::transcript::{lines_to_text, write_atomic, Line, Track};
use crate::library::load_transcript;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Srt,
    Txt,
}

impl ExportFormat {
    pub fn file_name(self) -> &'static str {
        match self {
            ExportFormat::Json => "transcript.json",
            ExportFormat::Srt => "transcript.srt",
            ExportFormat::Txt => "transcript.txt",
        }
    }
}

/// `HH:MM:SS,mmm` (hours are not capped).
fn srt_time(ms: u64) -> String {
    let s = ms / 1000;
    format!(
        "{:02}:{:02}:{:02},{:03}",
        s / 3600,
        s % 3600 / 60,
        s % 60,
        ms % 1000
    )
}

/// SubRip cues: `1`, `HH:MM:SS,mmm --> HH:MM:SS,mmm`, `<speaker>: <text>`, blank line.
pub fn to_srt(lines: &[Line], you: &str, others: &str) -> String {
    let mut out = String::new();
    let mut n = 0;
    for l in lines {
        // A blank line ends a cue, so the text is flattened onto one line.
        let text = l.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            continue;
        }
        n += 1;
        let who = match l.track {
            Track::Mic => you,
            Track::System => others,
        };
        out.push_str(&format!(
            "{n}\n{} --> {}\n{who}: {text}\n\n",
            srt_time(l.start_ms),
            srt_time(l.end_ms.max(l.start_ms))
        ));
    }
    out
}

/// Produces the export file for the recording in `dir` and returns its path.
pub fn export_transcript(
    dir: &Path,
    format: ExportFormat,
    you: &str,
    others: &str,
) -> io::Result<PathBuf> {
    // Loading first makes a missing or damaged transcript an error for every format.
    let lines = load_transcript(dir)?;
    let path = dir.join(format.file_name());
    match format {
        ExportFormat::Json => {}
        ExportFormat::Txt => {
            if !path.is_file() {
                write_atomic(dir, format.file_name(), &lines_to_text(&lines, you, others))?;
            }
        }
        ExportFormat::Srt => write_atomic(dir, format.file_name(), &to_srt(&lines, you, others))?,
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::transcript::{ModelInfo, Track, Transcript};
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn line(track: Track, start_ms: u64, end_ms: u64, text: &str) -> Line {
        Line {
            track,
            start_ms,
            end_ms,
            text: text.to_owned(),
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let d = std::env::temp_dir().join(format!(
            "mr-export-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_real(dir: &Path, lines: Vec<Line>) {
        let t = Transcript {
            model: ModelInfo {
                name: "m".into(),
                revision: "r".into(),
                precision: "int8".into(),
            },
            created: "2026-11-25T10:00:00".into(),
            lines,
        };
        crate::asr::transcript::write(dir, &t, "You", "Others").unwrap();
    }

    #[test]
    fn srt_has_numbered_cues_with_comma_milliseconds_and_speaker() {
        let srt = to_srt(
            &[
                line(Track::Mic, 0, 900, "a"),
                line(Track::System, 3_661_250, 3_662_000, "b"),
            ],
            "You",
            "Others",
        );
        assert_eq!(
            srt,
            "1\n00:00:00,000 --> 00:00:00,900\nYou: a\n\n2\n01:01:01,250 --> 01:01:02,000\nOthers: b\n\n"
        );
    }

    #[test]
    fn srt_text_never_contains_a_blank_line() {
        let srt = to_srt(
            &[
                line(Track::Mic, 0, 500, "x\n\ny"),
                line(Track::Mic, 600, 900, "  \n "),
                line(Track::System, 1000, 1500, "z"),
            ],
            "You",
            "Others",
        );
        assert_eq!(
            srt,
            "1\n00:00:00,000 --> 00:00:00,500\nYou: x y\n\n2\n00:00:01,000 --> 00:00:01,500\nOthers: z\n\n"
        );
    }

    #[test]
    fn srt_end_before_start_is_raised_to_start() {
        let srt = to_srt(&[line(Track::Mic, 2000, 1000, "a")], "You", "Others");
        assert!(srt.contains("00:00:02,000 --> 00:00:02,000"), "{srt}");
    }

    #[test]
    fn srt_of_no_lines_is_empty() {
        assert_eq!(to_srt(&[], "You", "Others"), "");
    }

    #[test]
    fn export_srt_writes_a_file_and_leaves_json_and_txt_alone() {
        let d = temp_dir("srt");
        write_real(&d, vec![line(Track::Mic, 0, 900, "สวัสดี")]);
        let json = fs::read(d.join("transcript.json")).unwrap();
        let txt = fs::read(d.join("transcript.txt")).unwrap();

        let srt = export_transcript(&d, ExportFormat::Srt, "You", "Others").unwrap();
        assert!(srt.ends_with("transcript.srt"));
        let body = fs::read_to_string(&srt).unwrap();
        assert!(body.starts_with("1\n00:00:00,000 --> 00:00:00,900\nYou: สวัสดี\n"));
        assert!(!d.join("transcript.srt.tmp").exists());

        let t = export_transcript(&d, ExportFormat::Txt, "ผม", "คนอื่น").unwrap();
        let j = export_transcript(&d, ExportFormat::Json, "You", "Others").unwrap();
        assert!(t.ends_with("transcript.txt") && j.ends_with("transcript.json"));
        assert_eq!(fs::read(d.join("transcript.json")).unwrap(), json);
        assert_eq!(fs::read(d.join("transcript.txt")).unwrap(), txt);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn export_txt_is_written_when_missing() {
        let d = temp_dir("txt");
        write_real(&d, vec![line(Track::System, 61_000, 62_000, "ครับ")]);
        fs::remove_file(d.join("transcript.txt")).unwrap();
        let p = export_transcript(&d, ExportFormat::Txt, "You", "Others").unwrap();
        assert_eq!(fs::read_to_string(p).unwrap(), "[00:01:01] Others: ครับ\n");
        assert!(!d.join("transcript.txt.tmp").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn export_of_a_damaged_or_missing_transcript_is_an_error() {
        let d = temp_dir("bad");
        for f in [ExportFormat::Json, ExportFormat::Srt, ExportFormat::Txt] {
            assert!(
                export_transcript(&d, f, "You", "Others").is_err(),
                "{f:?} missing"
            );
        }
        fs::write(d.join("transcript.json"), "{ not json").unwrap();
        for f in [ExportFormat::Json, ExportFormat::Srt, ExportFormat::Txt] {
            assert!(
                export_transcript(&d, f, "You", "Others").is_err(),
                "{f:?} damaged"
            );
        }
        assert!(!d.join("transcript.srt").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn file_names_are_the_three_transcript_files() {
        assert_eq!(ExportFormat::Json.file_name(), "transcript.json");
        assert_eq!(ExportFormat::Srt.file_name(), "transcript.srt");
        assert_eq!(ExportFormat::Txt.file_name(), "transcript.txt");
    }
}
