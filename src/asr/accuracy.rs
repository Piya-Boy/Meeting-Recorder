//! Accuracy and speed check of the whole chain (WAV file, segmenter, recognizer) on real
//! speech. Needs the model and clips, so it is ignored by default:
//!
//!   ASR_MODEL_DIR=<folder with encoder.int8.onnx(+.data), decoder_joint.onnx, tokens.txt>
//!   ASR_TEST_CLIPS=<folder with <name>.wav (16-bit mono PCM) and <name>.txt references>
//!   cargo test asr::accuracy -- --ignored --nocapture
//!
//! The project's clips are ten FLEURS Thai dev clips (google/fleurs, th_th, CC-BY 4.0, 5-15 s
//! each), converted to 16-bit PCM; they are not in the repository.

use std::path::PathBuf;
use std::time::Instant;

use crate::asr::engine::{OrtRecognizer, Recognizer};
use crate::asr::segment::segment_file;
use crate::asr::wavread::WavReader;

/// Edit distance between two texts, in characters.
fn edit_distance(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push(
                (prev[j + 1] + 1)
                    .min(cur[j] + 1)
                    .min(prev[j] + usize::from(ca != cb)),
            );
        }
        prev = cur;
    }
    prev[b.len()]
}

fn without_spaces(s: &str) -> Vec<char> {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Edits and reference length (spaces ignored) of one clip.
fn errors(reference: &str, hypothesis: &str) -> (usize, usize) {
    let (r, h) = (without_spaces(reference), without_spaces(hypothesis));
    (edit_distance(&r, &h), r.len())
}

#[test]
fn edit_distance_known_values() {
    let d = |a: &str, b: &str| {
        edit_distance(
            &a.chars().collect::<Vec<_>>(),
            &b.chars().collect::<Vec<_>>(),
        )
    };
    assert_eq!(d("kitten", "sitting"), 3);
    assert_eq!(d("", "abc"), 3);
    assert_eq!(d("abc", ""), 3);
    assert_eq!(d("ทำ", "ทำ"), 0);
}

#[test]
fn errors_ignore_spaces() {
    assert_eq!(errors("ก ข ค", "กขค"), (0, 3));
    assert_eq!(errors("กขค", "กงค"), (1, 3));
}

#[cfg(windows)]
fn peak_working_set_mib() -> f64 {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    // SAFETY: the counters struct is zeroed, sized correctly and lives through the call.
    unsafe {
        let mut c: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) == 0 {
            return 0.0;
        }
        c.PeakWorkingSetSize as f64 / (1024.0 * 1024.0)
    }
}

#[cfg(not(windows))]
fn peak_working_set_mib() -> f64 {
    0.0
}

fn env_dir(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set {name} to run this test")))
}

#[test]
#[ignore = "needs the model (ASR_MODEL_DIR) and clips (ASR_TEST_CLIPS)"]
fn fleurs_cer_stays_below_6_percent() {
    let threads = std::env::var("ASR_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4);
    let loaded = Instant::now();
    let mut rec = OrtRecognizer::load(&env_dir("ASR_MODEL_DIR"), threads).unwrap();
    println!(
        "model loaded in {:.1} s ({threads} threads)",
        loaded.elapsed().as_secs_f32()
    );

    let mut wavs: Vec<PathBuf> = std::fs::read_dir(env_dir("ASR_TEST_CLIPS"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "wav"))
        .collect();
    wavs.sort();
    assert!(!wavs.is_empty(), "no clips found");

    let (mut edits, mut chars) = (0usize, 0usize);
    let (mut audio_secs, mut proc_secs) = (0f32, 0f32);
    for wav in &wavs {
        let reference = std::fs::read_to_string(wav.with_extension("txt")).unwrap();
        let reader = WavReader::open(wav).unwrap();
        audio_secs += reader.total_frames() as f32 / reader.rate() as f32;
        drop(reader);

        let start = Instant::now();
        let mut parts = Vec::new();
        segment_file(
            wav,
            &mut |_, seg| parts.push(rec.recognize(seg).unwrap()),
            &mut |_, _| true,
        )
        .unwrap();
        proc_secs += start.elapsed().as_secs_f32();

        let hypothesis = parts.join(" ");
        let (e, n) = errors(&reference, &hypothesis);
        println!(
            "{}: CER {:.3}\n  REF {reference}\n  HYP {hypothesis}",
            wav.file_name().unwrap().to_string_lossy(),
            e as f32 / n as f32
        );
        edits += e;
        chars += n;
    }
    let cer = edits as f32 / chars as f32;
    println!(
        "CER {:.2}% over {} clips, {:.1} s of audio in {:.1} s (real-time factor {:.3}), peak RAM {:.0} MiB",
        cer * 100.0,
        wavs.len(),
        audio_secs,
        proc_secs,
        proc_secs / audio_secs,
        peak_working_set_mib()
    );
    assert!(cer <= 0.06, "CER {cer:.4} is above 0.06");
}
