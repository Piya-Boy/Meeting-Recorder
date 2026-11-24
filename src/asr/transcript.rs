//! The transcript of one session and the files it is written to.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Which recorded file a line came from. The mic track is the user, the system track is the
/// other participants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Track {
    Mic,
    System,
}

impl Track {
    fn key(self) -> &'static str {
        match self {
            Track::Mic => "mic",
            Track::System => "system",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub track: Track,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelInfo {
    pub name: String,
    pub revision: String,
    pub precision: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transcript {
    pub model: ModelInfo,
    /// ISO-8601 local time, given by the caller.
    pub created: String,
    pub lines: Vec<Line>,
}

/// Both tracks in time order; at the same start the mic line comes first.
pub fn merge(mic: Vec<Line>, system: Vec<Line>) -> Vec<Line> {
    let mut all: Vec<Line> = mic.into_iter().chain(system).collect();
    // Stable sort: mic lines were first in `all`, so they stay first on a tie.
    all.sort_by_key(|l| l.start_ms);
    all
}

/// `hh:mm:ss` of a time in milliseconds.
pub fn format_clock(ms: u64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// The `transcript.json` text (version 1).
pub fn to_json(t: &Transcript) -> String {
    let mut out = String::new();
    out.push_str("{\n  \"version\": 1,\n  \"language\": \"th\",\n");
    out.push_str(&format!(
        "  \"model\": {{ \"name\": \"{}\", \"revision\": \"{}\", \"precision\": \"{}\" }},\n",
        json_escape(&t.model.name),
        json_escape(&t.model.revision),
        json_escape(&t.model.precision)
    ));
    out.push_str(&format!(
        "  \"created\": \"{}\",\n",
        json_escape(&t.created)
    ));
    out.push_str("  \"tracks\": { \"mic\": \"mic.wav\", \"system\": \"system.wav\" },\n");
    if t.lines.is_empty() {
        out.push_str("  \"lines\": []\n}\n");
        return out;
    }
    out.push_str("  \"lines\": [\n");
    for (i, l) in t.lines.iter().enumerate() {
        out.push_str(&format!(
            "    {{ \"track\": \"{}\", \"start_ms\": {}, \"end_ms\": {}, \"text\": \"{}\" }}{}\n",
            l.track.key(),
            l.start_ms,
            l.end_ms,
            json_escape(&l.text),
            if i + 1 == t.lines.len() { "" } else { "," }
        ));
    }
    out.push_str("  ]\n}\n");
    out
}

/// The `transcript.txt` text: `[hh:mm:ss] <label>: <text>` per line.
pub fn to_text(t: &Transcript, you: &str, others: &str) -> String {
    lines_to_text(&t.lines, you, others)
}

/// The same text for a bare list of lines.
pub fn lines_to_text(lines: &[Line], you: &str, others: &str) -> String {
    if lines.is_empty() {
        return "No speech found.\n".to_owned();
    }
    let mut out = String::new();
    for l in lines {
        let who = match l.track {
            Track::Mic => you,
            Track::System => others,
        };
        out.push_str(&format!(
            "[{}] {}: {}\n",
            format_clock(l.start_ms),
            who,
            l.text
        ));
    }
    out
}

pub(crate) fn write_atomic(dir: &Path, name: &str, content: &str) -> io::Result<()> {
    let tmp = dir.join(format!("{name}.tmp"));
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, dir.join(name))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Writes `transcript.json` and `transcript.txt` into `dir`, each through a temp file and a
/// rename, so a crash never leaves a half-written file under the final name.
pub fn write(dir: &Path, t: &Transcript, you: &str, others: &str) -> io::Result<()> {
    write_atomic(dir, "transcript.json", &to_json(t))?;
    if let Err(e) = write_atomic(dir, "transcript.txt", &to_text(t, you, others)) {
        let _ = fs::remove_file(dir.join("transcript.json"));
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn line(track: Track, start_ms: u64, text: &str) -> Line {
        Line {
            track,
            start_ms,
            end_ms: start_ms + 1000,
            text: text.to_owned(),
        }
    }

    fn sample(lines: Vec<Line>) -> Transcript {
        Transcript {
            model: ModelInfo {
                name: "OpenThai2.0-ASR-Streaming-ONNX".into(),
                revision: "9f142a4".into(),
                precision: "int8".into(),
            },
            created: "2026-11-24T10:15:00+07:00".into(),
            lines,
        }
    }

    #[test]
    fn merge_orders_by_start_with_mic_first_on_ties() {
        let mic = vec![line(Track::Mic, 5000, "m")];
        let system = vec![
            line(Track::System, 1000, "a"),
            line(Track::System, 5000, "b"),
        ];
        let merged = merge(mic, system);
        let order: Vec<(u64, Track)> = merged.iter().map(|l| (l.start_ms, l.track)).collect();
        assert_eq!(
            order,
            vec![
                (1000, Track::System),
                (5000, Track::Mic),
                (5000, Track::System)
            ]
        );
    }

    #[test]
    fn json_has_header_and_line() {
        let json = to_json(&sample(vec![line(Track::Mic, 12000, "สวัสดีครับ")]));
        assert!(json.contains("\"version\": 1"), "{json}");
        assert!(json.contains("\"language\": \"th\""), "{json}");
        assert!(
            json.contains("\"name\": \"OpenThai2.0-ASR-Streaming-ONNX\""),
            "{json}"
        );
        assert!(json.contains("\"revision\": \"9f142a4\""), "{json}");
        assert!(json.contains("\"precision\": \"int8\""), "{json}");
        assert!(json.contains("\"track\": \"mic\""), "{json}");
        assert!(json.contains("\"start_ms\": 12000"), "{json}");
        assert!(json.contains("\"end_ms\": 13000"), "{json}");
        assert!(json.contains("\"text\": \"สวัสดีครับ\""), "{json}");
    }

    #[test]
    fn json_escapes_quotes_backslashes_and_newlines() {
        let json = to_json(&sample(vec![line(Track::System, 0, "a\"b\\c\nd")]));
        assert!(json.contains(r#""text": "a\"b\\c\nd""#), "{json}");
    }

    #[test]
    fn json_empty_lines_is_empty_array() {
        let json = to_json(&sample(vec![]));
        assert!(json.contains("\"lines\": []"), "{json}");
    }

    #[test]
    fn clock_formats_hours_minutes_seconds() {
        assert_eq!(format_clock(3_725_000), "01:02:05");
        assert_eq!(format_clock(0), "00:00:00");
        assert_eq!(format_clock(999), "00:00:00");
    }

    #[test]
    fn text_uses_labels_per_track() {
        let t = sample(vec![
            line(Track::Mic, 12000, "สวัสดีครับ"),
            line(Track::System, 15000, "ครับ"),
        ]);
        let text = to_text(&t, "You", "Others");
        assert_eq!(text, "[00:00:12] You: สวัสดีครับ\n[00:00:15] Others: ครับ\n");
        let th = to_text(&t, "คุณ", "คนอื่น");
        assert!(th.contains("[00:00:15] คนอื่น: ครับ"), "{th}");
    }

    #[test]
    fn text_of_empty_transcript_is_a_note() {
        assert_eq!(
            to_text(&sample(vec![]), "You", "Others"),
            "No speech found.\n"
        );
    }

    #[test]
    fn write_creates_both_files_and_no_tmp() {
        let dir = crate::audio::temp_dir("transcript-write");
        write(
            &dir,
            &sample(vec![line(Track::Mic, 0, "x")]),
            "You",
            "Others",
        )
        .unwrap();
        assert!(dir.join("transcript.json").is_file());
        assert!(dir.join("transcript.txt").is_file());
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn write_into_missing_dir_fails_and_leaves_nothing() {
        let base = crate::audio::temp_dir("transcript-missing");
        let missing = base.join("nope");
        assert!(write(&missing, &sample(vec![]), "You", "Others").is_err());
        assert!(!Path::new(&missing).exists());
    }
}
